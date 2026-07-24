#!/usr/bin/env python3
import copy
import math
import os
import stat
import time
from contextlib import contextmanager
from pathlib import Path

try:
    import fcntl
except ImportError:
    fcntl = None

from updater_protocol import BINARY, CANDIDATE_HEALTH_PROTOCOL, LOCK_TIMEOUT_SECONDS, MANIFEST, MANIFEST_SCHEMA, MAX_JSON_BYTES, UPDATER_PROTOCOL, UpdateError, read_json, same_path, version


class UpdaterStateMixin:
    profile: str
    root: Path
    releases: Path
    bin_link: Path
    service: str
    state_path: Path
    transaction_path: Path
    lock_path: Path
    test_mode: bool

    def validate_managed_directory(self, path: Path, label: str) -> None:
        try:
            metadata = path.lstat()
        except OSError as exc:
            raise UpdateError(f"Managed {label} is missing: {path}") from exc
        if not stat.S_ISDIR(metadata.st_mode) or path.is_symlink():
            raise UpdateError(f"Managed {label} is not a directory: {path}")
        if getattr(os, "geteuid", lambda: -1)() == 0 and metadata.st_uid != 0:
            raise UpdateError(f"Managed {label} is not root-owned: {path}")
        if os.name != "nt" and metadata.st_mode & 0o022:
            raise UpdateError(f"Managed {label} is group/world writable: {path}")

    def validate_lock_path(self) -> None:
        self.validate_managed_directory(self.root, "updater root")
        parent = self.lock_path.parent
        if not self.test_mode:
            root = self.root.resolve(strict=False)
            resolved_parent = parent.resolve(strict=False)
            if not same_path(resolved_parent, root) and root not in resolved_parent.parents:
                raise UpdateError("Updater lock is outside the managed root")
        self.validate_managed_directory(parent, "lock directory")
        try:
            metadata = self.lock_path.lstat()
        except FileNotFoundError:
            return
        except OSError as exc:
            raise UpdateError("Updater lock cannot be inspected safely") from exc
        if stat.S_ISLNK(metadata.st_mode) or (os.name != "nt" and (not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o077)):
            raise UpdateError("Updater lock has unsafe type or mode")
        if getattr(os, "geteuid", lambda: -1)() == 0 and metadata.st_uid != 0:
            raise UpdateError("Updater lock is not root-owned")

    def validate_manifest(self, path: Path, expected_version: str, allow_legacy: bool = False, expected_profile: str | None = None) -> dict:
        payload = read_json(path, MAX_JSON_BYTES)
        if not isinstance(payload, dict):
            raise UpdateError("Release manifest is not an object")
        manifest_profile = payload.get("board_profile")
        profile = expected_profile or self.profile
        if manifest_profile is None and not allow_legacy:
            raise UpdateError("Release manifest has no board profile")
        if manifest_profile is not None and profile and manifest_profile != profile:
            raise UpdateError("Release manifest board profile does not match this device")
        schema_version = payload.get("schema_version")
        if schema_version == MANIFEST_SCHEMA:
            if payload.get("updater_protocol") != UPDATER_PROTOCOL or payload.get("candidate_health_protocol") != CANDIDATE_HEALTH_PROTOCOL:
                raise UpdateError("Release manifest protocol declaration is invalid")
        elif not (allow_legacy and schema_version == 1):
            raise UpdateError("Release manifest schema is unsupported")
        required = {
            "tag": f"v{expected_version}",
            "version": expected_version,
            "arch": "aarch64-unknown-linux-gnu",
            "binary": BINARY,
        }
        if any(payload.get(key) != value for key, value in required.items()):
            raise UpdateError("Release manifest identity does not match this device")
        platforms = payload.get("platforms")
        if not isinstance(platforms, list) or "linux-aarch64-device" not in platforms:
            raise UpdateError("Release is not compatible with this device")
        if manifest_profile is not None and manifest_profile not in platforms:
            raise UpdateError("Release manifest board profile is not listed in platforms")
        return payload

    @staticmethod
    def current_protocol_manifest(manifest: dict) -> bool:
        return (
            manifest.get("schema_version") == MANIFEST_SCHEMA
            and manifest.get("updater_protocol") == UPDATER_PROTOCOL
            and manifest.get("candidate_health_protocol") == CANDIDATE_HEALTH_PROTOCOL
            and isinstance(manifest.get("board_profile"), str)
        )

    def mark_activation_attempted(self, payload: dict) -> None:
        from updater_protocol import atomic_json
        payload["activation_attempted"] = True
        atomic_json(self.transaction_path, payload)

    def bootstrap_legacy(self) -> str | None:
        current_link = self.root / "current"
        if not current_link.exists() and not current_link.is_symlink():
            return None
        current, _ = self.current_link()
        self.managed_bin_link()
        manifest = self.validate_release(self.releases / current, allow_legacy=True)
        if self.current_protocol_manifest(manifest):
            return current
        if manifest.get("schema_version") != 1:
            raise UpdateError("Legacy release manifest cannot be bootstrapped safely")
        state = self.state()
        previous = state.get("previous") if isinstance(state, dict) else None
        if previous is not None:
            if not version(previous):
                raise UpdateError("Legacy state previous release is invalid")
            self.validate_release(self.releases / previous, allow_legacy=True)
        asset = state.get("asset") if isinstance(state, dict) else None
        migrated = copy.deepcopy(manifest)
        migrated["schema_version"] = MANIFEST_SCHEMA
        migrated["updater_protocol"] = UPDATER_PROTOCOL
        migrated["candidate_health_protocol"] = CANDIDATE_HEALTH_PROTOCOL
        migrated["board_profile"] = migrated.get("board_profile") or self.profile
        platforms = list(migrated["platforms"])
        if self.profile not in platforms:
            platforms.insert(0, self.profile)
        migrated["platforms"] = platforms
        from updater_protocol import atomic_json
        atomic_json(self.releases / current / MANIFEST, migrated)
        self.immutable(self.releases / current)
        self.write_committed_state(current, previous, migrated, asset)
        return current

    def recover_pending(self, boot: bool = False) -> None:
        if boot:
            self.verify_service_inactive()
        if self.transaction_path.exists():
            try:
                payload = self.load_transaction(recovery=True)
            except UpdateError:
                self.recover_legacy(force=True)
                self.transaction_path.unlink(missing_ok=True)
                self.health_path.unlink(missing_ok=True)
            else:
                self.restore_transaction(payload, stop_service=bool(payload.get("activation_attempted", False)) and not boot)
        self.recover_legacy()
        if boot:
            self.verify_service_inactive()

    def validate_release(self, directory: Path, allow_legacy: bool = False, require_immutable: bool = False, expected_profile: str | None = None) -> dict:
        if not directory.is_dir() or directory.is_symlink() or not version(directory.name):
            raise UpdateError(f"Release directory is unmanaged: {directory}")
        self.validate_managed_directory(directory, "release directory")
        directory_stat = directory.stat()
        if getattr(os, "geteuid", lambda: -1)() == 0 and directory_stat.st_uid != 0:
            raise UpdateError(f"Release directory is not root-owned: {directory}")
        if require_immutable and directory_stat.st_mode & 0o222:
            raise UpdateError(f"Release directory is writable: {directory}")
        binary = directory / BINARY
        manifest = directory / MANIFEST
        if not binary.is_file() or binary.is_symlink() or not os.access(binary, os.X_OK):
            raise UpdateError(f"Release binary is invalid: {directory}")
        allowed = {BINARY, MANIFEST, "update-asset.json", "LICENSE", "NOTICE"}
        for child in directory.iterdir():
            if child.name not in allowed or child.is_symlink() or not child.is_file():
                raise UpdateError(f"Release contains an unsafe entry: {child}")
            child_stat = child.stat()
            if getattr(os, "geteuid", lambda: -1)() == 0 and child_stat.st_uid != 0:
                raise UpdateError(f"Release entry is not root-owned: {child}")
            if os.name != "nt" and child_stat.st_mode & 0o022:
                raise UpdateError(f"Release entry is group/world writable: {child}")
            if require_immutable and child_stat.st_mode & 0o222:
                raise UpdateError(f"Release entry is writable: {child}")
        return self.validate_manifest(manifest, directory.name, allow_legacy=allow_legacy, expected_profile=expected_profile)

    def current_link(self) -> tuple[str, str]:
        try:
            self.validate_managed_directory(self.root, "updater root")
            self.validate_managed_directory(self.releases, "releases directory")
        except UpdateError:
            raise
        if not (self.root / "current").is_symlink():
            raise UpdateError("Managed current release link is missing")
        link = self.root / "current"
        if getattr(os, "geteuid", lambda: -1)() == 0 and link.lstat().st_uid != 0:
            raise UpdateError("Current release link is not root-owned")
        raw = os.readlink(link)
        target = (link.parent / raw).resolve(strict=False)
        releases = self.releases.resolve(strict=False)
        if not same_path(target.parent, releases) or not version(target.name):
            raise UpdateError("Current release link is unmanaged or points to a dev build")
        self.validate_release(target, allow_legacy=True)
        return target.name, raw

    def managed_bin_link(self) -> str:
        if not self.bin_link.is_symlink():
            raise UpdateError("Binary path is not the managed symlink")
        if getattr(os, "geteuid", lambda: -1)() == 0 and self.bin_link.lstat().st_uid != 0:
            raise UpdateError("Binary path link is not root-owned")
        raw = os.readlink(self.bin_link)
        target = (self.bin_link.parent / raw).resolve(strict=False)
        expected = (self.root / "current" / BINARY).resolve(strict=False)
        if not same_path(target, expected):
            raise UpdateError("Binary path does not point through current")
        return raw

    def validate_service(self) -> None:
        service_path = Path(self.service)
        if service_path.is_symlink():
            raise UpdateError("Updater service unit path is unmanaged")
        try:
            lines = service_path.read_text(encoding="utf-8").splitlines()
        except OSError as exc:
            raise UpdateError("Updater-compatible systemd service is missing") from exc
        service_stat = service_path.stat()
        if getattr(os, "geteuid", lambda: -1)() == 0 and service_stat.st_uid != 0:
            raise UpdateError("Updater service unit is not root-owned")
        if getattr(os, "geteuid", lambda: -1)() == 0 and service_stat.st_mode & 0o022:
            raise UpdateError("Updater service unit is group/world writable")
        starts = [line.split("=", 1)[1].strip() for line in lines if line.startswith("ExecStart=")]
        if starts != [str(self.bin_link)]:
            raise UpdateError("Updater refuses a direct or unmanaged ExecStart")

    def state(self) -> object | None:
        if not self.state_path.exists():
            return None
        self.validate_control_file(self.state_path)
        return read_json(self.state_path, MAX_JSON_BYTES)

    def validate_control_file(self, path: Path) -> None:
        self.validate_managed_directory(path.parent, "control-file directory")
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode):
            raise UpdateError(f"Managed updater file is not regular: {path}")
        if getattr(os, "geteuid", lambda: -1)() == 0 and metadata.st_uid != 0:
            raise UpdateError(f"Managed updater file is not root-owned: {path}")
        if getattr(os, "geteuid", lambda: -1)() == 0 and metadata.st_mode & 0o022:
            raise UpdateError(f"Managed updater file is writable by group/world: {path}")

    def fallback(self) -> dict:
        current, current_raw = self.current_link()
        bin_raw = self.managed_bin_link()
        current_manifest = self.validate_release(self.releases / current, allow_legacy=True)
        state = self.state()
        previous = None
        if isinstance(state, dict):
            if state.get("schema_version") == 2 and state.get("phase") != "committed":
                raise UpdateError("Update state is not a committed transaction")
            recorded = state.get("current", state.get("active"))
            if recorded not in (None, current):
                raise UpdateError("State current does not match the managed current link")
            previous = state.get("previous")
        elif state is not None:
            raise UpdateError("Update state is not an object")
        if previous is not None:
            if not version(previous):
                raise UpdateError("State previous release is invalid")
            self.validate_release(self.releases / previous, allow_legacy=True)
        self.validate_service()
        return {
            "current": current,
            "current_link": current_raw,
            "previous": previous,
            "bin_link": bin_raw,
            "state": copy.deepcopy(state),
            "legacy": not self.current_protocol_manifest(current_manifest),
        }


