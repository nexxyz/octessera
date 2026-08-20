#!/usr/bin/env bash
# shellcheck disable=SC2154
module_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$module_dir/validation-assertions.sh"

octessera_require_orange_boot_service() {
  local service_content line
  service_content="$(read_file etc/systemd/system/octessera-orange-boot-splash.service)"
  for line in 'User=octessera-runtime' 'Group=octessera-runtime' 'ExecStart=/usr/local/sbin/octessera-orange-oled-logo boot-loop' 'RuntimeDirectory=octessera-boot' 'RuntimeDirectoryMode=0750' 'RuntimeDirectoryPreserve=yes' 'ProtectSystem=strict' 'DevicePolicy=closed' 'DeviceAllow=/dev/spidev1.0 rw' 'DeviceAllow=/dev/gpiochip1 rw' 'After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target'; do printf '%s\n' "$service_content" | grep -qFx "$line" || { echo "Orange boot service is missing: $line" >&2; exit 1; }; done
  octessera_reject_text_match 'Orange boot splash must not conflict with runtime.' "$service_content" -q '^Conflicts='
  require_root_mode etc/systemd/system/octessera-orange-boot-splash.service 644
  octessera_require_image_symlink etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service ../octessera-orange-boot-splash.service /etc/systemd/system/octessera-orange-boot-splash.service
}

octessera_require_orange_shutdown_service() {
  local service_content line
  service_content="$(read_file etc/systemd/system/octessera-orange-oled-shutdown.service)"
  for line in 'Type=oneshot' 'User=octessera-runtime' 'Group=octessera-runtime' 'ProtectSystem=strict' 'ReadWritePaths=/run/octessera-boot' 'DevicePolicy=closed' 'DeviceAllow=/dev/spidev1.0 rw' 'DeviceAllow=/dev/gpiochip1 rw' 'ExecStart=/bin/true' "ExecStop=/bin/sh -c 'sleep 4; /usr/local/sbin/octessera-orange-oled-logo off || true'" 'RemainAfterExit=yes' 'TimeoutStopSec=8'; do printf '%s\n' "$service_content" | grep -qFx "$line" || { echo "Orange shutdown service is missing: $line" >&2; exit 1; }; done
  octessera_reject_text_match 'Orange shutdown service must not use target choreography or write a logo.' "$service_content" -qE '^(Before=|WantedBy=shutdown\.target|WantedBy=reboot\.target|WantedBy=halt\.target)|orange-oled-logo (shutdown|boot)'
  require_root_mode etc/systemd/system/octessera-orange-oled-shutdown.service 644
}

octessera_require_orange_suspend_service() {
  local service_content line
  service_content="$(read_file etc/systemd/system/octessera-orange-oled-suspend.service)"
  for line in 'After=octessera.service' 'Requisite=octessera.service' 'Before=sleep.target' 'RequiredBy=sleep.target' 'StopWhenUnneeded=yes' 'Type=oneshot' 'RemainAfterExit=yes' 'User=octessera-runtime' 'Group=octessera-runtime' 'RuntimeDirectory=octessera-oled-suspend' 'RuntimeDirectoryMode=0700' 'RestrictAddressFamilies=AF_UNIX' 'ExecStart=/usr/local/sbin/octessera-orange-oled-suspend prepare' 'ExecStop=/usr/local/sbin/octessera-orange-oled-suspend resume' 'TimeoutStartSec=8' 'TimeoutStopSec=8'; do printf '%s\n' "$service_content" | grep -qFx "$line" || { echo "Orange suspend service is missing: $line" >&2; exit 1; }; done
  octessera_reject_text_match 'Orange suspend service must not require named supplementary groups.' "$service_content" -qFx 'SupplementaryGroups=audio i2c spi gpio'
  octessera_reject_text_match 'Orange suspend service contains a forbidden lifecycle dependency.' "$service_content" -qE '^(Conflicts=|BusName=)|systemctl'
  octessera_require_image_symlink etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service ../octessera-orange-oled-suspend.service /etc/systemd/system/octessera-orange-oled-suspend.service
  reject_path etc/systemd/system/sleep.target.wants/octessera-orange-oled-suspend.service
  reject_path lib/systemd/system-sleep/octessera-orange-oled
  reject_path usr/lib/systemd/system-sleep/octessera-orange-oled
}
