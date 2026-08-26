#!/usr/bin/env python3
import hashlib
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CLONE = ROOT / ".slim/clonedeps/repos/balena-os__wifi-connect"
PATCH = ROOT / "third_party/wifi-connect-4.11.84/portal-address-readiness.patch"
LICENSE = ROOT / "third_party/wifi-connect-4.11.84/LICENSE"
BUILD = ROOT / "tools/wifi-connect/build-patched.ps1"
BUILD_CONTAINER = ROOT / "tools/wifi-connect/build-patched.sh"
BUILD_CI = ROOT / "tools/wifi-connect/build-patched-ci.sh"
README = ROOT / "third_party/wifi-connect-4.11.84/README.md"
COMMIT = "5bd4c1bea548fb5714bedb18bbd12f088d5fa407"
PATCH_SHA256 = "3481ef27637c5c4a176b59f74af4e2c232f6c67de8399eaf705fe6431ffc8939"
BINARY_SHA256 = "929a5b937a771a0e4f96446242af217c61118aedaaaa053aff75af61151c6acc"


def git(*args):
    result = subprocess.run(["git", "-C", str(CLONE), *args], capture_output=True, text=True, check=False)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


assert git("rev-parse", "HEAD") == COMMIT
assert git("remote", "get-url", "origin") == "https://github.com/balena-os/wifi-connect.git"
before = git("status", "--short")
assert before == ""

patch_text = PATCH.read_text(encoding="utf-8")
assert hashlib.sha256(PATCH.read_bytes()).hexdigest() == PATCH_SHA256
changed = {match.group(1) for match in re.finditer(r"^diff --git a/(.+) b/", patch_text, re.MULTILINE)}
assert changed == {"src/errors.rs", "src/network.rs"}
for required in (
    "Modified by Octessera: wait for the configured portal address before starting dnsmasq and HTTP.",
    "SocketAddrV4",
    "TcpListener::bind",
    "AddrNotAvailable",
    "AddrInUse",
    "Duration::from_secs(10)",
    "Duration::from_millis(100)",
    "ConnectionState::Activated",
    ".activate()",
    "ensure_portal_activated",
    "PortalAddressReadinessTimeout",
    "PortalActivation",
    "Deactivated",
    "Unknown",
    "=> 25",
    "=> 26",
):
    assert required in patch_text, required
assert "UUID" not in patch_text

for args in (("apply", "--check", str(PATCH)), ("apply", "--check", "--unidiff-zero", str(PATCH))):
    result = subprocess.run(["git", "-C", str(CLONE), *args], capture_output=True, text=True, check=False)
    assert result.returncode == 0, result.stderr
assert git("status", "--short") == before

upstream_license = (CLONE / "LICENSE").read_bytes()
assert LICENSE.read_bytes() == upstream_license
assert hashlib.sha256(LICENSE.read_bytes()).hexdigest() == hashlib.sha256(upstream_license).hexdigest()

build_text = BUILD.read_text(encoding="utf-8")
container_text = BUILD_CONTAINER.read_text(encoding="utf-8")
ci_text = BUILD_CI.read_text(encoding="utf-8")
readme_text = README.read_text(encoding="utf-8")
assert "target/wifi-connect-patched/cargo-metadata.json" in readme_text
assert "target/wifi-connect-patched/source/cargo-metadata.json" not in readme_text
for required in (
    "5bd4c1bea548fb5714bedb18bbd12f088d5fa407",
    "build-patched.sh",
    "rust:1.76.0-bookworm",
):
    assert required in build_text, required
for required in (
    "git -C \"$source_root\" apply --check",
    "cargo metadata --locked --format-version 1",
    "cargo test --locked",
    "cargo build --locked --release --target",
    "aarch64-linux-gnu-readelf",
    "wifi-connect.metadata.json",
    "portal_activation_exit_code",
    "portal_activation_requirement",
    "network_manager_commit",
    PATCH_SHA256,
    BINARY_SHA256,
):
    assert required in container_text, required
for required in (
    "https://github.com/balena-os/wifi-connect.git",
    "5bd4c1bea548fb5714bedb18bbd12f088d5fa407",
    "git clone --filter=blob:none --depth=1 --no-checkout",
    "git -C \"$clone_root\" fetch --depth=1 origin \"$upstream_commit\"",
    "git -C \"$clone_root\" status --porcelain",
    "docker run --rm",
    "tools/wifi-connect/build-patched.sh",
    "929a5b937a771a0e4f96446242af217c61118aedaaaa053aff75af61151c6acc",
    "3481ef27637c5c4a176b59f74af4e2c232f6c67de8399eaf705fe6431ffc8939",
):
    assert required in ci_text, required

lock_text = (CLONE / "Cargo.lock").read_text(encoding="utf-8")
assert "network-manager.git#4da2e6a57de16b6ae911f74321f929d78af8b1ba" in lock_text

print("Pinned wifi-connect source, patch scope, license, dependency pin, and builder-shape checks passed")
