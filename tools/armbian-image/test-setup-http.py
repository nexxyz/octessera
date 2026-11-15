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
PUBLIC_KEY = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
PAYLOAD = {"sshMode": "none", "sshPublicKey": "", "accountPassword": "eight888", "accountPasswordConfirm": "eight888", "hostname": "", "wifiCountry": "US", "wifiSsid": "Home Net", "wifiPassphrase": "wifi pass", "wifiHidden": False}
NETWORKS = [{"ssid": "Home Net", "security": "wpa", "strength": 80}, {"ssid": "Cafe", "security": "none", "strength": 40}]
VALID_MODE_PAYLOADS = (
    {**PAYLOAD, "sshMode": "key", "sshPublicKey": PUBLIC_KEY},
    {**PAYLOAD, "sshMode": "password"},
    PAYLOAD,
    {**PAYLOAD, "wifiSsid": "Attic ☂", "wifiHidden": True},
    {**PAYLOAD, "wifiSsid": "Attic", "wifiPassphrase": "", "wifiHidden": True},
)
INVALID_PASSWORD_PAYLOADS = (
    {key: value for key, value in PAYLOAD.items() if key != "accountPassword"},
    {key: value for key, value in PAYLOAD.items() if key != "accountPasswordConfirm"},
    {**PAYLOAD, "accountPassword": "short", "accountPasswordConfirm": "short"},
    {**PAYLOAD, "accountPassword": "x" * 129, "accountPasswordConfirm": "x" * 129},
    {**PAYLOAD, "accountPassword": "        ", "accountPasswordConfirm": "        "},
    {**PAYLOAD, "accountPassword": "line\nbreak", "accountPasswordConfirm": "line\nbreak"},
    {**PAYLOAD, "accountPasswordConfirm": "different"},
)

INVALID_WIFI_PAYLOADS = (
    {key: value for key, value in PAYLOAD.items() if key != "wifiSsid"},
    {**PAYLOAD, "wifiSsid": "Hidden"},
    {**PAYLOAD, "wifiPassphrase": ""},
    {**PAYLOAD, "wifiPassphrase": "short"},
    {**PAYLOAD, "wifiPassphrase": "x" * 64},
    {**PAYLOAD, "wifiSsid": "Cafe"},
    {**PAYLOAD, "wifiSsid": "x" * 33, "wifiPassphrase": "eight888"},
    {key: value for key, value in PAYLOAD.items() if key != "wifiHidden"},
    {**PAYLOAD, "wifiHidden": "true"},
    {**PAYLOAD, "wifiHidden": 1},
    {**PAYLOAD, "wifiSsid": "", "wifiHidden": True},
    {**PAYLOAD, "wifiSsid": "☂" * 11, "wifiHidden": True},
    {**PAYLOAD, "wifiSsid": "Attic\x7f", "wifiHidden": True},
    {**PAYLOAD, "wifiSsid": "Attic", "wifiPassphrase": "short", "wifiHidden": True},
    {**PAYLOAD, "wifiSsid": "Attic", "wifiPassphrase": "x" * 64, "wifiHidden": True},
)


def load(path, name):
    config_path = path.parent.parent / "lib/octessera/setup_config.py"
    config_spec = importlib.util.spec_from_loader("setup_config", SourceFileLoader("setup_config", str(config_path)))
    assert config_spec is not None and config_spec.loader is not None
    config = importlib.util.module_from_spec(config_spec)
    sys.modules["setup_config"] = config
    config_spec.loader.exec_module(config)
    for library in ("setup_hotspot", "setup_wifi_join"):
        library_spec = importlib.util.spec_from_loader(library, SourceFileLoader(library, str(path.parent.parent / f"lib/octessera/{library}.py")))
        assert library_spec is not None and library_spec.loader is not None
        sys.modules[library] = importlib.util.module_from_spec(library_spec)
        library_spec.loader.exec_module(sys.modules[library])
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


