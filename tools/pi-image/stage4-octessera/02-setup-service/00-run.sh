#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"

rm -f \
    "$ROOTFS_DIR/etc/initramfs-tools/hooks/cellsymphony-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash" \
    "$ROOTFS_DIR/etc/systemd/system/cellsymphony-boot-splash.service" \
    "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service"

install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera.service.d/audio-realtime.conf" \
    "$ROOTFS_DIR/etc/systemd/system/octessera.service.d/audio-realtime.conf"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-usb-gadget.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-usb-gadget.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/modules-load.d/octessera-usb-gadget.conf" \
    "$ROOTFS_DIR/etc/modules-load.d/octessera-usb-gadget.conf"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-usb-gadget" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-usb-gadget"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-sd-card" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-sd-card"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-performance-governor.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-performance-governor.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-sd-card.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-sd-card.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/udev/rules.d/99-octessera-sd-card.rules" \
    "$ROOTFS_DIR/etc/udev/rules.d/99-octessera-sd-card.rules"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-boot-splash.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-boot-splash.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-oled-shutdown.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-oled-shutdown.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/journald.conf.d/10-octessera.conf" \
    "$ROOTFS_DIR/etc/systemd/journald.conf.d/10-octessera.conf"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf" \
    "$ROOTFS_DIR/etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/bin/octessera-network-health" \
    "$ROOTFS_DIR/usr/local/bin/octessera-network-health"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-network-health.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-network-health.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-network-health.timer" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-network-health.timer"
install -D -m 0440 \
    "$STAGE_FILES/root/etc/sudoers.d/octessera-shutdown" \
    "$ROOTFS_DIR/etc/sudoers.d/octessera-shutdown"
install -D -m 0440 \
    "$STAGE_FILES/root/etc/sudoers.d/octessera-usb-storage" \
    "$ROOTFS_DIR/etc/sudoers.d/octessera-usb-storage"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/profile.d/octessera-welcome.sh" \
    "$ROOTFS_DIR/etc/profile.d/octessera-welcome.sh"
install -D -m 0755 \
    "$STAGE_FILES/root/etc/initramfs-tools/hooks/octessera-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/hooks/octessera-boot-splash"
install -D -m 0755 \
    "$STAGE_FILES/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"

install -d "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants"
install -d "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants"
install -d "$ROOTFS_DIR/etc/systemd/system/timers.target.wants"
ln -sf ../octessera.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera.service"
ln -sf ../octessera-usb-gadget.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-usb-gadget.service"
ln -sf ../octessera-performance-governor.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-performance-governor.service"
ln -sf ../octessera-sd-card.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-sd-card.service"
ln -sf ../octessera-oled-shutdown.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-oled-shutdown.service"
ln -sf ../octessera-boot-splash.service \
    "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service"
ln -sf ../octessera-network-health.timer \
    "$ROOTFS_DIR/etc/systemd/system/timers.target.wants/octessera-network-health.timer"

rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/bluetooth.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/hciuart.service"

install -d -m 0755 "$ROOTFS_DIR/var/log/octessera"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples" "$ROOTFS_DIR/home/pi/presets"
install -d -m 0755 "$ROOTFS_DIR/home/pi/samples/sd-card"
chroot "$ROOTFS_DIR" chown -R pi:pi /home/pi/samples /home/pi/presets
chroot "$ROOTFS_DIR" update-initramfs -u
