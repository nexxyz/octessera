#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
service="$root/userpatches/overlay/etc/systemd/system/octessera.service"
udev_rule="$root/userpatches/overlay/etc/udev/rules.d/70-octessera-orange-runtime.rules"

[[ -f "$service" ]] || { echo "Missing Orange runtime service." >&2; exit 1; }
for required_line in \
  'User=octessera-runtime' \
  'Group=octessera-runtime' \
  'LimitRTPRIO=70' \
  'NoNewPrivileges=yes' \
  'ProtectSystem=strict' \
  'ReadWritePaths=/var/lib/octessera /run/octessera' \
  'PrivateTmp=yes' \
  'ProtectHome=yes'; do
  grep -qFx "$required_line" "$service" || { echo "Runtime service is missing: $required_line" >&2; exit 1; }
done
if grep -Eq '^(AmbientCapabilities|CapabilityBoundingSet)=|LimitRTPRIO=80' "$service"; then
  echo 'Runtime service grants ambient SYS_NICE or priority 80.' >&2
  exit 1
fi
expected_udev_rule=$'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"'
[[ "$(cat -- "$udev_rule")" == "$expected_udev_rule" ]] || { echo 'Orange runtime udev rule content is not exact.' >&2; exit 1; }
profile_gpio_label="$(sed -n 's/^GPIO_LABEL = "\(.*\)"/\1/p' "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo")"
profile_gpiochip=gpiochip1
[[ "$profile_gpio_label" == 300b000.pinctrl && "$profile_gpiochip" == gpiochip1 ]] || { echo 'Pinned Orange GPIO profile mapping changed.' >&2; exit 1; }
grep -qFx "KERNEL==\"$profile_gpiochip\", GROUP=\"octessera-runtime\", MODE=\"0660\"" "$udev_rule" || { echo 'Orange udev rule does not match the pinned GPIO profile mapping.' >&2; exit 1; }
if command -v udevadm >/dev/null 2>&1 && getent group octessera-runtime >/dev/null 2>&1 && udevadm help 2>&1 | grep -qE '(^|[[:space:]])verify([[:space:]]|$)'; then
  udevadm verify "$udev_rule"
else
  echo 'udevadm rule verification skipped; target runtime group is unavailable in this host.'
fi
if ! command -v systemd-analyze >/dev/null 2>&1; then
  echo 'systemd-analyze unavailable; static runtime service checks passed.'
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/etc/systemd/system" "$work/usr/local/bin" "$work/etc"
cp "$service" "$work/etc/systemd/system/octessera.service"
chmod 0644 "$work/etc/systemd/system/octessera.service"
printf '%s\n' '#!/bin/sh' 'exit 0' > "$work/usr/local/bin/octessera-pi"
chmod 0755 "$work/usr/local/bin/octessera-pi"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/sh' \
  'octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin' > "$work/etc/passwd"
printf '%s\n' 'root:x:0:' 'octessera-runtime:x:990:' > "$work/etc/group"
for unit in octessera-provision-musical-default.service octessera-orange-usb-gadget.service; do
  printf '%s\n' '[Unit]' "Description=$unit" '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$work/etc/systemd/system/$unit"
done
for unit in sysinit.target basic.target sound.target multi-user.target local-fs.target; do
  printf '%s\n' '[Unit]' "Description=$unit" > "$work/etc/systemd/system/$unit"
done
systemd-analyze --root="$work" verify octessera.service
printf '%s\n' 'Orange runtime service static and systemd checks passed'
