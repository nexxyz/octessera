#!/usr/bin/env bash
# shellcheck disable=SC2251
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
service="$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-boot-splash.service"
runtime="$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service"
setup_stage="$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
sanitize_stage="$root/tools/pi-image/stage4-octessera/04-sanitize-release-image/00-run.sh"
usb_role_helper="$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-usb-role"
usb_role_sudoers="$root/tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-usb-role"
template="$root/tools/pi/provision/files/etc/systemd/system/octessera.service.template"
shutdown_sudoers="$root/tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-shutdown"
shutdown_policy='pi ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /bin/systemctl poweroff, /usr/sbin/poweroff, /sbin/poweroff, /usr/bin/systemctl reboot, /bin/systemctl reboot, /usr/sbin/reboot, /sbin/reboot'

grep -qF "rm -f \"\$ROOTFS_DIR/etc/systemd/system/ssh.service\" \"\$ROOTFS_DIR/etc/systemd/system/ssh.socket\"" "$setup_stage"
if grep -Eq "ln -s /dev/null \"\$ROOTFS_DIR/etc/systemd/system/ssh\\.(service|socket)\"" "$setup_stage"; then
    echo 'Raspberry setup stage must not mask SSH units.' >&2
    exit 1
fi
grep -qF "rm -f \"\$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/ssh.service\"" "$sanitize_stage"
grep -qF "rm -f \"\$ROOTFS_DIR/etc/systemd/system/sockets.target.wants/ssh.socket\"" "$sanitize_stage"

for required_line in \
    'User=pi' \
    'Group=pi' \
    'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
    'RuntimeDirectoryMode=0750' \
    'UMask=0027' \
    'NoNewPrivileges=yes' \
    'ProtectSystem=strict' \
    'ReadWritePaths=/run/octessera-boot' \
    'PrivateTmp=yes' \
    'ProtectHome=yes' \
    'ProtectKernelTunables=yes' \
    'ProtectKernelModules=yes' \
    'ProtectControlGroups=yes' \
    'ProtectKernelLogs=yes' \
    'RestrictNamespaces=yes' \
    'LockPersonality=yes' \
    'RestrictAddressFamilies=AF_UNIX' \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev0.0 rw' \
    'DeviceAllow=/dev/gpiomem rw' \
    'DeviceAllow=/dev/gpiochip0 rw'; do
    grep -qFx "$required_line" "$service"
done
test -f "$usb_role_helper"
printf '%s\n' '# Exact Raspberry USB data-role commands only.' 'pi ALL=(root) NOPASSWD: /usr/bin/systemctl start octessera-usb-role@host.service, /usr/bin/systemctl start octessera-usb-role@gadget.service' | cmp -s - "$usb_role_sudoers"
if grep -Eiq '^[[:space:]]*[^#]*\bNOPASSWD[[:space:]]*:[[:space:]]*ALL([[:space:]]|$)' "$usb_role_sudoers"; then
    echo 'Raspberry USB role sudoers rule is broader than the exact role commands.' >&2
    exit 1
fi
printf '%s\n' 'pi ALL=(root) NOPASSWD: /usr/local/sbin/octessera-usb-gadget storage-start, /usr/local/sbin/octessera-usb-gadget storage-stop, /usr/bin/systemctl restart octessera-usb-gadget.service' | cmp -s - "$root/tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-usb-storage"

for required_line in \
    'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
    'NoNewPrivileges=no' \
    'SupplementaryGroups=tty input' \
    'AmbientCapabilities=CAP_SYS_NICE CAP_SYS_TTY_CONFIG' \
    'CapabilityBoundingSet=CAP_SYS_NICE CAP_SETUID CAP_SETGID CAP_SYS_TTY_CONFIG'; do
    grep -qFx "$required_line" "$runtime"
    grep -qFx "$required_line" "$template"
done
printf '%s\n' '# Exact fixed-board power commands only.' "$shutdown_policy" | cmp -s - "$shutdown_sudoers"
if grep -Eiq '^[[:space:]]*[^#]*\bNOPASSWD[[:space:]]*:[[:space:]]*ALL([[:space:]]|$)' "$shutdown_sudoers"; then
    echo 'Raspberry shutdown sudoers rule is broader than the exact power commands.' >&2
    exit 1
fi
for required_line in \
    'AmbientCapabilities=CAP_SYS_NICE CAP_SYS_TTY_CONFIG' \
    'CapabilityBoundingSet=CAP_SYS_NICE CAP_SETUID CAP_SETGID CAP_SYS_TTY_CONFIG'; do
    grep -qFx "$required_line" "$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service.d/audio-realtime.conf"
