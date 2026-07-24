#!/usr/bin/env python3
import copy
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
TAG_RE = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+$")
RASPBERRY_PROFILE = "raspberry-pi-zero-2w"
BINARY = "octessera-pi"
MANIFEST = "update-manifest.json"
TRANSACTION_SCHEMA = 2
MARKER_SCHEMA = CANDIDATE_HEALTH_PROTOCOL = 1
MANIFEST_SCHEMA = UPDATER_PROTOCOL = 2
MAX_JSON_BYTES = 2 * 1024 * 1024
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_SUMS_BYTES = 2 * 1024 * 1024
MAX_ENTRY_BYTES = 128 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED_BYTES = 128 * 1024 * 1024
MAX_ZIP_ENTRIES = 16
SYSTEMCTL_TIMEOUT_SECONDS = 15
LOCK_TIMEOUT_SECONDS = 10
DEFAULT_SERVICE = "octessera.service"
DEFAULT_SERVICE_PATH = "/etc/systemd/system/octessera.service"
DEFAULT_RECOVERY_SERVICE = "octessera-update-recovery.service"


class UpdateError(Exception):
    pass


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def version(value: object) -> bool:
    return isinstance(value, str) and bool(VERSION_RE.fullmatch(value))


def same_path(left: Path, right: Path) -> bool:
    normalize = lambda value: os.path.normcase(os.path.normpath(str(value).removeprefix("\\\\?\\")))
    return normalize(left) == normalize(right)


def read_json(path: Path, max_bytes: int | None = None) -> object:
    try:
        if max_bytes is not None and path.stat().st_size > max_bytes:
            raise UpdateError(f"JSON exceeds size limit: {path}")
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError) as exc:
        raise UpdateError(f"Invalid JSON: {path}") from exc


from updater_state import UpdaterStateMixin


def atomic_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}-{os.urandom(4).hex()}")
    payload = (json.dumps(value, indent=2) + "\n").encode("utf-8")
    try:
        with temporary.open("wb") as handle:
            os.chmod(temporary, 0o644)
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        try:
            directory = os.open(path.parent, os.O_RDONLY)
        except OSError:
            directory = -1
        if directory >= 0:
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def atomic_symlink(path: Path, target: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}-{os.urandom(4).hex()}")
    try:
        os.symlink(target, temporary)
        try:
            os.replace(temporary, path)
        except PermissionError:
            if os.name != "nt":
                raise
            path.unlink(missing_ok=True)
            os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


