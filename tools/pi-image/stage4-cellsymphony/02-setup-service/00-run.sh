#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"

install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/cellsymphony.service" \
    "$ROOTFS_DIR/etc/systemd/system/cellsymphony.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/cellsymphony-performance-governor.service" \
    "$ROOTFS_DIR/etc/systemd/system/cellsymphony-performance-governor.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/cellsymphony-boot-splash.service" \
    "$ROOTFS_DIR/etc/systemd/system/cellsymphony-boot-splash.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/cellsymphony-oled-shutdown.service" \
    "$ROOTFS_DIR/etc/systemd/system/cellsymphony-oled-shutdown.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/journald.conf.d/10-cellsymphony.conf" \
    "$ROOTFS_DIR/etc/systemd/journald.conf.d/10-cellsymphony.conf"
install -D -m 0440 \
    "$STAGE_FILES/root/etc/sudoers.d/cellsymphony-shutdown" \
    "$ROOTFS_DIR/etc/sudoers.d/cellsymphony-shutdown"
install -D -m 0755 \
    "$STAGE_FILES/root/etc/initramfs-tools/hooks/cellsymphony-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/hooks/cellsymphony-boot-splash"
install -D -m 0755 \
    "$STAGE_FILES/root/etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash"

install -d "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants"
install -d "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants"
ln -sf ../cellsymphony.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/cellsymphony.service"
ln -sf ../cellsymphony-performance-governor.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/cellsymphony-performance-governor.service"
ln -sf ../cellsymphony-oled-shutdown.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/cellsymphony-oled-shutdown.service"
ln -sf ../cellsymphony-boot-splash.service \
    "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service"

rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/bluetooth.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/hciuart.service"

install -d -m 0755 "$ROOTFS_DIR/var/log/cellsymphony"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples" "$ROOTFS_DIR/home/pi/presets"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples/sd-card"
chroot "$ROOTFS_DIR" chown -R pi:pi /home/pi/samples /home/pi/presets
chroot "$ROOTFS_DIR" update-initramfs -u
