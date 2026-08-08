#!/usr/bin/env python3
import errno
import importlib.util
import json
import os
import socket
import sys
import tempfile
import types
from importlib.machinery import SourceFileLoader
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend"
try:
    import fcntl  # noqa: F401
except ImportError:
    sys.modules["fcntl"] = types.ModuleType("fcntl")
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    setattr(pwd_stub, "getpwnam", lambda name: (_ for _ in ()).throw(KeyError(name)))
    sys.modules["pwd"] = pwd_stub
spec = importlib.util.spec_from_loader("orange_oled_suspend_helper", SourceFileLoader("orange_oled_suspend_helper", str(SOURCE)))
assert spec is not None and spec.loader is not None
helper: Any = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
real_render = helper._render
real_request = helper._request
real_handoff = helper._handoff

if not hasattr(os, "geteuid") or not hasattr(os, "O_NOFOLLOW"):
    print("Orange OLED suspend helper behavior tests skipped outside Unix")
    raise SystemExit(0)


TOKEN = "0123456789abcdef0123456789abcdef"
BOOT_ID = "01234567-89ab-cdef-0123-456789abcdef"


def assert_raises(operation, expected):
    try:
        operation()
    except expected:
        return
    raise AssertionError("expected exception was not raised")


with tempfile.TemporaryDirectory() as directory:
    state_directory = Path(directory) / "state"
    state_directory.mkdir(mode=0o700)
    os.chmod(state_directory, 0o700)
    helper.STATE_PATH = str(state_directory / "token.json")
    helper._runtime_identity = lambda: (getattr(os, "geteuid")(), getattr(os, "getegid")())
    helper._boot_id = lambda: BOOT_ID
    requests = []
    rendered = []

    def request(action, token, boot_id):
        requests.append(action)
        if action == "prepare/commit":
            assert helper._read_state() == (token, boot_id, "staged")

    helper._request = request
    helper._render = lambda kind, boot_id: rendered.append((kind, boot_id))
    helper._prepare()
    state = helper._read_state()
    assert state[1:] == (BOOT_ID, "committed")
    assert requests == ["prepare/release", "prepare/commit"]
    assert rendered == [("sleep", BOOT_ID)]

    helper._remove_state()
    requests.clear()
    helper._request = lambda action, token, boot_id: (_ for _ in ()).throw(RuntimeError("commit rejected")) if action == "prepare/commit" else requests.append(action)
    try:
        helper._prepare()
    except RuntimeError as error:
        assert str(error) == "commit rejected"
    else:
        raise AssertionError("commit failure was accepted")
    assert not Path(helper.STATE_PATH).exists()
    assert requests == ["prepare/release", "rollback"]

    helper._remove_state() if Path(helper.STATE_PATH).exists() else None
    helper._write_state(TOKEN, BOOT_ID, "committed")
    requests.clear()
    helper._request = lambda action, token, boot_id: requests.append(action)
    helper._render = lambda kind, boot_id: rendered.append((kind, boot_id))
    helper._resume()
    assert not Path(helper.STATE_PATH).exists()
    assert requests == ["resume/release", "resume/complete"]

    requests.clear()
    helper._resume()
    assert requests == []

    helper._write_state(TOKEN, BOOT_ID, "staged")
    requests.clear()
    helper._resume()
    assert requests == ["rollback"]
    assert not Path(helper.STATE_PATH).exists()


class FakeLock:
    def __init__(self):
        self.closed = False

    def close(self):
        self.closed = True


class FakeOled:
    def __init__(self):
        self.commands = []
        self.frames = []
        self.closed = False

    def command(self, value):
        self.commands.append(value)

    def frame(self, value):
        self.frames.append(value)

    def close(self, cleanup):
        assert cleanup is False
        self.closed = True


fake_lock = FakeLock()
fake_oled = FakeOled()


class FakeHandoff:
    UTILITY_LOCK_TIMEOUT_SECONDS = 0.25
    REQUEST_ID_RE = real_handoff.REQUEST_ID_RE

    class Handoff:
        @staticmethod
        def utility_lock(timeout):
            assert timeout > 0
            return fake_lock


class FakeLogo:
    class Oled:
        def command(self, value):
            fake_oled.command(value)

        def frame(self, value):
            fake_oled.frame(value)

        def close(self, cleanup):
            fake_oled.close(cleanup)

    @staticmethod
    def logo_canvas(kind):
        assert kind == "sleep"
        return "canvas"

    @staticmethod
    def render_canvas(canvas):
        assert canvas == "canvas"
        return b"frame"


helper._handoff = FakeHandoff
helper._logo = FakeLogo
helper._status_for_render = lambda handoff, boot_id: None
helper._runtime_identity = lambda: (getattr(os, "geteuid")(), getattr(os, "getegid")())
assert helper._logo is FakeLogo
helper._render = real_render
helper._render("sleep", BOOT_ID)
assert fake_oled.commands == [0xA6, 0xAF], fake_oled.commands
assert fake_oled.frames == [b"frame"], fake_oled.frames
assert fake_oled.closed and fake_lock.closed


class FakeSocket:
    failures = 0
    attempts = 0
    retry_errno = errno.ENOENT

    def __init__(self, *_args):
        self.response = None

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def settimeout(self, _timeout):
        pass

    def connect(self, _path):
        FakeSocket.attempts += 1
        if FakeSocket.failures:
            FakeSocket.failures -= 1
            raise OSError(FakeSocket.retry_errno, "not ready")

    def sendall(self, payload):
        request = json.loads(payload.decode().strip())
        self.response = json.dumps({
            "schema": 1,
            "action": request["action"],
            "token": request["token"],
            "bootId": request["bootId"],
            "ok": True,
            "error": None,
        }, separators=(",", ":")).encode() + b"\n"

    def shutdown(self, _how):
        pass

    def recv(self, _size):
        response, self.response = self.response, b""
        return response


real_socket = helper.socket.socket
real_sleep = helper.time.sleep
try:
    helper._request = real_request
    helper.socket.socket = FakeSocket
    helper.time.sleep = lambda _delay: None
    FakeSocket.failures = 2
    FakeSocket.attempts = 0
    helper._request("rollback", TOKEN, BOOT_ID)
    assert FakeSocket.attempts == 3
    FakeSocket.failures = 1
    FakeSocket.retry_errno = errno.EPERM
    FakeSocket.attempts = 0
    assert_raises(lambda: helper._request("rollback", TOKEN, BOOT_ID), OSError)
    assert FakeSocket.attempts == 1
finally:
    helper.socket.socket = real_socket
    helper.time.sleep = real_sleep

print("Orange OLED suspend helper staged-state, cleanup, retry, mocked handoff, and resume tests passed")
