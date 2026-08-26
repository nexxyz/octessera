#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
unit="$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service"
helper="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend"
customize="$root/userpatches/customize-image.sh"
runtime_assets="$root/userpatches/overlay/usr/local/lib/octessera/orange-runtime-assets-install.sh"

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
for required_text in \
  'prepare/release' 'prepare/commit' 'resume/release' 'resume/complete' 'rollback' \
  'SOCKET_RETRY_DELAYS' 'stage' \
  'MAX_MESSAGE_BYTES = 1024' 'first_menu_rendered' \
  'oled.command(0xA6)' 'oled.command(0xAF)' 'oled.close(False)' \
  'os.replace(temporary, STATE_PATH)'; do
  grep -qF "$required_text" "$helper" || { echo "Orange OLED suspend helper is missing: $required_text" >&2; exit 1; }
done
grep -qF 'SO_PEERCRED' "$root/apps/pi-zero/src/orange_oled_suspend.rs"
grep -qF 'OledOwnershipStage::Rollback' "$root/apps/pi-zero/src/orange_oled_suspend_policy.rs"
grep -qF 'completed_token' "$root/apps/pi-zero/src/orange_oled_suspend_policy.rs"
grep -qF 'select_snapshot_render' "$root/apps/pi-zero/src/render_loop.rs"
grep -qF 'retry_oled_decision' "$root/apps/pi-zero/src/render_loop.rs"
grep -qF 'retry_oled_if_due' "$root/apps/pi-zero/src/render_loop.rs"
grep -qF 'force_latest_frame' "$root/apps/pi-zero/src/render/oled_ownership.rs"
grep -qF 'detach_preserving' "$root/apps/pi-zero/src/boot_oled_handoff_unix.rs"
grep -qF 'FirstMenuRendered' "$root/apps/pi-zero/src/boot_oled_handoff_unix.rs"
grep -qF 'should_run_cleanup' "$root/crates/hal/src/orange_hardware.rs"
octessera_reject_file_match 'Orange OLED suspend helper contains an unrelated privilege or lifecycle fallback.' -qE 'systemctl|dbus|runuser|sudo|su ' "$helper"
[[ ! -e "$root/userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled" ]] || { echo 'Obsolete Orange system-sleep hook remains.' >&2; exit 1; }
octessera_reject_file_match 'Image installer still installs the obsolete sleep hook.' -qF 'system-sleep/octessera-orange-oled' "$customize"
grep -qF 'install_overlay_file usr/local/sbin/octessera-orange-oled-suspend' "$runtime_assets"
grep -qF 'systemctl enable octessera-orange-oled-suspend.service' "$runtime_assets"
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

printf '%s\n' 'Orange OLED suspend installer, strict helper, and systemd contract tests passed'
