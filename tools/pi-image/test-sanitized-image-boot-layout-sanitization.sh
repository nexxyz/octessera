#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/pi-image/test-sanitized-image-boot-layout-fixture.sh
source "$script_dir/test-sanitized-image-boot-layout-fixture.sh"

reset_fixture
mkdir -p \
    "$fixture/root/etc/initramfs-tools/scripts/init-premount" \
    "$fixture/root/opt/octessera/releases/1.2.3" \
    "$fixture/root/usr/local/bin" \
    "$fixture/archive/scripts/init-premount" \
    "$fixture/archive/usr/local/bin" \
    "$fixture/archive/usr/bin"
cp "$script_dir/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
chmod 0755 "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
cp "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$fixture/archive/scripts/init-premount/octessera-boot-splash"
chmod 0755 "$fixture/archive/scripts/init-premount/octessera-boot-splash"
printf '%s\n' 'constructor-runtime-binary' > "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi"
chmod 0755 "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi"
cp "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi" "$fixture/archive/usr/local/bin/octessera-pi"
ln -s /opt/octessera/releases/1.2.3 "$fixture/root/opt/octessera/current"
ln -s /opt/octessera/current/octessera-pi "$fixture/root/usr/local/bin/octessera-pi"
ln -s usr/bin "$fixture/archive/bin"
ln -s dash "$fixture/archive/usr/bin/sh"
for archive_entry in usr/bin/dash usr/bin/setsid usr/bin/sleep usr/bin/cat usr/bin/mv usr/bin/chmod usr/bin/chown usr/bin/rm; do
    mkdir -p "$fixture/archive/${archive_entry%/*}"
    printf '%s\n' "fixture-$archive_entry" > "$fixture/archive/$archive_entry"
    chmod 0755 "$fixture/archive/$archive_entry"
done
mkdir -p "$fixture/archive/lib/modules/fixture/kernel/drivers/spi"
for required_module in spi-bcm2835 spidev; do
    printf '%s\n' "fixture-$required_module" > "$fixture/archive/lib/modules/fixture/kernel/drivers/spi/$required_module.ko"
done
make_initramfs() {
    local output="$fixture/boot/octessera-bound.img"
    mkdir -p "$fixture/boot"
    (cd "$fixture/archive" && find . -print | cpio -o -H newc --quiet | gzip -n > "$output")
}
make_initramfs
require_octessera_initramfs_rootfs_bindings "$fixture/boot/octessera-bound.img" "$fixture/root" "$contract_path"
printf '%s\n' 'stale-script' > "$fixture/archive/scripts/init-premount/octessera-boot-splash"
make_initramfs
if require_octessera_initramfs_rootfs_bindings "$fixture/boot/octessera-bound.img" "$fixture/root" "$contract_path"; then
    echo 'Boot layout accepted a stale initramfs script.' >&2
    exit 1
fi
cp "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$fixture/archive/scripts/init-premount/octessera-boot-splash"
printf '%s\n' 'stale-binary' > "$fixture/archive/usr/local/bin/octessera-pi"
make_initramfs
if require_octessera_initramfs_rootfs_bindings "$fixture/boot/octessera-bound.img" "$fixture/root" "$contract_path"; then
    echo 'Boot layout accepted a stale initramfs binary.' >&2
    exit 1
fi
