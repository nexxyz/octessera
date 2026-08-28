#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
upstream_origin=https://github.com/balena-os/wifi-connect.git
upstream_commit=5bd4c1bea548fb5714bedb18bbd12f088d5fa407
clone_root="$repository_root/.slim/clonedeps/repos/balena-os__wifi-connect"
artifact_root="$repository_root/target/wifi-connect-patched"
patch_path="$repository_root/third_party/wifi-connect-4.11.84/portal-address-readiness.patch"
container_image=rust:1.76.0-bookworm@sha256:d36f9d8a9a4c76da74c8d983d0d4cb146dd2d19bb9bd60b704cdcf70ef868d3a
expected_binary_sha256=4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627
expected_patch_sha256=c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6

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
[[ -f "$patch_path" && ! -L "$patch_path" ]] || { echo "wifi-connect patch is missing or symlinked." >&2; exit 1; }
echo "$expected_patch_sha256  $patch_path" | sha256sum -c -
command -v docker >/dev/null 2>&1 || { echo "Docker is required for the pinned wifi-connect builder." >&2; exit 1; }
mkdir -p "$repository_root/target"

docker run --rm \
  -e WIFI_CONNECT_TARGET=aarch64-unknown-linux-gnu \
  -e WIFI_CONNECT_CONTAINER_IMAGE="$container_image" \
  -v "$repository_root:/work" \
  -v octessera-wifi-connect-cargo-registry:/usr/local/cargo/registry \
  -v octessera-wifi-connect-cargo-git:/usr/local/cargo/git \
  -v octessera-wifi-connect-rustup:/usr/local/rustup \
  -w /work \
  "$container_image" \
  bash /work/tools/wifi-connect/build-patched.sh

for artifact in wifi-connect wifi-connect.metadata.json cargo-metadata.json; do
  [[ -f "$artifact_root/$artifact" && ! -L "$artifact_root/$artifact" ]] || { echo "Patched wifi-connect build output is missing: $artifact" >&2; exit 1; }
done
echo "$expected_binary_sha256  $artifact_root/wifi-connect" | sha256sum -c -
python3 - "$artifact_root/wifi-connect.metadata.json" "$artifact_root/cargo-metadata.json" "$expected_binary_sha256" "$expected_patch_sha256" <<'PY'
import json
import sys

metadata = json.loads(open(sys.argv[1], encoding="utf-8").read())
json.load(open(sys.argv[2], encoding="utf-8"))
assert metadata["binary_sha256"] == sys.argv[3]
assert metadata["patch_sha256"] == sys.argv[4]
assert metadata["target"] == "aarch64-unknown-linux-gnu"
PY
