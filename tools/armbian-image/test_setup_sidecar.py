#!/usr/bin/env python3
import importlib.util
from importlib.machinery import SourceFileLoader
from pathlib import Path

root = Path(__file__).resolve().parents[2]
path = root / "userpatches" / "overlay" / "usr" / "local" / "sbin" / "octessera-setup-sidecar"
spec = importlib.util.spec_from_loader("sidecar", SourceFileLoader("sidecar", str(path)))
assert spec is not None
assert spec.loader is not None
sidecar = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sidecar)

assert sidecar.valid_hostname("")
assert sidecar.valid_hostname("octessera-box")
assert not sidecar.valid_hostname("-bad")
assert sidecar.valid_country("")
assert sidecar.valid_country("US")
assert not sidecar.valid_country("usa")
assert sidecar.valid_public_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeKey test")
assert not sidecar.valid_public_key("-----BEGIN OPENSSH PRIVATE KEY-----")
assert sidecar.valid_password("long-enough-pass")
assert not sidecar.valid_password("line\nbreak-injection")
assert sidecar.validate_stage({"sshMode": "none"})["sshMode"] == "none"
assert sidecar.validate_stage({"sshMode": "key", "sshPublicKey": "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeKey test"})["sshMode"] == "key"
assert sidecar.validate_stage({"sshMode": "password", "sshPassword": "long-enough-pass", "sshPasswordConfirm": "long-enough-pass"})["sshMode"] == "password"
try:
    sidecar.validate_stage({"sshMode": "password", "password": "short", "passwordConfirm": "short"})
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