@contextmanager
def updater_lock(updater):
    updater.validate_lock_path()
    flags = os.O_RDWR | os.O_CREAT
    if os.name != "nt":
        flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(updater.lock_path, flags, 0o600)
    except OSError as exc:
        raise updater.error("Updater lock cannot be opened safely") from exc
    try:
        metadata = os.fstat(descriptor)
        if stat.S_ISLNK(metadata.st_mode) or (os.name != "nt" and (not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o077)):
            raise updater.error("Updater lock has unsafe type or mode")
        if getattr(os, "geteuid", lambda: -1)() == 0 and metadata.st_uid != 0:
            raise updater.error("Updater lock is not root-owned")
        with os.fdopen(descriptor, "a+") as handle:
            descriptor = -1
            if fcntl is None:
                if not updater.test_mode:
                    raise updater.error("Updater locking is unavailable")
            else:
                try:
                    lock_timeout = float(os.environ.get("OCTESSERA_UPDATE_LOCK_TIMEOUT") or LOCK_TIMEOUT_SECONDS)
                except ValueError as exc:
                    raise updater.error("Invalid updater lock timeout") from exc
                if not math.isfinite(lock_timeout) or lock_timeout <= 0:
                    raise updater.error("Invalid updater lock timeout")
                deadline = time.monotonic() + max(0.01, min(lock_timeout, LOCK_TIMEOUT_SECONDS))
                while True:
                    try:
                        getattr(fcntl, "flock")(handle.fileno(), getattr(fcntl, "LOCK_EX") | getattr(fcntl, "LOCK_NB"))
                        break
                    except BlockingIOError as exc:
                        if time.monotonic() >= deadline:
                            raise updater.error("Updater lock is busy") from exc
                        time.sleep(0.1)
            yield handle
    finally:
        if descriptor >= 0:
            os.close(descriptor)
