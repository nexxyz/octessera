from __future__ import annotations

import os
import fnmatch
import secrets
import shutil
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    from .inventory import Inventory, InventoryError, build_inventory, changed_paths, remove_path
    from .runtime_contract import MutationError
except ImportError:
    from inventory import Inventory, InventoryError, build_inventory, changed_paths, remove_path
    from runtime_contract import MutationError


def _safe_path(root: Path, relative: str) -> Path:
    current = Path(root).resolve(strict=False)
    if relative == ".":
        return current
    for part in relative.split("/"):
        if part in {"", ".", ".."}:
            raise MutationError(f"snapshot path is unsafe: {relative}")
        try:
            metadata = current.lstat()
        except OSError as exc:
            raise MutationError(f"snapshot parent disappeared: {relative}") from exc
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise MutationError(f"snapshot path has a symlink parent: {relative}")
        current /= part
    return current


def _set_metadata(path: Path, entry: dict[str, Any]) -> None:
    chown = getattr(os, "chown", None)
    if chown is not None and os.name != "nt":
        chown(path, entry["uid"], entry["gid"], follow_symlinks=entry["type"] != "symlink")
    if entry["type"] != "symlink":
        os.chmod(path, entry["mode"])
    setxattr = getattr(os, "setxattr", None)
    removexattr = getattr(os, "removexattr", None)
    listxattr = getattr(os, "listxattr", None)
    if setxattr is None or removexattr is None or listxattr is None:
        if entry["xattrs"]:
            raise MutationError(f"cannot restore extended attributes: {path}")
        return
    try:
        current = {str(name) for name in listxattr(path, follow_symlinks=False)}
    except OSError as exc:
        if not entry["xattrs"] and exc.errno in {getattr(os, "ENOTSUP", 95), getattr(os, "EOPNOTSUPP", 95)}:
            return
        raise MutationError(f"cannot list rollback extended attributes: {path}") from exc
    for name in current - set(entry["xattrs"]):
        removexattr(path, name, follow_symlinks=False)
    for name, value in entry["xattrs"].items():
        setxattr(path, name, bytes.fromhex(value), follow_symlinks=False)


@dataclass
class MutableSnapshot:
    root: Path
    entries: Inventory
    mutable_roots: tuple[str, ...]
    cleanup_patterns: tuple[str, ...]
    directory: Path

    @classmethod
    def capture(cls, root: Path, before: Inventory, mutable_roots: tuple[str, ...], cleanup_patterns: tuple[str, ...]) -> "MutableSnapshot":
        directory = Path(tempfile.mkdtemp(prefix="octessera-runtime-snapshot-"))
        entries = {path: entry for path, entry in before.items() if any(path == mutable or path.startswith(mutable + "/") for mutable in mutable_roots)}
        snapshot = cls(Path(root).resolve(strict=False), entries, mutable_roots, cleanup_patterns, directory)
        try:
            for relative, entry in entries.items():
                if entry["type"] != "file":
                    continue
                source = _safe_path(snapshot.root, relative)
                destination = directory / Path(relative)
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, destination)
            return snapshot
        except Exception:
            snapshot.close()
            raise

    def close(self) -> None:
        remove_path(self.directory)

    def _restore_entry(self, relative: str, entry: dict[str, Any]) -> None:
        path = _safe_path(self.root, relative)
        try:
            current = path.lstat()
        except FileNotFoundError:
            current = None
        if entry["type"] == "file":
            if current is None or not stat.S_ISREG(current.st_mode) or stat.S_ISLNK(current.st_mode):
                if current is not None:
                    remove_path(path)
                path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.directory / Path(relative), path)
        elif entry["type"] == "directory":
            if current is None:
                path.mkdir(parents=True)
            elif not stat.S_ISDIR(current.st_mode) or stat.S_ISLNK(current.st_mode):
                remove_path(path)
                path.mkdir(parents=True)
        elif entry["type"] == "symlink":
            if current is None or not stat.S_ISLNK(current.st_mode) or os.readlink(path) != entry["target"]:
                if current is not None:
                    remove_path(path)
                path.parent.mkdir(parents=True, exist_ok=True)
                os.symlink(entry["target"], path)
        else:
            special_types = {"fifo": stat.S_ISFIFO, "character-device": stat.S_ISCHR, "block-device": stat.S_ISBLK, "socket": stat.S_ISSOCK}
            if current is None or not special_types.get(entry["type"], lambda _: False)(current.st_mode):
                raise MutationError(f"rollback cannot recreate special path: {relative}")
        _set_metadata(path, entry)

    def _is_mutable(self, relative: str) -> bool:
        return any(relative == mutable or relative.startswith(mutable + "/") for mutable in self.mutable_roots) or any(fnmatch.fnmatchcase(relative, pattern) for pattern in self.cleanup_patterns)

    def restore(self) -> None:
        current = build_inventory(self.root)
        current_mutable = {path for path in current if self._is_mutable(path)}
        created = sorted(current_mutable - set(self.entries), key=lambda value: (value.count("/"), value), reverse=True)
        for relative in created:
            remove_path(_safe_path(self.root, relative))
        for relative, entry in sorted(self.entries.items(), key=lambda item: (item[0].count("/"), item[0])):
            self._restore_entry(relative, entry)
        restored = build_inventory(self.root)
        restored_mutable = {path: entry for path, entry in restored.items() if self._is_mutable(path)}
        if restored_mutable != self.entries:
            before_full = {path: entry for path, entry in restored.items() if path in self.entries}
            raise MutationError(f"rollback mutable inventory mismatch: {changed_paths(self.entries, restored_mutable)}; retained={sorted(set(before_full) - set(self.entries))}")


