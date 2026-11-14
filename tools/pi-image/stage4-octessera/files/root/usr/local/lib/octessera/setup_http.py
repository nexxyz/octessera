#!/usr/bin/env python3
import http.server
import ipaddress
import json
import os
import re
import subprocess
import urllib.parse
from typing import cast

import setup_config
import setup_hotspot


AP_ADDRESS = setup_hotspot.AP_ADDRESS
AP_NETWORK = ipaddress.ip_network("192.168.42.0/24")
ALLOWED_ORIGINS = frozenset(("http://192.168.42.1", "http://192.168.42.1:80"))
ALLOWED_HOSTS = frozenset(("192.168.42.1", "192.168.42.1:80"))
UI_ROOT = "/usr/local/share/octessera-setup-ui"
CONTENT_TYPES = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8", ".svg": "image/svg+xml"}
MAX_BODY = 16384


class SetupHandler(http.server.BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(10)

    def log_message(self, format, *_args):
        return

    def _one_header(self, name):
        values = self.headers.get_all(name, [])
        return values[0] if len(values) == 1 else None

    def _from_portal(self):
        try:
            return ipaddress.ip_address(self.client_address[0]) in AP_NETWORK
        except ValueError:
            return False

    def _body(self):
        if self.headers.get_all("Transfer-Encoding", []):
            return None, 400
        value = self._one_header("Content-Length")
        if value is None or len(value) > 5 or not re.fullmatch(r"[1-9][0-9]*", value):
            return None, 400
        length = int(value)
        if length > MAX_BODY:
            return None, 413
        body = self.rfile.read(length)
        return (body, 200) if len(body) == length else (None, 400)

    def _send_bytes(self, code, content_type, body):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send(self, code, payload):
        self._send_bytes(code, "application/json", json.dumps(payload, separators=(",", ":")).encode("utf-8"))

    def _redirect_to_portal(self):
        self.send_response(302)
        self.send_header("Location", f"http://{AP_ADDRESS}/")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def _static_file(self, path):
        relative = "index.html" if path == "/" else path.removeprefix("/")
        parts = relative.split("/")
        extension = os.path.splitext(relative)[1]
        if not path.startswith("/") or extension not in CONTENT_TYPES or any(part in ("", ".", "..") or "\\" in part for part in parts):
            return None
        root = os.path.realpath(UI_ROOT)
        candidate = os.path.realpath(os.path.join(root, *parts))
        if not candidate.startswith(root + os.sep) or not os.path.isfile(candidate):
            return None
        with open(candidate, "rb") as handle:
            return CONTENT_TYPES[extension], handle.read()

    def do_GET(self):
        if not self._from_portal():
            self._send(403, {"error": "forbidden"})
            return
        if self._one_header("Host") not in ALLOWED_HOSTS:
            self._redirect_to_portal()
            return
        path = urllib.parse.urlsplit(self.path).path
        if path == "/networks":
            self._send(200, cast(SetupHTTPServer, self.server).coordinator.networks)
            return
        try:
            found = self._static_file(path)
        except OSError:
            found = None
        if found is None:
            self._send(404, {"error": "not_found"})
            return
        self._send_bytes(200, *found)

    def do_POST(self):
        if not self._from_portal() or self._one_header("Host") not in ALLOWED_HOSTS or self._one_header("Origin") not in ALLOWED_ORIGINS:
            self._send(403, {"error": "forbidden"})
            return
        if self._one_header("Content-Type") != "application/json":
            self._send(403, {"error": "forbidden"})
            return
        try:
            body, body_status = self._body()
            if body_status != 200 or body is None:
                self._send(body_status, {"error": "invalid_body"})
                return
            data = json.loads(body.decode("utf-8"), parse_constant=lambda _value: (_ for _ in ()).throw(ValueError()))
            if self.path == "/country":
                country = setup_config.validate_country_payload(data)
                setup_config.apply_country(country)
            elif self.path == "/stage":
                staged = setup_config.validate_stage(data)
                coordinator = cast(SetupHTTPServer, self.server).coordinator
                coordinator.check_network(staged)
                setup_config.apply_country(staged["country"])
                coordinator.stage(staged)
            else:
                self._send(404, {"error": "not_found"})
                return
        except (ValueError, UnicodeDecodeError, json.JSONDecodeError):
            self._send(400, {"error": "invalid_input"})
            return
        except (OSError, subprocess.CalledProcessError):
            self._send(500, {"error": "operation_failed"})
            return
        self._send(200, {"ok": True})


class SetupHTTPServer(http.server.ThreadingHTTPServer):
    allow_reuse_address = True

    def __init__(self, address, coordinator):
        self.coordinator = coordinator
        super().__init__(address, SetupHandler)
