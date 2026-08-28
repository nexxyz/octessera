#!/usr/bin/env bash
set -euo pipefail

target="${WIFI_CONNECT_TARGET:?WIFI_CONNECT_TARGET is required}"
container_image="${WIFI_CONNECT_CONTAINER_IMAGE:?WIFI_CONNECT_CONTAINER_IMAGE is required}"
output_root=/work/target/wifi-connect-patched
source_root=/tmp/wifi-connect-patched
clone_root=/work/.slim/clonedeps/repos/balena-os__wifi-connect
patch_path=/work/third_party/wifi-connect-4.11.84/portal-address-readiness.patch

rm -rf "$output_root"
rm -rf "$source_root"
mkdir -p "$output_root"
mkdir -p "$source_root"
git config --global --add safe.directory "$clone_root"
git -C "$clone_root" archive --format=tar HEAD | tar -xf - -C "$source_root"
cp -a "$clone_root/.git" "$source_root/"
git config --global --add safe.directory "$source_root"
git -C "$source_root" apply --check "$patch_path"
git -C "$source_root" apply "$patch_path"
changed_files="$(git -C "$source_root" diff --name-only HEAD)"
test "$changed_files" = "src/errors.rs
src/network.rs
src/server.rs"

patch_sha256="$(sha256sum "$patch_path" | cut -d ' ' -f 1)"
test "$patch_sha256" = c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6

dpkg --add-architecture arm64
apt-get update
apt-get install -y --no-install-recommends \
  binutils-aarch64-linux-gnu \
  ca-certificates \
  file \
  gcc-aarch64-linux-gnu \
  git \
  libc6-dev-arm64-cross \
  libdbus-1-dev \
  libdbus-1-dev:arm64 \
  pkg-config \
  python3
rm -rf /var/lib/apt/lists/*

cargo metadata --locked --format-version 1 --manifest-path "$source_root/Cargo.toml" > "$output_root/cargo-metadata.json"
cargo test --locked --manifest-path "$source_root/Cargo.toml"
rustup target add "$target"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
export PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
cargo build --locked --release --target "$target" --manifest-path "$source_root/Cargo.toml"

binary_source="$source_root/target/$target/release/wifi-connect"
test -f "$binary_source"
aarch64-linux-gnu-strip "$binary_source"
cp "$binary_source" "$output_root/wifi-connect"
file "$output_root/wifi-connect" | grep -Eq 'ELF 64-bit.*ARM aarch64'
aarch64-linux-gnu-readelf -h "$output_root/wifi-connect" | grep -Eq '^[[:space:]]*Class:[[:space:]]*ELF64[[:space:]]*$'
aarch64-linux-gnu-readelf -h "$output_root/wifi-connect" | grep -Eq '^[[:space:]]*Machine:[[:space:]]*AArch64[[:space:]]*$'

rm -rf "$source_root/target"
mkdir -p "$output_root/source"
cp -a "$source_root/." "$output_root/source/"
rm -rf "$output_root/source/.git"
cp "$output_root/cargo-metadata.json" "$output_root/source/cargo-metadata.json"

binary_sha256="$(sha256sum "$output_root/wifi-connect" | cut -d ' ' -f 1)"
test "$binary_sha256" = 4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627
rustc_version="$(rustc --version)"
cargo_version="$(cargo --version)"
python3 - "$output_root/wifi-connect.metadata.json" "$patch_sha256" "$binary_sha256" "$rustc_version" "$cargo_version" "$target" "$container_image" <<'PY'
import json
import sys

path, patch_sha256, binary_sha256, rustc_version, cargo_version, target, container_image = sys.argv[1:]
with open(path, "w", encoding="utf-8") as handle:
    json.dump({
        "artifact": "wifi-connect",
        "network_manager_commit": "4da2e6a57de16b6ae911f74321f929d78af8b1ba",
        "upstream_ref": "v4.11.84",
        "upstream_commit": "5bd4c1bea548fb5714bedb18bbd12f088d5fa407",
        "patch_sha256": patch_sha256,
        "portal_activation_exit_code": 26,
        "portal_activation_requirement": "ConnectionState::Activated before gateway:port readiness wait",
        "portal_address_readiness_timeout_seconds": 10,
        "target": target,
        "container": container_image,
        "rustc": rustc_version,
        "cargo": cargo_version,
        "binary_sha256": binary_sha256,
    }, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
python3 - "$output_root/wifi-connect.metadata.json" "$output_root/wifi-connect" <<'PY'
import hashlib
import json
import sys

metadata_path, binary_path = sys.argv[1:]
with open(metadata_path, encoding="utf-8") as handle:
    metadata = json.load(handle)
with open(binary_path, "rb") as handle:
    actual = hashlib.sha256(handle.read()).hexdigest()
assert metadata["binary_sha256"] == actual
PY
