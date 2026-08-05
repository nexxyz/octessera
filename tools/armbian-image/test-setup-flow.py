#!/usr/bin/env python3
import importlib.util
import json
import tempfile
from types import SimpleNamespace
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HELPERS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup-request",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request",
)
WRAPPERS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-wifi-connect",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-wifi-connect",
)
SIDECARS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup-sidecar",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar",
)
STATUS_SOURCES = (
    ROOT / "userpatches/overlay/usr/local/lib/octessera/setup-status.py",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status.py",
)


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


for index, path in enumerate(HELPERS):
    helper = load(path, f"flow_helper_{index}")
    with tempfile.TemporaryDirectory() as directory:
        claim_path = Path(directory) / "claim"
        token = "0123456789abcdef0123456789abcdef"
        setattr(helper, "_profile_is_valid", lambda: True)
        setattr(helper, "_claim_request", lambda: (str(claim_path), token, (1, 1)))
        deleted = []
        setattr(helper, "_delete_claim", lambda *values: deleted.append(values))
        status_calls = []
        service_results = []

        def status_call(args, capture=False):
            status_calls.append(args)
            if args[0] == "start-or-attach":
                return SimpleNamespace(returncode=0, stdout=json.dumps({"decision": "attached", "attemptId": "a" * 32}) + "\n")
            return SimpleNamespace(returncode=0, stdout="")

        setattr(helper, "_status", status_call)
        setattr(helper.subprocess, "run", lambda *_args, **_kwargs: service_results.pop(0))
        assert helper.main() == 0
        assert status_calls[0][0] == "start-or-attach" and len(status_calls[0]) == 4
        assert len(deleted) == 1

        status_calls.clear()
        service_results.append(SimpleNamespace(returncode=0, stdout=""))
        def new_status(args, capture=False):
            status_calls.append(args)
            if args[0] == "start-or-attach":
                return SimpleNamespace(returncode=0, stdout=json.dumps({"decision": "new", "attemptId": "b" * 32}) + "\n")
            return SimpleNamespace(returncode=0, stdout="")
        setattr(helper, "_status", new_status)
        assert helper.main() == 0
        assert not any(call[0] == "start-failed" for call in status_calls)
        assert len(status_calls) == 1

        status_calls.clear()
        service_results.append(SimpleNamespace(returncode=1, stdout=""))
        assert helper.main() == 1
        assert ["start-failed", token] in status_calls

        status_calls.clear()
        original_run = helper.subprocess.run
        def interrupted_run(*_args, **_kwargs):
            raise KeyboardInterrupt()
        helper.subprocess.run = interrupted_run
        try:
            helper.main()
        except KeyboardInterrupt:
            pass
        finally:
            helper.subprocess.run = original_run
        assert ["start-failed", token] in status_calls

    source = path.read_text(encoding="utf-8")
    assert '["systemctl", "start", SETUP_UNIT]' in source
    assert "start-or-attach" in source
    assert "os.link(" not in source

for path in WRAPPERS:
    source = path.read_text(encoding="utf-8")
    for required in ("portal_result", "operation_failed", "timed_out", "interrupted", "/sys/class/net/$interface/address", "remaining_seconds", "setup-complete"):
        assert required in source
    assert 'update "$1" "$2" "$portal_suffix"' in source
    assert 'terminal "$1" "$2" "$portal_suffix"' not in source
    assert "setup-force" not in source
    assert "systemctl" not in source
    assert "reboot" not in source.lower()

for index, path in enumerate(SIDECARS):
    sidecar = load(path, f"flow_sidecar_{index}")
    sidecar.staged.clear()
    events = []
    setattr(sidecar, "run", lambda args, input_text=None: events.append((args, input_text)))
    setattr(sidecar, "set_country", lambda _country: None)
    setattr(sidecar, "remove_key", lambda: None)
    setattr(sidecar, "set_password_auth", lambda _enabled: None)
    setattr(sidecar, "_write_atomic", lambda path, content, mode, owner=0, group=0: events.append((path, content)))
    sidecar.staged.update({"sshMode": "none", "hostname": "", "country": ""})
    sidecar.finalize()
    assert not sidecar.staged
    assert any(event[0] == sidecar.MARKER for event in events)

    sidecar.staged.update({"sshMode": "none", "hostname": "", "country": ""})
    marker_writes = sum(event[0] == sidecar.MARKER for event in events)
    setattr(sidecar, "run", lambda _args, input_text=None: (_ for _ in ()).throw(OSError("expected test failure")))
    try:
        sidecar.finalize()
    except OSError:
        pass
    else:
        raise AssertionError("finalization failure was accepted")
    assert not sidecar.staged
    assert sum(event[0] == sidecar.MARKER for event in events) == marker_writes

for index, path in enumerate(STATUS_SOURCES):
    status = load(path, f"flow_status_{index}")
    valid = (
        ("starting", "accepted", None, None),
        ("starting", "already_running", None, None),
        ("portal_ready", None, "abcd", None),
        ("finalizing", None, None, None),
        ("succeeded", None, None, None),
        ("failed", None, None, "operation_failed"),
        ("failed", None, None, "invalid_payload"),
        ("timed_out", None, None, "unavailable"),
        ("unsupported", None, None, "unsupported"),
    )
    for phase, disposition, suffix, error in valid:
        value = status.make_status(phase, disposition, suffix, error)
        assert value["type"] == "setup_portal_status" and value["rebootRequired"] is False
        assert all(item is not None for item in value.values())
    invalid = (
        ("starting", None, None, None),
        ("portal_ready", None, None, None),
        ("portal_ready", None, "ABCD", None),
        ("finalizing", None, "abcd", None),
        ("succeeded", None, None, "operation_failed"),
        ("failed", None, None, "unsupported"),
        ("timed_out", None, None, "operation_failed"),
        ("unsupported", None, None, "operation_failed"),
    )
    for args in invalid:
        try:
            status.make_status(*args)
        except ValueError:
            pass
        else:
            raise AssertionError(f"invalid status accepted: {args}")

print("Setup already-running, outcome, interruption, envelope, and status tests passed")
