#!/bin/bash
set -euo pipefail

ZIP_PATH="${1:?usage: verify-sanitized-image.sh <image.zip>}"
WORK_DIR="$(mktemp -d)"
LOOP_DEV=""

require_path() {
    local path="$1"
    local label="$2"
    if [ ! -e "$path" ]; then
        echo "Sanitation check failed: missing $label at $path" >&2
        exit 1
    fi
}

require_executable() {
    local path="$1"
    local label="$2"
    if [ ! -x "$path" ]; then
        echo "Sanitation check failed: missing executable $label at $path" >&2
        exit 1
    fi
}

require_root_mode() {
    local path="$1"
    local mode="$2"
    local owner actual_mode
    owner="$(stat -c '%u' "$path")"
    actual_mode="$(stat -c '%a' "$path")"
    if [ "$owner" != 0 ] || [ "$actual_mode" != "$mode" ]; then
        echo "Sanitation check failed: unsafe updater ownership/mode at $path" >&2
        exit 1
    fi
}

require_boot_config_marker() {
    if grep -q 'octessera additions' "$WORK_DIR/boot/config.txt" 2>/dev/null; then
        return
    fi
    if grep -q 'octessera additions' "$WORK_DIR/root/boot/firmware/config.txt" 2>/dev/null; then
        return
    fi
    echo "Sanitation check failed: missing octessera boot config marker" >&2
    exit 1
}

require_boot_overlay() {
    if [ -f "$WORK_DIR/boot/overlays/i2s-dac-no20.dtbo" ]; then
        return
    fi
    if [ -f "$WORK_DIR/root/boot/firmware/overlays/i2s-dac-no20.dtbo" ]; then
        return
    fi
    echo "Sanitation check failed: missing i2s-dac-no20 boot overlay" >&2
    exit 1
}

require_raspberry_board_profile() {
    local profile_file="$WORK_DIR/root/etc/octessera/board-profile.env"
    local metadata_file="$WORK_DIR/root/etc/octessera/board-profile.json"
    if ! grep -qx 'OCTESSERA_BOARD_PROFILE_ID=raspberry-pi-zero-2w' "$profile_file"; then
        echo "Sanitation check failed: image board profile is not raspberry-pi-zero-2w" >&2
        exit 1
    fi
    if ! grep -q '"board_profile": "raspberry-pi-zero-2w"' "$metadata_file"; then
        echo "Sanitation check failed: image board metadata does not match raspberry-pi-zero-2w" >&2
        exit 1
    fi
}

require_updater_protocol() {
    for path in \
        "$WORK_DIR/root/usr/local/sbin/octessera-update" \
        "$WORK_DIR/root/usr/local/sbin/octessera-update-guard" \
        "$WORK_DIR/root/usr/local/sbin/octessera-update-recovery" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_protocol.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_state.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_assets.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_guard.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_cli.py" \
        "$WORK_DIR/root/etc/systemd/system/octessera-update-guard.service" \
        "$WORK_DIR/root/etc/systemd/system/octessera-update-recovery.service" \
        "$WORK_DIR/root/etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service"; do
        require_path "$path" "updater protocol path"
    done
    require_path "$WORK_DIR/root/etc/sudoers.d/octessera-update" "updater sudoers rule"
    require_root_mode "$WORK_DIR/root/etc/sudoers.d/octessera-update" 440
    require_root_mode "$WORK_DIR/root/usr/local/sbin/octessera-update" 755
    require_root_mode "$WORK_DIR/root/usr/local/sbin/octessera-update-guard" 755
    require_root_mode "$WORK_DIR/root/usr/local/sbin/octessera-update-recovery" 755
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_protocol.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_state.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_assets.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_guard.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_cli.py" 644
    if grep -Eq 'octessera-update-(guard|recovery)' "$WORK_DIR/root/etc/sudoers.d/octessera-update"; then
        echo "Sanitation check failed: updater internals are exposed through sudoers" >&2
        exit 1
    fi
    grep -qx 'ExecStart=/usr/local/bin/octessera-pi' "$WORK_DIR/root/etc/systemd/system/octessera.service" || {
        echo "Sanitation check failed: service uses a direct executable path" >&2
        exit 1
    }
    grep -qx 'Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json' "$WORK_DIR/root/etc/systemd/system/octessera.service" || {
        echo "Sanitation check failed: service has no candidate health path" >&2
        exit 1
    }
    grep -qx 'Requires=octessera-update-recovery.service' "$WORK_DIR/root/etc/systemd/system/octessera.service" || {
        echo "Sanitation check failed: runtime does not require recovery" >&2
        exit 1
    }
    grep -qx 'RemainAfterExit=yes' "$WORK_DIR/root/etc/systemd/system/octessera-update-recovery.service" || {
        echo "Sanitation check failed: recovery is not retained for the boot" >&2
        exit 1
    }
    if grep -q '^ConditionPathExists=' "$WORK_DIR/root/etc/systemd/system/octessera-update-recovery.service"; then
        echo "Sanitation check failed: recovery is conditional instead of always active" >&2
        exit 1
    fi
}

