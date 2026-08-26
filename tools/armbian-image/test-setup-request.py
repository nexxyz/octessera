#!/usr/bin/env python3
import importlib.util
import os
import stat
import tempfile
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
    coordinator, _http_module = load(path, f"request_coordinator_{index}")
    valid = SimpleNamespace(st_mode=stat.S_IFREG | 0o600, st_uid=1000, st_gid=1000, st_nlink=1, st_size=6)
    assert coordinator.valid_request_metadata(valid, 1000, 1000, b"start\n")
    for invalid, content in (
        (SimpleNamespace(**{**valid.__dict__, "st_mode": stat.S_IFLNK | 0o600}), b"start\n"),
        (SimpleNamespace(**{**valid.__dict__, "st_mode": stat.S_IFREG | 0o644}), b"start\n"),
        (SimpleNamespace(**{**valid.__dict__, "st_nlink": 2}), b"start\n"),
        (SimpleNamespace(**{**valid.__dict__, "st_size": 7}), b"start\n"),
        (SimpleNamespace(**{**valid.__dict__, "st_uid": 1001}), b"start\n"),
        (valid, b"START\n"),
        (valid, b"start"),
    ):
        assert not coordinator.valid_request_metadata(invalid, 1000, 1000, content)
    source = path.read_text(encoding="utf-8")
    assert 'REQUEST_PATH = f"{INBOX_DIR}/start"' in source
    assert 'PathExists=/run/octessera-setup-request/inbox/start' not in source
    assert "start-or-attach" not in source
    assert "os.link(" not in source
    assert "claims" not in source
    assert "replay" not in source.lower()

    if getattr(os, "geteuid", lambda: -1)() == 0:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request_dir = root / "request"
            inbox = request_dir / "inbox"
            inbox.mkdir(parents=True)
            request_dir.chmod(0o711)
            inbox.chmod(0o700)
            marker = inbox / "start"
            coordinator.REQUEST_DIR = str(request_dir)
            coordinator.INBOX_DIR = str(inbox)
            coordinator.REQUEST_PATH = str(marker)
            coordinator._expected_owner = lambda _profile: (os.getuid(), os.getgid())
            profile = {"request_owner": "test", "status_group": "test"}
            marker.write_bytes(b"start\n")
            marker.chmod(0o600)
            assert coordinator.consume_request(profile) is None
            assert marker.exists()
            instance = coordinator.Coordinator(profile)
            instance.cleanup_request_marker()
            assert not marker.exists()
            marker.write_bytes(b"wrong\n")
            marker.chmod(0o600)
            try:
                coordinator.consume_request(profile)
            except ValueError:
                pass
            else:
                raise AssertionError("invalid marker accepted")
            assert marker.exists()

print("Setup marker exact-content, ownership, mode, and no-link tests passed")
