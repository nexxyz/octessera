#!/usr/bin/env bash
# shellcheck disable=SC2154

octessera_require_wifi_foundation() {
  local helper_path=usr/local/sbin/octessera-wifi-foundation
  local unit_path=etc/systemd/system/octessera-wifi-foundation.service
  local binary_path=usr/local/bin/wifi-connect helper_content unit_content path
  for path in "$helper_path" "$unit_path" "$binary_path"; do
    stat_path "$path" || { echo "Missing inactive Wi-Fi foundation path: $path." >&2; exit 1; }
  done
  require_root_mode "$helper_path" 755
  require_root_mode "$unit_path" 644
  require_root_mode "$binary_path" 755
  helper_content="$(read_file "$helper_path")"
  unit_content="$(read_file "$unit_path")"
  printf '%s\n' "$helper_content" | grep -qF -- '--portal-interface wlan0'
  printf '%s\n' "$helper_content" | grep -qF -- '--portal-gateway 192.168.42.1'
  printf '%s\n' "$helper_content" | grep -qF -- '900s'
  printf '%s\n' "$unit_content" | grep -qFx 'User=root'
  printf '%s\n' "$unit_content" | grep -qFx 'Group=root'
  printf '%s\n' "$unit_content" | grep -qFx 'ExecStart=/usr/local/sbin/octessera-wifi-foundation'
  printf '%s\n' "$unit_content" | grep -qFx 'TimeoutStartSec=905s'
  if printf '%s\n' "$helper_content" "$unit_content" | grep -Eiq 'sidecar|hostname|ssh|password|country|setup[-_ ]?(complete|force)|credential|secret|/sys/class/net|iw[[:space:]]+dev|nmcli.*device|mac|wpa_passphrase|chpasswd|ssid=|psk=|BEGIN (RSA|OPENSSH|PRIVATE) KEY'; then
    echo 'Wi-Fi foundation contains forbidden behavior or secret handling.' >&2
    exit 1
  fi
}
