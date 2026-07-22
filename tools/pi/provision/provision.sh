#!/bin/sh
set -eu

PACKAGE_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
IMAGE_ROOT="$PACKAGE_ROOT/root"
PROVISION_ROOT="$PACKAGE_ROOT/files"
REMOTE_REPO=${REMOTE_REPO:-/home/pi/octessera-dev}
SERVICE=${SERVICE:-octessera.service}
UPDATE_INITRAMFS=${UPDATE_INITRAMFS:-0}
WAKE_TRACE=${WAKE_TRACE:-0}

install_file() {
    mode="$1"
    source="$2"
    destination="$3"
    test -f "$source"
    sudo install -D -m "$mode" "$source" "$destination"
}

ensure_boot_config_line() {
    line="$1"
    if ! grep -qxF "$line" "$BOOT_CONFIG"; then
        printf '%s\n' "$line" | sudo tee -a "$BOOT_CONFIG" >/dev/null
    fi
}

disable_service_if_present() {
    service="$1"
    sudo systemctl disable --now "$service" >/dev/null 2>&1 || true
}

escape_sed_replacement() {
    printf '%s' "$1" | sed 's/[\\&|]/\\&/g'
}

BOOT_CONFIG=/boot/firmware/config.txt
if [ ! -f "$BOOT_CONFIG" ]; then
    BOOT_CONFIG=/boot/config.txt
fi
test -f "$BOOT_CONFIG"

while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
        ''|'#'*) continue ;;
    esac
    ensure_boot_config_line "$line"
done < "$PROVISION_ROOT/boot/config.txt.append"

sudo rm -f \
    /etc/initramfs-tools/hooks/cellsymphony-boot-splash \
    /etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash \
    /etc/systemd/system/cellsymphony-boot-splash.service \
    /etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service

install_file 0755 "$IMAGE_ROOT/usr/local/sbin/octessera-usb-gadget" /usr/local/sbin/octessera-usb-gadget
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-usb-gadget.service" /etc/systemd/system/octessera-usb-gadget.service
install_file 0644 "$IMAGE_ROOT/etc/modules-load.d/octessera-usb-gadget.conf" /etc/modules-load.d/octessera-usb-gadget.conf
install_file 0440 "$IMAGE_ROOT/etc/sudoers.d/octessera-usb-storage" /etc/sudoers.d/octessera-usb-storage
sudo install -d -m 0755 "/etc/systemd/system/$SERVICE.d"
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera.service.d/audio-realtime.conf" "/etc/systemd/system/$SERVICE.d/audio-realtime.conf"
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-boot-splash.service" /etc/systemd/system/octessera-boot-splash.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-oled-shutdown.service" /etc/systemd/system/octessera-oled-shutdown.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-performance-governor.service" /etc/systemd/system/octessera-performance-governor.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-network-health.service" /etc/systemd/system/octessera-network-health.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-network-health.timer" /etc/systemd/system/octessera-network-health.timer
install_file 0644 "$IMAGE_ROOT/etc/systemd/journald.conf.d/10-octessera.conf" /etc/systemd/journald.conf.d/10-octessera.conf
install_file 0644 "$IMAGE_ROOT/etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf" /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf
install_file 0755 "$IMAGE_ROOT/usr/local/bin/octessera-network-health" /usr/local/bin/octessera-network-health
install_file 0440 "$IMAGE_ROOT/etc/sudoers.d/octessera-shutdown" /etc/sudoers.d/octessera-shutdown
install_file 0644 "$IMAGE_ROOT/etc/profile.d/octessera-welcome.sh" /etc/profile.d/octessera-welcome.sh

sudo sed -i 's/\r$//' \
    /usr/local/sbin/octessera-usb-gadget \
    /usr/local/bin/octessera-network-health \
    /etc/systemd/system/octessera-usb-gadget.service \
    /etc/systemd/system/octessera-boot-splash.service \
    /etc/systemd/system/octessera-oled-shutdown.service \
    /etc/systemd/system/octessera-performance-governor.service \
    /etc/systemd/system/octessera-network-health.service \
    /etc/systemd/system/octessera-network-health.timer \
    /etc/systemd/journald.conf.d/10-octessera.conf \
    /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf \
    /etc/profile.d/octessera-welcome.sh

