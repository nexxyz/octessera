#!/usr/bin/env python3
import importlib.util
import io
import os
import stat
import socket
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace
import sys
import types


ROOT = Path(__file__).resolve().parents[2]
path = ROOT / "userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot"
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    setattr(pwd_stub, "getpwnam", lambda name: (_ for _ in ()).throw(KeyError(name)))
    sys.modules["pwd"] = pwd_stub
spec = importlib.util.spec_from_loader("orange_device_apply", SourceFileLoader("orange_device_apply", str(path)))
assert spec is not None and spec.loader is not None
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

calls = []
validate_config = helper._validate_config
events = []
validation_calls = []

def successful_reboot(command, check):
    events.append("systemctl")
    calls.append((command, check))

setattr(helper.subprocess, "run", successful_reboot)

valid_directory = tempfile.TemporaryDirectory()
valid_config = Path(valid_directory.name) / "default.json"
valid_config.write_text('{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false},"usb":{"midiOutEnabled":false}}}', encoding="utf-8")
valid_config.chmod(0o644)
valid_owner = valid_config.stat()
setattr(helper, "CONFIG_PATH", str(valid_config))
helper.pwd.getpwnam = lambda name: SimpleNamespace(pw_uid=valid_owner.st_uid, pw_gid=valid_owner.st_gid)
setattr(helper, "_load_config", lambda config_path: {"dac": True, "usb": False, "hdmi": False, "midi": False})

def validation_probe(config_path):
    validation_calls.append(config_path)


def unexpected_validation(config_path):
    raise AssertionError(f"poweroff validated config: {config_path}")


setattr(helper, "_validate_config", validation_probe)

class OrderedOutput(io.BytesIO):
    def write(self, value):
        assert events == ["systemctl"]
        return super().write(value)

output = OrderedOutput()
helper.handle_request(io.BytesIO(helper.REBOOT_REQUEST), output)
assert helper.REBOOT_REQUEST == b"reboot\n"
assert helper.POWEROFF_REQUEST == b"poweroff\n"
assert helper.ACCEPTED == b"accepted\n"
assert helper.REJECTED == b"rejected\n"
assert output.getvalue() == helper.ACCEPTED
assert calls == [([helper.SYSTEMCTL_PATH, "reboot"], True)]
assert validation_calls == [str(valid_config)]

calls.clear()
events.clear()
validation_calls.clear()
malformed_config = Path(valid_directory.name) / "malformed-default.json"
malformed_config.write_text("{not-json", encoding="utf-8")
malformed_config.chmod(0o644)
setattr(helper, "CONFIG_PATH", str(malformed_config))
setattr(helper, "_validate_config", unexpected_validation)
poweroff_output = io.BytesIO()
helper.handle_request(io.BytesIO(helper.POWEROFF_REQUEST), poweroff_output)
assert poweroff_output.getvalue() == helper.ACCEPTED
assert calls == [([helper.SYSTEMCTL_PATH, "poweroff"], True)]
assert validation_calls == []
setattr(helper, "CONFIG_PATH", str(valid_config))
setattr(helper, "_validate_config", validation_probe)

def failed_reboot(command, check):
    calls.append((command, check))
    raise helper.subprocess.CalledProcessError(1, command)


setattr(helper.subprocess, "run", failed_reboot)
rejected_output = io.BytesIO()
try:
    helper.handle_request(io.BytesIO(helper.REBOOT_REQUEST), rejected_output)
except helper.subprocess.SubprocessError:
    assert rejected_output.getvalue() == helper.REJECTED
else:
    raise AssertionError("failed reboot accepted")

with tempfile.TemporaryDirectory() as directory:
    config = Path(directory) / "default.json"
    config.write_text("{}", encoding="utf-8")
    config.chmod(0o644)
    setattr(helper, "CONFIG_PATH", str(config))
    owner = config.stat()
    helper.pwd.getpwnam = lambda name: SimpleNamespace(pw_uid=owner.st_uid, pw_gid=owner.st_gid)
    if os.name == "posix":
        for mode in (0o600, 0o664):
            config.chmod(mode)
            try:
                validate_config(str(config))
            except ValueError:
                pass
            else:
                raise AssertionError(mode)
        config.chmod(0o644)
        if getattr(os, "geteuid", lambda: -1)() == 0:
            getattr(os, "chown")(config, 65534, 65534)
            try:
                validate_config(str(config))
            except ValueError:
                pass
            else:
                raise AssertionError("wrong owner accepted")
    if os.name == "posix":
        try:
            validate_config(str(config))
        except ValueError:
            pass
        directory_config = Path(directory) / "directory-config"
        directory_config.mkdir()
        try:
            validate_config(str(directory_config))
        except ValueError:
            pass
        else:
            raise AssertionError("directory config accepted")