def _temporary_name(path: Path) -> Path:
    for _ in range(16):
        candidate = path.with_name(f".{path.name}.image-respin-{secrets.token_hex(16)}")
        if not candidate.exists() and not candidate.is_symlink():
            return candidate
    raise MutationError(f"cannot allocate a private temporary path beside: {path}")


def _open_temporary(path: Path) -> tuple[Path, int]:
    for _ in range(16):
        candidate = path.with_name(f".{path.name}.image-respin-{secrets.token_hex(16)}")
        try:
            return candidate, os.open(candidate, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
        except FileExistsError:
            continue
    raise MutationError(f"cannot allocate a private temporary file beside: {path}")


def _reject_destination_symlink(path: Path) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISLNK(metadata.st_mode):
        raise MutationError(f"atomic replacement destination is a symlink: {path}")


def _fsync_directory(path: Path) -> None:
    if os.name == "nt":
        return
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic_bytes(path: Path, payload: bytes, mode: int) -> None:
    _reject_destination_symlink(path)
    temporary, descriptor = _open_temporary(path)
    try:
        try:
            fchmod = getattr(os, "fchmod", None)
            if os.name != "nt" and hasattr(os, "fchown"):
                os.fchown(descriptor, 0, 0)
                fchmod(descriptor, mode)
            else:
                if fchmod is not None:
                    fchmod(descriptor, mode)
                else:
                    os.chmod(temporary, mode)
            with os.fdopen(descriptor, "wb", closefd=True) as stream:
                descriptor = -1
                stream.write(payload)
                stream.flush()
                os.fsync(stream.fileno())
        except Exception:
            if descriptor >= 0:
                os.close(descriptor)
            raise
        os.replace(temporary, path)
        _fsync_directory(path.parent)
    except OSError as exc:
        raise MutationError(f"cannot atomically write: {path}") from exc
    finally:
        temporary.unlink(missing_ok=True)


def atomic_link(path: Path, target: str) -> None:
    try:
        metadata = path.lstat()
        if not stat.S_ISLNK(metadata.st_mode):
            raise MutationError(f"atomic link destination is not a symlink: {path}")
    except FileNotFoundError:
        pass
    temporary = _temporary_name(path)
    try:
        for _ in range(16):
            try:
                os.symlink(target, temporary)
                break
            except FileExistsError:
                temporary = _temporary_name(path)
        else:
            raise MutationError(f"cannot allocate a private link path beside: {path}")
        if os.name != "nt" and hasattr(os, "lchown"):
            os.lchown(temporary, 0, 0)
        os.replace(temporary, path)
        _fsync_directory(path.parent)
    except OSError as exc:
        raise MutationError(f"cannot atomically replace link: {path}") from exc
    finally:
        temporary.unlink(missing_ok=True)
