#!/bin/bash
set -euo pipefail

if [ "$#" -ne 3 ] || [ "$1" != --runtime-bundle ]; then
    echo "Usage: $0 --runtime-bundle <dir> <image.zip>" >&2
    exit 2
fi
RUNTIME_BUNDLE="$2"
ZIP_PATH="$3"
REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d)"
BOOT="$WORK_DIR/boot"
ROOT="$WORK_DIR/root"
LOOP_DEV=""

fail() {
    echo "Raspberry image smoke check failed: $*" >&2
    exit 1
}

cleanup() {
    set +e
    mountpoint -q "$ROOT" && umount "$ROOT"
    mountpoint -q "$BOOT" && umount "$BOOT"
    if [ -n "$LOOP_DEV" ]; then
        kpartx -dv "$LOOP_DEV" >/dev/null 2>&1
        losetup -d "$LOOP_DEV" >/dev/null 2>&1
    fi
    rm -rf "$WORK_DIR"
}
trap cleanup EXIT

unzip -q "$ZIP_PATH" -d "$WORK_DIR"
mapfile -t IMAGE_PATHS < <(find "$WORK_DIR" -type f -name '*.img' -print)
[ "${#IMAGE_PATHS[@]}" -eq 1 ] || fail "expected exactly one .img inside $ZIP_PATH, found ${#IMAGE_PATHS[@]}"
LOOP_DEV="$(losetup --find --show "${IMAGE_PATHS[0]}")"
kpartx -av "$LOOP_DEV" >/dev/null
sleep 2
mkdir -p "$BOOT" "$ROOT"
mount -o ro "/dev/mapper/$(basename "$LOOP_DEV")p1" "$BOOT"
mount -o ro "/dev/mapper/$(basename "$LOOP_DEV")p2" "$ROOT"

[ -f "$ROOT/etc/systemd/system/octessera.service" ] || fail "octessera.service is not installed"
[ -L "$ROOT/etc/systemd/system/multi-user.target.wants/octessera.service" ] || fail "octessera.service is not enabled"

[ "$(readlink "$ROOT/usr/local/bin/octessera-pi")" = /opt/octessera/current/octessera-pi ] || fail "octessera-pi does not point at the current release"
cmp -s "$ROOT$(readlink "$ROOT/opt/octessera/current")/octessera-pi" "$RUNTIME_BUNDLE/octessera-pi" || fail "installed release binary differs from the runtime bundle"

for overlay in i2s-dac-no20 octessera-oled-sd octessera-wifi-pio; do
    [ -f "$BOOT/octessera/overlays/$overlay.dtbo" ] || [ -f "$BOOT/overlays/$overlay.dtbo" ] || fail "missing $overlay.dtbo"
done
grep -qxF '# --- octessera additions ---' "$BOOT/config.txt" || fail "config.txt has no octessera additions marker"

mapfile -t INITRAMFS_PATHS < <(find "$BOOT/octessera" -maxdepth 1 -type f -name 'initrd.img-*' -print)
[ "${#INITRAMFS_PATHS[@]}" -eq 1 ] || fail "expected exactly one initrd.img-* in the boot octessera directory"
INITRAMFS_LISTING="$(lsinitramfs "${INITRAMFS_PATHS[0]}")"
for entry in usr/local/bin/octessera-pi scripts/init-premount/octessera-boot-splash; do
    grep -Eq "(^|/)$entry\$" <<< "$INITRAMFS_LISTING" || fail "initramfs is missing $entry"
done

[ -z "$(find "$ROOT/etc/ssh" "$ROOT/home" "$ROOT/root" \( -name 'ssh_host_*_key*' -o -path '*/.ssh/authorized_keys' \) -print -quit)" ] || fail "SSH keys are baked into the image"
[ ! -d "$ROOT/etc/NetworkManager/system-connections" ] || [ -z "$(find "$ROOT/etc/NetworkManager/system-connections" -type f -print -quit)" ] || fail "NetworkManager connection profiles are baked into the image"

python3 "$REPOSITORY_ROOT/tools/legal/stage_notices.py" --repository-root "$REPOSITORY_ROOT" --destination-root "$ROOT" --check >/dev/null || fail "installed legal notice tree is not canonical"

echo "Raspberry image smoke check passed"