setattr(helper, "CONFIG_PATH", str(valid_config))

for request in (
    b"",
    b"wrong\n",
    helper.REBOOT_REQUEST + b"extra",
    helper.POWEROFF_REQUEST + b"extra",
    b"x" * (helper.MAX_REQUEST_BYTES + 1),
):
    calls.clear()
    rejected_output = io.BytesIO()
    try:
        helper.handle_request(io.BytesIO(request), rejected_output)
    except ValueError:
        assert rejected_output.getvalue() == helper.REJECTED
        assert calls == []
    else:
        raise AssertionError(request)


if os.name == "posix":
    def run_fd_request(request, shutdown_write=True):
        sender, receiver = socket.socketpair()
        input_stream = receiver.makefile("rb")
        output = io.BytesIO()
        try:
            sender.sendall(request)
            if shutdown_write:
                sender.shutdown(socket.SHUT_WR)
            try:
                helper.handle_request(input_stream, output)
            except (OSError, ValueError, helper.subprocess.SubprocessError, KeyError):
                pass
            return output.getvalue()
        finally:
            input_stream.close()
            receiver.close()
            sender.close()

    calls.clear()
    events.clear()
    validation_calls.clear()
    setattr(helper.subprocess, "run", successful_reboot)
    setattr(helper, "_validate_config", validation_probe)
    assert run_fd_request(helper.REBOOT_REQUEST) == helper.ACCEPTED
    assert calls == [([helper.SYSTEMCTL_PATH, "reboot"], True)]
    assert validation_calls == [str(valid_config)]

    calls.clear()
    events.clear()
    validation_calls.clear()
    setattr(helper, "_validate_config", unexpected_validation)
    assert run_fd_request(helper.POWEROFF_REQUEST) == helper.ACCEPTED
    assert calls == [([helper.SYSTEMCTL_PATH, "poweroff"], True)]
    assert validation_calls == []

    for request in (b"unknown\n", helper.REBOOT_REQUEST + b"extra", b""):
        calls.clear()
        events.clear()
        assert run_fd_request(request) == helper.REJECTED
        assert calls == []

    calls.clear()
    events.clear()
    assert run_fd_request(b"reboot", shutdown_write=False) == helper.REJECTED
    assert calls == []
else:
    print("Socketpair fd-parser tests require POSIX; run this test under Linux or WSL.")

assert helper.SYSTEMCTL_PATH == "/usr/bin/systemctl"
source_bytes = path.read_bytes()
assert source_bytes.splitlines(keepends=True)[0] == b"#!/usr/bin/env python3\n"
assert b"\r" not in source_bytes
service_path = ROOT / "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service"
socket_path = ROOT / "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket"
runtime_service_path = ROOT / "userpatches/overlay/etc/systemd/system/octessera.service"
service_bytes = service_path.read_bytes()
socket_bytes = socket_path.read_bytes()
runtime_service_bytes = runtime_service_path.read_bytes()
assert b"\r" not in service_bytes
assert b"\r" not in socket_bytes
assert b"\r" not in runtime_service_bytes
for line in (
    b"StartLimitIntervalSec=30s\n",
    b"StartLimitBurst=3\n",
    b"Restart=on-failure\n",
    b"RestartPreventExitStatus=78\n",
    b"RestartSec=5s\n",
):
    assert line in runtime_service_bytes
service = service_bytes.decode("utf-8")
socket_unit = socket_bytes.decode("utf-8")
for line in ("User=root", "Group=root", "StandardInput=socket", "StandardOutput=socket", "ExecStart=/usr/local/sbin/octessera-device-apply-reboot", "NoNewPrivileges=yes", "ProtectSystem=strict"):
    assert line in service
for line in ("Before=sound.target octessera.service", "After=local-fs.target", "ListenStream=/run/octessera-device-apply/reboot.sock", "SocketMode=0660", "SocketUser=root", "SocketGroup=octessera-runtime", "Accept=yes"):
    assert line in socket_unit
assert "After=local-fs.target octessera-provision-musical-default.service" not in socket_unit
assert "Description=Octessera narrow device power request socket" in socket_unit
assert "Description=Octessera validated device power request" in service
assert "sudoers" not in service and "systemctl reboot" not in service
print("Orange device apply request and unit contract tests passed")
valid_directory.cleanup()
