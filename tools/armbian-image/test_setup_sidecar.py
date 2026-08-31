#!/usr/bin/env python3
import contextlib
import importlib.util
import io
import subprocess
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path
from unittest.mock import patch


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


class SSHFake:
    def __init__(self, units, policy_path, marker_path, fail_event=None):
        self.units = frozenset(units)
        self.policy_path = policy_path
        self.marker_path = marker_path
        self.fail_event = fail_event
        self.events = []
        self.service_active = True
        self.socket_active = True
        self.masked = set()
        self.activation_events = []
        self.quiescence_reached = False
        self.failed_after_quiescence = False
        self.marker_attempted = False
        self.marker_replaced = False
        self.marker_failure_phase = None
        self.marker_persisted = False

    @property
    def quiesced(self):
        return not self.service_active and not self.socket_active and self.masked == self.units

    @property
    def listener_active(self):
        return self.service_active or self.socket_active

    def protected_event(self, event):
        if event[0] in {"configure-key", "remove-key"}:
            return True
        if event[0] == "write":
            return event[1] == self.policy_path
        if event[0] == "command":
            return event[1] == ("chpasswd",) or event[1][:2] == ("passwd", "-l")
        return False

    def record(self, event):
        if event[0] == "write" and event[1] == self.marker_path:
            self.marker_attempted = True
        if self.protected_event(event):
            assert self.quiesced, f"SSH mutation before quiescence: {event}"
        self.events.append(event)
        if event == self.fail_event:
            self.failed_after_quiescence = self.quiescence_reached
            if event[0] == "write" and event[1] == self.marker_path:
                self.marker_failure_phase = "pre-replace"
            self.events.append(("failure", event))
            raise RuntimeError("transition failed")
        if event[0] == "write" and event[1] == self.marker_path:
            self.marker_replaced = True
            self.marker_persisted = True

    def run(self, args, input_text=None, timeout=None):
        command = tuple(args)
        self.record(("command", command, input_text))
        if command == ("systemctl", "disable", "--now", "ssh.socket"):
            self.socket_active = False
        elif command == ("systemctl", "disable", "--now", "ssh.service"):
            self.service_active = False
        elif command[:2] == ("systemctl", "mask"):
            self.masked.add(command[2])
        elif command[:2] == ("systemctl", "unmask"):
            self.masked.remove(command[2])
        elif command == ("systemctl", "enable", "--now", "ssh.service"):
            self.service_active = True
            self.activation_events.append(command)
        self.quiescence_reached = self.quiescence_reached or self.quiesced


def observe_finalize(config, profile_name, mode, fail_event=None, deadline=None):
    events = []
    timeouts = []
    profile = config.PROFILES[profile_name]
    fake = SSHFake(profile["ssh_units"], config.SSH_POLICY_PATH, config.MARKER_PATH, fail_event)
    originals = {name: getattr(config, name) for name in ("run", "_write_atomic", "remove_key", "configure_key", "persist_country")}

    def write(path, content, *args, **kwargs):
        fake.record(("write", path, content))

    def remove(value):
        fake.record(("remove-key", value["user"]))

    def configure(key, value):
        fake.record(("configure-key", value["user"], key))

    def country(value):
        fake.record(("country", value))

    def run(args, input_text=None, timeout=None):
        timeouts.append(timeout)
        fake.run(args, input_text, timeout)

    config.run = run
    config._write_atomic = write
    config.remove_key = remove
    config.configure_key = configure
    config.persist_country = country
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
    return fake.events, timeouts, error, fake


def assert_atomic_writer_metadata(config):
    descriptor = 17
    temporary = "/etc/.target.tmp"
    events = []

    class Handle:
        def __enter__(self):
            events.append(("open", descriptor))
            return self

        def __exit__(self, *args):
            events.append(("close", descriptor))

        def write(self, value):
            events.append(("write", value))

        def flush(self):
            events.append(("flush",))

        def fileno(self):
            return descriptor

    def metadata_call(name):
        def call(*args):
            events.append((name, *args))
            raise OSError(f"unexpected destination {name}")

        return call

    with (
        patch.object(config.os, "makedirs", side_effect=lambda *args, **kwargs: None),
        patch.object(config.tempfile, "mkstemp", return_value=(descriptor, temporary)),
        patch.object(config.os, "fchmod", side_effect=lambda fd, mode: events.append(("fchmod", fd, mode)), create=True),
        patch.object(config.os, "fchown", side_effect=lambda fd, owner, group: events.append(("fchown", fd, owner, group)), create=True),
        patch.object(config.os, "fdopen", side_effect=lambda fd, *args, **kwargs: Handle()),
        patch.object(config.os, "fsync", side_effect=lambda fd: events.append(("fsync", fd))),
        patch.object(config.os, "replace", side_effect=lambda source, target: events.append(("replace", source, target))),
        patch.object(config.os, "unlink", side_effect=lambda path: (_ for _ in ()).throw(FileNotFoundError(path))),
        patch.object(config.os, "chmod", side_effect=metadata_call("chmod"), create=True),
        patch.object(config.os, "chown", side_effect=metadata_call("chown"), create=True),
    ):
        config._write_atomic("/etc/target", "payload\n", 0o640, 123, 456)

    replace_index = next(index for index, event in enumerate(events) if event[0] == "replace")
    fchmod_index = next(index for index, event in enumerate(events) if event[0] == "fchmod")
    fchown_index = next(index for index, event in enumerate(events) if event[0] == "fchown")
    assert events[fchmod_index] == ("fchmod", descriptor, 0o640)
    assert events[fchown_index] == ("fchown", descriptor, 123, 456)
    assert fchmod_index < replace_index
    assert fchown_index < replace_index
    assert next(index for index, event in enumerate(events) if event[0] == "fsync") < replace_index
    assert not any(event[0] in {"chmod", "chown"} for event in events)


