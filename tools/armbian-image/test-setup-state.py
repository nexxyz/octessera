#!/usr/bin/env python3
import contextlib
import importlib.util
import io
import json
import os
import shutil
import stat
import tempfile
from concurrent.futures import ThreadPoolExecutor
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
STATUS_PATHS = (
    ROOT / "userpatches/overlay/usr/local/lib/octessera/setup-status.py",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status.py",
)


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def assert_envelope(payload, status_keys):
    assert set(payload) == {"schema", "bootId", "attemptId", "sequence", "status"}
    assert payload["schema"] == 1
    assert len(payload["attemptId"]) == 32
    assert payload["status"]["type"] == "setup_portal_status"
    assert set(payload["status"]) == status_keys
    assert payload["status"]["rebootRequired"] is False


def run_dynamic(status, directory):
    if getattr(status, "fcntl", None) is None or getattr(os, "geteuid", lambda: -1)() != 0:
        return False
    root = Path(directory)
    cli = load(Path(status.__file__).with_name("setup-status-cli.py"), f"state_cli_{status.PUBLIC_GROUP}")
    def configure(base, marker_present):
        control = base / "control"
        public = base / "public"
        boot = base / "boot_id"
        marker = base / "setup-complete"
        base.mkdir(parents=True, exist_ok=True)
        boot.write_text("01234567-89ab-cdef-0123-456789abcdef\n", encoding="ascii")
        if marker_present:
            marker.write_text("complete\n", encoding="ascii")
        status.CONTROL_DIR = str(control)
        status.PUBLIC_DIR = str(public)
        status.RECEIPT_DIR = str(public / "receipts")
        status.LOCK_PATH = str(control / "status.lock")
        status.ACTIVE_PATH = str(control / "active.json")
        status.SEQUENCE_PATH = str(control / "sequence")
        status.BOOT_ID_PATH = str(boot)
        status.MARKER_PATH = str(marker)
        return control, public, boot, marker
    status.group_id = lambda: 0
    status.unit_state = lambda: "inactive"
    first_control, first_public, _, _ = configure(root / "firstboot", False)
    assert status.ensure_firstboot() == 0
    first_active = json.loads((first_control / "active.json").read_text(encoding="utf-8"))
    first_current = json.loads((first_public / "current.json").read_text(encoding="utf-8"))
    assert first_active["reentry"] is False and first_active["priorSetupComplete"] is False
    assert first_current["status"] == {"type": "setup_portal_status", "phase": "starting", "disposition": "accepted", "rebootRequired": False}
    assert status.stop() == 0 and not (first_control / "active.json").exists()
    shutil.rmtree(root / "firstboot")
    control, public, boot, marker = configure(root, True)
    token = "0123456789abcdef0123456789abcdef"
    claim = str(control / "claims" / "claim-test")
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(token, True, claim) == 0
    decision = json.loads(output.getvalue())
    assert decision["decision"] == "new"
    attempt = decision["attemptId"]
    current = json.loads((public / "current.json").read_text(encoding="utf-8"))
    receipt = json.loads((public / "receipts" / f"{token}.json").read_text(encoding="utf-8"))
    assert_envelope(current, {"type", "phase", "disposition", "rebootRequired"})
    assert_envelope(receipt, {"type", "phase", "disposition", "rebootRequired"})
    assert current["sequence"] == receipt["sequence"] == 1
    assert current["status"]["phase"] == "starting" and current["status"]["disposition"] == "accepted"
    assert receipt["status"] == current["status"]
    active = json.loads((control / "active.json").read_text(encoding="utf-8"))
    assert active["attemptId"] == attempt and active["sequence"] == current["sequence"] and active["reentry"] is True and active["priorSetupComplete"] is True
    assert stat.S_IMODE(os.stat(public).st_mode) == 0o750
    assert stat.S_IMODE(os.stat(public / "current.json").st_mode) == 0o640
    assert stat.S_IMODE(os.stat(public / "receipts" / f"{token}.json").st_mode) == 0o640
    assert stat.S_IMODE(os.stat(control).st_mode) == 0o700
    assert stat.S_IMODE(os.stat(control / "active.json").st_mode) == 0o600

    status.unit_state = lambda: "active"
    second_token = "abcdef0123456789abcdef0123456789"
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(second_token, True, str(control / "claims" / "claim-second")) == 0
    assert json.loads(output.getvalue())["decision"] == "attached"
    running_receipt = json.loads((public / "receipts" / f"{second_token}.json").read_text(encoding="utf-8"))
    assert_envelope(running_receipt, {"type", "phase", "disposition", "rebootRequired"})
    assert running_receipt["sequence"] == current["sequence"] == active["sequence"]
    assert running_receipt["status"]["phase"] == "starting" and running_receipt["status"]["disposition"] == "already_running"
    assert int((control / "sequence").read_text(encoding="ascii")) == 1

    status.unit_state = lambda: "inactive"
    record = status.read_active()
    sequences = []
    def advance(_index):
        inner = status.lock()
        try:
            return status.write_current(record, "finalizing")[0]
        finally:
            status.unlock(inner)
    with ThreadPoolExecutor(max_workers=6) as executor:
        sequences = list(executor.map(advance, range(12)))
    assert len(set(sequences)) == len(sequences)
    assert sorted(sequences) == list(range(min(sequences), max(sequences) + 1))

    boot.write_text("fedcba98-7654-3210-fedc-ba9876543210\n", encoding="ascii")
    descriptor = status.lock()
    try:
        assert status.read_active() is None
    finally:
        status.unlock(descriptor)
    assert not (control / "active.json").exists()

    boot.write_text("01234567-89ab-cdef-0123-456789abcdef\n", encoding="ascii")
    status.unit_state = lambda: "failed"
    failed_token = "123456789abcdef0123456789abcdef0"
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(failed_token, True, str(control / "claims" / "claim-failed")) == 0
    failed_decision = json.loads(output.getvalue())
    failed_current = json.loads((public / "current.json").read_text(encoding="utf-8"))
    failed_receipt = json.loads((public / "receipts" / f"{failed_token}.json").read_text(encoding="utf-8"))
    assert failed_decision["decision"] == "failed"
    assert failed_current["sequence"] == failed_receipt["sequence"]
    assert failed_current["status"] == failed_receipt["status"]
    assert failed_current["status"]["phase"] == "failed" and failed_current["status"]["errorCode"] == "operation_failed"
    assert not (control / "active.json").exists()

    status.unit_state = lambda: "unknown"
    unavailable_token = "23456789abcdef0123456789abcdef01"
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(unavailable_token, True, str(control / "claims" / "claim-unavailable")) == 0
    unavailable_receipt = json.loads((public / "receipts" / f"{unavailable_token}.json").read_text(encoding="utf-8"))
    assert unavailable_receipt["status"]["phase"] == "failed" and unavailable_receipt["status"]["errorCode"] == "unavailable"

    status.unit_state = lambda: "inactive"
    orphan_token = "3456789abcdef0123456789abcdef012"
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(orphan_token, True, str(control / "claims" / "claim-orphan")) == 0
    orphan_attempt = json.loads(output.getvalue())["attemptId"]
    next_token = "456789abcdef0123456789abcdef0123"
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(next_token, True, str(control / "claims" / "claim-next")) == 0
    next_attempt = json.loads(output.getvalue())["attemptId"]
    orphan_receipt = json.loads((public / "receipts" / f"{orphan_token}.json").read_text(encoding="utf-8"))
    next_receipt = json.loads((public / "receipts" / f"{next_token}.json").read_text(encoding="utf-8"))
    assert next_attempt != orphan_attempt
    assert orphan_receipt["status"] == {"type": "setup_portal_status", "phase": "failed", "rebootRequired": False, "errorCode": "operation_failed"}
    assert next_receipt["status"] == {"type": "setup_portal_status", "phase": "starting", "disposition": "accepted", "rebootRequired": False}
    assert cli.main(status, ["setup-status.py", "fail-pending"]) == 0
    assert not (control / "active.json").exists()

    status.unit_state = lambda: "inactive"
    terminal_token = "56789abcdef0123456789abcdef01234"
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        assert status.start_or_attach(terminal_token, True, str(control / "claims" / "claim-terminal")) == 0
    terminal_attempt = json.loads(output.getvalue())["attemptId"]
    assert status.terminal_status("succeeded", None, None) == 0
    terminal_current = json.loads((public / "current.json").read_text(encoding="utf-8"))
    terminal_receipt = json.loads((public / "receipts" / f"{terminal_token}.json").read_text(encoding="utf-8"))
    assert not (control / "active.json").exists()
    assert terminal_current["attemptId"] == terminal_receipt["attemptId"] == terminal_attempt
    assert terminal_current["sequence"] == terminal_receipt["sequence"]
    assert terminal_current["status"] == terminal_receipt["status"] == {"type": "setup_portal_status", "phase": "succeeded", "rebootRequired": False}
    assert len(list((public / "receipts").glob(f"{terminal_token}.json"))) == 1

    restart = load(Path(status.__file__), "state_status_restart")
    for name, value in {
        "CONTROL_DIR": str(control),
        "PUBLIC_DIR": str(public),
        "RECEIPT_DIR": str(public / "receipts"),
        "LOCK_PATH": str(control / "status.lock"),
        "ACTIVE_PATH": str(control / "active.json"),
        "SEQUENCE_PATH": str(control / "sequence"),
        "BOOT_ID_PATH": str(boot),
        "MARKER_PATH": str(marker),
        "group_id": lambda: 0,
    }.items():
        setattr(restart, name, value)
    assert restart.stop() == 0
    assert not (control / "active.json").exists()
    assert json.loads((public / "current.json").read_text(encoding="utf-8")) == terminal_current
    assert json.loads((public / "receipts" / f"{terminal_token}.json").read_text(encoding="utf-8")) == terminal_receipt
    return True


for index, path in enumerate(STATUS_PATHS):
    status = load(path, f"state_status_{index}")
    with tempfile.TemporaryDirectory() as directory:
        dynamic = run_dynamic(status, directory)
    if not dynamic:
        source = path.read_text(encoding="utf-8")
        assert "fcntl.flock" in source and '"status"' in source and "RECEIPT_DIR" in source
        assert "start_or_attach" in source

print("Setup status concurrency, envelope, correlation, stale-state, and race-proof tests passed")
