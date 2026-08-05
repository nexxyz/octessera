#!/usr/bin/env python3
import http.client
import importlib.util
import ipaddress
import json
import os
import socket
import tempfile
import threading
from email.message import Message
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
SIDECARS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup-sidecar",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar",
)
PAYLOAD = {"sshMode": "none", "sshPublicKey": "", "sshPassword": "", "sshPasswordConfirm": "", "hostname": "", "wifiCountry": "US"}


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def request(port, payload, headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    request_headers = {"Origin": "http://192.168.42.1", "Content-Type": "application/json"}
    request_headers.update(headers or {})
    connection.request("POST", "/stage", body=payload, headers=request_headers)
    response = connection.getresponse()
    body = response.read()
    connection.close()
    return response.status, body


def raw_status(port, headers, body=b""):
    lines = ["POST /stage HTTP/1.1", "Host: 127.0.0.1", "Connection: close"]
    lines.extend(headers)
    packet = ("\r\n".join(lines) + "\r\n\r\n").encode("ascii") + body
    with socket.create_connection(("127.0.0.1", port), timeout=3) as connection:
        connection.sendall(packet)
        response = connection.recv(4096)
    return int(response.split(b" ", 2)[1])


for index, path in enumerate(SIDECARS):
    sidecar = load(path, f"http_sidecar_{index}")
    source = path.read_text(encoding="utf-8")
    assert "READINESS_PATH" in source
    assert "_publish_readiness" in source
    assert "self.connection.settimeout(10)" in source
    assert 'HTTPServer(("0.0.0.0", 8080), Handler)' in source
    setattr(sidecar, "SETUP_CLIENT_NET", ipaddress.ip_network("127.0.0.0/8"))
    server = sidecar.http.server.HTTPServer(("127.0.0.1", 0), sidecar.Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        port = server.server_address[1]
        good = json.dumps(PAYLOAD).encode("utf-8")
        status, body = request(port, good)
        assert status == 200 and body == b'{"ok":true}'
        assert sidecar.staged["sshMode"] == "none"
        status, body = request(port, json.dumps({**PAYLOAD, "unexpected": "value"}).encode("utf-8"))
        assert status == 400 and b"unexpected" not in body
        assert not sidecar.staged
        status, _ = request(port, good, {"Content-Type": "application/json; charset=utf-8"})
        assert status == 403
        status, _ = request(port, good, {"Origin": "http://192.168.42.1:81"})
        assert status == 403
        status, _ = request(port, good, {"Transfer-Encoding": "chunked"})
        assert status == 400
        status, _ = request(port, b"x", {"Content-Length": "0"})
        assert status == 400
        common = ["Origin: http://192.168.42.1", "Content-Type: application/json"]
        assert raw_status(port, common + ["Content-Length: -1"]) == 400
        assert raw_status(port, common + ["Content-Length: 1.0"]) == 400
        assert raw_status(port, common + ["Content-Length: 1", "Content-Length: 1"], b"x") == 400
        assert raw_status(port, common) == 400
        assert raw_status(port, common + ["Content-Length: 16385"]) == 413

        nonce_file = tempfile.NamedTemporaryFile(prefix="octessera-nonce-", delete=False)
        nonce_path = Path(nonce_file.name)
        try:
            nonce_file.write(b"nonce-value-abcdefghijklmnopqrstuvwxyz")
            nonce_file.close()
            nonce_path.chmod(0o600)
            setattr(sidecar, "NONCE_PATH", str(nonce_path))
            headers = Message()
            headers.add_header("X-Octessera-Setup-Nonce", "nonce-value-abcdefghijklmnopqrstuvwxyz")
            fake_handler = SimpleNamespace(client_address=("127.0.0.1", 1), headers=headers)
            geteuid = getattr(os, "geteuid", None)
            if geteuid is not None and geteuid() == 0:
                assert sidecar._consume_nonce(fake_handler)
                assert not nonce_path.exists()
                assert not sidecar._consume_nonce(fake_handler)
        finally:
            nonce_path.unlink(missing_ok=True)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)

print("Setup HTTP framing and nonce tests passed")
