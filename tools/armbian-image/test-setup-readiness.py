#!/usr/bin/env python3
import importlib.util
import ipaddress
import json
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
COORDINATORS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup",
)


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


empty = [{"addr_info": []}]
portal = [{"addr_info": [{"family": "inet", "local": "192.168.42.1", "prefixlen": 24, "scope": "global", "flags": ["permanent"]}]}]
global_ipv4 = [{"addr_info": [{"family": "inet", "local": "192.168.1.42", "scope": "global", "flags": ["permanent"]}]}]


for index, path in enumerate(COORDINATORS):
    coordinator, http_module = load(path, f"readiness_coordinator_{index}")
    assert not coordinator._portal_ipv4(json.dumps(empty))
    assert not coordinator._portal_ipv4(json.dumps([{**portal[0], "addr_info": [{**portal[0]["addr_info"][0], "prefixlen": 25}]}]))
    assert not coordinator._portal_ipv4(json.dumps([{**portal[0], "addr_info": [{**portal[0]["addr_info"][0], "flags": ["tentative"]}]}]))
    assert coordinator._portal_ipv4(json.dumps(portal))
    assert not coordinator._global_ipv4(json.dumps(empty))
    assert not coordinator._global_ipv4(json.dumps([{ "addr_info": [{"family": "inet", "local": "10.0.0.5", "scope": "global", "flags": ["DADFAILED"]}]}]))
    assert coordinator._global_ipv4(json.dumps(global_ipv4))

    outputs = {
        ("ip", "-j", "-4", "addr", "show", "dev", "wlan0"): json.dumps(portal),
        ("ss", "-H", "-lunp", "sport", "=", ":67"): "dnsmasq 123\n",
    }
    command = lambda args: SimpleNamespace(returncode=0, stdout=outputs[tuple(args)])
    instance = coordinator.Coordinator({"status_group": "root", "request_owner": "root", "user": "pi"}, command=command)
    instance.http_ready = lambda: True
    assert instance.portal_ready()
    instance.command = lambda args: SimpleNamespace(returncode=0, stdout=json.dumps(global_ipv4))
    assert instance.global_ipv4_ready()
    source = path.read_text(encoding="utf-8")
    assert '"ip", "-j", "-4", "addr", "show", "dev", INTERFACE' in source
    assert '"ss", "-H", f"-l{protocol}np"' in source
    http_source = (path.parent.parent / "lib/octessera/setup_http.py").read_text(encoding="utf-8")
    assert 'HTTPConnection(setup_http.AP_ADDRESS, 80, timeout=2)' in source
    assert 'class SetupHandler' in http_source and 'class SetupHTTPServer' in http_source
    assert 'ipaddress.ip_network("192.168.42.0/24")' in http_source
    assert "tentative" in source and "dadfailed" in source
    assert "default-route" not in source and "resolv" not in source
    assert "retry" not in source.lower()

print("Setup exact AP readiness and usable wlan0 IPv4 tests passed")