REMOTE_REPO_ESCAPED=$(escape_sed_replacement "$REMOTE_REPO")
if [ "$WAKE_TRACE" = "1" ]; then
    WAKE_TRACE_LINE=Environment=OCTESSERA_WAKE_TRACE=1
else
    WAKE_TRACE_LINE=
fi
WAKE_TRACE_ESCAPED=$(escape_sed_replacement "$WAKE_TRACE_LINE")
sed \
    -e "s|@REMOTE_REPO@|$REMOTE_REPO_ESCAPED|g" \
    -e "s|@WAKE_TRACE@|$WAKE_TRACE_ESCAPED|g" \
    "$PROVISION_ROOT/etc/systemd/system/octessera.service.template" |
    sudo tee "/etc/systemd/system/$SERVICE" >/dev/null
sudo chmod 0644 "/etc/systemd/system/$SERVICE"
sudo sed -i 's/\r$//' \
    "/etc/systemd/system/$SERVICE" \
    "/etc/systemd/system/$SERVICE.d/audio-realtime.conf"

sudo visudo -cf /etc/sudoers.d/octessera-shutdown >/dev/null
sudo visudo -cf /etc/sudoers.d/octessera-usb-storage >/dev/null

if [ "$UPDATE_INITRAMFS" = "1" ]; then
    if ! grep -qxF "# octessera required boot settings" "$BOOT_CONFIG" && ! grep -qxF "# Octessera required boot settings" "$BOOT_CONFIG"; then
        printf '\n' | sudo tee -a "$BOOT_CONFIG" >/dev/null
        sudo tee -a "$BOOT_CONFIG" < "$PROVISION_ROOT/boot/config.txt.initramfs.append" >/dev/null
    fi
    ensure_boot_config_line "dtparam=spi=on"
    ensure_boot_config_line "auto_initramfs=1"

    if ! command -v update-initramfs >/dev/null 2>&1; then
        sudo apt-get update
        sudo apt-get install -y --no-install-recommends initramfs-tools
    fi
    install_file 0755 "$IMAGE_ROOT/etc/initramfs-tools/hooks/octessera-boot-splash" /etc/initramfs-tools/hooks/octessera-boot-splash
    install_file 0755 "$IMAGE_ROOT/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash
    sudo sed -i 's/\r$//' \
        /etc/initramfs-tools/hooks/octessera-boot-splash \
        /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash
    sudo install -d -m 0755 /etc/initramfs-tools
    grep -qxF "spi-bcm2835" /etc/initramfs-tools/modules || printf '%s\n' "spi-bcm2835" | sudo tee -a /etc/initramfs-tools/modules >/dev/null
    grep -qxF "spidev" /etc/initramfs-tools/modules || printf '%s\n' "spidev" | sudo tee -a /etc/initramfs-tools/modules >/dev/null
    sudo update-initramfs -u
else
    echo "Skipping initramfs update. Pass -UpdateInitramfs to refresh the early boot splash initramfs."
fi

sudo install -d -m 0750 /etc/sudoers.d
sudo systemctl restart systemd-journald
disable_service_if_present bluetooth.service
disable_service_if_present hciuart.service
sudo iw dev wlan0 set power_save off >/dev/null 2>&1 || true
sudo nmcli connection modify preconfigured 802-11-wireless.powersave 2 >/dev/null 2>&1 || true
sudo nmcli device reapply wlan0 >/dev/null 2>&1 || true

sudo systemctl daemon-reload
sudo systemctl enable octessera-usb-gadget.service >/dev/null
sudo systemctl enable --now octessera-network-health.timer >/dev/null
sudo systemctl enable octessera-oled-shutdown.service >/dev/null
sudo systemctl start octessera-oled-shutdown.service
sudo systemctl enable octessera-performance-governor.service >/dev/null
sudo systemctl start octessera-performance-governor.service
sudo systemctl enable "$SERVICE" >/dev/null
sudo systemctl disable octessera-boot-splash.service >/dev/null 2>&1 || true