cleanup() {
    set +e
    mountpoint -q "$WORK_DIR/root" && umount "$WORK_DIR/root"
    mountpoint -q "$WORK_DIR/boot" && umount "$WORK_DIR/boot"
    if [ -n "$LOOP_DEV" ]; then
        kpartx -dv "$LOOP_DEV" >/dev/null 2>&1 || true
        losetup -d "$LOOP_DEV" >/dev/null 2>&1 || true
    fi
    rm -rf "$WORK_DIR"
}
trap cleanup EXIT

unzip -q "$ZIP_PATH" -d "$WORK_DIR"
IMG_PATH="$(find "$WORK_DIR" -name '*.img' | head -n 1)"
if [ -z "$IMG_PATH" ]; then
    echo "No .img found inside $ZIP_PATH" >&2
    exit 1
fi

LOOP_DEV="$(losetup --find --show "$IMG_PATH")"
kpartx -av "$LOOP_DEV" >/dev/null
sleep 2
BASE="$(basename "$LOOP_DEV")"
mkdir -p "$WORK_DIR/boot" "$WORK_DIR/root"
mount -o ro "/dev/mapper/${BASE}p1" "$WORK_DIR/boot"
mount -o ro "/dev/mapper/${BASE}p2" "$WORK_DIR/root"

for path in \
    "$WORK_DIR/boot/ssh" \
    "$WORK_DIR/boot/ssh.txt" \
    "$WORK_DIR/boot/wpa_supplicant.conf" \
    "$WORK_DIR/boot/network-config" \
    "$WORK_DIR/boot/user-data" \
    "$WORK_DIR/root/etc/wpa_supplicant/wpa_supplicant.conf"; do
    if [ -e "$path" ]; then
        echo "Sanitation check failed: found $path" >&2
        exit 1
    fi
done

if find "$WORK_DIR/root" \( -path '*/.ssh/authorized_keys' -o -path '*/.ssh/id_*' \) | grep -q .; then
    echo "Sanitation check failed: found SSH keys" >&2
    exit 1
fi

for path in \
    "$WORK_DIR/root/etc/systemd/system/multi-user.target.wants/ssh.service" \
    "$WORK_DIR/root/etc/systemd/system/sockets.target.wants/ssh.socket"; do
    if [ -e "$path" ]; then
        echo "Sanitation check failed: SSH is enabled by default at $path" >&2
        exit 1
    fi
done

if find "$WORK_DIR/root/etc/NetworkManager/system-connections" -type f 2>/dev/null | grep -q .; then
    echo "Sanitation check failed: found NetworkManager connection profiles" >&2
    exit 1
fi

if grep -RIE '(BEGIN (RSA|OPENSSH) PRIVATE KEY|ghp_|github_pat_|ssid=|psk=)' \
    "$WORK_DIR/boot" \
    "$WORK_DIR/root/etc" \
    "$WORK_DIR/root/home" \
    "$WORK_DIR/root/root" >/dev/null 2>&1; then
    echo "Sanitation check failed: found credential-like material" >&2
    exit 1
fi

require_executable "$WORK_DIR/root/usr/local/bin/octessera-pi" "octessera-pi"
require_path "$WORK_DIR/root/etc/systemd/system/octessera.service" "octessera.service"
require_path "$WORK_DIR/root/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service" "enabled boot splash service"
require_path "$WORK_DIR/root/etc/sudoers.d/octessera-shutdown" "shutdown sudoers rule"
require_path "$WORK_DIR/root/etc/octessera/board-profile.json" "board profile metadata"
require_raspberry_board_profile
require_boot_config_marker
require_boot_overlay
require_updater_protocol

echo "Pi image sanitation check passed"
