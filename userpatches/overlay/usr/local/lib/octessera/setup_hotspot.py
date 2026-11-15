#!/usr/bin/env python3
import os
import re
import signal
import subprocess

import setup_config


AP_ADDRESS = "192.168.42.1"
INTERFACE = "wlan0"
HOSTAPD_CONFIG = "/run/octessera-setup-hostapd.conf"
RESCAN_SECONDS = 10
DNSMASQ_ARGS = (
    "--keep-in-foreground",
    "--conf-file",
    "--pid-file",
    "--no-hosts",
    "--no-resolv",
    "--leasefile-ro",
    f"--interface={INTERFACE}",
    "--except-interface=lo",
    "--bind-interfaces",
    f"--address=/#/{AP_ADDRESS}",
    "--dhcp-range=192.168.42.2,192.168.42.254",
    f"--dhcp-option=option:router,{AP_ADDRESS}",
)


def _terse_fields(line):
    fields = [""]
    escaped = False
    for character in line:
        if escaped:
            fields[-1] += character
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == ":":
            fields.append("")
        else:
            fields[-1] += character
    return fields


def _security(value):
    if value in ("", "--"):
        return "none"
    if "802.1X" in value or "WEP" in value:
        return None
    return "wpa"


def parse_networks(output):
    strongest = {}
    for line in (output or "").splitlines():
        fields = _terse_fields(line)
        if len(fields) != 3 or not fields[0] or not fields[2].isdigit() or _security(fields[1]) is None:
            continue
        network = {"ssid": fields[0], "security": _security(fields[1]), "strength": min(100, int(fields[2]))}
        if fields[0] not in strongest or network["strength"] > strongest[fields[0]]["strength"]:
            strongest[fields[0]] = network
    return sorted(strongest.values(), key=lambda network: (-network["strength"], network["ssid"]))


def hostapd_config(ssid, passphrase, management_frame_protection):
    # The Raspberry's firmware always advertises PSK and PSK-SHA256; hostapd must offer both, with required PMF, to match.
    key_management = ("wpa_key_mgmt=WPA-PSK WPA-PSK-SHA256", "ieee80211w=2") if management_frame_protection else ("wpa_key_mgmt=WPA-PSK",)
    return "\n".join((
        f"interface={INTERFACE}",
        "driver=nl80211",
        f"ssid={ssid}",
        "hw_mode=g",
        "channel=6",
        "ieee80211n=1",
        "wmm_enabled=1",
        "auth_algs=1",
        "wpa=2",
        *key_management,
        "rsn_pairwise=CCMP",
        f"wpa_passphrase={passphrase}",
        "",
    ))


class Hotspot:
    def __init__(self, run, process_factory, clock, sleeper):
        self.run = run
        self.process_factory = process_factory
        self.clock = clock
        self.sleeper = sleeper
        self.processes = []

    def scan(self):
        try:
            result = self.run(["nmcli", "-t", "-e", "yes", "-f", "SSID,SECURITY,SIGNAL", "device", "wifi", "list", "ifname", INTERFACE, "--rescan", "yes"], self.clock() + RESCAN_SECONDS)
        except subprocess.TimeoutExpired:
            return []
        return parse_networks(result.stdout) if result.returncode == 0 else []

    def _start(self, args):
        self.processes.append(self.process_factory(args, stdin=subprocess.DEVNULL, start_new_session=True))

    def start_access_point(self, ssid, passphrase, management_frame_protection):
        setup_config._write_atomic(HOSTAPD_CONFIG, hostapd_config(ssid, passphrase, management_frame_protection), 0o600)
        self._start(["hostapd", HOSTAPD_CONFIG])

    def access_point_up(self):
        result = self.run(["iw", "dev", INTERFACE, "info"])
        return result.returncode == 0 and bool(re.search(r"^\s*type AP\s*$", result.stdout or "", re.MULTILINE))

    def serve(self):
        if self.run(["ip", "addr", "add", f"{AP_ADDRESS}/24", "dev", INTERFACE]).returncode != 0:
            return False
        self._start(["dnsmasq", *DNSMASQ_ARGS])
        return True

    def running(self):
        return bool(self.processes) and all(process.poll() is None for process in self.processes)

    def _signal(self, process, value):
        try:
            os.killpg(os.getpgid(process.pid), value)
        except (AttributeError, OSError):
            try:
                if value == getattr(signal, "SIGKILL", None):
                    process.kill()
                else:
                    process.terminate()
            except OSError:
                pass

    def _stop_process(self, process, deadline):
        if process.poll() is not None:
            return
        self._signal(process, signal.SIGTERM)
        while process.poll() is None:
            if deadline is not None and self.clock() >= deadline:
                self._signal(process, signal.SIGKILL)
                break
            self.sleeper(0.05)
        try:
            process.wait(timeout=None if deadline is None else max(0.0, deadline - self.clock()))
        except (OSError, subprocess.SubprocessError):
            pass

    def stop(self, deadline=None):
        for process in reversed(self.processes):
            self._stop_process(process, deadline)
        self.processes = []
        for args in (["ip", "addr", "del", f"{AP_ADDRESS}/24", "dev", INTERFACE], ["nmcli", "device", "set", INTERFACE, "managed", "yes"]):
            try:
                self.run(args, deadline)
            except (OSError, subprocess.SubprocessError):
                pass
        try:
            os.remove(HOSTAPD_CONFIG)
        except FileNotFoundError:
            pass
