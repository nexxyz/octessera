#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"

install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-performance-governor.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-performance-governor.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-boot-splash.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-boot-splash.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-oled-shutdown.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-oled-shutdown.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/journald.conf.d/10-octessera.conf" \
    "$ROOTFS_DIR/etc/systemd/journald.conf.d/10-octessera.conf"
install -D -m 0440 \
    "$STAGE_FILES/root/etc/sudoers.d/octessera-shutdown" \
    "$ROOTFS_DIR/etc/sudoers.d/octessera-shutdown"
install -D -m 0755 \
    "$STAGE_FILES/root/etc/initramfs-tools/hooks/octessera-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/hooks/octessera-boot-splash"
install -D -m 0755 \
    "$STAGE_FILES/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"

install -d "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants"
install -d "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants"
ln -sf ../octessera.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera.service"
ln -sf ../octessera-performance-governor.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-performance-governor.service"
ln -sf ../octessera-oled-shutdown.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-oled-shutdown.service"
ln -sf ../octessera-boot-splash.service \
    "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service"

rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/bluetooth.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/hciuart.service"

install -d -m 0755 "$ROOTFS_DIR/var/log/octessera"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples" "$ROOTFS_DIR/home/pi/presets"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples/sd-card"
chroot "$ROOTFS_DIR" chown -R pi:pi /home/pi/samples /home/pi/presets
chroot "$ROOTFS_DIR" update-initramfs -u
