#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"
LEGAL_REPOSITORY_ROOT="${OCTESSERA_REPOSITORY_ROOT:?OCTESSERA_REPOSITORY_ROOT must point to the canonical source checkout}"
LEGAL_STAGER="$LEGAL_REPOSITORY_ROOT/tools/legal/stage_notices.py"
test -f "$LEGAL_STAGER"
# shellcheck source=tools/pi-image/validate-rpi-parent-sudoers.sh
source "$LEGAL_REPOSITORY_ROOT/tools/pi-image/validate-rpi-parent-sudoers.sh"

python3 "$LEGAL_STAGER" \
    --repository-root "$LEGAL_REPOSITORY_ROOT" \
    --destination-root "$STAGE_FILES/root" \
    --check >/dev/null || {
    echo "Pi image setup requires the exactly pre-staged canonical legal notice tree; run tools/legal/stage_notices.py first." >&2
    exit 2
}

for updater_file in \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_protocol.py" \
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_contract.py" \
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

wifi_connect_artifact_root="$LEGAL_REPOSITORY_ROOT/target/wifi-connect-patched"
wifi_connect_legal_root="$LEGAL_REPOSITORY_ROOT/third_party/wifi-connect-4.11.84"
wifi_connect_expected_sha256=929a5b937a771a0e4f96446242af217c61118aedaaaa053aff75af61151c6acc
wifi_connect_patch_sha256=3481ef27637c5c4a176b59f74af4e2c232f6c67de8399eaf705fe6431ffc8939
for wifi_connect_file in wifi-connect wifi-connect.metadata.json cargo-metadata.json; do
    test -f "$wifi_connect_artifact_root/$wifi_connect_file" && test ! -L "$wifi_connect_artifact_root/$wifi_connect_file"
done
for wifi_connect_file in LICENSE THIRD-PARTY-NOTICES.md portal-address-readiness.patch; do
    test -f "$wifi_connect_legal_root/$wifi_connect_file" && test ! -L "$wifi_connect_legal_root/$wifi_connect_file"
done
echo "$wifi_connect_expected_sha256  $wifi_connect_artifact_root/wifi-connect" | sha256sum -c -
echo "$wifi_connect_patch_sha256  $wifi_connect_legal_root/portal-address-readiness.patch" | sha256sum -c -
python3 - "$wifi_connect_artifact_root/wifi-connect.metadata.json" "$wifi_connect_expected_sha256" "$wifi_connect_patch_sha256" <<'PY'
import json
import sys

metadata = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert metadata["binary_sha256"] == sys.argv[2]
assert metadata["patch_sha256"] == sys.argv[3]
assert metadata["target"] == "aarch64-unknown-linux-gnu"
PY
install -D -o root -g root -m 0755 \
    "$wifi_connect_artifact_root/wifi-connect" \
    "$ROOTFS_DIR/usr/local/bin/wifi-connect"
for wifi_connect_doc in LICENSE THIRD-PARTY-NOTICES.md; do
    install -D -o root -g root -m 0644 \
        "$wifi_connect_legal_root/$wifi_connect_doc" \
        "$ROOTFS_DIR/usr/local/share/doc/octessera/wifi-connect/$wifi_connect_doc"
done
for wifi_connect_doc in wifi-connect.metadata.json cargo-metadata.json; do
    install -D -o root -g root -m 0644 \
        "$wifi_connect_artifact_root/$wifi_connect_doc" \
        "$ROOTFS_DIR/usr/local/share/doc/octessera/wifi-connect/$wifi_connect_doc"
done

octessera_remove_raspberry_parent_sudoers "$ROOTFS_DIR"

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
    "$LEGAL_REPOSITORY_ROOT/tools/storage/octessera-sd-card" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-sd-card"
install -D -o root -g root -m 0644 \
    "$LEGAL_REPOSITORY_ROOT/tools/storage/octessera-sd-card-lib.sh" \
    "$ROOTFS_DIR/usr/local/lib/octessera/octessera-sd-card-lib.sh"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-wifi-foundation" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-wifi-foundation"
install -D -o root -g root -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-setup" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/setup_config.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup_config.py"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/lib/octessera/setup_http.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup_http.py"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/octessera/setup-profile" \
    "$ROOTFS_DIR/etc/octessera/setup-profile"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/default/locale" \
    "$ROOTFS_DIR/etc/default/locale"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/tmpfiles.d/octessera-setup-request.conf" \
    "$ROOTFS_DIR/etc/tmpfiles.d/octessera-setup-request.conf"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-setup.service" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-setup.service"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/etc/systemd/system/octessera-setup-request.path" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-setup-request.path"
rm -f \
    "$ROOTFS_DIR/usr/local/sbin/octessera-wifi-connect" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-sidecar" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-request" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-request-cleanup" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-start" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-setup-cleanup" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup-status.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup-status-cli.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/setup-call.py" \
    "$ROOTFS_DIR/etc/tmpfiles.d/octessera-setup-queue.conf" \
    "$ROOTFS_DIR/etc/systemd/system/octessera-setup-request.service" \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-setup-request.service"
rm -f \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/app.js" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/styles.css" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/octessera-mark.svg" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/octessera-wordmark.svg"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/index.html" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/index.html"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/js/app.js" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/js/app.js"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/css/styles.css" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/css/styles.css"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/README.md" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/README.md"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/img/octessera-mark.svg" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/img/octessera-mark.svg"
install -D -o root -g root -m 0644 \
    "$STAGE_FILES/root/usr/local/share/octessera-setup-ui/img/octessera-wordmark.svg" \
    "$ROOTFS_DIR/usr/local/share/octessera-setup-ui/img/octessera-wordmark.svg"
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
    "$STAGE_FILES/root/usr/local/lib/octessera/updater_contract.py" \
    "$ROOTFS_DIR/usr/local/lib/octessera/updater_contract.py"
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

rm -f \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/octessera-setup.service" \
    "$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/dnsmasq.service" \
    "$ROOTFS_DIR/etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service" \
    "$ROOTFS_DIR/etc/systemd/system/network-online.target.wants/NetworkManager-wait-online.service"
rm -f "$ROOTFS_DIR/etc/systemd/system/ssh.service" "$ROOTFS_DIR/etc/systemd/system/ssh.socket"
ln -s /dev/null "$ROOTFS_DIR/etc/systemd/system/ssh.service"
ln -s /dev/null "$ROOTFS_DIR/etc/systemd/system/ssh.socket"

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
bashrc="$ROOTFS_DIR$pi_home/.bashrc"
if [ -f "$bashrc" ] && [ ! -L "$bashrc" ]; then
    sed -i -E '/^[[:space:]]*(export[[:space:]]+)?(LANG|LANGUAGE|LC_[[:alnum:]_]+)[[:space:]]*=/d' "$bashrc"
fi
chroot "$ROOTFS_DIR" chown "$pi_user:$pi_user" "$pi_home/.hushlogin"
chroot "$ROOTFS_DIR" chown -R pi:pi /home/pi/samples /home/pi/presets
chroot "$ROOTFS_DIR" chmod 0644 /home/pi/presets/default.json
