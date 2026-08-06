#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"
BOOT_DIR="$ROOTFS_DIR/boot/firmware"
if [ ! -d "$BOOT_DIR" ]; then
    BOOT_DIR="$ROOTFS_DIR/boot"
fi

if [ -f "$STAGE_FILES/boot/config.txt.append" ]; then
    sed -i -E '/^[[:space:]]*(dtoverlay=disable-bt|enable_uart=)/d' "$BOOT_DIR/config.txt"
    {
        echo ""
        echo "# --- octessera additions ---"
        cat "$STAGE_FILES/boot/config.txt.append"
    } >> "$BOOT_DIR/config.txt"
fi

cmdline="$BOOT_DIR/cmdline.txt"
test -f "$cmdline"
while grep -Eq '(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)' "$cmdline"; do
    sed -i -E 's/(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)/\1\4/' "$cmdline"
done
if grep -Eq '(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)' "$cmdline"; then
    echo "Serial console token remains in the Raspberry Pi kernel command line." >&2
    exit 1
fi
grep -qxF 'dtoverlay=disable-bt' "$BOOT_DIR/config.txt"
grep -qxF 'enable_uart=0' "$BOOT_DIR/config.txt"

if [ -f "$STAGE_FILES/boot/overlays/i2s-dac-no20.dts" ]; then
    install -d "$BOOT_DIR/octessera/overlays"
    if [ ! -e "$BOOT_DIR/octessera/overlays/i2s-dac-no20.dtbo" ]; then
        dtc -@ -I dts -O dtb \
            -o "$BOOT_DIR/octessera/overlays/i2s-dac-no20.dtbo" \
            "$STAGE_FILES/boot/overlays/i2s-dac-no20.dts"
    fi
fi

rm -f "$BOOT_DIR/ssh" "$BOOT_DIR/ssh.txt"
rm -f "$BOOT_DIR/wpa_supplicant.conf" "$BOOT_DIR/network-config" "$BOOT_DIR/user-data"

chroot "$ROOTFS_DIR" /usr/local/sbin/octessera-finalize-rpi-kernel
