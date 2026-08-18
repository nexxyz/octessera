#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"
LEGAL_REPOSITORY_ROOT="${OCTESSERA_REPOSITORY_ROOT:?OCTESSERA_REPOSITORY_ROOT must point to the canonical source checkout}"
LEGAL_STAGER="$LEGAL_REPOSITORY_ROOT/tools/legal/stage_notices.py"
test -f "$LEGAL_STAGER"

python3 "$LEGAL_STAGER" \
    --repository-root "$LEGAL_REPOSITORY_ROOT" \
    --destination-root "$STAGE_FILES/root" \
    --check >/dev/null || {
    echo "Pi image setup requires the exactly pre-staged canonical legal notice tree; run tools/legal/stage_notices.py first." >&2
    exit 2
}

for updater_file in \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_protocol.py" \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_state.py" \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_assets.py" \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_guard.py" \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_cli.py" \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_profiles.py"; do
    if [ ! -f "$updater_file" ]; then
        echo "Pi image setup requires the canonical updater runtime artifacts; run the release staging copy first." >&2
        exit 2
    fi
done
for wifi_foundation_file in \
    "$STAGE_FILES/root/usr/local/sbin/octessera-wifi-foundation" \
    "$STAGE_FILES/root/etc/systemd/system/octessera-wifi-foundation.service"; do
    if [ ! -f "$wifi_foundation_file" ]; then
        echo "Pi image setup requires the inactive Wi-Fi foundation artifacts." >&2
        exit 2
    fi
done

rm -f \
    "$ROOTFS_DIR/etc/initramfs-tools/hooks/cellsymphony-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash" \
    "$ROOTFS_DIR/etc/systemd/system/cellsymphony-boot-splash.service" \
    "$ROOTFS_DIR/etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service" \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/cellsymphony-boot-splash.service" \
    "$ROOTFS_DIR/etc/initramfs-tools/hooks/octessera-boot-splash" \
    "$ROOTFS_DIR/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-boot-splash.service"

install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera.service"
test -f "$STAGE_FILES/root/etc/profile.d/octessera-welcome.sh" && [ ! -L "$STAGE_FILES/root/etc/profile.d/octessera-welcome.sh" ]
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/profile.d/octessera-welcome.sh" \
    "$ROOTFS_DIR/etc/profile.d/octessera-welcome.sh"
rm -f "$ROOTFS_DIR/usr/local/lib/octessera/rpi_uart_release.py"
while IFS= read -r -d '' legal_file; do
    legal_relative="${legal_file#"$STAGE_FILES/root/usr/share/doc/octessera/"}"
    install -D -o root -g root -m 0644 \
        "$legal_file" \
        "$ROOTFS_DIR/usr/share/doc/octessera/$legal_relative"
done < <(find -P "$STAGE_FILES/root/usr/share/doc/octessera" -type f -print0)
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
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/device_config.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/device_config.py"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-sd-card" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-sd-card"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-wifi-foundation" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-wifi-foundation"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-wifi-connect" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-wifi-connect"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-setup-sidecar" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-sidecar"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-setup-request" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-request"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-setup-request-cleanup" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-request-cleanup"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-setup-start" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-start"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-setup-cleanup" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-cleanup"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/lib/octessera/setup-status.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup-status.py"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/setup-status-cli.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup-status-cli.py"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/lib/octessera/setup-call.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup-call.py"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/octessera/setup-profile" \
    "$ROOTFS_DIR/etc/octessera/setup-profile"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-setup.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-setup.service"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-setup-request.path" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-setup-request.path"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-setup-request.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-setup-request.service"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/index.html" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/index.html"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/app.js" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/app.js"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/styles.css" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/styles.css"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/README.md" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/README.md"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/octessera-mark.svg" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/octessera-mark.svg"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/octessera-wordmark.svg" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/octessera-wordmark.svg"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-update" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-update"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-update-guard" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-update-guard"
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-update-recovery" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-update-recovery"
install -D -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_protocol.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_protocol.py"
install -D -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_state.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_state.py"
install -D -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_assets.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_assets.py"
install -D -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_guard.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_guard.py"
install -D -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_cli.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_cli.py"
install -D -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_profiles.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_profiles.py"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-update-guard.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-update-guard.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-update-recovery.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-update-recovery.service"
install -D -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-performance-governor.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-performance-governor.service"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-wifi-foundation.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-wifi-foundation.service"
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
install -D -m 0440 \
    "$STAGE_FILES/root/etc/sudoers.d/octessera-update" \
    "$ROOTFS_DIR/etc/sudoers.d/octessera-update"
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
ln -sf ../octessera-update-recovery.service \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service"
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
ln -sf ../octessera-setup-request.path \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-setup-request.path"

rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/bluetooth.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/hciuart.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/getty.target.wants"/serial-getty@*.service
for unit in serial-getty@ttyAMA0.service serial-getty@ttyS0.service serial-getty@serial0.service bluetooth.service hciuart.service; do
    rm -f "$ROOTFS_DIR/etc/systemd/system/$unit"
    ln -s /dev/null "$ROOTFS_DIR/etc/systemd/system/$unit"
    test "$(readlink "$ROOTFS_DIR/etc/systemd/system/$unit")" = /dev/null
done

install -d -m 0755 "$ROOTFS_DIR/var/log/octessera"
bash "$LEGAL_REPOSITORY_ROOT/tools/pi-image/install-musical-assets.sh" "$STAGE_FILES/root" "$ROOTFS_DIR"
install -d -m 0755 "$ROOTFS_DIR/home/pi/presets"
install -D -o root -g root -m 0644 \
    "$LEGAL_REPOSITORY_ROOT/config/generated/pi/default.json" \
    "$ROOTFS_DIR/home/pi/presets/default.json"
pi_record="$(awk -F: '$1 == "pi" { print; count++ } END { if (count != 1) exit 1 }' "$ROOTFS_DIR/etc/passwd")"
IFS=: read -r pi_user _ pi_uid pi_gid _ pi_home pi_shell <<< "$pi_record"
if [ "$pi_home" != /home/pi ] || [ "$pi_shell" != /bin/bash ] || [ ! -d "$ROOTFS_DIR$pi_home" ] || [ -L "$ROOTFS_DIR$pi_home" ]; then
    echo "Pi image setup requires pi:/home/pi:/bin/bash with a real home directory." >&2
    exit 2
fi
hushlogin="$ROOTFS_DIR$pi_home/.hushlogin"
if [ -e "$hushlogin" ] || [ -L "$hushlogin" ]; then
    if [ ! -f "$hushlogin" ] || [ -L "$hushlogin" ] || [ "$(stat -c '%u:%g:%a:%s' "$hushlogin")" != "$pi_uid:$pi_gid:644:0" ] || [ -s "$hushlogin" ]; then
        echo "Pi .hushlogin exists with unexpected type, owner, mode, or content." >&2
        exit 2
    fi
else
    install -D -m 0644 /dev/null "$hushlogin"
fi
chroot "$ROOTFS_DIR" chown "$pi_user:$pi_user" "$pi_home/.hushlogin"
chroot "$ROOTFS_DIR" chown -R pi:pi /home/pi/samples /home/pi/presets
chroot "$ROOTFS_DIR" chmod 0644 /home/pi/presets/default.json
