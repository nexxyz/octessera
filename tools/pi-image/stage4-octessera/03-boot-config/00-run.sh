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
cmdline_tmp="$(mktemp "${cmdline}.console.XXXXXX")"
if ! awk '
    {
        if (NR > 1) {
            failed = 1
        }
        count = 0
        output = ""
        for (position = 1; position <= NF; position++) {
            token = $position
            if (token ~ /^console=(serial0|ttyAMA0|ttyS0)(,.*)?$/) {
                continue
            }
            if (token == "console=tty1") {
                count++
            }
            output = output (output == "" ? "" : " ") token
        }
        if (count == 0) {
            output = output (output == "" ? "" : " ") "console=tty1"
        }
        if (count > 1) {
            failed = 1
        }
        print output
    }
    END { exit(failed ? 1 : 0) }
  ' "$cmdline" > "$cmdline_tmp"; then
    rm -f "$cmdline_tmp"
    echo "Raspberry Pi kernel command line must contain exactly one console=tty1 token." >&2
    exit 1
fi
chmod --reference="$cmdline" "$cmdline_tmp"
chown --reference="$cmdline" "$cmdline_tmp"
mv -f "$cmdline_tmp" "$cmdline"
if [[ "$(grep -oE '(^|[[:space:]])console=tty1([[:space:]]|$)' "$cmdline" | wc -l)" != 1 ]]; then
    echo "Raspberry Pi kernel command line must contain exactly one console=tty1 token." >&2
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
