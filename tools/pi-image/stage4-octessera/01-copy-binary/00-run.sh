#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"

version="${OCTESSERA_RELEASE_VERSION:-}"
tag="${OCTESSERA_RELEASE_TAG:-}"
board_profile="${OCTESSERA_BOARD_PROFILE_ID:-raspberry-pi-zero-2w}"
if [ "$board_profile" = orange-pi-zero-2w ]; then
    echo "Orange Pi profile is not supported by the Raspberry Pi image pipeline; use the separate Armbian workflow." >&2
    exit 2
fi
if [ "$board_profile" != raspberry-pi-zero-2w ]; then
    echo "Raspberry Pi image pipeline accepts only raspberry-pi-zero-2w; got $board_profile." >&2
    exit 2
fi

install -d -m 0755 "$ROOTFS_DIR/etc/octessera"
printf 'OCTESSERA_BOARD_PROFILE_ID=%s\n' "$board_profile" > "$ROOTFS_DIR/etc/octessera/board-profile.env"
cat > "$ROOTFS_DIR/etc/octessera/board-profile.json" <<EOF
{
  "schema_version": 1,
  "board_profile": "$board_profile",
  "binary": "octessera-pi",
  "arch": "aarch64-unknown-linux-gnu"
}
EOF

if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$tag" == "v$version" ]]; then
    release_dir="/opt/octessera/releases/$version"
    install -D -m 0755 \
        "$STAGE_FILES/root/usr/local/bin/octessera-pi" \
        "$ROOTFS_DIR$release_dir/octessera-pi"
    cat > "$ROOTFS_DIR$release_dir/update-manifest.json" <<EOF
{
  "schema_version": 2,
  "updater_protocol": 2,
  "candidate_health_protocol": 1,
  "tag": "$tag",
  "version": "$version",
  "board_profile": "$board_profile",
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
  "schema_version": 2,
  "phase": "committed",
  "current": "$version",
  "previous": null,
  "next": null,
  "updated_at": "1970-01-01T00:00:00Z",
  "release": {
    "schema_version": 2,
    "updater_protocol": 2,
    "candidate_health_protocol": 1,
    "tag": "$tag",
    "version": "$version",
    "board_profile": "$board_profile",
    "arch": "aarch64-unknown-linux-gnu",
    "binary": "octessera-pi",
    "platforms": ["raspberry-pi-zero-2w", "linux-aarch64-device"]
  },
  "asset": null
}
EOF
else
    echo "Raspberry Pi image setup requires a semver release version and matching tag." >&2
    exit 2
fi
