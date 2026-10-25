#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
unit="$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service"
helper="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend"
customize="$root/userpatches/customize-image.sh"

[[ -f "$unit" && -f "$helper" ]] || { echo 'Orange OLED suspend sources are missing.' >&2; exit 1; }
for required_line in \
  'After=octessera.service' \
  'Requisite=octessera.service' \
  'Before=sleep.target' \
  'StopWhenUnneeded=yes' \
  'Type=oneshot' \
  'RemainAfterExit=yes' \
  'RequiredBy=sleep.target' \
  'User=octessera-runtime' \
  'RuntimeDirectory=octessera-oled-suspend' \
  'RuntimeDirectoryMode=0700' \
  'RestrictAddressFamilies=AF_UNIX' \
  'DevicePolicy=closed' \
  'DeviceAllow=/dev/spidev1.0 rw' \
  'DeviceAllow=/dev/gpiochip1 rw' \
  'ExecStart=/usr/local/sbin/octessera-orange-oled-suspend prepare' \
  'ExecStop=/usr/local/sbin/octessera-orange-oled-suspend resume' \
  'TimeoutStartSec=8' \
  'TimeoutStopSec=8'; do
  grep -qFx "$required_line" "$unit" || { echo "Orange OLED suspend unit is missing: $required_line" >&2; exit 1; }
done
octessera_reject_file_match 'Orange OLED suspend unit must not use a non-required sleep target dependency.' -qFx 'WantedBy=sleep.target' "$unit"
octessera_reject_file_match 'Orange OLED suspend unit must not use the wants directory.' -qF 'sleep.target.wants' "$unit"
octessera_reject_file_match 'Orange OLED suspend unit must not require named supplementary groups.' -qFx 'SupplementaryGroups=audio i2c spi gpio' "$unit"
octessera_reject_file_match 'Orange OLED suspend unit has a forbidden dependency.' -qE '^(Conflicts=|Requires=|BusName=)|systemctl|dbus' "$unit"
octessera_reject_file_match 'Orange OLED suspend helper contains an unrelated privilege or lifecycle fallback.' -qE 'systemctl|dbus|runuser|sudo|su ' "$helper"
octessera_reject_file_match 'Image installer must not create a soft sleep target dependency.' -qF 'sleep.target.wants' "$customize"

if command -v systemd-analyze >/dev/null 2>&1; then
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  mkdir -p "$work/etc/systemd/system" "$work/usr/local/sbin"
  cp "$unit" "$work/etc/systemd/system/octessera-orange-oled-suspend.service"
  chmod 0644 "$work/etc/systemd/system/octessera-orange-oled-suspend.service"
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$work/usr/local/sbin/octessera-orange-oled-suspend"
  chmod 0755 "$work/usr/local/sbin/octessera-orange-oled-suspend"
  for target in sleep.target sysinit.target basic.target local-fs.target; do
    printf '%s\n' '[Unit]' "Description=$target" > "$work/etc/systemd/system/$target"
  done
  printf '%s\n' '[Unit]' 'Description=runtime' '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$work/etc/systemd/system/octessera.service"
  mkdir -p "$work/bin"
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$work/bin/true"
  chmod 0755 "$work/bin/true"
  systemd-analyze --root="$work" verify octessera-orange-oled-suspend.service
  systemctl --root="$work" enable octessera-orange-oled-suspend.service >/dev/null
  [[ -L "$work/etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service" ]] || { echo 'RequiredBy=sleep.target did not create the required symlink.' >&2; exit 1; }
  [[ ! -e "$work/etc/systemd/system/sleep.target.wants/octessera-orange-oled-suspend.service" ]] || { echo 'RequiredBy=sleep.target created a stale wants symlink.' >&2; exit 1; }
fi

printf '%s\n' 'Orange OLED suspend unit, helper privilege, and systemd contract tests passed'
