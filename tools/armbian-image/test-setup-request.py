#!/usr/bin/env python3
import importlib.util
import os
import stat
import tempfile
import time
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
HELPERS = (
    (ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup-request", "orange-pi-zero-2w", "octessera-runtime"),
    (ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request", "raspberry-pi-zero-2w", "pi"),
)


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


for index, (path, profile, owner) in enumerate(HELPERS):
    helper = load(path, f"request_{index}")
    assert helper.PROFILE == profile
    assert helper.REQUEST_OWNER == owner
    assert helper.SETUP_UNIT == "octessera-setup.service"
    valid = SimpleNamespace(st_mode=stat.S_IFREG | 0o600, st_uid=1000, st_nlink=1, st_size=33, st_mtime=95.0)
    assert helper._valid_request_metadata(valid, 1000, 100.0)
    for invalid in (
        SimpleNamespace(**{**valid.__dict__, "st_mode": stat.S_IFLNK | 0o600}),
        SimpleNamespace(**{**valid.__dict__, "st_mode": stat.S_IFREG | 0o644}),
        SimpleNamespace(**{**valid.__dict__, "st_nlink": 2}),
        SimpleNamespace(**{**valid.__dict__, "st_size": 1}),
        SimpleNamespace(**{**valid.__dict__, "st_uid": 1001}),
        SimpleNamespace(**{**valid.__dict__, "st_mtime": 89.0}),
        SimpleNamespace(**{**valid.__dict__, "st_mtime": 101.0}),
    ):
        assert not helper._valid_request_metadata(invalid, 1000, 100.0)
    source = path.read_text(encoding="utf-8")
    assert '["systemctl", "start", SETUP_UNIT]' in source
    assert "start-or-attach" in source
    assert "systemctl\", \"restart" not in source
    assert "list-unit-files" not in source
    assert "shell=True" not in source
    assert "setup-force" not in source
    assert "os.rename(REQUEST_PATH, claim_path)" in source
    assert "os.link(" not in source

    if getattr(os, "geteuid", lambda: -1)() != 0:
        continue
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        request_dir = root / "request"
        control = root / "control"
        request_dir.mkdir()
        request = request_dir / "setup-portal.request"
        helper.REQUEST_PATH = str(request)
        helper.CONTROL_DIR = str(control)
        helper.CLAIM_DIR = str(control / "claims")
        helper._expected_owner_uid = lambda: 65534

        def write_marker(content=b"0" * 32 + b"\n", age=0, hardlink=None):
            request.write_bytes(content)
            request.chmod(0o600)
            os.chown(request, 65534, 65534)
            if age:
                old = time.time() - age
                os.utime(request, (old, old))
            if hardlink is not None:
                os.link(request, hardlink)

        write_marker(age=11)
        assert helper._claim_request() is None
        assert not request.exists() and not list((control / "claims").iterdir())

        write_marker(hardlink=root / "surviving-link")
        assert helper._claim_request() is None
        assert not request.exists() and (root / "surviving-link").exists() and not list((control / "claims").iterdir())
        (root / "surviving-link").unlink()

        write_marker()
        claimed = helper._claim_request()
        assert claimed is not None
        claim_path, token, inode = claimed
        assert token == "0" * 32
        replacement = request
        replacement.write_bytes(b"1" * 32 + b"\n")
        replacement.chmod(0o600)
        os.chown(replacement, 65534, 65534)
        helper._delete_claim(claim_path, inode)
        assert replacement.exists() and not list((control / "claims").iterdir())

        write_marker()
        real_rename = os.rename
        def rename_with_retrigger(source, destination):
            real_rename(source, destination)
            request.write_bytes(b"2" * 32 + b"\n")
            request.chmod(0o600)
            os.chown(request, 65534, 65534)
        helper.os.rename = rename_with_retrigger
        try:
            claimed = helper._claim_request()
        finally:
            helper.os.rename = real_rename
        assert claimed is not None and request.exists()
        helper._delete_claim(claimed[0], claimed[2])
        assert request.exists() and not list((control / "claims").iterdir())

        request.unlink()
        request.symlink_to(root / "missing-target")
        assert helper._claim_request() is None
        assert not request.exists() and not list((control / "claims").iterdir())

print("Setup request validation, quarantine, hardlink, replacement, and retrigger tests passed")
