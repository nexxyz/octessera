#!/usr/bin/env python3
import importlib.util
import subprocess
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
COORDINATORS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup",
)
SCAN = ("nmcli", "-t", "-e", "yes", "-f", "SSID,SECURITY,SIGNAL", "device", "wifi", "list", "ifname", "wlan0", "--rescan", "yes")
STATE = ("nmcli", "-g", "GENERAL.STATE", "device", "show", "wlan0")
AP_INFO = "Interface wlan0\n\tssid Octessera Setup abcd\n\ttype AP\n"


def load(path, name):
    for library in ("setup_config", "setup_hotspot", "setup_wifi_join", "setup_http"):
        library_spec = importlib.util.spec_from_loader(library, SourceFileLoader(library, str(path.parent.parent / f"lib/octessera/{library}.py")))
        assert library_spec is not None and library_spec.loader is not None
        sys.modules[library] = importlib.util.module_from_spec(library_spec)
        library_spec.loader.exec_module(sys.modules[library])
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Process:
    def __init__(self, result=None):
        self.result = result
        self.terminated = False
        self.pid = 999999

    def poll(self):
        return -15 if self.terminated else self.result

    def terminate(self):
        self.terminated = True

    def kill(self):
        self.terminated = True

    def wait(self, timeout=None):
        self.terminated = True
        return -15


class Board:
    def __init__(self, state="100 (connected)", ap_info=AP_INFO, scan="hermetes4:WPA2:80\n", fail=()):
        self.events = []
        self.state = state
        self.ap_info = ap_info
        self.scan = scan
        self.fail = set(fail)
        self.profiles = "home:802-11-wireless\nportal:802-11-wireless\nother:802-11-wireless\n"

    def command(self, args, timeout=None):
        args = tuple(args)
        self.events.append(("command", args, timeout))
        if args in self.fail or args[:4] in self.fail:
            return SimpleNamespace(returncode=1, stdout="")
        if args == ("nmcli", "device", "set", "wlan0", "managed", "no"):
            self.state = "10 (unmanaged)"
        elif args == ("nmcli", "device", "set", "wlan0", "managed", "yes"):
            self.state = "30 (disconnected)"
        outputs = {
            SCAN: self.scan,
            STATE: self.state + "\n",
            ("iw", "dev", "wlan0", "info"): self.ap_info,
            ("nmcli", "-t", "-f", "UUID,TYPE", "connection", "show"): self.profiles,
            ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "home"): "infrastructure\n",
            ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "home"): "hermetes4\n",
            ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "portal"): "ap\n",
            ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "portal"): "hermetes4\n",
            ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "other"): "infrastructure\n",
            ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "other"): "Neighbour\n",
        }
        return SimpleNamespace(returncode=0, stdout=outputs.get(args, ""))


def coordinator_for(coordinator, profile, board, clock=lambda: 0.0, processes=None):
    started = [] if processes is None else processes
    instance = coordinator.Coordinator(
        profile,
        command=board.command,
        process_factory=lambda args, **kwargs: board.events.append(("launch", tuple(args), kwargs)) or started.append(Process()) or started[-1],
        clock=clock,
        sleeper=lambda seconds: board.events.append(("sleep", seconds)),
    )
    instance.interface_suffix = lambda: "abcd"
    return instance, started


def expect_failure(action, code):
    try:
        action()
    except coordinator.SetupFailure as failure:
        assert failure.error_code == code, failure.error_code
    else:
        raise AssertionError(f"expected {code}")


