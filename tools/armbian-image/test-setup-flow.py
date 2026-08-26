#!/usr/bin/env python3
import importlib.util
import ipaddress
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
COORDINATORS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup",
)


def load(path, name):
    config_path = path.parent.parent / "lib/octessera/setup_config.py"
    config_spec = importlib.util.spec_from_loader("setup_config", SourceFileLoader("setup_config", str(config_path)))
    assert config_spec is not None and config_spec.loader is not None
    config = importlib.util.module_from_spec(config_spec)
    sys.modules["setup_config"] = config
    config_spec.loader.exec_module(config)
    http_path = path.parent.parent / "lib/octessera/setup_http.py"
    http_spec = importlib.util.spec_from_loader("setup_http", SourceFileLoader("setup_http", str(http_path)))
    assert http_spec is not None and http_spec.loader is not None
    http_module = importlib.util.module_from_spec(http_spec)
    sys.modules["setup_http"] = http_module
    http_spec.loader.exec_module(http_module)
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module, http_module


class Process:
    def __init__(self, result=None):
        self.result = result
        self.terminated = False
        self.pid = 999999

    def poll(self):
        return self.result if not self.terminated else -15

    def terminate(self):
        self.terminated = True

    def wait(self, timeout=None):
        self.terminated = True
        return -15