def assert_secret_safe_command_failure(config):
    canary = "setup-secret-canary"
    failures = (
        subprocess.CalledProcessError(23, ["chpasswd", canary], output=canary, stderr=canary),
        subprocess.TimeoutExpired(["chpasswd", canary], 1, output=canary, stderr=canary),
        OSError(13, canary),
    )
    for failure in failures:
        stream = io.StringIO()
        with patch.object(config.subprocess, "run", side_effect=failure) as command, contextlib.redirect_stderr(stream):
            try:
                config.run(["chpasswd", canary], input_text=canary)
            except Exception as error:
                assert error is failure
            else:
                raise AssertionError(f"command failure was swallowed: {type(failure).__name__}")
        report = stream.getvalue()
        assert canary not in report
        assert "chpasswd" in report
        assert type(failure).__name__ in report
        assert command.call_args.kwargs["stdout"] is config.subprocess.DEVNULL
        assert command.call_args.kwargs["stderr"] is config.subprocess.DEVNULL


def assert_hostname_coherence(config):
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        hostname_path = root / "hostname"
        hosts_path = root / "hosts"
        initial_hosts = "127.0.0.1 localhost\n127.0.1.1 orangepizero2w local-alias # orangepizero2w comment\n::1 localhost ip6-localhost ip6-loopback orangepizero2w # orangepizero2w comment\n10.0.0.2 unrelated # keep this comment\n"
        updated_hosts = initial_hosts.replace("127.0.1.1 orangepizero2w", "127.0.1.1 octessera-opi").replace("loopback orangepizero2w", "loopback octessera-opi")
        writes = []
        commands = []

        def write(path, content, *_args, **_kwargs):
            writes.append((path, content))
            Path(path).write_text(content, encoding="utf-8")

        def invoke(args):
            commands.append(tuple(args))
            if len(commands) == 1:
                raise RuntimeError("hostnamectl failed")

        def expect_rejection():
            try:
                config.apply_hostname("octessera-opi", invoke)
            except ValueError:
                pass
            else:
                raise AssertionError("invalid hostname state was accepted")

        with patch.object(config, "HOSTNAME_PATH", str(hostname_path)), patch.object(config, "HOSTS_PATH", str(hosts_path)), patch.object(config, "_write_atomic", write):
            hostname_path.write_text("orangepizero2w\n", encoding="utf-8")
            hosts_path.write_text(initial_hosts, encoding="utf-8")
            try:
                config.apply_hostname("octessera-opi", invoke)
            except RuntimeError:
                pass
            else:
                raise AssertionError("hostnamectl failure was swallowed")
            assert hosts_path.read_text(encoding="utf-8") == updated_hosts
            config.apply_hostname("octessera-opi", invoke)
            assert commands == [("hostnamectl", "set-hostname", "octessera-opi")] * 2
            assert writes == [(str(hosts_path), updated_hosts)]
            config.apply_hostname("", invoke)
            assert hosts_path.read_text(encoding="utf-8") == updated_hosts
            assert writes == [(str(hosts_path), updated_hosts)] and commands == [("hostnamectl", "set-hostname", "octessera-opi")] * 2
            hosts_path.write_text("127.0.1.1 localhost # orangepizero2w\n", encoding="utf-8")
            expect_rejection()
            hostname_path.write_text("\n", encoding="utf-8")
            expect_rejection()