for index, path in enumerate(COORDINATORS):
    coordinator = load(path, f"flow_coordinator_{index}")
    written = []
    coordinator.setup_config._write_atomic = lambda target, content, mode, *_args: written.append((target, content, mode))
    profile = {"user": "octessera", "request_owner": "octessera-runtime", "status_group": "root"}
    orange = dict(profile, radio_crash_markers=("WCN Assert", "WCN: marlin power off"), access_point_failure="radio_restart_required")

    board = Board()
    instance, started = coordinator_for(coordinator, profile, board)
    instance.launch()
    names = [event[1] if event[0] == "command" else event for event in board.events]
    assert names[:4] == [("nmcli", "radio", "wifi", "on"), SCAN, ("sleep", 5), ("nmcli", "device", "set", "wlan0", "managed", "no")]
    assert board.events[1][2] == 10.0
    launches = [event for event in board.events if event[0] == "launch"]
    assert [event[1][0] for event in launches] == ["hostapd", "dnsmasq"]
    assert launches[0][1] == ("hostapd", "/run/octessera-setup-hostapd.conf")
    assert all(event[2] == {"stdin": subprocess.DEVNULL, "start_new_session": True} for event in launches)
    assert "--address=/#/192.168.42.1" in launches[1][1] and "--bind-interfaces" in launches[1][1]
    position = {name: names.index(name) for name in (("nmcli", "device", "set", "wlan0", "managed", "no"), ("iw", "dev", "wlan0", "info"), ("ip", "addr", "add", "192.168.42.1/24", "dev", "wlan0"))}
    assert position[("nmcli", "device", "set", "wlan0", "managed", "no")] < names.index(launches[0]) < position[("iw", "dev", "wlan0", "info")] < position[("ip", "addr", "add", "192.168.42.1/24", "dev", "wlan0")] < names.index(launches[1])
    assert SCAN not in names[names.index(launches[0]):]
    assert written[-1][0] == "/run/octessera-setup-hostapd.conf" and written[-1][2] == 0o600
    assert "ssid=Octessera Setup abcd\n" in written[-1][1]
    passphrase = instance.portal_passphrase
    assert len(passphrase) == 9 and passphrase[4] == "-" and all(character in coordinator.PASSPHRASE_ALPHABET for character in passphrase[:4] + passphrase[5:])
    assert f"wpa=2\nwpa_key_mgmt=WPA-PSK\nrsn_pairwise=CCMP\nwpa_passphrase={passphrase}\n" in written[-1][1]
    pmf_board = Board()
    pmf_instance, _ = coordinator_for(coordinator, dict(profile, hotspot_management_frame_protection=True), pmf_board)
    pmf_instance.launch()
    assert "wpa_key_mgmt=WPA-PSK-SHA256\nieee80211w=2\n" in written[-1][1]
    assert not any(passphrase in " ".join(event[1]) for event in board.events if event[0] in ("command", "launch"))
    assert coordinator._status("portal_ready", portal_suffix="abcd", portal_passphrase=passphrase)["portalPassphrase"] == passphrase
    for status in (lambda: coordinator._status("portal_ready", portal_suffix="abcd"), lambda: coordinator._status("finalizing", portal_passphrase=passphrase), lambda: coordinator._status("portal_ready", portal_suffix="abcd", portal_passphrase="O0l1-abcd")):
        try:
            status()
        except ValueError:
            pass
        else:
            raise AssertionError("invalid portal passphrase status was accepted")
    assert len({coordinator.Coordinator(profile).portal_passphrase for _ in range(32)}) > 1
    assert instance.networks == [{"ssid": "hermetes4", "security": "wpa", "strength": 80}]
    assert instance.hotspot.running()

    slow = Board()
    slow_fallback = slow.command

    def slow_scan(args, timeout=None):
        if tuple(args) == SCAN:
            raise subprocess.TimeoutExpired(args, timeout)
        return slow_fallback(args, timeout)

    slow.command = slow_scan
    slow_instance, slow_started = coordinator_for(coordinator, profile, slow)
    slow_instance.launch()
    assert slow_instance.networks == [] and len(slow_started) == 2

    for selected, code in ((profile, "operation_failed"), (orange, "radio_restart_required")):
        silent = Board(ap_info="Interface wlan0\n\ttype managed\n")
        ticks = iter(range(0, 1000))
        silent_instance, silent_started = coordinator_for(coordinator, selected, silent, clock=lambda: float(next(ticks)))
        silent_instance.radio_crashed = lambda: False
        expect_failure(silent_instance.launch, code)
        assert len(silent_started) == 1
        assert not any(event[1][:3] == ("ip", "addr", "add") for event in silent.events if event[0] == "command")
        exited = Board(ap_info="")
        exited_instance, exited_started = coordinator_for(coordinator, selected, exited)
        exited_instance.process_factory = lambda args, **kwargs: Process(1)
        expect_failure(exited_instance.launch, code)

    stuck = Board()
    stuck.command = lambda args, timeout=None: SimpleNamespace(returncode=0, stdout="100 (connected)\n") if tuple(args) == STATE else Board().command(args, timeout)
    ticks = iter(range(0, 1000))
    stuck_instance, stuck_started = coordinator_for(coordinator, profile, stuck, clock=lambda: float(next(ticks)))
    expect_failure(stuck_instance.launch, "operation_failed")
    assert stuck_started == []

    failing = Board(fail={("nmcli", "radio", "wifi", "on")})
    failure_status = []
    fake_server = SimpleNamespace(serve_forever=lambda: None, shutdown=lambda: None, server_close=lambda: None)
    original_setup_server = coordinator.setup_http.SetupHTTPServer
    coordinator.setup_http.SetupHTTPServer = lambda *_args: fake_server
    try:
        failure_instance, failure_started = coordinator_for(coordinator, profile, failing)
        failure_instance.publish = lambda phase, **kwargs: failure_status.append((phase, kwargs))
        failure_instance.cleanup = lambda _deadline: None
        assert failure_instance.run(0.0) == 0
    finally:
        coordinator.setup_http.SetupHTTPServer = original_setup_server
    assert [event[1] for event in failing.events] == [("nmcli", "radio", "wifi", "on")] and failure_started == []
    assert failure_status[-1] == ("failed", {"error_code": "operation_failed", "terminal": True})

    instance.portal_ready = lambda: True
    instance.wait_for_portal_readiness(10.0)
    timeout_instance, _ = coordinator_for(coordinator, profile, Board(), clock=lambda: 10.0)
    timeout_instance.hotspot.processes = [Process()]
    timeout_instance.portal_ready = lambda: False
    expect_failure(lambda: timeout_instance.wait_for_portal_readiness(10.0), "operation_failed")
    dead_instance, _ = coordinator_for(coordinator, profile, Board(), clock=lambda: 1.0)
    dead_instance.hotspot.processes = [Process(), Process(0)]
    dead_instance.portal_ready = lambda: False
    expect_failure(lambda: dead_instance.wait_for_portal_readiness(10.0), "operation_failed")
    instance.staged = None
    instance.clock = lambda: 600.0
    try:
        instance.wait_for_stage(600.0)
    except coordinator.SetupTimeout:
        pass
    else:
        raise AssertionError("portal timeout was not enforced")

    instance.networks = [{"ssid": "hermetes4", "security": "wpa", "strength": 80}, {"ssid": "Cafe", "security": "none", "strength": 50}, {"ssid": "Office", "security": "enterprise", "strength": 40}]
    instance.check_network({"ssid": "hermetes4", "passphrase": "eight888"})
    instance.check_network({"ssid": "Cafe", "passphrase": ""})
    for value in ({"ssid": "hermetes4", "passphrase": ""}, {"ssid": "Cafe", "passphrase": "eight888"}, {"ssid": "Office", "passphrase": "eight888"}, {"ssid": "Hidden", "passphrase": "eight888"}):
        try:
            instance.check_network(value)
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted {value}")

    staged = {"sshMode": "none", "hostname": "", "country": "", "ssid": "hermetes4", "passphrase": "eight888"}
    board = Board()
    apply_instance, apply_started = coordinator_for(coordinator, profile, board, clock=lambda: 1.0)
    apply_instance.hotspot.processes = [Process(), Process()]
    apply_instance.staged = dict(staged)
    apply_instance.staged_at = 1.0
    apply_instance.global_ipv4_ready = lambda: True
    phases = []
    apply_instance.publish = lambda phase, **_kwargs: phases.append(phase)
    finalized = []
    original_finalize = coordinator.setup_config.finalize
    coordinator.setup_config.finalize = lambda data, selected, **kwargs: finalized.append((dict(data), selected, kwargs["deadline"]))
    written.clear()
    apply_instance.wait_for_apply(670.0)
    commands = [event[1] for event in board.events if event[0] == "command"]
    assert board.events[0] == ("sleep", 1.0)
    assert commands.index(("ip", "addr", "del", "192.168.42.1/24", "dev", "wlan0")) < commands.index(("nmcli", "device", "set", "wlan0", "managed", "yes")) < commands.index(STATE)
    profile_file = written[-1]
    joined = profile_file[0].removeprefix("/etc/NetworkManager/system-connections/octessera-setup-").removesuffix(".nmconnection")
    assert profile_file[2] == 0o600 and f"uuid={joined}\n" in profile_file[1] and "psk=eight888\n" in profile_file[1]
    load_index = commands.index(("nmcli", "connection", "load", profile_file[0]))
    up = next(command for command in commands if command[-3:] == ("up", "uuid", joined))
    assert up[:3] == ("nmcli", "--wait", "60") and commands.index(up) > load_index
    assert commands.index(("nmcli", "connection", "delete", "uuid", "home")) > commands.index(up)
    assert ("nmcli", "connection", "delete", "uuid", "portal") not in commands and ("nmcli", "connection", "delete", "uuid", "other") not in commands
    assert not any("eight888" in " ".join(command) for command in commands)
    assert phases == ["finalizing"] and finalized == [(staged, profile, 61.0)]
    assert apply_instance.joining_profile is None and all(process.terminated for process in apply_started + apply_instance.hotspot.processes)
    coordinator.setup_config.finalize = original_finalize

    for failed_step in (("nmcli", "--wait"), ("nmcli", "connection", "load")):
        board = Board()
        board.command_original = board.command
        board.command = lambda args, timeout=None, board=board, step=failed_step: SimpleNamespace(returncode=1, stdout="") if tuple(args)[: len(step)] == step else board.command_original(args, timeout)
        rejected, _ = coordinator_for(coordinator, profile, board, clock=lambda: 1.0)
        rejected.staged = dict(staged)
        rejected.staged_at = 1.0
        rejected.publish = lambda phase, **_kwargs: None
        rejected.cleanup_request_marker = lambda: None
        expect_failure(lambda: rejected.wait_for_apply(670.0), "operation_failed")
        joined = rejected.joining_profile
        assert joined is not None
        removed = []
        original_remove = coordinator.setup_wifi_join.os.remove
        coordinator.setup_wifi_join.os.remove = lambda target: removed.append(target)
        try:
            rejected.cleanup(670.0)
        finally:
            coordinator.setup_wifi_join.os.remove = original_remove
        commands = [event[1] for event in board.events if event[0] == "command"]
        assert ("nmcli", "connection", "delete", "uuid", joined) in commands
        assert ("nmcli", "connection", "delete", "uuid", "home") not in commands
        assert removed[-1] == f"/etc/NetworkManager/system-connections/octessera-setup-{joined}.nmconnection"
        assert rejected.joining_profile is None

    late = Board()
    late_instance, _ = coordinator_for(coordinator, profile, late, clock=lambda: 61.0)
    late_instance.staged = dict(staged)
    late_instance.staged_at = 1.0
    try:
        late_instance.wait_for_apply(670.0)
    except (coordinator.SetupFailure, subprocess.TimeoutExpired):
        pass
    else:
        raise AssertionError("internal apply deadline was not enforced")

    no_address = Board()
    ticks = iter(range(1, 1000))
    no_address_instance, _ = coordinator_for(coordinator, profile, no_address, clock=lambda: float(next(ticks)) / 10)
    no_address_instance.staged = dict(staged)
    no_address_instance.staged_at = 1.0
    no_address_instance.global_ipv4_ready = lambda: False
    no_address_instance.publish = lambda phase, **_kwargs: None
    expect_failure(lambda: no_address_instance.wait_for_apply(670.0), "operation_failed")
    assert not any(event[1] == ("nmcli", "connection", "delete", "uuid", "home") for event in no_address.events if event[0] == "command")

    instance = coordinator.Coordinator(profile)
    assert instance.portal_window_deadline(10.0, 610.0) == 610.0
    expect_failure(lambda: instance.portal_window_deadline(10.1, 610.0), "operation_failed")

    board = Board()
    cleanup_instance, _ = coordinator_for(coordinator, profile, board)
    cleanup_instance.cleanup_request_marker = lambda: None
    removed = []
    original_remove = coordinator.setup_hotspot.os.remove
    coordinator.setup_hotspot.os.remove = lambda target: removed.append(target)
    try:
        cleanup_instance.cleanup(10.0)
    finally:
        coordinator.setup_hotspot.os.remove = original_remove
    commands = [event[1] for event in board.events if event[0] == "command"]
    assert commands == [("ip", "addr", "del", "192.168.42.1/24", "dev", "wlan0"), ("nmcli", "device", "set", "wlan0", "managed", "yes")]
    assert removed == ["/run/octessera-setup-hostapd.conf"]

    cleanup_events = []
    original_coordinator = coordinator.Coordinator
    original_status_is_terminal = coordinator.status_is_terminal
    original_write_status = coordinator.write_status

    class CleanupCoordinator:
        def __init__(self, selected):
            cleanup_events.append(("construct", selected))

        def cleanup(self, deadline):
            cleanup_events.append(("cleanup", deadline))

    coordinator.Coordinator = CleanupCoordinator
    coordinator.status_is_terminal = lambda _profile: False
    coordinator.write_status = lambda status, selected: cleanup_events.append(("status", status, selected))
    coordinator.cleanup_mode(profile, clock=lambda: 40.0)
    assert cleanup_events[0][0] == "status"
    assert cleanup_events[1:] == [("construct", profile), ("cleanup", 50.0)]
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

    coordinator.time.monotonic = lambda: clock_value[0]
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
    assert entry_events == [("profile", 100.0), ("request", profile, 140.0), ("construct", profile, 200.0), ("run", 100.0, 760.0, 200.0)]

    crash_log = "WCN: mdbg_assert_read:WCN Assert in rf_marlin.c line 1016\nWCN: marlin power off!\n"

    def kernel_log(text):
        def command(args, timeout=None):
            if args[0] == "journalctl":
                return SimpleNamespace(returncode=0, stdout=text)
            return Board().command(args, timeout)
        return command

    dead_status = []
    dead_launches = []
    coordinator.setup_http.SetupHTTPServer = lambda *_args: fake_server
    try:
        dead = coordinator.Coordinator(orange, command=kernel_log(crash_log), process_factory=lambda args, **kwargs: dead_launches.append(args), clock=lambda: 0.0)
        dead.interface_suffix = lambda: "abcd"
        dead.publish = lambda phase, **kwargs: dead_status.append((phase, kwargs))
        dead.cleanup = lambda _deadline: None
        assert dead.run(0.0) == 0
    finally:
        coordinator.setup_http.SetupHTTPServer = original_setup_server
    assert dead_launches == []
    assert dead_status[-1] == ("failed", {"error_code": "radio_restart_required", "terminal": True})

    waiting = coordinator.Coordinator(orange, command=kernel_log(crash_log), clock=lambda: 5.0, sleeper=lambda _seconds: None)
    waiting.hotspot.processes = [Process()]
    expect_failure(lambda: waiting.wait_for_stage(600.0), "radio_restart_required")

    healthy = coordinator.Coordinator(orange, command=kernel_log("WCN: the first power on start\n"))
    assert not healthy.radio_crashed()
    unchecked = []
    raspberry = coordinator.Coordinator(profile, command=lambda args, timeout=None: unchecked.append(tuple(args)) or SimpleNamespace(returncode=0, stdout=crash_log))
    assert not raspberry.radio_crashed() and unchecked == []

print("Setup launch order, hotspot failure codes, readiness, portal timeout, network check, apply, profile cleanup, and radio crash tests passed")
