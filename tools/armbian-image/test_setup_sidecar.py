#!/usr/bin/env python3
import importlib.util
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
CONFIGS = (
    ROOT / "userpatches/overlay/usr/local/lib/octessera/setup_config.py",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup_config.py",
)
PUBLIC_KEY = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def payload(mode="none", password=""):
    return {
        "sshMode": mode,
        "sshPublicKey": PUBLIC_KEY if mode == "key" else "",
        "sshPassword": password if mode == "password" else "",
        "sshPasswordConfirm": password if mode == "password" else "",
        "hostname": "octessera-box",
        "wifiCountry": "US",
    }


for index, path in enumerate(CONFIGS):
    config = load(path, f"setup_config_{index}")
    assert config.validate_stage(payload())["sshMode"] == "none"
    assert config.validate_stage(payload("key"))["sshKey"] == PUBLIC_KEY
    assert config.validate_stage(payload("password", "eight888"))["password"] == "eight888"
    assert config.validate_country_payload({"wifiCountry": "us"}) == "US"
    for invalid in (
        {**payload(), "unexpected": "value"},
        {**payload("password", "short"), "sshPasswordConfirm": "short"},
        {**payload("password", "eight888"), "sshPasswordConfirm": "different"},
        {**payload("key"), "sshPassword": "x"},
        {**payload(), "wifiCountry": "USA"},
    ):
        try:
            config.validate_stage(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError(f"invalid setup payload accepted: {invalid}")
    assert config.valid_password("eight888")
    assert not config.valid_password("seven77")
    assert not config.valid_password("line\nbreak")
    assert config.PROFILES["orange-pi-zero-2w"]["user"] == "octessera"
    assert config.PROFILES["raspberry-pi-zero-2w"]["user"] == "pi"

    commands = []
    writes = []
    timeouts = []
    original_run = config.run
    original_write = config._write_atomic
    original_remove = config.remove_key
    config.run = lambda args, input_text=None, timeout=None: commands.append((tuple(args), input_text)) or timeouts.append(timeout)
    config._write_atomic = lambda *args, **kwargs: writes.append((args, kwargs))
    config.remove_key = lambda profile: commands.append((("remove-key", profile["user"]), None))
    profile = config.PROFILES["raspberry-pi-zero-2w"] if index else config.PROFILES["orange-pi-zero-2w"]
    final_payload = payload("password", "eight888")
    final_payload["hostname"] = ""
    config.finalize(config.validate_stage(final_payload), profile)
    assert commands[0][0] == ("remove-key", profile["user"])
    assert commands[1][0] == ("chpasswd",)
    assert commands[1][1] == f"{profile['user']}:eight888\n"
    assert any(write[0][0] == config.MARKER_PATH and write[0][1] == "complete\n" for write in writes)
    assert any(write[0][0] == config.SSH_POLICY_PATH and "PermitRootLogin no" in write[0][1] for write in writes)
    assert "eight888" not in repr(writes)
    commands.clear()
    writes.clear()
    timeouts.clear()
    config.finalize(config.validate_stage(final_payload), profile, deadline=100.0, clock=lambda: 40.0)
    assert timeouts and all(timeout == 60.0 for timeout in timeouts)
    config.run = original_run
    config._write_atomic = original_write
    config.remove_key = original_remove
    assert "setup-status" not in path.read_text(encoding="utf-8")

print("Setup configuration validation, password, SSH modes, and secret-shape tests passed")
