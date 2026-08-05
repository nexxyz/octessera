from __future__ import annotations

import contextlib
import os
import platform
import shutil
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

try:
    from .disk_layout import DiskLayout, DiskLayoutError, capture_layout
except ImportError:
    from disk_layout import DiskLayout, DiskLayoutError, capture_layout


class DiskMountError(RuntimeError):
    def __init__(self, message: str, *, retain_workspace: bool = False, backing_path: Path | None = None) -> None:
        super().__init__(message)
        self.retain_workspace = retain_workspace
        self.backing_path = Path(backing_path) if backing_path is not None else None


def require_linux_root() -> None:
    if platform.system() != "Linux":
        raise DiskMountError("disk-image respin requires Linux")
    if getattr(os, "geteuid", lambda: -1)() != 0:
        raise DiskMountError("disk-image respin requires root")


def _run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(command, check=True, capture_output=capture, text=True)
    except (OSError, subprocess.CalledProcessError) as exc:
        raise DiskMountError(f"disk command failed: {' '.join(command)}") from exc


def _unmount(path: Path) -> None:
    last_error: DiskMountError | None = None
    for attempt in range(3):
        try:
            _run(["umount", str(path)])
            return
        except DiskMountError as exc:
            last_error = exc
            if attempt < 2:
                time.sleep(0.1)
    raise DiskMountError(f"cannot unmount {path} after three normal attempts") from last_error


@dataclass
class MountedRuntime:
    image: Path
    board_profile: str
    loop_device: str | None = None
    root_mount: Path | None = None
    pre_layout: DiskLayout | None = None
    post_layout: DiskLayout | None = None
    root_unmounted: bool = False
    mounted: bool = False
    attached: bool = False
    retain_workspace: bool = False
    _closed: bool = False

    @property
    def backing_path(self) -> Path:
        return self.image.parent

    def open(self) -> None:
        self.loop_device = _run(["losetup", "--find", "--show", "--partscan", str(self.image)], capture=True).stdout.strip()
        if not self.loop_device:
            raise DiskMountError("losetup did not return a loop device")
        self.attached = True
        _run(["udevadm", "settle"])
        try:
            self.pre_layout = capture_layout(self.image, self.board_profile, self.loop_device)
        except DiskLayoutError as exc:
            raise DiskMountError(str(exc)) from exc
        self._check_filesystems(self.pre_layout)
        self.root_mount = Path(tempfile.mkdtemp(prefix="octessera-runtime-root-"))
        root_partition = self.pre_layout.partitions[-1]
        try:
            _run(["mount", "-o", "rw,noatime,nodev,nosuid,noexec", root_partition.node, str(self.root_mount)])
            self.mounted = True
        except Exception:
            shutil.rmtree(self.root_mount, ignore_errors=True)
            self.root_mount = None
            raise

    def _check_filesystems(self, layout: DiskLayout) -> None:
        root_partition = layout.partitions[-1]
        _run(["e2fsck", "-fn", root_partition.node])
        if self.board_profile == "raspberry-pi-zero-2w":
            _run(["fsck.vfat", "-n", layout.partitions[0].node])

    def close(self) -> None:
        if self._closed:
            return
        errors: list[BaseException] = []
        if self.mounted and self.root_mount is not None:
            try:
                _run(["sync"])
                _unmount(self.root_mount)
                self.root_unmounted = True
                self.mounted = False
            except BaseException as exc:
                errors.append(exc)
        if not self.mounted and self.attached and self.loop_device is not None and self.pre_layout is not None:
            try:
                self._check_filesystems(self.pre_layout)
            except BaseException as exc:
                errors.append(exc)
            try:
                self.post_layout = capture_layout(self.image, self.board_profile, self.loop_device)
            except BaseException as exc:
                errors.append(exc)
        if not self.mounted and self.attached and self.loop_device is not None:
            try:
                _run(["losetup", "-d", self.loop_device])
                self.attached = False
            except BaseException as exc:
                errors.append(exc)
        if self.root_mount is not None and not self.mounted and not self.attached and not errors:
            try:
                self.root_mount.rmdir()
            except OSError as exc:
                errors.append(exc)
        if errors:
            self.retain_workspace = True
            root_mount = f"; root mount retained at {self.root_mount}" if self.root_mount is not None else ""
            error = DiskMountError(f"disk cleanup failed: {errors[0]}; retained workspace at {self.backing_path}{root_mount}", retain_workspace=True, backing_path=self.backing_path)
            raise error from errors[0]
        self.retain_workspace = False
        self._closed = True


@contextlib.contextmanager
def mounted_runtime(image: Path, board_profile: str) -> Iterator[MountedRuntime]:
    require_linux_root()
    session = MountedRuntime(Path(image), board_profile)
    try:
        session.open()
        yield session
    except BaseException as original:
        try:
            session.close()
        except BaseException as cleanup:
            retain_workspace = getattr(cleanup, "retain_workspace", False)
            backing_path = getattr(cleanup, "backing_path", None)
            raise DiskMountError(f"disk respin failed and cleanup failed: {cleanup}", retain_workspace=retain_workspace, backing_path=backing_path) from original
        raise
    else:
        session.close()
