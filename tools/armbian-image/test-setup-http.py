#!/usr/bin/env python3
import http.client
import importlib.util
import ipaddress
import json
import socket
import sys
import threading
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
COORDINATORS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup",
)
PAYLOAD = {"sshMode": "none", "sshPublicKey": "", "sshPassword": "", "sshPasswordConfirm": "", "hostname": "", "wifiCountry": "US"}


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


def request(port, payload, headers=None, path="/stage"):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    request_headers = {"Host": "127.0.0.1", "Origin": "http://192.168.42.1", "Content-Type": "application/json"}
    request_headers.update(headers or {})
    connection.request("POST", path, body=json.dumps(payload).encode("utf-8"), headers=request_headers)
    response = connection.getresponse()
    body = response.read()
    connection.close()
    return response.status, body


for index, path in enumerate(COORDINATORS):
    coordinator_module, http_module = load(path, f"http_coordinator_{index}")
    http_module.AP_NETWORK = ipaddress.ip_network("127.0.0.0/8")
    http_module.ALLOWED_HOSTS = frozenset(("127.0.0.1",))
    country_calls = []
    coordinator_module.setup_config.apply_country = lambda country: country_calls.append(country)
    instance = coordinator_module.Coordinator({"status_group": "root", "request_owner": "root", "user": "pi"})
    server = http_module.SetupHTTPServer(("127.0.0.1", 0), instance)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        port = server.server_address[1]
        status, body = request(port, PAYLOAD)
        assert status == 200 and body == b'{"ok":true}'
        assert instance.staged["sshMode"] == "none" and country_calls == ["US"]
        status, body = request(port, {"wifiCountry": "us"}, path="/country")
        assert status == 200 and body == b'{"ok":true}' and country_calls[-1] == "US"
        status, _ = request(port, {"wifiCountry": "USA"}, path="/country")
        assert status == 400
        status, _ = request(port, {**PAYLOAD, "unexpected": "x"})
        assert status == 400
        status, _ = request(port, PAYLOAD, {"Content-Type": "application/json; charset=utf-8"})
        assert status == 403
        status, _ = request(port, PAYLOAD, {"Origin": "http://192.168.42.1:81"})
        assert status == 403
        status, _ = request(port, PAYLOAD, {"Host": "127.0.0.2"})
        assert status == 403
        status, _ = request(port, {**PAYLOAD, "sshMode": "password", "sshPassword": "eight888", "sshPasswordConfirm": "eight888"})
        assert status == 400
        assert "eight888" not in body.decode("utf-8")

        raw = socket.create_connection(("127.0.0.1", port), timeout=3)
        raw.sendall(b"POST /stage HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: http://192.168.42.1\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nContent-Length: 1\r\n\r\nx")
        assert int(raw.recv(4096).split(b" ", 2)[1]) == 400
        raw.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=3)

print("Setup HTTP subnet, host, origin, body, country, stage, and secret tests passed")
