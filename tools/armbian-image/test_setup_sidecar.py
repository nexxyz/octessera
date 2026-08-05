#!/usr/bin/env python3
import importlib.util
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SIDECARS = (
    ROOT / "userpatches/overlay/usr/local/sbin/octessera-setup-sidecar",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar",
)
PUBLIC_KEY = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def valid_payload(mode="none"):
    return {
        "sshMode": mode,
        "sshPublicKey": PUBLIC_KEY if mode == "key" else "",
        "sshPassword": "long-enough-password" if mode == "password" else "",
        "sshPasswordConfirm": "long-enough-password" if mode == "password" else "",
        "hostname": "octessera-box",
        "wifiCountry": "US",
    }


for index, path in enumerate(SIDECARS):
    sidecar = load(path, f"sidecar_{index}")
    assert sidecar.valid_hostname("")
    assert sidecar.valid_hostname("octessera-box")
    assert not sidecar.valid_hostname("-bad")
    assert not sidecar.valid_hostname("a" * 64)
    assert sidecar.valid_country("")
    assert sidecar.valid_country("US")
    assert not sidecar.valid_country("usa")
    assert sidecar.valid_public_key(PUBLIC_KEY)
    assert not sidecar.valid_public_key(PUBLIC_KEY + "\nsecond")
    assert not sidecar.valid_public_key("ssh-ed25519 not-base64")
    assert sidecar.valid_password("long-enough-pass")
    assert not sidecar.valid_password("line\nbreak-injection")
    assert sidecar.validate_stage(valid_payload())["sshMode"] == "none"
    assert sidecar.validate_stage(valid_payload("key"))["sshMode"] == "key"
    assert sidecar.validate_stage(valid_payload("password"))["sshMode"] == "password"
    for unknown in ("country", "password", "sshKey", "extra"):
        payload = valid_payload()
        payload[unknown] = "unexpected"
        try:
            sidecar.validate_stage(payload)
        except ValueError:
            pass
        else:
            raise AssertionError(f"unknown field accepted: {unknown}")
    try:
        sidecar.validate_stage({"sshMode": "password", "sshPublicKey": "", "sshPassword": "short", "sshPasswordConfirm": "short", "hostname": "", "wifiCountry": "US"})
    except ValueError:
        pass
    else:
        raise AssertionError("short password accepted")
    sidecar.staged.clear()
    try:
        sidecar.finalize()
    except ValueError:
        pass
    else:
        raise AssertionError("finalize accepted missing staged payload")
    source = path.read_text(encoding="utf-8")
    assert "os.environ" not in source
    assert "ThreadingHTTPServer" not in source
    assert "sudoers" not in source
    assert "setup-force" not in source

print("Setup sidecar validation tests passed")
