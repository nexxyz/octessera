#!/usr/bin/env python3
import hashlib
import io
import re
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CLONE = ROOT / ".slim/clonedeps/repos/balena-os__wifi-connect"
PATCH = ROOT / "third_party/wifi-connect-4.11.84/portal-address-readiness.patch"
LICENSE = ROOT / "third_party/wifi-connect-4.11.84/LICENSE"
BUILD = ROOT / "tools/wifi-connect/build-patched.sh"
COMMIT = "5bd4c1bea548fb5714bedb18bbd12f088d5fa407"
PATCH_SHA256 = "c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6"
NETWORK_LIST_PATCH = ROOT / "third_party/wifi-connect-4.11.84/network-list-strength.patch"
NETWORK_LIST_PATCH_SHA256 = "ebfff3dce001860d0b53519be48fccf836e62dc1f9f7a3ce31151e6735947d69"


def git(*args):
    result = subprocess.run(["git", "-C", str(CLONE), *args], capture_output=True, text=True, check=False)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


assert git("rev-parse", "HEAD") == COMMIT
assert git("remote", "get-url", "origin") == "https://github.com/balena-os/wifi-connect.git"
before = git("status", "--short")
assert before == ""

patch_text = PATCH.read_text(encoding="utf-8")
patch_sha256 = hashlib.sha256(PATCH.read_bytes()).hexdigest()
assert patch_sha256 == PATCH_SHA256
changed = {match.group(1) for match in re.finditer(r"^diff --git a/(.+) b/", patch_text, re.MULTILINE)}
assert changed == {"src/errors.rs", "src/network.rs", "src/server.rs"}
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
    "NoStoreMiddleware",
    "CacheDirective::NoStore",
    "fn after(&self, _req: &mut Request, mut response: Response)",
    "chain.link_after(NoStoreMiddleware)",
    "Deactivated",
    "Unknown",
    "=> 25",
    "=> 26",
):
    assert required in patch_text, required
assert "UUID" not in patch_text

network_list_patch_text = NETWORK_LIST_PATCH.read_text(encoding="utf-8")
assert hashlib.sha256(NETWORK_LIST_PATCH.read_bytes()).hexdigest() == NETWORK_LIST_PATCH_SHA256
assert set(re.findall(r"^diff --git a/(.+) b/", network_list_patch_text, re.MULTILINE)) == {"src/network.rs"}
added = [line[1:].strip() for line in network_list_patch_text.splitlines() if line.startswith("+") and not line.startswith("+++")]
assert added == [
    "// Modified by Octessera: report NetworkManager's 0-100 access point strength.",
    "strength: u32,",
    "strength: access_point.strength,",
]
assert not any(line.startswith("-") and not line.startswith("---") for line in network_list_patch_text.splitlines())

for args in (("apply", "--check", str(PATCH)), ("apply", "--check", "--unidiff-zero", str(PATCH))):
    result = subprocess.run(["git", "-C", str(CLONE), *args], capture_output=True, text=True, check=False)
    assert result.returncode == 0, result.stderr
assert git("status", "--short") == before

with tempfile.TemporaryDirectory(prefix="octessera-wifi-connect-patch-") as temporary:
    source_root = Path(temporary)
    archive = subprocess.run(["git", "-C", str(CLONE), "archive", "HEAD"], capture_output=True, check=True).stdout
    with tarfile.open(fileobj=io.BytesIO(archive)) as archive_file:
        if sys.version_info >= (3, 12):
            archive_file.extractall(source_root, filter="data")
        else:
            archive_file.extractall(source_root)
    for patch in (PATCH, NETWORK_LIST_PATCH):
        result = subprocess.run(["git", "-C", str(source_root), "apply", str(patch)], capture_output=True, text=True, check=False)
        assert result.returncode == 0, result.stderr
    patched_server = (source_root / "src/server.rs").read_text(encoding="utf-8")
    assert patched_server.count("struct NoStoreMiddleware;") == 1
    assert patched_server.count("chain.link_after(NoStoreMiddleware);") == 1
    assert patched_server.count("CacheDirective::NoStore") == 1
    assert "if response.status == Some(status::Ok)" in patched_server
    patched_network = (source_root / "src/network.rs").read_text(encoding="utf-8")
    assert "    security: String,\n    // Modified by Octessera: report NetworkManager's 0-100 access point strength.\n    strength: u32,\n}" in patched_network
    assert "        strength: access_point.strength,\n    }" in patched_network
    assert patched_network.count("access_points.retain(|ap| inserted.insert(ap.ssid.clone()));") == 1

upstream_license = (CLONE / "LICENSE").read_bytes()
assert LICENSE.read_bytes() == upstream_license
assert hashlib.sha256(LICENSE.read_bytes()).hexdigest() == hashlib.sha256(upstream_license).hexdigest()

build_text = BUILD.read_text(encoding="utf-8")
for required in (
    "https://github.com/balena-os/wifi-connect.git",
    "5bd4c1bea548fb5714bedb18bbd12f088d5fa407",
    "rust:1.76.0-bookworm",
    "-e RUSTUP_TOOLCHAIN=1.76.0",
    "git clone --filter=blob:none --depth=1 --no-checkout",
    "git -C \"$clone_root\" fetch --depth=1 origin \"$upstream_commit\"",
    "git -C \"$clone_root\" status --porcelain",
    "docker run --rm",
    "git -C \"$source_root\" apply --check",
    "cargo metadata --locked --format-version 1",
    "cargo test --locked",
    "cargo build --locked --release --target",
    "aarch64-linux-gnu-readelf",
    "wifi-connect.metadata.json",
    "portal_activation_exit_code",
    "portal_activation_requirement",
    "network_manager_commit",
    "src/server.rs",
    PATCH_SHA256,
    NETWORK_LIST_PATCH_SHA256,
    "network-list-strength.patch",
    'git -C "$source_root" apply --check "$network_list_patch_path"',
    "grep -qF 'access_points.sort_by_key(|ap| ap.strength);' \"${network_manager_checkouts[0]}/src/wifi.rs\"",
    "grep -qF 'access_points.reverse();' \"${network_manager_checkouts[0]}/src/wifi.rs\"",
    "network_list_patch_sha256",
    "portal_network_list",
):
    assert required in build_text, required
assert '(\n  cd "$source_root"\n  cargo build --locked --release --target "$target"\n)' in build_text
target_build_lines = [
    line.strip()
    for line in build_text.splitlines()
    if line.strip().startswith("cargo build") and '--target "$target"' in line
]
assert target_build_lines == ['cargo build --locked --release --target "$target"']

lock_text = (CLONE / "Cargo.lock").read_text(encoding="utf-8")
assert "network-manager.git#4da2e6a57de16b6ae911f74321f929d78af8b1ba" in lock_text

print("Pinned wifi-connect source, patch scope, license, dependency pin, and builder-shape checks passed")
print(f"Verified patched source: {patch_sha256} ({PATCH.stat().st_size} bytes)")
