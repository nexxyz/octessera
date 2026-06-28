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
    "$STAGE_FILES/root/etc/systemd/journald.conf.d/10-cellsymphony.conf" \
    "$ROOTFS_DIR/etc/systemd/journald.conf.d/10-cellsymphony.conf"

install -d "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants"
ln -sf ../cellsymphony.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/cellsymphony.service"
ln -sf ../cellsymphony-performance-governor.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/cellsymphony-performance-governor.service"

rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/bluetooth.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/hciuart.service"

install -d -m 0755 "$ROOTFS_DIR/var/log/cellsymphony"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples" "$ROOTFS_DIR/home/pi/presets"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples/sd-card"
chroot "$ROOTFS_DIR" chown -R pi:pi /home/pi/samples /home/pi/presets
