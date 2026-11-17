#!/usr/bin/env python3
import os
import re

import setup_config


CONNECTIONS_DIR = "/etc/NetworkManager/system-connections"
JOINING_PATH = "/var/lib/octessera/setup-joining"


def _keyfile_string(value):
    return value.replace("\\", "\\\\").replace(" ", "\\s")


def profile_path(uuid):
    return f"{CONNECTIONS_DIR}/octessera-setup-{uuid}.nmconnection"


def keyfile(uuid, ssid, passphrase, hidden):
    lines = [
        "[connection]",
        f"id={_keyfile_string(ssid)}",
        f"uuid={uuid}",
        "type=wifi",
        "interface-name=wlan0",
        "",
        "[wifi]",
        "mode=infrastructure",
        "ssid=" + "".join(f"{byte};" for byte in ssid.encode("utf-8")),
    ]
    lines += ["hidden=true", ""] if hidden else [""]
    if passphrase:
        lines += ["[wifi-security]", "key-mgmt=wpa-psk", f"psk={_keyfile_string(passphrase)}", ""]
    lines += ["[ipv4]", "method=auto", "", "[ipv6]", "method=auto", ""]
    return "\n".join(lines)


def _unescape(value):
    result = ""
    escaped = False
    for character in value:
        if escaped or character != "\\":
            result += character
            escaped = False
        else:
            escaped = True
    return result


def profiles_for_ssid(run, ssid):
    listing = run(["nmcli", "-t", "-f", "UUID,TYPE", "connection", "show"])
    if listing.returncode != 0:
        return []
    matches = []
    for line in listing.stdout.splitlines():
        fields = line.split(":", 1)
        if len(fields) != 2 or fields[1] != "802-11-wireless":
            continue
        uuid = fields[0]
        mode = run(["nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", uuid])
        current = run(["nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", uuid])
        if mode.returncode == 0 and current.returncode == 0 and mode.stdout.strip() != "ap" and _unescape(current.stdout.rstrip("\n")) == ssid:
            matches.append(uuid)
    return matches


def _remove(path):
    try:
        os.remove(path)
    except FileNotFoundError:
        pass


def join(run, uuid, ssid, passphrase, hidden, wait_seconds):
    setup_config._write_atomic(JOINING_PATH, f"{uuid}\n", 0o600)
    path = profile_path(uuid)
    setup_config._write_atomic(path, keyfile(uuid, ssid, passphrase, hidden), 0o600)
    return run(["nmcli", "connection", "load", path]).returncode == 0 and run(["nmcli", "--wait", str(wait_seconds), "connection", "up", "uuid", uuid]).returncode == 0


def keep():
    _remove(JOINING_PATH)


def forget(run):
    try:
        with open(JOINING_PATH, encoding="ascii") as handle:
            uuid = handle.read().strip()
    except (OSError, UnicodeError):
        return
    try:
        if re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", uuid):
            try:
                run(["nmcli", "connection", "delete", "uuid", uuid])
            finally:
                _remove(profile_path(uuid))
    finally:
        _remove(JOINING_PATH)
