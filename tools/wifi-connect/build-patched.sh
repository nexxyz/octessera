#!/usr/bin/env bash
set -euo pipefail

target=aarch64-unknown-linux-gnu
container_image=rust:1.76.0-bookworm@sha256:d36f9d8a9a4c76da74c8d983d0d4cb146dd2d19bb9bd60b704cdcf70ef868d3a
upstream_origin=https://github.com/balena-os/wifi-connect.git
upstream_commit=5bd4c1bea548fb5714bedb18bbd12f088d5fa407
patch_sha256=c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6

if [[ "${1:-}" != --in-container ]]; then
  repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  clone_root="$repository_root/.slim/clonedeps/repos/balena-os__wifi-connect"
  artifact_root="$repository_root/target/wifi-connect-patched"
  if [[ -e "$clone_root" ]]; then
    [[ -d "$clone_root" && ! -L "$clone_root" ]] || { echo "wifi-connect clone path is not a real directory: $clone_root" >&2; exit 1; }
  else
    mkdir -p "$(dirname "$clone_root")"
    git clone --filter=blob:none --depth=1 --no-checkout "$upstream_origin" "$clone_root"
    git -C "$clone_root" fetch --depth=1 origin "$upstream_commit"
    git -C "$clone_root" checkout --detach "$upstream_commit"
  fi
  [[ "$(git -C "$clone_root" remote get-url origin)" == "$upstream_origin" ]] || { echo "wifi-connect clone origin is not pinned." >&2; exit 1; }
  [[ "$(git -C "$clone_root" rev-parse HEAD)" == "$upstream_commit" ]] || { echo "wifi-connect clone HEAD is not pinned." >&2; exit 1; }
  [[ -z "$(git -C "$clone_root" status --porcelain)" ]] || { echo "wifi-connect source clone is dirty; refusing to use it." >&2; exit 1; }
  command -v docker >/dev/null 2>&1 || { echo "Docker is required for the pinned wifi-connect builder." >&2; exit 1; }
  mkdir -p "$repository_root/target"
  docker run --rm \
    -v "$repository_root:/work" \
    -v octessera-wifi-connect-cargo-registry:/usr/local/cargo/registry \
    -v octessera-wifi-connect-cargo-git:/usr/local/cargo/git \
    -v octessera-wifi-connect-rustup:/usr/local/rustup \
    -w /work \
    "$container_image" \
    bash /work/tools/wifi-connect/build-patched.sh --in-container
  for artifact in wifi-connect wifi-connect.metadata.json cargo-metadata.json; do
    [[ -f "$artifact_root/$artifact" && ! -L "$artifact_root/$artifact" ]] || { echo "Patched wifi-connect build output is missing: $artifact" >&2; exit 1; }
  done
  exit 0
fi

output_root=/work/target/wifi-connect-patched
source_root=/tmp/wifi-connect-patched
clone_root=/work/.slim/clonedeps/repos/balena-os__wifi-connect
patch_path=/work/third_party/wifi-connect-4.11.84/portal-address-readiness.patch

rm -rf "$output_root" "$source_root"
mkdir -p "$output_root" "$source_root"
echo "$patch_sha256  $patch_path" | sha256sum -c -
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
(
  cd "$source_root"
  cargo build --locked --release --target "$target"
)

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
rustc_version="$(rustc --version)"
cargo_version="$(cargo --version)"
python3 - "$output_root/wifi-connect.metadata.json" "$patch_sha256" "$binary_sha256" "$rustc_version" "$cargo_version" "$target" "$container_image" "$upstream_commit" <<'PY'
import json
import sys

path, patch_sha256, binary_sha256, rustc_version, cargo_version, target, container_image, upstream_commit = sys.argv[1:]
with open(path, "w", encoding="utf-8") as handle:
    json.dump({
        "artifact": "wifi-connect",
        "network_manager_commit": "4da2e6a57de16b6ae911f74321f929d78af8b1ba",
        "upstream_ref": "v4.11.84",
        "upstream_commit": upstream_commit,
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
