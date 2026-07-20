#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"

version="${OCTESSERA_RELEASE_VERSION:-}"
tag="${OCTESSERA_RELEASE_TAG:-}"

if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$tag" == "v$version" ]]; then
    release_dir="/opt/octessera/releases/$version"
    install -D -m 0755 \
        "$STAGE_FILES/root/usr/local/bin/octessera-pi" \
        "$ROOTFS_DIR$release_dir/octessera-pi"
    cat > "$ROOTFS_DIR$release_dir/update-manifest.json" <<EOF
{
  "schema_version": 1,
  "tag": "$tag",
  "version": "$version",
  "arch": "aarch64-unknown-linux-gnu",
  "binary": "octessera-pi",
  "platforms": ["raspberry-pi-zero-2w", "linux-aarch64-device"]
}
EOF
    install -d -m 0755 "$ROOTFS_DIR/opt/octessera" "$ROOTFS_DIR/usr/local/bin"
    ln -sfn "$release_dir" "$ROOTFS_DIR/opt/octessera/current"
    ln -sfn /opt/octessera/current/octessera-pi "$ROOTFS_DIR/usr/local/bin/octessera-pi"
    cat > "$ROOTFS_DIR/opt/octessera/update-state.json" <<EOF
{
  "current": "$version",
  "previous": null,
  "next": null,
  "updated_at": "1970-01-01T00:00:00Z",
  "release": {
    "tag": "$tag",
    "version": "$version",
    "arch": "aarch64-unknown-linux-gnu",
    "binary": "octessera-pi",
    "platforms": ["raspberry-pi-zero-2w", "linux-aarch64-device"]
  },
  "asset": null
}
EOF
else
    install -D -m 0755 \
        "$STAGE_FILES/root/usr/local/bin/octessera-pi" \
        "$ROOTFS_DIR/usr/local/bin/octessera-pi"
fi