for index, path in enumerate(COORDINATORS):
    coordinator, http_module = load(path, f"flow_coordinator_{index}")
    source = path.read_text(encoding="utf-8")
    http_source = (path.parent.parent / "lib/octessera/setup_http.py").read_text(encoding="utf-8")
    assert source.count("/usr/local/bin/wifi-connect") == 1
    assert source.count("PORTAL_WINDOW_SECONDS = 600") == 1
    assert source.count("INTERNAL_APPLY_SECONDS = 60") == 1
    for obsolete in ("setup-status.py", "setup-status-cli.py", "setup-call.py", "setup-sidecar", "setup-force", "nonce"):
        assert obsolete not in source
    assert "retry" not in source.lower()
    assert "systemctl" not in source
    assert "NetworkManager" not in source
    assert '"/stage"' in http_source and '"/country"' in http_source
    assert "def cleanup_request_marker" in source
    assert '"/usr/local/share/octessera-setup-ui"' in source
    assert source.index("service_started_at = time.monotonic()") < source.index("profile = setup_config.load_profile()")
    profile = {"user": "octessera", "request_owner": "octessera-runtime", "status_group": "root"}
    commands = []
    command_result = lambda args, timeout=None: commands.append(tuple(args)) or SimpleNamespace(returncode=0, stdout="")
    instance = coordinator.Coordinator(profile, command=command_result, clock=lambda: 0.0, sleeper=lambda _seconds: None)
    instance.interface_suffix = lambda: "abcd"
    process = Process()
    launched = []
    instance.process_factory = lambda args, **kwargs: launched.append((args, kwargs)) or process
    instance.launch()
    assert len(launched) == 1
    assert launched[0][0][0] == "/usr/local/bin/wifi-connect"
    assert "Octessera Setup abcd" in launched[0][0]
    assert "/usr/local/share/octessera-setup-ui" in launched[0][0]

    instance.process = process
    instance.portal_ready = lambda: True
    instance.wait_for_portal_readiness(10.0)
    timeout_instance = coordinator.Coordinator(profile, command=command_result, clock=lambda: 10.0, sleeper=lambda _seconds: None)
    timeout_instance.portal_ready = lambda: False
    try:
        timeout_instance.wait_for_portal_readiness(10.0)
    except coordinator.SetupFailure as failure:
        assert failure.error_code == "operation_failed"
    else:
        raise AssertionError("startup readiness expiry was not classified as failure")
    instance.staged = None
    instance.clock = lambda: 600.0
    try:
        instance.wait_for_stage(600.0)
    except coordinator.SetupTimeout:
        pass
    else:
        raise AssertionError("portal timeout was not enforced")
    assert process.terminated

    process = Process(0)
    instance = coordinator.Coordinator(profile, command=command_result, clock=lambda: 1.0, sleeper=lambda _seconds: None)
    instance.process = process
    instance.staged = {"sshMode": "none", "hostname": "", "country": ""}
    instance.staged_at = 1.0
    instance.global_ipv4_ready = lambda: True
    events = []
    instance.publish = lambda phase, **_kwargs: events.append(phase)
    finalized = []
    coordinator.setup_config.finalize = lambda data, selected, **_kwargs: finalized.append((data, selected))
    instance.wait_for_apply(670.0)
    assert events == ["finalizing"] and len(finalized) == 1

    process = Process(1)
    instance.process = process
    try:
        instance.wait_for_apply(670.0)
    except coordinator.SetupFailure as failure:
        assert failure.error_code == "operation_failed"
    else:
        raise AssertionError("process failure was accepted")

    process = Process()
    instance = coordinator.Coordinator(profile, command=command_result, clock=lambda: 61.0, sleeper=lambda _seconds: None)
    instance.process = process
    instance.staged = {"sshMode": "none", "hostname": "", "country": ""}
    instance.staged_at = 1.0
    try:
        instance.wait_for_apply(670.0)
    except coordinator.SetupFailure as failure:
        assert failure.error_code == "operation_failed"
    else:
        raise AssertionError("internal apply deadline was not enforced")

    process = Process()
    instance = coordinator.Coordinator(profile, command=command_result, clock=lambda: 670.0, sleeper=lambda _seconds: None)
    instance.process = process
    instance.staged = {"sshMode": "none", "hostname": "", "country": ""}
    instance.staged_at = 610.0
    try:
        instance.wait_for_apply(670.0)
    except coordinator.SetupFailure as failure:
        assert failure.error_code == "operation_failed"
    else:
        raise AssertionError("staged apply expiry was not classified as failure")

    instance = coordinator.Coordinator(profile)
    assert instance.portal_window_deadline(10.0, 610.0) == 610.0
    try:
        instance.portal_window_deadline(10.1, 610.0)
    except coordinator.SetupFailure as failure:
        assert failure.error_code == "operation_failed"
    else:
        raise AssertionError("truncated portal window was accepted")
    source = path.read_text(encoding="utf-8")
    assert source.index("portal_deadline = self.portal_window_deadline") < source.index('self.publish("portal_ready"')

    cleanup_events = []
    original_coordinator = coordinator.Coordinator
    original_status_is_terminal = coordinator.status_is_terminal
    original_write_status = coordinator.write_status

    class CleanupCoordinator:
        def __init__(self, selected):
            cleanup_events.append(("construct", selected))
            self.portal_suffix = None

        def interface_suffix(self):
            cleanup_events.append(("interface",))
            return "abcd"

        def cleanup(self, deadline):
            cleanup_events.append(("cleanup", deadline))

    coordinator.Coordinator = CleanupCoordinator
    coordinator.status_is_terminal = lambda _profile: False
    coordinator.write_status = lambda status, selected: cleanup_events.append(("status", status, selected))
    coordinator.cleanup_mode(profile, clock=lambda: 40.0)
    assert cleanup_events[0][0] == "status"
    assert cleanup_events[1][0:2] == ("construct", profile)
    assert cleanup_events[-1] == ("cleanup", 50.0)
    cleanup_events.clear()
    coordinator.status_is_terminal = lambda _profile: True
    coordinator.cleanup_mode(profile, clock=lambda: 40.0)
    assert all(event[0] != "status" for event in cleanup_events)

    coordinator.Coordinator = original_coordinator
    coordinator.status_is_terminal = original_status_is_terminal
    coordinator.write_status = original_write_status

    entry_events = []
    clock_value = [100.0]
    original_monotonic = coordinator.time.monotonic
    original_load_profile = coordinator.setup_config.load_profile
    original_consume_request = coordinator.consume_request
    original_coordinator = coordinator.Coordinator

    def entry_clock():
        return clock_value[0]

    def load_profile():
        entry_events.append(("profile", clock_value[0]))
        clock_value[0] = 140.0
        return profile

    def consume_request(selected):
        entry_events.append(("request", selected, clock_value[0]))
        clock_value[0] = 200.0

    class EntryCoordinator:
        def __init__(self, selected):
            entry_events.append(("construct", selected, clock_value[0]))

        def run(self, service_started_at):
            entry_events.append(("run", service_started_at, service_started_at + 660.0, clock_value[0]))
            return 0

    coordinator.time.monotonic = entry_clock
    coordinator.setup_config.load_profile = load_profile
    coordinator.consume_request = consume_request
    coordinator.Coordinator = EntryCoordinator
    original_argv = sys.argv
    sys.argv = ["octessera-setup"]
    try:
        assert coordinator.main() == 0
    finally:
        sys.argv = original_argv
        coordinator.time.monotonic = original_monotonic
        coordinator.setup_config.load_profile = original_load_profile
        coordinator.consume_request = original_consume_request
        coordinator.Coordinator = original_coordinator
    assert entry_events == [
        ("profile", 100.0),
        ("request", profile, 140.0),
        ("construct", profile, 200.0),
        ("run", 100.0, 760.0, 200.0),
    ]

    calls = []
    def cleanup_command(args):
        calls.append(tuple(args))
        output = {
            ("nmcli", "-t", "-f", "UUID,TYPE", "connection", "show"): "portal:802-11-wireless\ninfra:802-11-wireless\n",
            ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "portal"): "ap\n",
            ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "portal"): "Octessera Setup abcd\n",
            ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "infra"): "infrastructure\n",
            ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "infra"): "Home\n",
        }.get(tuple(args), "")
        return SimpleNamespace(returncode=0, stdout=output)
    instance = coordinator.Coordinator(profile, command=cleanup_command)
    instance.portal_suffix = "abcd"
    instance.cleanup_profiles()
    assert ("nmcli", "connection", "delete", "uuid", "portal") in calls
    assert ("nmcli", "connection", "delete", "uuid", "infra") not in calls
    assert all("restart" not in call for call in calls)

print("Setup one-launch, readiness, portal timeout, apply, failure, and exact cleanup tests passed")
