#!/usr/bin/env python3
import importlib.util
import io
import os
import socket
import subprocess
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tools/device-update/octessera-update-broker"
OVERLAY = ROOT / "userpatches/overlay/usr/local/sbin/octessera-update-broker"


def load_helper():
    spec = importlib.util.spec_from_loader(
        "octessera_update_broker", SourceFileLoader("octessera_update_broker", str(SOURCE))
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


helper = load_helper()
calls = []


def run(command, **kwargs):
    calls.append((command, kwargs))
    return SimpleNamespace(returncode=0, stdout=b"broker output\n", stderr=b"")


helper.subprocess.run = run
for request in (helper.CHECK_REQUEST, helper.APPLY_REQUEST, helper.ROLLBACK_REQUEST):
    output = io.BytesIO()
    assert helper.handle_request(io.BytesIO(request), output)
assert [command for command, _ in calls] == [
    [helper.UPDATE_PATH, "check"],
    [helper.UPDATE_PATH, "apply"],
    [helper.UPDATE_PATH, "rollback"],
]
assert output.getvalue() == b"ok\nbroker output"
assert all(kwargs["check"] is False and kwargs["capture_output"] for _, kwargs in calls)


def noisy_run(command, **kwargs):
    return subprocess.CompletedProcess(command, 0, stdout=b"x" * (helper.MAX_OUTPUT_BYTES + 100), stderr=b"")


helper.subprocess.run = noisy_run
bounded = io.BytesIO()
assert helper.handle_request(io.BytesIO(helper.CHECK_REQUEST), bounded)
assert bounded.getvalue() == helper.OK_RESPONSE + b"x" * helper.MAX_OUTPUT_BYTES


def failed_run(command, **kwargs):
    calls.append((command, kwargs))
    return subprocess.CompletedProcess(command, 1, stdout=b"fallback", stderr=b"rejected\n")


helper.subprocess.run = failed_run
failure = io.BytesIO()
assert not helper.handle_request(io.BytesIO(helper.APPLY_REQUEST), failure)
assert failure.getvalue() == b"error\nrejected"

for request in (b"", b"check", b"check\nextra", b"unknown\n", b"x" * (helper.MAX_REQUEST_BYTES + 1)):
    calls.clear()
    output = io.BytesIO()
    try:
        helper.handle_request(io.BytesIO(request), output)
    except ValueError:
        pass
    else:
        raise AssertionError(f"malformed request accepted: {request!r}")
    assert output.getvalue().startswith(helper.ERROR_RESPONSE)
    assert calls == []

if os.name == "posix":
    helper.subprocess.run = noisy_run
    sender, receiver = socket.socketpair()
    input_stream = receiver.makefile("rb")
    output = io.BytesIO()
    try:
        sender.sendall(helper.ROLLBACK_REQUEST)
        sender.shutdown(socket.SHUT_WR)
        assert helper.handle_request(input_stream, output)
        assert output.getvalue().startswith(helper.OK_RESPONSE)
    finally:
        input_stream.close()
        receiver.close()
        sender.close()

assert SOURCE.read_bytes() == OVERLAY.read_bytes()
socket = (ROOT / "userpatches/overlay/etc/systemd/system/octessera-update.socket").read_text(encoding="utf-8")
service = (ROOT / "userpatches/overlay/etc/systemd/system/octessera-update@.service").read_text(encoding="utf-8")
for line in (
    "ListenStream=/run/octessera-update/update.sock",
    "SocketMode=0660",
    "SocketUser=root",
    "SocketGroup=octessera-runtime",
    "DirectoryMode=0755",
    "Accept=yes",
):
    assert line in socket
for line in (
    "User=root",
    "Group=root",
    "StandardInput=socket",
    "StandardOutput=socket",
    "ExecStart=/usr/local/sbin/octessera-update-broker",
    "ProtectSystem=strict",
):
    assert line in service
assert "octessera-runtime" not in service
assert "sudo" not in service
print("Orange update broker protocol, bounds, ownership, and service tests passed")
