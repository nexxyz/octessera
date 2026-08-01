from __future__ import annotations

import os
import stat
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator


RASPI_FIRMWARE_HOOKS = (
    "etc/initramfs/post-update.d/z50-raspi-firmware",
    "etc/kernel/postinst.d/z50-raspi-firmware",
)


class RaspiFirmwareHookMaskError(ValueError):
    pass


@dataclass(frozen=True)
class _HookState:
    path: Path
    contents: bytes
    mode: int
    uid: int
    gid: int


def _snapshot(path: Path) -> _HookState:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        raise RaspiFirmwareHookMaskError(f"missing Raspberry firmware hook: {path}") from None
    except OSError as error:
        raise RaspiFirmwareHookMaskError(f"cannot inspect Raspberry firmware hook: {path}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise RaspiFirmwareHookMaskError(f"unexpected Raspberry firmware hook path type: {path}")
    mode = stat.S_IMODE(metadata.st_mode)
    if not mode & 0o111:
        raise RaspiFirmwareHookMaskError(f"Raspberry firmware hook is not executable: {path}")
    try:
        contents = path.read_bytes()
    except OSError as error:
        raise RaspiFirmwareHookMaskError(f"cannot snapshot Raspberry firmware hook: {path}") from error
    return _HookState(path, contents, mode, metadata.st_uid, metadata.st_gid)


def _require_regular(path: Path) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise RaspiFirmwareHookMaskError(f"Raspberry firmware hook disappeared during restoration: {path}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise RaspiFirmwareHookMaskError(f"unexpected Raspberry firmware hook path type during restoration: {path}")


def _restore(state: _HookState) -> None:
    path = state.path
    try:
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            with path.open("xb") as handle:
                handle.write(state.contents)
        else:
            if not stat.S_ISREG(metadata.st_mode):
                raise RaspiFirmwareHookMaskError(f"unexpected Raspberry firmware hook path type during restoration: {path}")
            path.write_bytes(state.contents)
        _require_regular(path)
        os.chown(path, state.uid, state.gid)  # type: ignore[attr-defined]
        os.chmod(path, state.mode)
        _require_regular(path)
        if path.read_bytes() != state.contents:
            raise RaspiFirmwareHookMaskError(f"Raspberry firmware hook bytes were not restored: {path}")
        metadata = path.stat()
        if stat.S_IMODE(metadata.st_mode) != state.mode:
            raise RaspiFirmwareHookMaskError(f"Raspberry firmware hook mode was not restored: {path}")
        if metadata.st_uid != state.uid or metadata.st_gid != state.gid:
            raise RaspiFirmwareHookMaskError(f"Raspberry firmware hook ownership was not restored: {path}")
    except RaspiFirmwareHookMaskError:
        raise
    except OSError as error:
        raise RaspiFirmwareHookMaskError(f"cannot restore Raspberry firmware hook: {path}") from error


def _restore_all(states: list[_HookState]) -> list[RaspiFirmwareHookMaskError]:
    failures: list[RaspiFirmwareHookMaskError] = []
    for state in reversed(states):
        try:
            _restore(state)
        except RaspiFirmwareHookMaskError as error:
            failures.append(error)
    return failures


def _restoration_error(failures: list[RaspiFirmwareHookMaskError]) -> RaspiFirmwareHookMaskError:
    detail = "; ".join(str(error) for error in failures)
    return RaspiFirmwareHookMaskError(f"Raspberry firmware hook restoration failed: {detail}")


@contextmanager
def temporarily_mask_raspi_firmware_hooks(rootfs: Path) -> Iterator[None]:
    states = [_snapshot(rootfs / relative) for relative in RASPI_FIRMWARE_HOOKS]
    try:
        for state in states:
            os.chmod(state.path, state.mode & ~0o111)
    except OSError as error:
        failures = _restore_all(states)
        if failures:
            raise RaspiFirmwareHookMaskError(
                f"Raspberry firmware hook masking failed: {error}; {_restoration_error(failures)}"
            ) from error
        raise RaspiFirmwareHookMaskError("cannot mask Raspberry firmware hooks") from error
    try:
        yield
    except BaseException as error:
        failures = _restore_all(states)
        if failures:
            raise RaspiFirmwareHookMaskError(
                f"Raspberry firmware hook operation failed: {type(error).__name__}: {error}; "
                f"{_restoration_error(failures)}"
            ) from error
        raise
    else:
        failures = _restore_all(states)
        if failures:
            raise _restoration_error(failures) from failures[0]