def get(port, path, host="127.0.0.1"):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    connection.putrequest("GET", path, skip_host=True)
    if host is not None:
        connection.putheader("Host", host)
    connection.endheaders()
    response = connection.getresponse()
    body = response.read()
    connection.close()
    return response.status, response.getheader("Content-Type"), response.getheader("Location"), body


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
    http_module.UI_ROOT = str(path.parent.parent / "share/octessera-setup-ui")
    country_calls = []
    coordinator_module.setup_config.apply_country = lambda country: country_calls.append(country)
    clock_value = [10.0]
    instance = coordinator_module.Coordinator(
        {"status_group": "root", "request_owner": "root", "user": "pi"},
        clock=lambda: clock_value[0],
    )
    instance.networks = NETWORKS
    server = http_module.SetupHTTPServer(("127.0.0.1", 0), instance)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        port = server.server_address[1]
        for mode_payload in VALID_MODE_PAYLOADS:
            mode_country_calls = []
            coordinator_module.setup_config.apply_country = lambda country: mode_country_calls.append(country)
            mode_instance = coordinator_module.Coordinator(
                {"status_group": "root", "request_owner": "root", "user": "pi"},
                clock=lambda: clock_value[0],
            )
            mode_instance.networks = NETWORKS
            mode_server = http_module.SetupHTTPServer(("127.0.0.1", 0), mode_instance)
            mode_thread = threading.Thread(target=mode_server.serve_forever, daemon=True)
            mode_thread.start()
            try:
                status, body = request(mode_server.server_address[1], mode_payload)
                assert status == 200 and body == b'{"ok":true}'
                assert mode_instance.staged["sshMode"] == mode_payload["sshMode"]
                assert mode_instance.staged["accountPassword"] == "eight888"
                assert ("sshKey" in mode_instance.staged) == (mode_payload["sshMode"] == "key")
                assert (mode_instance.staged["ssid"], mode_instance.staged["passphrase"], mode_instance.staged["hidden"]) == (mode_payload["wifiSsid"], mode_payload["wifiPassphrase"], mode_payload["wifiHidden"])
                assert mode_country_calls == ["US"]
            finally:
                mode_server.shutdown()
                mode_server.server_close()
                mode_thread.join(timeout=3)
        coordinator_module.setup_config.apply_country = lambda country: country_calls.append(country)
        for invalid in INVALID_PASSWORD_PAYLOADS + INVALID_WIFI_PAYLOADS:
            status, body = request(port, invalid)
            assert status == 400 and body == b'{"error":"invalid_input"}'
            assert instance.staged is None and country_calls == []
        status, body = request(port, PAYLOAD)
        assert status == 200 and body == b'{"ok":true}'
        assert set(instance.staged) == {"sshMode", "accountPassword", "hostname", "country", "ssid", "passphrase", "hidden"}
        assert instance.staged["ssid"] == "Home Net" and instance.staged["passphrase"] == "wifi pass"
        assert instance.staged["accountPassword"] == "eight888" and country_calls == ["US"]
        staged = dict(instance.staged)
        staged_at = instance.staged_at
        clock_value[0] = 20.0
        status, body = request(port, PAYLOAD)
        assert status == 200 and body == b'{"ok":true}'
        assert instance.staged == staged and instance.staged_at == staged_at
        status, body = request(port, {**PAYLOAD, "hostname": "different"})
        assert status == 400 and body == b'{"error":"invalid_input"}'
        assert instance.staged == staged and instance.staged_at == staged_at
        status, body = request(port, {"wifiCountry": "us"}, path="/country")
        assert status == 200 and body == b'{"ok":true}' and country_calls[-1] == "US"
        status, _ = request(port, {"wifiCountry": "USA"}, path="/country")
        assert status == 400
        status, _ = request(port, {**PAYLOAD, "unexpected": "x"})
        assert status == 400
        status, body, location, content = get(port, "/")
        assert status == 200 and body == "text/html; charset=utf-8" and b"<title>Octessera setup</title>" in content and location is None
        status, body, _, content = get(port, "/js/app.js?v=4")
        assert status == 200 and body == "text/javascript; charset=utf-8" and b"const API" in content
        status, body, _, content = get(port, "/networks")
        assert status == 200 and body == "application/json" and json.loads(content) == NETWORKS
        for missing in ("/README.md", "/nope.html", "/../setup_http.py", "/js/../index.html", "/js//app.js", "/js\\app.js"):
            assert get(port, missing)[0] == 404, missing
        for host in ("www.msftconnecttest.com", "captive.apple.com", "connectivitycheck.gstatic.com", None):
            status, _, location, content = get(port, "/generate_204", host)
            assert status == 302 and location == "http://192.168.42.1/" and content == b""
        assert get(port, "/", "captive.apple.com")[0] == 302
        http_module.AP_NETWORK = ipaddress.ip_network("10.0.0.0/8")
        assert get(port, "/")[0] == 403 and get(port, "/networks")[0] == 403
        assert request(port, PAYLOAD)[0] == 403
        http_module.AP_NETWORK = ipaddress.ip_network("127.0.0.0/8")
        status, _ = request(port, PAYLOAD, {"Content-Type": "application/json; charset=utf-8"})
        assert status == 403
        status, _ = request(port, PAYLOAD, {"Origin": "http://192.168.42.1:81"})
        assert status == 403
        status, _ = request(port, PAYLOAD, {"Origin": "http://192.168.42.1:8080"})
        assert status == 403
        status, _ = request(port, PAYLOAD, {"Host": "127.0.0.2"})
        assert status == 403
        status, body = request(port, {**PAYLOAD, "sshMode": "password", "sshPassword": "eight888"})
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

print("Setup HTTP static files, networks, captive redirect, subnet, host, origin, body, Wi-Fi fields, country, duplicate-stage, and secret tests passed")
