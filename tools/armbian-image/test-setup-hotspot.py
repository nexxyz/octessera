#!/usr/bin/env python3
import importlib.util
import os
import subprocess
import sys
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
LIBRARIES = (
    ROOT / "userpatches/overlay/usr/local/lib/octessera",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera",
)
SCAN = "\n".join((
    "hermetes4:WPA2:70",
    "hermetes4:WPA2:92",
    r"Kitchen\:Guest:WPA1 WPA2:55",
    r"back\\slash::40",
    "Cafe:--:40",
    ":WPA2:99",
    "Office:WPA2 802.1X:61",
    "Old:WEP:20",
    "Broken:WPA2:strong",
    "Too:many:fields:1",
    "Mesh:WPA3:61",
    "",
))


def load(directory):
    modules = {}
    for library in ("setup_config", "setup_hotspot", "setup_wifi_join"):
        spec = importlib.util.spec_from_loader(library, SourceFileLoader(library, str(directory / f"{library}.py")))
        assert spec is not None and spec.loader is not None
        sys.modules[library] = modules[library] = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(modules[library])
    return modules["setup_hotspot"], modules["setup_wifi_join"]


for directory in LIBRARIES:
    hotspot, join = load(directory)
    assert hotspot.parse_networks(SCAN) == [
        {"ssid": "hermetes4", "security": "wpa", "strength": 92},
        {"ssid": "Mesh", "security": "wpa", "strength": 61},
        {"ssid": "Kitchen:Guest", "security": "wpa", "strength": 55},
        {"ssid": "Cafe", "security": "none", "strength": 40},
        {"ssid": "back\\slash", "security": "none", "strength": 40},
    ]
    assert hotspot.parse_networks("") == [] and hotspot.parse_networks(None) == []

    raspberry = hotspot.hostapd_config("Octessera Setup abcd", "k7Mx-9pQa", True).splitlines()
    assert raspberry[8:12] == ["wpa=2", "wpa_key_mgmt=WPA-PSK WPA-PSK-SHA256", "ieee80211w=2", "rsn_pairwise=CCMP"]
    config = hotspot.hostapd_config("Octessera Setup abcd", "k7Mx-9pQa", False)
    assert config.splitlines() == ["interface=wlan0", "driver=nl80211", "ssid=Octessera Setup abcd", "hw_mode=g", "channel=6", "ieee80211n=1", "wmm_enabled=1", "auth_algs=1", "wpa=2", "wpa_key_mgmt=WPA-PSK", "rsn_pairwise=CCMP", "wpa_passphrase=k7Mx-9pQa"]
    assert "country_code" not in config and "ieee80211w" not in config and "wps" not in config

    keyfile = join.keyfile("1234", "My Net\\", "pa ss\\word", False)
    assert "id=My\\sNet\\\\\n" in keyfile and "uuid=1234\n" in keyfile and "interface-name=wlan0\n" in keyfile
    assert "ssid=77;121;32;78;101;116;92;\n" in keyfile
    assert "[wifi-security]\nkey-mgmt=wpa-psk\npsk=pa\\sss\\\\word\n" in keyfile
    assert "[ipv4]\nmethod=auto\n" in keyfile and "[ipv6]\nmethod=auto\n" in keyfile
    assert "wifi-security" not in join.keyfile("1234", "Cafe", "", False)
    assert "hidden" not in keyfile
    hidden = join.keyfile("1234", "Attic", "eight888", True)
    assert "[wifi]\nmode=infrastructure\nssid=65;116;116;105;99;\nhidden=true\n\n[wifi-security]\nkey-mgmt=wpa-psk\npsk=eight888\n" in hidden
    assert "ssid=65;116;116;105;99;\nhidden=true\n\n[ipv4]" in join.keyfile("1234", "Attic", "", True)
    assert join.profile_path("1234") == "/etc/NetworkManager/system-connections/octessera-setup-1234.nmconnection"

    outputs = {
        ("nmcli", "-t", "-f", "UUID,TYPE", "connection", "show"): "a:802-11-wireless\nb:802-11-wireless\nc:802-3-ethernet\nd:802-11-wireless\n",
        ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "a"): "infrastructure\n",
        ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "a"): "Kitchen\\:Guest\n",
        ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "b"): "ap\n",
        ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "b"): "Kitchen\\:Guest\n",
        ("nmcli", "-g", "802-11-wireless.mode", "connection", "show", "uuid", "d"): "\n",
        ("nmcli", "-g", "802-11-wireless.ssid", "connection", "show", "uuid", "d"): "Kitchen\\:Guest\n",
    }
    run = lambda args: SimpleNamespace(returncode=0, stdout=outputs.get(tuple(args), ""))
    assert join.profiles_for_ssid(run, "Kitchen:Guest") == ["a", "d"]
    assert join.profiles_for_ssid(run, "Kitchen") == []
    assert join.profiles_for_ssid(lambda _args: SimpleNamespace(returncode=1, stdout=""), "Kitchen:Guest") == []

    marker = Path(tempfile.mkdtemp()) / "joining"
    join.JOINING_PATH = str(marker)
    written = []
    join.setup_config._write_atomic = lambda path, content, mode: written.append((path, mode)) or (marker.write_text(content) if path == str(marker) else None)
    calls = []
    ok = lambda args: calls.append(tuple(args)) or SimpleNamespace(returncode=0, stdout="")
    assert join.join(ok, "1234", "Home", "eight888", False, 42)
    assert written == [(str(marker), 0o600), (join.profile_path("1234"), 0o600)]
    assert calls == [("nmcli", "connection", "load", join.profile_path("1234")), ("nmcli", "--wait", "42", "connection", "up", "uuid", "1234")]
    calls.clear()
    assert not join.join(lambda args: calls.append(tuple(args)) or SimpleNamespace(returncode=1, stdout=""), "1234", "Home", "eight888", False, 42)
    assert calls == [("nmcli", "connection", "load", join.profile_path("1234"))]
    join.keep()
    assert not marker.exists()
    calls.clear()
    join.forget(ok)
    assert calls == []
    uuid = "0f1e2d3c-4b5a-6978-8a9b-acbdcedf0011"
    removed = []
    original_remove = os.remove
    join.os.remove = lambda path: removed.append(path) or (marker.unlink() if path == str(marker) else None)

    def refuse(args):
        raise subprocess.TimeoutExpired(args, 0)

    marker.write_text(uuid + "\n")
    try:
        join.forget(refuse)
    except subprocess.TimeoutExpired:
        pass
    assert removed == [join.profile_path(uuid), str(marker)] and not marker.exists()
    removed.clear()
    marker.write_text(uuid + "\n")
    join.forget(ok)
    assert calls == [("nmcli", "connection", "delete", "uuid", uuid)] and removed == [join.profile_path(uuid), str(marker)]
    removed.clear()
    marker.write_text("../../etc/passwd\n")
    join.forget(ok)
    assert removed == [str(marker)] and len(calls) == 1
    join.os.remove = original_remove
    marker.parent.rmdir()

print("Setup hotspot scan parsing, hostapd config, Wi-Fi profile keyfile, same-SSID lookup, and join tests passed")
