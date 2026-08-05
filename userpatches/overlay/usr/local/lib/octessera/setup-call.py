#!/usr/bin/env python3
import http.client
import os
import sys


NONCE_PATH = "/run/octessera-setup/nonce"
ENDPOINTS = {"finalize", "discard"}


def call(endpoint):
    if endpoint not in ENDPOINTS:
        raise ValueError("invalid setup endpoint")
    with open(NONCE_PATH, "r", encoding="ascii") as handle:
        nonce = handle.read()
    if len(nonce) < 32 or len(nonce) > 128 or any(character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_" for character in nonce):
        raise ValueError("invalid setup nonce")
    connection = http.client.HTTPConnection("127.0.0.1", 8080, timeout=5)
    try:
        connection.request("POST", f"/{endpoint}", body=b"_", headers={"X-Octessera-Setup-Nonce": nonce})
        response = connection.getresponse()
        response.read()
        return response.status == 200
    finally:
        connection.close()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(2)
    raise SystemExit(0 if call(sys.argv[1]) else 1)