done
for unit in "$runtime" "$template" "$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service.d/audio-realtime.conf"; do
    [[ "$(grep -Ec '^(AmbientCapabilities|CapabilityBoundingSet)=' "$unit")" == 2 ]]
    if [[ "$unit" != *audio-realtime.conf ]]; then
        [[ "$(grep -c "^SupplementaryGroups=" "$unit")" == 1 ]]
    fi
    octessera_reject_file_match 'Raspberry runtime contains a prohibited tty, device, graphics, or forced-mode directive.' -Eiq '^(StandardInput=tty|TTY(VHangup|VTDisallocate|Force|Fail)=|ExecStopPost=|DevicePolicy=|DeviceAllow=)|(^|[^[:alnum:]_])(Xorg|Wayland|Weston|sway|chvt|xrandr|wlr-randr|modetest|video=)([^[:alnum:]_]|$)' "$unit"
done
octessera_reject_file_match 'Raspberry constructor must not force a graphics mode or switch virtual terminals.' -Eiq 'chvt|xrandr|wlr-randr|modetest|video=|StandardInput=tty' "$root/tools/pi-image/stage4-octessera/03-boot-config/00-run.sh"

fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

if command -v systemd-analyze >/dev/null 2>&1; then
    systemd_root="$fixture/systemd-root"
    mkdir -p "$systemd_root/etc/systemd/system" "$systemd_root/home/pi" "$systemd_root/usr/local/bin"
    cp "$service" "$runtime" "$systemd_root/etc/systemd/system/"
    chmod 0644 "$systemd_root/etc/systemd/system/octessera-boot-splash.service" "$systemd_root/etc/systemd/system/octessera.service"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_root/usr/local/bin/octessera-pi"
    chmod 0755 "$systemd_root/usr/local/bin/octessera-pi"
    printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' 'pi:x:1000:1000:pi:/home/pi:/bin/sh' > "$systemd_root/etc/passwd"
    printf '%s\n' 'root:x:0:' 'pi:x:1000:' 'tty:x:5:' 'input:x:104:' > "$systemd_root/etc/group"
    for unit in \
        sysinit.target \
        systemd-modules-load.service \
        systemd-udevd.service \
        systemd-udev-trigger.service \
        systemd-udev-settle.service \
        octessera-usb-gadget.service \
        sound.target; do
        if [[ "$unit" == *.service ]]; then
            printf '%s\n' '[Unit]' "Description=$unit" '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$systemd_root/etc/systemd/system/$unit"
        else
            printf '%s\n' '[Unit]' "Description=$unit" > "$systemd_root/etc/systemd/system/$unit"
        fi
    done
    cat > "$systemd_root/etc/systemd/system/systemd-udev-settle.service" <<'EOF'
[Unit]
Description=Fixture udev settle
DefaultDependencies=no
Before=sysinit.target

[Service]
Type=oneshot
ExecStart=/usr/bin/udevadm settle
EOF
    if ! positive_output="$(systemd-analyze --root="$systemd_root" verify octessera-boot-splash.service octessera.service 2>&1)"; then
        printf '%s\n' "$positive_output" >&2
        echo 'systemd verify rejected the positive Raspberry service graph' >&2
        exit 1
    fi
    if printf '%s\n' "$positive_output" | grep -Eiq 'ordering cycle|Found ordering cycle|job deletion|job .*deleted|deleted .*job'; then
        printf '%s\n' "$positive_output" >&2
        echo 'systemd verify reported an ordering cycle or job deletion for the positive Raspberry service graph' >&2
        exit 1
    fi
    cat > "$systemd_root/etc/systemd/system/systemd-udev-settle.service" <<'EOF'
[Unit]
Description=Fixture cyclic udev settle
Before=sysinit.target
After=octessera-boot-splash.service

[Service]
Type=oneshot
ExecStart=/usr/bin/udevadm settle
EOF
    cycle_output="$(systemd-analyze --root="$systemd_root" verify octessera-boot-splash.service octessera.service 2>&1 || true)"
    if ! printf '%s\n' "$cycle_output" | grep -qF 'Found ordering cycle'; then
        echo 'systemd verify accepted a udev-settle cycle' >&2
        exit 1
    fi
else
    printf '%s\n' 'systemd-analyze unavailable; static Raspberry service graph checks passed'
fi

printf '%s\n' 'Raspberry systemd service and ordering tests passed'