def assert_failed_finalize(events, fake):
    failure_indices = [index for index, event in enumerate(events) if event[0] == "failure"]
    assert len(failure_indices) == 1
    failure_index = failure_indices[0]
    if fake.marker_attempted:
        assert fake.marker_failure_phase == "pre-replace"
        assert not fake.marker_replaced
        assert not fake.marker_persisted
        assert events[failure_index + 1 :] == []
        if fake.activation_events:
            assert fake.service_active and not fake.socket_active
            assert not fake.masked
        else:
            assert not fake.listener_active
        return
    assert not fake.marker_attempted
    assert fake.marker_failure_phase is None
    assert not fake.marker_replaced
    assert not fake.marker_persisted
    if not fake.failed_after_quiescence:
        assert not any(fake.protected_event(event) for event in events)
        return
    assert not fake.listener_active
    assert not fake.activation_events
    assert not any(
        event[0] == "command" and event[1] == ("systemctl", "enable", "--now", "ssh.service")
        for event in events[failure_index + 1 :]
    )


for index, path in enumerate(CONFIGS):
    config = load(path, f"setup_config_{index}")
    assert_atomic_writer_metadata(config)
    assert_secret_safe_command_failure(config)
    assert_hostname_coherence(config)
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
    quiesce_events = [
        ("command", ("systemctl", "disable", "--now", "ssh.socket"), None),
        ("command", ("systemctl", "disable", "--now", "ssh.service"), None),
        *[("command", ("systemctl", "mask", unit), None) for unit in expected_units],
    ]
    marker_event = ("write", config.MARKER_PATH, "complete\n")

    for mode, expected in (
        (
            "key",
            [
                ("country", "US"),
                *quiesce_events,
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers {profile['user']}\n"),
                ("configure-key", profile["user"], PUBLIC_KEY),
                ("command", ("ssh-keygen", "-A"), None),
                *[("command", ("systemctl", "unmask", unit), None) for unit in expected_units],
                ("command", ("systemctl", "enable", "--now", "ssh.service"), None),
                marker_event,
            ],
        ),
        (
            "password",
            [
                ("country", "US"),
                *quiesce_events,
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers {profile['user']}\n"),
                ("remove-key", profile["user"]),
                ("command", ("chpasswd",), f"{profile['user']}:eight888\n"),
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication yes\nAllowUsers {profile['user']}\n"),
                ("command", ("ssh-keygen", "-A"), None),
                *[("command", ("systemctl", "unmask", unit), None) for unit in expected_units],
                ("command", ("systemctl", "enable", "--now", "ssh.service"), None),
                marker_event,
            ],
        ),
        (
            "none",
            [
                ("country", "US"),
                *quiesce_events,
                ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers {profile['user']}\n"),
                ("remove-key", profile["user"]),
                ("command", ("passwd", "-l", profile["user"]), None),
                marker_event,
            ],
        ),
    ):
        events, timeouts, error, fake = observe_finalize(config, profile_name, mode)
        assert error is None
        assert events == expected
        assert events[-1] == marker_event
        assert fake.marker_attempted and fake.marker_replaced and fake.marker_persisted
        assert timeouts and all(timeout is None for timeout in timeouts)
        assert "eight888" not in repr([event for event in events if event[0] == "write"])
        if mode == "none":
            assert not fake.listener_active and fake.masked == set(expected_units)
        else:
            assert fake.service_active and not fake.socket_active
            assert fake.activation_events == [("systemctl", "enable", "--now", "ssh.service")]

    events, timeouts, error, _ = observe_finalize(config, profile_name, "password", deadline=100.0)
    assert error is None
    assert timeouts and all(timeout == 60.0 for timeout in timeouts)

    deny_policy = ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers {profile['user']}\n")
    allow_policy = ("write", config.SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication yes\nAllowUsers {profile['user']}\n")
    failure_cases = [(mode, event) for mode in ("key", "password", "none") for event in quiesce_events]
    failure_cases.extend(("key", event) for event in (deny_policy, ("configure-key", profile["user"], PUBLIC_KEY), ("command", ("ssh-keygen", "-A"), None), marker_event))
    failure_cases.extend(("key", ("command", ("systemctl", "unmask", unit), None)) for unit in expected_units)
    failure_cases.append(("key", ("command", ("systemctl", "enable", "--now", "ssh.service"), None)))
    failure_cases.extend(("password", event) for event in (deny_policy, ("remove-key", profile["user"]), ("command", ("chpasswd",), f"{profile['user']}:eight888\n"), allow_policy, ("command", ("ssh-keygen", "-A"), None), marker_event))
    failure_cases.extend(("password", ("command", ("systemctl", "unmask", unit), None)) for unit in expected_units)
    failure_cases.append(("password", ("command", ("systemctl", "enable", "--now", "ssh.service"), None)))
    failure_cases.extend(("none", event) for event in (deny_policy, ("remove-key", profile["user"]), ("command", ("passwd", "-l", profile["user"]), None), marker_event))
    for mode, fail_event in failure_cases:
        events, _, error, fake = observe_finalize(config, profile_name, mode, fail_event=fail_event)
        assert isinstance(error, RuntimeError)
        assert_failed_finalize(events, fake)
    assert "setup-status" not in path.read_text(encoding="utf-8")

print("Setup configuration validation, password, SSH modes, and secret-shape tests passed")
