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

require_boot_config_marker() {
    if grep -q 'Cell Symphony additions' "$WORK_DIR/boot/config.txt" 2>/dev/null; then
        return
    fi
    if grep -q 'Cell Symphony additions' "$WORK_DIR/root/boot/firmware/config.txt" 2>/dev/null; then
        return
    fi
    echo "Sanitation check failed: missing Cell Symphony boot config marker" >&2
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

require_executable "$WORK_DIR/root/usr/local/bin/cellsymphony-pi" "octessera-pi"
require_path "$WORK_DIR/root/etc/systemd/system/cellsymphony.service" "cellsymphony.service"
require_path "$WORK_DIR/root/etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service" "enabled boot splash service"
require_path "$WORK_DIR/root/etc/sudoers.d/cellsymphony-shutdown" "shutdown sudoers rule"
require_boot_config_marker
require_boot_overlay

echo "Pi image sanitation check passed"
