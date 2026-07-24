#!/usr/bin/env python3
import fcntl
import os
import stat
import sys
from pathlib import Path
from typing import Callable, cast


def fail(message: str, status: int = 1) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(status)


if len(sys.argv) < 5:
    fail("fast deployment lock helper received too few arguments", 2)

lock_path = Path(sys.argv[1])
expected_uid = 0
try:
    expected_uid = int(sys.argv[2])
except ValueError:
    fail("fast deployment lock helper received an invalid owner UID", 2)
transaction_path = Path(sys.argv[3])
command = sys.argv[4:]
if not command:
    fail("fast deployment lock helper received no command", 2)
nofollow = cast(int | None, getattr(os, "O_NOFOLLOW", None))
if nofollow is None:
    fail("fast deployment lock helper requires O_NOFOLLOW")
nofollow_value = cast(int, nofollow)

descriptor = -1
try:
    parent = lock_path.parent
    parent_stat = parent.lstat()
    if not stat.S_ISDIR(parent_stat.st_mode) or parent.is_symlink() or parent_stat.st_uid != expected_uid or parent_stat.st_mode & 0o022:
        fail("fast deployment lock directory is unsafe")
    flags = os.O_RDWR | os.O_CREAT | nofollow_value
    descriptor = os.open(lock_path, flags, 0o600)
except OSError as exc:
    fail(f"fast deployment lock cannot be opened safely: {exc}")

try:
    if descriptor < 0:
        fail("fast deployment lock descriptor was not created")
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != expected_uid:
        fail("fast deployment lock is not an owned regular file")
    fchmod = cast(Callable[[int, int], None] | None, getattr(os, "fchmod", None))
    if fchmod is None:
        fail("fast deployment lock helper requires fchmod")
    cast(Callable[[int, int], None], fchmod)(descriptor, 0o600)
    metadata = os.fstat(descriptor)
    if metadata.st_mode & 0o077:
        fail("fast deployment lock mode could not be restricted")
    try:
        flock = cast(Callable[[int, int], None] | None, getattr(fcntl, "flock", None))
        lock_ex = getattr(fcntl, "LOCK_EX", 0)
        lock_nb = getattr(fcntl, "LOCK_NB", 0)
        if flock is None or not lock_ex or not lock_nb:
            fail("fast deployment lock helper requires flock")
        cast(Callable[[int, int], None], flock)(descriptor, lock_ex | lock_nb)
    except BlockingIOError:
        fail("Updater transaction lock is busy; fast deployment was refused.", 75)
    try:
        transaction_path.lstat()
    except FileNotFoundError:
        pass
    except OSError as exc:
        fail(f"fast deployment transaction state cannot be inspected safely: {exc}")
    else:
        fail("Refusing fast deployment while an updater transaction is pending; use recovery or rollback first.", 75)
    os.set_inheritable(descriptor, True)
    os.execv("/bin/bash", ["bash", *command])
except SystemExit:
    os.close(descriptor)
    raise
except OSError as exc:
    os.close(descriptor)
    fail(f"fast deployment lock failed: {exc}")
