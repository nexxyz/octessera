#!/usr/bin/env bash
# shellcheck disable=SC2251
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
service="$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-boot-splash.service"
runtime="$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service"
template="$root/tools/pi/provision/files/etc/systemd/system/octessera.service.template"

for required_line in \
    'Type=simple' \
    'User=pi' \
    'Group=pi' \
    'After=systemd-modules-load.service systemd-udevd.service systemd-udev-trigger.service' \
    'Before=sysinit.target octessera.service' \
    'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
    'RuntimeDirectory=octessera-boot' \
    'RuntimeDirectoryMode=0750' \
    'RuntimeDirectoryPreserve=yes' \
    'UMask=0027' \
    'KillMode=control-group' \
    'TimeoutStopSec=2' \
    'Restart=no' \
    'ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop' \
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
    'DeviceAllow=/dev/gpiomem rw'; do
    grep -qFx "$required_line" "$service"
done
! grep -q '^Type=oneshot$' "$service"
! grep -q '^ExecStart=-' "$service"
! grep -q '^Conflicts=' "$service"

for required_line in \
    'Wants=octessera-boot-splash.service' \
    'After=octessera-boot-splash.service' \
    'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1'; do
    grep -qFx "$required_line" "$runtime"
    grep -qFx "$required_line" "$template"
done
! grep -q '^Conflicts=' "$runtime"
! grep -q '^Conflicts=' "$template"
grep -qFx 'ExecStart=/usr/local/bin/octessera-pi' "$runtime"
grep -qFx 'ExecStart=/usr/local/bin/octessera-pi' "$template"
grep -qF '/etc/systemd/system/multi-user.target.wants/octessera-boot-splash.service' "$root/tools/pi/provision/provision.sh"
grep -qF 'systemctl enable octessera-boot-splash.service' "$root/tools/pi/provision/provision.sh"

fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
mkdir -p \
    "$fixture/etc/systemd/system/sysinit.target.wants" \
    "$fixture/etc/systemd/system/multi-user.target.wants"
cp "$service" "$fixture/etc/systemd/system/octessera-boot-splash.service"
cp "$runtime" "$fixture/etc/systemd/system/octessera.service"
chmod 0644 "$fixture/etc/systemd/system/octessera-boot-splash.service" "$fixture/etc/systemd/system/octessera.service"
ln -s ../octessera-boot-splash.service "$fixture/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service"
ln -s ../octessera.service "$fixture/etc/systemd/system/multi-user.target.wants/octessera.service"
# shellcheck disable=SC1091
source "$root/tools/pi-image/verify-boot-layout.sh"
if [ "$(id -u)" -eq 0 ]; then
    chown -R 0:0 "$fixture"
    require_octessera_boot_service_layout "$fixture"
    ln -s ../octessera-boot-splash.service "$fixture/etc/systemd/system/multi-user.target.wants/second-splash.service"
    if require_octessera_boot_service_layout "$fixture"; then
        echo 'dual early splash writers were accepted' >&2
        exit 1
    fi
    rm "$fixture/etc/systemd/system/multi-user.target.wants/second-splash.service"
else
    printf '%s\n' 'root ownership fixture skipped; static service ownership checks passed'
fi

if command -v systemd-analyze >/dev/null 2>&1; then
    systemd_root="$fixture/systemd-root"
    mkdir -p "$systemd_root/etc/systemd/system" "$systemd_root/home/pi" "$systemd_root/usr/local/bin"
    cp "$service" "$runtime" "$systemd_root/etc/systemd/system/"
    chmod 0644 "$systemd_root/etc/systemd/system/octessera-boot-splash.service" "$systemd_root/etc/systemd/system/octessera.service"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_root/usr/local/bin/octessera-pi"
    chmod 0755 "$systemd_root/usr/local/bin/octessera-pi"
    printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' 'pi:x:1000:1000:pi:/home/pi:/bin/sh' > "$systemd_root/etc/passwd"
    printf '%s\n' 'root:x:0:' 'pi:x:1000:' > "$systemd_root/etc/group"
    for unit in \
        sysinit.target \
        systemd-modules-load.service \
        systemd-udevd.service \
        systemd-udev-trigger.service \
        octessera-usb-gadget.service \
        octessera-update-recovery.service \
        sound.target; do
        if [[ "$unit" == *.service ]]; then
            printf '%s\n' '[Unit]' "Description=$unit" '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$systemd_root/etc/systemd/system/$unit"
        else
            printf '%s\n' '[Unit]' "Description=$unit" > "$systemd_root/etc/systemd/system/$unit"
        fi
    done
    systemd-analyze --root="$systemd_root" verify octessera-boot-splash.service octessera.service
else
    printf '%s\n' 'systemd-analyze unavailable; static Raspberry service graph checks passed'
fi

if [ "$(id -u)" -eq 0 ]; then
    original_service="$fixture/etc/systemd/system/octessera-boot-splash.service"
    for missing_line in \
        'After=systemd-modules-load.service systemd-udevd.service systemd-udev-trigger.service' \
        'DevicePolicy=closed' \
        'DeviceAllow=/dev/spidev0.0 rw' \
        'DeviceAllow=/dev/gpiomem rw'; do
        grep -vFx "$missing_line" "$service" > "$original_service"
        chmod 0644 "$original_service"
        chown 0:0 "$original_service"
        if require_octessera_boot_service_layout "$fixture"; then
            echo "boot service accepted fixture missing $missing_line" >&2
            exit 1
        fi
        cp "$service" "$original_service"
        chmod 0644 "$original_service"
        chown 0:0 "$original_service"
    done
fi

printf '%s\n' 'Raspberry systemd service, ordering, ownership, and dual-writer tests passed'
