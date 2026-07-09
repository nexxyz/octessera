#!/bin/bash
set -e

find "$ROOTFS_DIR/root" "$ROOTFS_DIR/home" -name authorized_keys -type f -delete 2>/dev/null || true
find "$ROOTFS_DIR/root" "$ROOTFS_DIR/home" \( -name id_rsa -o -name id_ed25519 -o -name known_hosts \) -type f -delete 2>/dev/null || true
rm -f "$ROOTFS_DIR/etc/wpa_supplicant/wpa_supplicant.conf"
rm -rf "$ROOTFS_DIR/etc/NetworkManager/system-connections"/*
rm -rf "$ROOTFS_DIR/var/log"/* "$ROOTFS_DIR/tmp"/* "$ROOTFS_DIR/var/tmp"/*
rm -rf "$ROOTFS_DIR/home/pi/.cache" "$ROOTFS_DIR/home/pi/.cargo" "$ROOTFS_DIR/home/pi/.ssh"
rm -f "$ROOTFS_DIR/home/pi/.bash_history" "$ROOTFS_DIR/root/.bash_history"
rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/ssh.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/sockets.target.wants/ssh.socket"
