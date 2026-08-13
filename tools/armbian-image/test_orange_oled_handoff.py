#!/usr/bin/env python3
import importlib.util
import json
import os
import subprocess
import stat
import sys
import tempfile
import types
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    pwd_stub.getpwnam = lambda name: (_ for _ in ()).throw(KeyError(name))
    sys.modules["pwd"] = pwd_stub
if "fcntl" not in sys.modules:
    try:
        import fcntl  # noqa: F401
    except ImportError:
        stub = types.ModuleType("fcntl")
        sys.modules["fcntl"] = stub
spec = importlib.util.spec_from_loader("orange_oled_handoff", SourceFileLoader("orange_oled_handoff", str(SOURCE)))
assert spec is not None and spec.loader is not None
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)
sys.modules["octessera_orange_oled_handoff"] = handoff
assert 0 < handoff.UTILITY_LOCK_TIMEOUT_SECONDS < handoff.SHUTDOWN_LOCK_TIMEOUT_SECONDS < 5


def expect_error(operation):
    try:
        operation()
    except (OSError, RuntimeError, ValueError):
        return
    raise AssertionError("invalid OLED handoff input was accepted")


def runtime_available():
    try:
        handoff.runtime_identity()
    except RuntimeError:
        return False
    return True


for phase in ("animating", "release_requested", "released", "native_owned", "first_menu_rendered", "failed"):
    request = None if phase == "animating" else "0123456789abcdef0123456789abcdef"
    value = handoff._status(phase, "01234567-89ab-cdef-0123-456789abcdef", 42, 3, request)
    assert value["schema"] == 1 and value["phase"] == phase
    assert handoff.parse_status(value) == value
expect_error(lambda: handoff.parse_status({"schema": 1, "phase": "animating"}))
expect_error(lambda: handoff.parse_status({"schema": 1, "phase": "animating", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "pid": 42, "cycleCount": 1, "requestId": "0" * 32}))
expect_error(lambda: handoff.parse_stop({"schema": 1, "bootId": "01234567-89ab-cdef-0123-456789abcdef", "pid": 42, "requestId": "A" * 32}))


class SlowProcess:
    def __init__(self):
        self.terminated = False
        self.killed = False
        self.waits = 0

    def poll(self):
        return None

    def terminate(self):
        self.terminated = True

    def kill(self):
        self.killed = True

    def wait(self, timeout=None):
        self.waits += 1
        if self.waits == 1:
            raise subprocess.TimeoutExpired("gpioset", 1)


process = SlowProcess()
try:
    import importlib.machinery
    logo_spec = importlib.util.spec_from_loader("orange_oled_logo_for_cleanup", importlib.machinery.SourceFileLoader("orange_oled_logo_for_cleanup", str(ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo")))
    logo = importlib.util.module_from_spec(logo_spec)
    logo_spec.loader.exec_module(logo)
    logo.GpioLines._stop(process)
except TimeoutError:
    raise AssertionError("GPIO child cleanup did not reap a timed-out child")
assert process.terminated and process.killed and process.waits == 2


if getattr(handoff, "fcntl", None) is not None and hasattr(handoff.fcntl, "flock") and hasattr(os, "geteuid") and os.geteuid() == 0 and runtime_available():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "octessera-boot"
        root.mkdir(mode=0o750)
        os.chmod(root, 0o750)
        handoff.HANDOFF_ROOT = str(root)
        h = handoff.Handoff.open(True)
        h.start()
        assert h._read_status()["phase"] == "animating"
        h.mark_failed()
        failed_status = h._read_status()
        failed_stop = h._read_stop()
        assert failed_status["phase"] == "failed"
        assert failed_status["bootId"] == h.boot_id
        assert failed_status["requestId"] == failed_stop["requestId"]
        assert failed_stop["bootId"] == h.boot_id
        real_flock = handoff.fcntl.flock

        def blocked_flock(descriptor, flags):
            if flags & handoff.fcntl.LOCK_NB:
                raise BlockingIOError()
            return real_flock(descriptor, flags)

        handoff.fcntl.flock = blocked_flock
        started = handoff.time.monotonic()
        expect_error(lambda: handoff.Handoff.utility_lock(0.05))
        elapsed = handoff.time.monotonic() - started
        assert 0.04 <= elapsed < 0.5
        assert not (root / "stop.request").exists()
        handoff.fcntl.flock = real_flock
        h._create_stop()
        assert h.stop_requested()
        assert h._read_status()["phase"] == "release_requested"
        h.release()
        h.close()
        utility = handoff.Handoff.utility_lock()
        assert utility._read_status()["phase"] == "released"
        utility.close()
        marker = root.parent / "marker"
        marker.write_text(json.dumps({"schema": 1, "bootId": handoff.current_boot_id()}) + "\n", encoding="utf-8")
        os.chmod(marker, 0o644)
        handoff.INITRAMFS_MARKER = str(marker)
        assert handoff.validate_marker()
        marker.unlink()
        marker.symlink_to(root)
        expect_error(handoff.validate_marker)
        marker.unlink()
        (root / "status.json").unlink()
        (root / "status.json").symlink_to(root / "oled.lock")
        expect_error(h._read_status)

print("Orange OLED handoff schema, marker, lock, transition, and child cleanup tests passed")
