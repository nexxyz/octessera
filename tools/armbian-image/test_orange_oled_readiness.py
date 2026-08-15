#!/usr/bin/env python3
import importlib.util
import io
import os
import sys
import types
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
HANDOFF = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
sys.path.insert(0, str(SCRIPT.parent))
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    pwd_stub.getpwnam = lambda name: (_ for _ in ()).throw(KeyError(name))
    sys.modules["pwd"] = pwd_stub
fcntl_stub = types.ModuleType("fcntl")
fcntl_stub.ioctl = lambda *args: None
sys.modules.setdefault("fcntl", fcntl_stub)
handoff_spec = importlib.util.spec_from_loader(
    "orange_oled_readiness_handoff", SourceFileLoader("orange_oled_readiness_handoff", str(HANDOFF))
)
handoff = importlib.util.module_from_spec(handoff_spec)
handoff_spec.loader.exec_module(handoff)
sys.modules["octessera_orange_oled_handoff"] = handoff
spec = importlib.util.spec_from_loader(
    "orange_oled_readiness_logo", SourceFileLoader("orange_oled_readiness_logo", str(SCRIPT))
)
logo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logo)


class FakeProcess:
    def __init__(self, command, **kwargs):
        self.command = command
        self.kwargs = kwargs
        self.stderr = io.StringIO()
        self.exit_code = None

    def poll(self):
        return self.exit_code

    def terminate(self):
        self.exit_code = 0

    def wait(self, timeout=None):
        return self.exit_code


processes = []
real_stat = logo.os.stat
real_is_char = logo.stat.S_ISCHR
real_popen = logo.subprocess.Popen
real_sleep = logo.time.sleep
try:
    logo.os.stat = lambda path: types.SimpleNamespace(st_mode=0)
    logo.stat.S_ISCHR = lambda mode: True
    logo.subprocess.Popen = lambda command, **kwargs: processes.append(FakeProcess(command, **kwargs)) or processes[-1]
    logo.time.sleep = lambda seconds: None
    gpio = logo.GpioLines()
    gpio.set("dc", logo.GPIO_DC, 0)
    processes[-1].exit_code = 1
    try:
        gpio.set("dc", logo.GPIO_DC, 0)
    except RuntimeError as error:
        assert str(error) == "H618 GPIO dc holder exited unexpectedly"
    else:
        raise AssertionError("dead GPIO holder was reused")
finally:
    logo.os.stat = real_stat
    logo.stat.S_ISCHR = real_is_char
    logo.subprocess.Popen = real_popen
    logo.time.sleep = real_sleep


events = []
class FakeNotifySocket:
    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def sendto(self, payload, address):
        events.append((payload, address))


real_ready_env = os.environ.get(logo.READY_NOTIFY_REQUIRED_ENV)
real_notify_socket = os.environ.get("NOTIFY_SOCKET")
real_socket_module = sys.modules.get("socket")
try:
    fake_socket = types.ModuleType("socket")
    fake_socket.AF_UNIX = 1
    fake_socket.SOCK_DGRAM = 2
    fake_socket.socket = lambda *_args: FakeNotifySocket()
    sys.modules["socket"] = fake_socket
    os.environ[logo.READY_NOTIFY_REQUIRED_ENV] = "1"
    os.environ["NOTIFY_SOCKET"] = "@octessera-test"
    logo.notify_systemd_ready()
    assert events[-1] == (b"READY=1\n", "\0octessera-test")
    os.environ.pop("NOTIFY_SOCKET", None)
    try:
        logo.notify_systemd_ready()
    except RuntimeError as error:
        assert str(error) == "systemd OLED readiness notification is required but NOTIFY_SOCKET is missing"
    else:
        raise AssertionError("missing required systemd OLED readiness socket was accepted")
finally:
    if real_ready_env is None:
        os.environ.pop(logo.READY_NOTIFY_REQUIRED_ENV, None)
    else:
        os.environ[logo.READY_NOTIFY_REQUIRED_ENV] = real_ready_env
    if real_notify_socket is None:
        os.environ.pop("NOTIFY_SOCKET", None)
    else:
        os.environ["NOTIFY_SOCKET"] = real_notify_socket
    if real_socket_module is None:
        sys.modules.pop("socket", None)
    else:
        sys.modules["socket"] = real_socket_module

print("Orange OLED GPIO ownership and systemd readiness tests passed")
