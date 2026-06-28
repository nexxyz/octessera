#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/../.." && pwd)/files"
BOOT_DIR="$ROOTFS_DIR/boot/firmware"
if [ ! -d "$BOOT_DIR" ]; then
    BOOT_DIR="$ROOTFS_DIR/boot"
fi

if [ -f "$STAGE_FILES/boot/config.txt.append" ]; then
    {
        echo ""
        echo "# --- Cell Symphony additions ---"
        cat "$STAGE_FILES/boot/config.txt.append"
    } >> "$BOOT_DIR/config.txt"
fi

if [ -f "$STAGE_FILES/boot/overlays/i2s-dac-no20.dts" ]; then
    install -d "$BOOT_DIR/overlays"
    dtc -@ -I dts -O dtb \
        -o "$BOOT_DIR/overlays/i2s-dac-no20.dtbo" \
        "$STAGE_FILES/boot/overlays/i2s-dac-no20.dts"
fi

rm -f "$BOOT_DIR/ssh" "$BOOT_DIR/ssh.txt"
rm -f "$BOOT_DIR/wpa_supplicant.conf" "$BOOT_DIR/network-config" "$BOOT_DIR/user-data"
