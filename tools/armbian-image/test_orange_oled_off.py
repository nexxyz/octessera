#!/usr/bin/env python3
import importlib.util
import sys
import types
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
HANDOFF = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
try:
    import fcntl  # noqa: F401
except ImportError:
    sys.modules["fcntl"] = types.ModuleType("fcntl")
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    pwd_stub.getpwnam = lambda name: (_ for _ in ()).throw(KeyError(name))
    sys.modules["pwd"] = pwd_stub

handoff_spec = importlib.util.spec_from_loader("octessera_orange_oled_handoff", SourceFileLoader("octessera_orange_oled_handoff", str(HANDOFF)))
assert handoff_spec is not None and handoff_spec.loader is not None
handoff = importlib.util.module_from_spec(handoff_spec)
handoff_spec.loader.exec_module(handoff)
sys.modules["octessera_orange_oled_handoff"] = handoff
logo_spec = importlib.util.spec_from_loader("octessera_orange_oled_logo_off", SourceFileLoader("octessera_orange_oled_logo_off", str(SCRIPT)))
assert logo_spec is not None and logo_spec.loader is not None
logo = importlib.util.module_from_spec(logo_spec)
logo_spec.loader.exec_module(logo)
logo.BOOT_RGB565_SOURCE = str(ROOT / "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565")
logo.SHUTDOWN_RGB565_SOURCE = str(ROOT / "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565")


class FakeLock:
    def __init__(self, timeout):
        self.timeout = timeout
        self.closed = False

    def close(self):
        self.closed = True


class FakeOled:
    instances = []

    def __init__(self):
        self.initialized = False
        self.frames = []
        self.commands = []
        self.close_args = []
        self.__class__.instances.append(self)

    def initialize(self):
        self.initialized = True

    def frame(self, payload):
        self.frames.append(payload)

    def command(self, *values):
        self.commands.append(values)

    def close(self, display_off=True):
        self.close_args.append(display_off)


locks = []
real_oled = logo.Oled
real_handoff = logo.Handoff
real_drop = logo.drop_to_runtime
try:
    logo.Oled = FakeOled
    logo.Handoff = types.SimpleNamespace(utility_lock=lambda timeout: locks.append(FakeLock(timeout)) or locks[-1])
    logo.drop_to_runtime = lambda: None
    logo.run("boot-static")
    logo.run("sleep")
    logo.run("shutdown")
    logo.run("off")
finally:
    logo.Oled = real_oled
    logo.Handoff = real_handoff
    logo.drop_to_runtime = real_drop

boot_oled, sleep_oled, shutdown_oled, off_oled = FakeOled.instances
assert boot_oled.initialized and len(boot_oled.frames) == 1 and boot_oled.close_args == [False]
assert sleep_oled.initialized and len(sleep_oled.frames) == 1 and sleep_oled.close_args == [False]
assert shutdown_oled.initialized and len(shutdown_oled.frames) == 1 and shutdown_oled.close_args == [False]
assert not off_oled.initialized and not off_oled.frames and off_oled.commands == [(0xAE,)] and off_oled.close_args == [False]
assert [lock.timeout for lock in locks] == [logo._handoff_module.UTILITY_LOCK_TIMEOUT_SECONDS, logo._handoff_module.SHUTDOWN_LOCK_TIMEOUT_SECONDS, logo._handoff_module.UTILITY_LOCK_TIMEOUT_SECONDS]
assert all(lock.closed for lock in locks)


def busy_lock(timeout):
    raise TimeoutError("test OLED lock contention")


before = len(FakeOled.instances)
logo.Oled = FakeOled
logo.Handoff = types.SimpleNamespace(utility_lock=busy_lock)
logo.drop_to_runtime = lambda: None
logo.run("off")
assert len(FakeOled.instances) == before

print("Orange OLED off utility lock, display-off command, and no-frame tests passed")
