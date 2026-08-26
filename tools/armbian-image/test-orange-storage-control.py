#!/usr/bin/env python3
from __future__ import annotations

import io
import importlib.machinery
import socket
import subprocess
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]


def load(path: Path):
    loader = importlib.machinery.SourceFileLoader("orange_storage_control", str(path))
    module = loader.load_module()
    return module


def test_protocol() -> None:
    source = load(ROOT / "tools/storage/octessera-orange-storage-control")
    with patch.object(source.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, b"HOST_STATE=configured\n", b"")) as run:
        output = io.BytesIO()
        assert source.handle_request(io.BytesIO(b"storage-start\n"), output)
        assert output.getvalue() == b"accepted\nHOST_STATE=configured\n"
        run.assert_called_once_with(
            [source.STORAGE_PATH, "storage-start"],
            check=False,
            capture_output=True,
            timeout=source.STORAGE_TIMEOUT_SECONDS,
        )

    with patch.object(
        source.subprocess,
        "run",
        return_value=subprocess.CompletedProcess([], 1, b"stdout\n", b"bad\n\xff"),
    ):
        output = io.BytesIO()
        assert not source.handle_request(io.BytesIO(b"storage-stop\n"), output)
        assert output.getvalue().startswith(b"rejected\nHOST_STATE=unknown\nERROR=bad ?\n")
        assert len(output.getvalue()) <= source.MAX_RESPONSE_BYTES

    with patch.object(
        source.subprocess,
        "run",
        side_effect=subprocess.TimeoutExpired(source.STORAGE_PATH, source.STORAGE_TIMEOUT_SECONDS),
    ):
        output = io.BytesIO()
        try:
            source.handle_request(io.BytesIO(b"storage-start\n"), output)
        except subprocess.TimeoutExpired:
            pass
        else:
            raise AssertionError("storage action timeout was not propagated")
        assert output.getvalue().startswith(b"rejected\nHOST_STATE=unknown\nERROR=")
        assert len(output.getvalue()) <= source.MAX_RESPONSE_BYTES

    for request in (b"storage-start\nextra", b"storage-start", b"storage-start\r\n", b"other\n", b"storage-stop\n\x00"):
        output = io.BytesIO()
        try:
            source.handle_request(io.BytesIO(request), output)
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted invalid storage request: {request!r}")
        assert output.getvalue().startswith(b"rejected\nHOST_STATE=unknown\nERROR=")

    output = io.BytesIO()
    try:
        source.handle_request(io.BytesIO(b"x" * (source.MAX_REQUEST_BYTES + 1)), output)
    except ValueError:
        pass
    else:
        raise AssertionError("accepted oversized storage request")
    assert len(output.getvalue()) <= source.MAX_RESPONSE_BYTES

    reader, writer = socket.socketpair()
    try:
        with patch.object(source, "REQUEST_TIMEOUT_SECONDS", 0.01):
            try:
                source._read_request(reader.makefile("rb"))
            except TimeoutError:
                pass
            else:
                raise AssertionError("storage request timeout was not enforced")
    finally:
        reader.close()
        writer.close()


def test_security_shape() -> None:
    broker = (ROOT / "tools/storage/octessera-orange-storage-control").read_text(encoding="utf-8")
    lifecycle = (ROOT / "tools/storage/octessera-orange-storage").read_text(encoding="utf-8")
    assert "sudo" not in broker and "systemctl" not in broker
    assert "--config" not in lifecycle and "OCTESSERA_SD_" not in lifecycle
    assert "musb-hdrc.4.auto" in lifecycle
    assert (ROOT / "tools/storage/octessera-orange-storage").read_bytes() == (
        ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-storage"
    ).read_bytes()
    assert (ROOT / "tools/storage/octessera-orange-storage-control").read_bytes() == (
        ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-storage-control"
    ).read_bytes()
    assert "SocketMode=0660" in (ROOT / "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control.socket").read_text()
    assert "SocketGroup=octessera-runtime" in (ROOT / "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control.socket").read_text()
    assert "TimeoutStartSec=5s" in (ROOT / "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control@.service").read_text()


if __name__ == "__main__":
    test_protocol()
    test_security_shape()
    print("Orange storage-control protocol and security fixture tests passed")
