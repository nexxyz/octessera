#!/usr/bin/env python3
import http.server
import ipaddress
import json
import re
import subprocess
from typing import cast

import setup_config


AP_ADDRESS = "192.168.42.1"
AP_NETWORK = ipaddress.ip_network("192.168.42.0/24")
ALLOWED_ORIGINS = frozenset(("http://192.168.42.1", "http://192.168.42.1:80"))
ALLOWED_HOSTS = frozenset(("192.168.42.1:8080",))
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

    def _allowed(self):
        try:
            address_allowed = ipaddress.ip_address(self.client_address[0]) in AP_NETWORK
        except ValueError:
            address_allowed = False
        return address_allowed and self._one_header("Host") in ALLOWED_HOSTS and self._one_header("Origin") in ALLOWED_ORIGINS

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

    def _send(self, code, payload):
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        origin = self._one_header("Origin")
        if origin in ALLOWED_ORIGINS:
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        if not self._allowed():
            self._send(403, {"error": "forbidden"})
            return
        self.send_response(204)
        origin = self._one_header("Origin")
        assert origin is not None
        self.send_header("Access-Control-Allow-Origin", origin)
        self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Access-Control-Max-Age", "0")
        self.end_headers()

    def do_POST(self):
        if not self._allowed():
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
                setup_config.apply_country(staged["country"])
                cast(SetupHTTPServer, self.server).coordinator.stage(staged)
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


class SetupHTTPServer(http.server.HTTPServer):
    allow_reuse_address = True

    def __init__(self, address, coordinator):
        self.coordinator = coordinator
        super().__init__(address, SetupHandler)
