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


def observe_finalize(config, profile_name, mode, fail_command=None, deadline=None):
    events = []
    timeouts = []
    profile = config.PROFILES[profile_name]
    originals = {name: getattr(config, name) for name in ("run", "_write_atomic", "remove_key", "configure_key", "persist_country")}

    def fake_run(args, input_text=None, timeout=None):
        command = tuple(args)
        events.append(("command", command, input_text))
        timeouts.append(timeout)
        if command == fail_command:
            raise RuntimeError("transition failed")

    config.run = fake_run
    config._write_atomic = lambda path, content, *args, **kwargs: events.append(("write", path, content))
    config.remove_key = lambda value: events.append(("remove-key", value["user"]))
    config.configure_key = lambda key, value: events.append(("configure-key", value["user"], key))
    config.persist_country = lambda value: events.append(("country", value))
    data = payload(mode, "eight888") if mode == "password" else payload(mode)
    data["hostname"] = ""
    error = None
    try:
        config.finalize(config.validate_stage(data), profile, deadline=deadline, clock=lambda: 40.0)
    except Exception as exc:
        error = exc
    finally:
        for name, value in originals.items():
            setattr(config, name, value)
    return events, timeouts, error


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

    profile_name = "orange-pi-zero-2w" if index == 0 else "raspberry-pi-zero-2w"
    profile = config.PROFILES[profile_name]
    expected_units = ("ssh.service", "ssh.socket", "sshd.service", "sshd.socket") if profile_name == "orange-pi-zero-2w" else ("ssh.service", "ssh.socket")
    assert profile["ssh_units"] == expected_units

    for mode, expected in (
        (
            "key",
            [
                ("country", "US"),
                ("configure-key", profile["user"], PUBLIC_KEY),
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers {profile['user']}\n"),
                ("command", ("ssh-keygen", "-A"), None),
                *[("command", ("systemctl", "unmask", unit), None) for unit in expected_units],
                ("command", ("systemctl", "enable", "--now", "ssh.service"), None),
                ("command", ("systemctl", "reload", "ssh.service"), None),
                ("write", config.MARKER_PATH, "complete\n"),
            ],
        ),
        (
            "password",
            [
                ("country", "US"),
                ("remove-key", profile["user"]),
                ("command", ("chpasswd",), f"{profile['user']}:eight888\n"),
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication yes\nAllowUsers {profile['user']}\n"),
                ("command", ("ssh-keygen", "-A"), None),
                *[("command", ("systemctl", "unmask", unit), None) for unit in expected_units],
                ("command", ("systemctl", "enable", "--now", "ssh.service"), None),
                ("command", ("systemctl", "reload", "ssh.service"), None),
                ("write", config.MARKER_PATH, "complete\n"),
            ],
        ),
        (
            "none",
            [
                ("country", "US"),
                ("remove-key", profile["user"]),
                ("command", ("passwd", "-l", profile["user"]), None),
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers {profile['user']}\n"),
                ("command", ("systemctl", "disable", "--now", "ssh.service"), None),
                ("command", ("systemctl", "disable", "--now", "ssh.socket"), None),
                *[("command", ("systemctl", "mask", unit), None) for unit in expected_units],
                ("write", config.MARKER_PATH, "complete\n"),
            ],
        ),
    ):
        events, timeouts, error = observe_finalize(config, profile_name, mode)
        assert error is None
        assert events == expected
        assert events[-1] == ("write", config.MARKER_PATH, "complete\n")
        assert timeouts and all(timeout is None for timeout in timeouts)
        assert "eight888" not in repr([event for event in events if event[0] == "write"])

    events, timeouts, error = observe_finalize(config, profile_name, "password", deadline=100.0)
    assert error is None
    assert timeouts and all(timeout == 60.0 for timeout in timeouts)

    for mode, fail_command in (
        ("key", ("systemctl", "unmask", expected_units[-1])),
        ("password", ("systemctl", "reload", "ssh.service")),
        ("none", ("systemctl", "mask", expected_units[-1])),
    ):
        events, _, error = observe_finalize(config, profile_name, mode, fail_command)
        assert isinstance(error, RuntimeError)
        assert not any(event[0] == "write" and event[1] == config.MARKER_PATH for event in events)
    assert "setup-status" not in path.read_text(encoding="utf-8")

print("Setup configuration validation, password, SSH modes, and secret-shape tests passed")
