#!/bin/bash
set -e

# Install systemd service for auto-start
cp /files/root/etc/systemd/system/cellsymphony.service /etc/systemd/system/
cp /files/root/etc/systemd/system/cellsymphony-performance-governor.service /etc/systemd/system/
systemctl enable cellsymphony.service
systemctl enable cellsymphony-performance-governor.service

# Install journald config for volatile capped logs
install -D -m 0644 /files/root/etc/systemd/journald.conf.d/10-cellsymphony.conf /etc/systemd/journald.conf.d/10-cellsymphony.conf

# Disable Bluetooth services when present
for service in bluetooth.service hciuart.service; do
    systemctl disable --now "$service" >/dev/null 2>&1 || true
done

# Create log directory
mkdir -p /var/log/cellsymphony

# Create user storage locations used by the Pi app.
install -d -o pi -g pi -m 0755 /home/pi/samples /home/pi/presets
install -d -o pi -g pi -m 0755 /home/pi/samples/sd-card
