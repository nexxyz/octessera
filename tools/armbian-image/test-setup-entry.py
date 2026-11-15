#!/usr/bin/env python3
import importlib.util
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
COORDINATORS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup",
)


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


for index, path in enumerate(COORDINATORS):
    coordinator = load(path, f"entry_coordinator_{index}")
    profile = {"user": "octessera", "request_owner": "octessera-runtime", "status_group": "root"}

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
    assert cleanup_events[:2] == [("construct", profile), ("cleanup", 50.0)]
    assert [event[0] for event in cleanup_events[2:]] == ["status"] and cleanup_events[2][1]["phase"] == "failed"
    cleanup_events.clear()
    coordinator.status_is_terminal = lambda _profile: True
    coordinator.cleanup_mode(profile, clock=lambda: 40.0)
    assert cleanup_events == []
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

    rejected_events = []

    def reject_request(_selected):
        raise ValueError("invalid setup request")

    class RejectCoordinator:
        def __init__(self, selected):
            rejected_events.append(("construct", selected))

        def cleanup_request_marker(self):
            rejected_events.append(("marker",))

    coordinator.setup_config.load_profile = lambda: profile
    coordinator.consume_request = reject_request
    coordinator.Coordinator = RejectCoordinator
    sys.argv = ["octessera-setup"]
    try:
        coordinator.main()
    except ValueError:
        pass
    else:
        raise AssertionError("an invalid request was accepted")
    finally:
        sys.argv = original_argv
        coordinator.setup_config.load_profile = original_load_profile
        coordinator.consume_request = original_consume_request
        coordinator.Coordinator = original_coordinator
    assert rejected_events == [("construct", profile), ("marker",)]

print("Setup service entry, ExecStopPost cleanup, and rejected request tests passed")
