#!/usr/bin/env python3
import importlib.util
import io
import os
import stat
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
path = ROOT / "userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot"
spec = importlib.util.spec_from_loader("orange_device_apply", SourceFileLoader("orange_device_apply", str(path)))
assert spec is not None and spec.loader is not None
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

calls = []
validate_config = helper._validate_config
events = []

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
setattr(helper, "_validate_config", validate_config)

class OrderedOutput(io.BytesIO):
    def write(self, value):
        assert events == ["systemctl"]
        return super().write(value)

output = OrderedOutput()
helper.handle_request(io.BytesIO(helper.REQUEST), output)
assert helper.REQUEST == b"reboot\n"
assert helper.ACCEPTED == b"accepted\n"
assert helper.REJECTED == b"rejected\n"
assert output.getvalue() == helper.ACCEPTED
assert calls == [([helper.SYSTEMCTL_PATH, "reboot"], True)]

def failed_reboot(command, check):
    calls.append((command, check))
    raise helper.subprocess.CalledProcessError(1, command)


setattr(helper.subprocess, "run", failed_reboot)
rejected_output = io.BytesIO()
try:
    helper.handle_request(io.BytesIO(helper.REQUEST), rejected_output)
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
    directory_config = Path(directory) / "directory-config"
    directory_config.mkdir()
    try:
        validate_config(str(directory_config))
    except ValueError:
        pass
    else:
        raise AssertionError("directory config accepted")

for request in (b"", b"wrong\n", helper.REQUEST + b"extra", b"x" * (helper.MAX_REQUEST_BYTES + 1)):
    calls.clear()
    rejected_output = io.BytesIO()
    try:
        helper.handle_request(io.BytesIO(request), rejected_output)
    except ValueError:
        assert rejected_output.getvalue() == helper.REJECTED
        assert calls == []
    else:
        raise AssertionError(request)

assert helper.SYSTEMCTL_PATH == "/usr/bin/systemctl"
service = (ROOT / "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service").read_text(encoding="utf-8")
socket = (ROOT / "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket").read_text(encoding="utf-8")
for line in ("User=root", "Group=root", "StandardInput=socket", "StandardOutput=socket", "ExecStart=/usr/local/sbin/octessera-device-apply-reboot", "NoNewPrivileges=yes", "ProtectSystem=strict"):
    assert line in service
for line in ("ListenStream=/run/octessera-device-apply/reboot.sock", "SocketMode=0660", "SocketUser=root", "SocketGroup=octessera-runtime", "Accept=yes"):
    assert line in socket
assert "sudoers" not in service and "systemctl reboot" not in service
print("Orange device apply request and unit contract tests passed")
valid_directory.cleanup()