class Updater(UpdaterStateMixin):
    error = UpdateError

    def __init__(self) -> None:
        self.repo = os.environ.get("OCTESSERA_UPDATE_REPO", "nexxyz/octessera")
        self.root = Path(os.environ.get("OCTESSERA_UPDATE_ROOT", "/opt/octessera"))
        self.releases = self.root / "releases"
        self.state_path = self.root / "update-state.json"
        self.transaction_path = self.root / "update-transaction.json"
        self.legacy_next_path = self.state_path.with_name("update-state.json.next")
        self.bin_link = Path(os.environ.get("OCTESSERA_UPDATE_BIN_LINK", "/usr/local/bin/octessera-pi"))
        self.lock_path = Path(os.environ.get("OCTESSERA_UPDATE_LOCK", str(self.root / ".update.lock")))
        self.service = os.environ.get("OCTESSERA_UPDATE_SERVICE", DEFAULT_SERVICE_PATH)
        self.service_name = os.environ.get("OCTESSERA_UPDATE_SERVICE_NAME", DEFAULT_SERVICE)
        self.recovery_service_name = DEFAULT_RECOVERY_SERVICE
        self.systemctl = os.environ.get("OCTESSERA_UPDATE_SYSTEMCTL", "systemctl")
        self.curl_command = os.environ.get("OCTESSERA_UPDATE_CURL", "curl")
        self.health_path = Path(os.environ.get("OCTESSERA_CANDIDATE_HEALTH_PATH", "/run/octessera/candidate-ready.json"))
        self.test_mode = os.environ.get("OCTESSERA_UPDATE_TEST_MODE") == "1"
        self.require_default_service()
        self.profile = self.profile_from_environment()

    def require_default_service(self) -> None:
        if self.test_mode:
            return
        if self.service != DEFAULT_SERVICE_PATH or self.service_name != DEFAULT_SERVICE:
            raise UpdateError("Updater refuses a nondefault runtime service")

    def profile_from_environment(self) -> str:
        explicit = os.environ.get("OCTESSERA_UPDATE_BOARD_PROFILE")
        effective_uid = getattr(os, "geteuid", lambda: -1)()
        if explicit and effective_uid != 0:
            return explicit
        for path in (
            Path("/etc/octessera/board-profile.env"),
            Path("/etc/octessera/build-metadata.env"),
            self.root / "etc/octessera/board-profile.env",
        ):
            try:
                for line in path.read_text(encoding="utf-8").splitlines():
                    if line.startswith("OCTESSERA_BOARD_PROFILE_ID="):
                        return line.split("=", 1)[1].strip()
            except OSError:
                continue
        return ""

    def require_profile(self) -> None:
        if not self.profile:
            raise UpdateError("Board profile is unavailable; provision or reflash this device before updating")

    def require_profile_asset(self) -> None:
        self.require_profile()
        if self.profile != RASPBERRY_PROFILE:
            raise UpdateError(
                f"No published updater asset exists for board profile {self.profile}; refusing network access."
            )

    def require_repo(self) -> None:
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", self.repo):
            raise UpdateError("Invalid repository")

    def release_api(self, tag: str) -> str:
        suffix = f"/releases/tags/{tag}" if tag else "/releases/latest"
        return f"https://api.github.com/repos/{self.repo}{suffix}"

    def curl(self, url: str, output: Path, max_bytes: int) -> None:
        try:
            subprocess.run(
                [self.curl_command, "--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--tlsv1.2", "--connect-timeout", "10", "--max-time", "120", "--max-filesize", str(max_bytes), "--header", "Accept: application/vnd.github+json", url, "--output", str(output)],
                check=True,
                timeout=130,
            )
            if output.stat().st_size > max_bytes:
                raise UpdateError(f"Downloaded file exceeds size limit: {url}")
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
            raise UpdateError(f"Download failed: {url}") from exc

    def release_json(self, tag: str) -> dict:
        if tag and not TAG_RE.fullmatch(tag):
            raise UpdateError("Invalid release tag")
        with tempfile.TemporaryDirectory(prefix="octessera-update-") as temporary:
            path = Path(temporary) / "release.json"
            self.curl(self.release_api(tag), path, MAX_JSON_BYTES)
            payload = read_json(path, MAX_JSON_BYTES)
        if not isinstance(payload, dict):
            raise UpdateError("Release API returned invalid JSON")
        actual_tag = payload.get("tag_name")
        if not isinstance(actual_tag, str) or not TAG_RE.fullmatch(actual_tag):
            raise UpdateError(f"Release tag is invalid: {actual_tag}")
        if tag and actual_tag != tag:
            raise UpdateError("Release API tag does not match the requested tag")
        return payload

    def asset_names(self, release_version: str) -> tuple[str, str]:
        return (
            f"octessera-{release_version}-{self.profile}-device-aarch64.zip",
            f"SHA256SUMS-{self.profile}-device.txt",
        )

    def asset_url(self, payload: dict, name: str, tag: str) -> str:
        assets = payload.get("assets")
        if not isinstance(assets, list):
            raise UpdateError("Release has no asset list")
        matches = [asset for asset in assets if isinstance(asset, dict) and asset.get("name") == name]
        if len(matches) != 1:
            raise UpdateError(f"Release asset is missing or duplicated: {name}")
        expected = f"https://github.com/{self.repo}/releases/download/{tag}/{name}"
        if matches[0].get("browser_download_url") != expected:
            raise UpdateError(f"Unexpected release asset URL: {name}")
        return expected

    def immutable(self, directory: Path) -> None:
        for path in sorted(directory.rglob("*"), reverse=True):
            os.chmod(path, 0o555 if path.is_dir() else (0o555 if path.name == BINARY else 0o444))
        os.chmod(directory, 0o555)

    def atomic_json(self, path: Path, value: object) -> None:
        atomic_json(path, value)

    def now_iso(self) -> str:
        return now_iso()

    def extract_zip(self, archive: Path, destination: Path, expected_version: str) -> dict:
        from updater_assets import extract_zip
        return extract_zip(self, archive, destination, expected_version)

    def download_candidate(self, tag: str) -> tuple[Path, dict]:
        from updater_assets import download_candidate
        return download_candidate(self, tag)

    def transaction(self, candidate: Path, manifest: dict, fallback: dict, operation: str) -> dict:
        return {
            "schema_version": TRANSACTION_SCHEMA,
            "phase": "prepared",
            "operation": operation,
            "candidate_source": "downloaded" if operation == "apply" else "installed",
            "board_profile": self.profile,
            "candidate_health_protocol": CANDIDATE_HEALTH_PROTOCOL,
            "activation_attempted": False,
            "transaction_id": os.urandom(16).hex(),
            "prepared_at": now_iso(),
            "service": self.service_name,
            "health_path": str(self.health_path),
            "candidate": {"path": str(candidate.resolve()), "version": candidate.name, "manifest": manifest},
            "fallback": fallback,
        }

    def load_transaction(self, recovery: bool = False) -> dict:
        self.validate_control_file(self.transaction_path)
        payload = read_json(self.transaction_path, MAX_JSON_BYTES)
        if not isinstance(payload, dict) or payload.get("schema_version") != TRANSACTION_SCHEMA:
            raise UpdateError("Update transaction schema is invalid")
        if payload.get("phase") not in ("prepared", "validating"):
            raise UpdateError("Update transaction phase is invalid")
        candidate = payload.get("candidate")
        fallback = payload.get("fallback")
        if not isinstance(candidate, dict) or not isinstance(fallback, dict) or not version(candidate.get("version")):
            raise UpdateError("Update transaction identity is invalid")
        activation_attempted = payload.get("activation_attempted", False)
        if not isinstance(activation_attempted, bool):
            raise UpdateError("Update transaction activation state is invalid")
        candidate_path = Path(candidate.get("path", "")).resolve(strict=False)
        if not same_path(candidate_path, (self.releases / candidate["version"]).resolve(strict=False)):
            raise UpdateError("Update transaction candidate is outside releases")
        candidate_source = payload.get("candidate_source")
        if candidate_source not in ("downloaded", "installed"):
            raise UpdateError("Update transaction candidate source is invalid")
        transaction_profile = payload.get("board_profile")
        actual_manifest = self.validate_release(candidate_path, allow_legacy=candidate_source == "installed", require_immutable=candidate_source == "downloaded", expected_profile=transaction_profile)
        if candidate.get("manifest") != actual_manifest:
            raise UpdateError("Update transaction candidate manifest is not authentic")
        if transaction_profile and actual_manifest.get("board_profile") not in (None, transaction_profile):
            raise UpdateError("Update transaction board profile is invalid")
        if not recovery and transaction_profile and self.profile and transaction_profile != self.profile:
            raise UpdateError("Update transaction board profile does not match this device")
        if not recovery and candidate_source == "downloaded" and (not transaction_profile or payload.get("candidate_health_protocol") != CANDIDATE_HEALTH_PROTOCOL or (self.profile and actual_manifest.get("board_profile") != self.profile)):
            raise UpdateError("Downloaded candidate health protocol is not protocol 1")
        if payload.get("service") != self.service_name or payload.get("health_path") != str(self.health_path):
            raise UpdateError("Update transaction adapter identity is invalid")
        fallback_current = fallback.get("current")
        if not version(fallback_current):
            raise UpdateError("Update transaction fallback is invalid")
        self.validate_release(self.releases / str(fallback_current), allow_legacy=True)
        fallback_current_link = fallback.get("current_link")
        fallback_bin_link = fallback.get("bin_link")
        if not isinstance(fallback_current_link, str) or not same_path((self.root / "current").parent / fallback_current_link, self.releases / str(fallback_current)):
            raise UpdateError("Update transaction current fallback link is unsafe")
        if not isinstance(fallback_bin_link, str) or not same_path(self.bin_link.parent / fallback_bin_link, self.root / "current" / BINARY):
            raise UpdateError("Update transaction binary fallback link is unsafe")
        fallback_previous = fallback.get("previous")
        if fallback_previous is not None:
            if not version(fallback_previous):
                raise UpdateError("Update transaction previous fallback is invalid")
            self.validate_release(self.releases / str(fallback_previous), allow_legacy=True)
        return payload

    def write_committed_state(self, current: str, previous: str | None, manifest: dict, asset: object | None) -> None:
        atomic_json(self.state_path, {"schema_version": 2, "phase": "committed", "current": current, "previous": previous, "updated_at": now_iso(), "release": manifest, "asset": asset})

    def restore_transaction(self, payload: dict, stop_service: bool) -> None:
        fallback = payload["fallback"]
        if stop_service:
            self.stop_service_verified()
        atomic_symlink(self.root / "current", fallback["current_link"])
        if not self.bin_link.is_symlink() or os.readlink(self.bin_link) != fallback["bin_link"]:
            atomic_symlink(self.bin_link, fallback["bin_link"])
        if fallback.get("state") is None:
            self.state_path.unlink(missing_ok=True)
        else:
            atomic_json(self.state_path, fallback["state"])
        self.transaction_path.unlink(missing_ok=True)
        self.health_path.unlink(missing_ok=True)
        candidate = Path(payload["candidate"]["path"])
        if candidate.name.startswith(".candidate-"):
            shutil.rmtree(candidate, ignore_errors=True)
        if stop_service:
            self.start_service_verified(fallback["current"])

    def recover_legacy(self, force: bool = False) -> None:
        state = self.state()
        if self.legacy_next_path.exists():
            self.validate_control_file(self.legacy_next_path)
        pending = self.legacy_next_path.exists() or (isinstance(state, dict) and state.get("next"))
        if not pending and not force:
            return
        recorded_value = state.get("current", state.get("active")) if isinstance(state, dict) else None
        if not version(recorded_value):
            raise UpdateError("Incomplete legacy update has no safe recorded current release")
        recorded = str(recorded_value)
        self.validate_release(self.releases / recorded, allow_legacy=True)
        current = None
        try:
            current, _ = self.current_link()
        except UpdateError:
            pass
        if current != recorded:
            atomic_symlink(self.root / "current", str(self.releases / recorded))
        atomic_symlink(self.bin_link, str(self.root / "current" / BINARY))
        if isinstance(state, dict) and pending:
            migrated = copy.deepcopy(state)
            migrated.pop("next", None)
            migrated["schema_version"] = 2
            migrated["phase"] = "committed"
            migrated["current"] = recorded
            self.write_committed_state(recorded, migrated.get("previous"), self.validate_release(self.releases / recorded, allow_legacy=True), migrated.get("asset"))
        self.legacy_next_path.unlink(missing_ok=True)

    def require_recovery_active(self) -> None:
        from updater_guard import require_recovery_active; require_recovery_active(self)

    def verify_service_inactive(self) -> None:
        from updater_guard import verify_service_inactive; verify_service_inactive(self)

    def systemctl_call(self, arguments: list[str], tolerate: bool = False) -> str:
        try:
            result = subprocess.run([self.systemctl, *arguments], check=not tolerate, text=True, capture_output=True, timeout=SYSTEMCTL_TIMEOUT_SECONDS)
        except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
            if tolerate:
                return ""
            raise UpdateError(f"systemd scheduling failed: {' '.join(arguments)}") from exc
        return result.stdout.strip()

    def prepare(self, candidate: Path, manifest: dict, operation: str) -> None:
        self.recover_pending()
        self.require_recovery_active()
        fallback = self.fallback()
        if candidate.name == fallback["current"]:
            raise UpdateError("Requested release is already current")
        payload = self.transaction(candidate, manifest, fallback, operation)
        atomic_json(self.transaction_path, payload)
        self.health_path.unlink(missing_ok=True)
        try:
            self.systemctl_call(["stop", "octessera-update-guard.service"], tolerate=True)
            self.systemctl_call(["start", "octessera-update-guard.service"])
            self.confirm_guard_scheduled()
            atomic_symlink(self.root / "current", str(candidate))
            payload["phase"] = "validating"
            atomic_json(self.transaction_path, payload)
        except UpdateError:
            if self.transaction_path.exists():
                self.restore_transaction(payload, stop_service=False)
            raise

    def confirm_guard_scheduled(self) -> None:
        output = self.systemctl_call(["show", "octessera-update-guard.service", "-p", "ActiveState", "-p", "SubState"])
        properties = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
        if properties.get("ActiveState") != "active" or properties.get("SubState") not in ("running", "start"):
            raise UpdateError("Update guard was not scheduled")

    def stop_service_verified(self) -> None:
        try:
            self.systemctl_properties()
        except UpdateError:
            pass
        self.systemctl_call(["stop", self.service_name])
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            state = self.systemctl_properties()
            if state.get("MainPID", "0") == "0" and state.get("ActiveState") in ("inactive", "failed"):
                return
            time.sleep(0.1)
        raise UpdateError("Candidate service did not stop")

    def start_service_verified(self, expected_version: str) -> None:
        self.systemctl_call(["start", self.service_name])
        expected = (self.releases / expected_version / BINARY).resolve(strict=False)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            state = self.systemctl_properties()
            if state.get("ActiveState") == "active" and int(state.get("MainPID", "0") or "0") > 0:
                executable = Path(os.readlink(Path(os.environ.get("OCTESSERA_UPDATE_PROC_ROOT", "/proc")) / state["MainPID"] / "exe")).resolve(strict=False)
                if not same_path(executable, expected):
                    raise UpdateError("Fallback service executable identity mismatch")
                return
            time.sleep(0.1)
        raise UpdateError("Fallback service did not become active")

    def check(self, requested_tag: str) -> None:
        self.require_profile_asset()
        self.require_repo()
        payload = self.release_json(requested_tag)
        tag = payload["tag_name"]
        release_version = tag[1:]
        archive_name, sums_name = self.asset_names(release_version)
        self.asset_url(payload, archive_name, tag)
        self.asset_url(payload, sums_name, tag)
        try:
            current, _ = self.current_link()
            compatibility = "ready" if self.current_protocol_manifest(self.validate_release(self.releases / current, allow_legacy=True)) else "legacy-provision-required"
        except UpdateError:
            current = "unmanaged"
            compatibility = "provision-required"
        pending = "yes" if self.transaction_path.exists() else "no"
        print(f"available={tag} current={current} compatibility={compatibility} pending={pending}")

    def apply(self, requested_tag: str) -> None:
        self.require_profile_asset()
        self.require_repo()
        self.require_recovery_active()
        self.recover_pending()
        fallback = self.fallback()
        if fallback["legacy"]:
            raise UpdateError("Legacy installation requires provisioning or reflash before online apply")
        self.releases.mkdir(parents=True, exist_ok=True)
        candidate, manifest = self.download_candidate(requested_tag)
        self.prepare(candidate, manifest, "apply")
        print("Update health validation scheduled.")

    def rollback(self) -> None:
        self.require_profile()
        self.require_recovery_active()
        self.recover_pending()
        fallback = self.fallback()
        if not fallback["previous"]:
            raise UpdateError("No previous release recorded")
        candidate = self.releases / fallback["previous"]
        manifest = self.validate_release(candidate, allow_legacy=True)
        self.immutable(candidate)
        self.prepare(candidate, manifest, "rollback")
        print("Update health validation scheduled.")

    def bootstrap(self) -> None:
        self.require_profile()
        self.require_recovery_active()
        current = self.bootstrap_legacy()
        if current is None:
            print("No managed release requires legacy bootstrap.")
        else:
            print(f"Managed release bootstrap complete: {current}")

    def guard(self) -> None:
        from updater_guard import guard_transaction
        guard_transaction(self)

    def systemctl_properties(self) -> dict[str, str]:
        from updater_guard import systemctl_properties
        return systemctl_properties(self)

    def recover(self, boot: bool = False) -> None: self.recover_pending(boot=boot)

    def locked(self, operation: str, *args: str) -> None:
        from updater_state import updater_lock

        with updater_lock(self):
            if operation == "check":
                self.check(args[0] if args else "")
            if operation == "apply":
                self.apply(args[0] if args else "")
            elif operation == "rollback":
                self.rollback()
            elif operation == "bootstrap":
                self.bootstrap()
            elif operation == "guard":
                self.guard()
            elif operation == "recover":
                self.recover(args[0] == "--boot" if args else False)
