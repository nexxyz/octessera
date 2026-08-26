#!/usr/bin/env python3
import importlib.util
import json
import os
import stat
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
COORDINATORS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup",
)
TMPFILES = (
    ROOT / "userpatches/overlay/etc/tmpfiles.d/octessera-setup-request.conf",
    ROOT / "tools/pi-image/stage4-octessera/files/root/etc/tmpfiles.d/octessera-setup-request.conf",
)


def load(path, name):
    config_path = path.parent.parent / "lib/octessera/setup_config.py"
    config_spec = importlib.util.spec_from_loader("setup_config", SourceFileLoader("setup_config", str(config_path)))
    assert config_spec is not None and config_spec.loader is not None
    config = importlib.util.module_from_spec(config_spec)
    import sys
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


for index, path in enumerate(COORDINATORS):
    coordinator, _http_module = load(path, f"status_coordinator_{index}")
    expected_status = (
        "d /run/octessera-setup-status 0750 root octessera-runtime -"
        if index == 0
        else "d /run/octessera-setup-status 0750 root pi -"
    )
    assert expected_status in TMPFILES[index].read_text(encoding="utf-8").splitlines()
    valid = (
        coordinator._status("starting", disposition="accepted"),
        coordinator._status("portal_ready", portal_suffix="abcd"),
        coordinator._status("finalizing"),
        coordinator._status("succeeded"),
        coordinator._status("failed", error_code="operation_failed"),
        coordinator._status("timed_out", error_code="unavailable"),
    )
    expected = (
        {"type": "setup_portal_status", "phase": "starting", "disposition": "accepted", "rebootRequired": False},
        {"type": "setup_portal_status", "phase": "portal_ready", "portalSuffix": "abcd", "rebootRequired": False},
        {"type": "setup_portal_status", "phase": "finalizing", "rebootRequired": False},
        {"type": "setup_portal_status", "phase": "succeeded", "rebootRequired": False},
        {"type": "setup_portal_status", "phase": "failed", "rebootRequired": False, "errorCode": "operation_failed"},
        {"type": "setup_portal_status", "phase": "timed_out", "rebootRequired": False, "errorCode": "unavailable"},
    )
    for status, exact in zip(valid, expected):
        assert status == exact
        assert "transfer" not in status
    for args in (
        ("starting", None, None, None),
        ("portal_ready", None, "ABCD", None),
        ("failed", None, None, "unavailable"),
        ("timed_out", None, None, "operation_failed"),
        ("unsupported", None, None, "unsupported"),
    ):
        try:
            coordinator._status(*args)
        except ValueError:
            pass
        else:
            raise AssertionError(f"invalid status accepted: {args}")

    if getattr(os, "geteuid", lambda: -1)() == 0:
        with tempfile.TemporaryDirectory() as directory:
            coordinator.STATUS_DIR = str(Path(directory) / "status")
            coordinator.STATUS_PATH = str(Path(coordinator.STATUS_DIR) / "current.json")
            profile = {"status_group": "root"}
            coordinator.write_status(valid[0], profile)
            coordinator.write_status(valid[-1], profile)
            payload = json.loads(Path(coordinator.STATUS_PATH).read_text(encoding="utf-8"))
            assert set(payload) == {"schema", "status"}
            assert payload["schema"] == 1 and payload["status"] == expected[-1]
            metadata = os.stat(coordinator.STATUS_PATH)
            assert metadata.st_uid == 0 and metadata.st_nlink == 1 and stat.S_IMODE(metadata.st_mode) == 0o640
            assert not list(Path(coordinator.STATUS_DIR).glob("*.tmp"))

print("Setup status envelope, phase combinations, atomic mode, and terminal-current tests passed")
