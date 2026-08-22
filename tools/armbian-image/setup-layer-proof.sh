#!/usr/bin/env bash

require_absent_setup_path() {
  local path="$1" status
  if stat_path "$path"; then
    echo "Orange setup path must be absent: $path." >&2
    exit 1
  else
    status=$?
  fi
  [[ "$status" == 1 ]] || { echo "Unable to inspect Orange setup path: $path." >&2; exit 1; }
}

require_setup_layer() {
  local profile_file=etc/octessera/setup-profile
  local sidecar=usr/local/sbin/octessera-setup-sidecar
  local wrapper=usr/local/sbin/octessera-wifi-connect
  local request_helper=usr/local/sbin/octessera-setup-request
  local request_cleanup=usr/local/sbin/octessera-setup-request-cleanup
  local start_helper=usr/local/sbin/octessera-setup-start
  local cleanup_helper=usr/local/sbin/octessera-setup-cleanup
  local status_tool=usr/local/lib/octessera/setup-status.py
  local status_cli=usr/local/lib/octessera/setup-status-cli.py
  local call_tool=usr/local/lib/octessera/setup-call.py
  local setup_unit=etc/systemd/system/octessera-setup.service
  local request_path=etc/systemd/system/octessera-setup-request.path
  local request_unit=etc/systemd/system/octessera-setup-request.service
  for path in "$profile_file" "$sidecar" "$wrapper" "$request_helper" "$request_cleanup" "$start_helper" "$cleanup_helper" "$status_tool" "$status_cli" "$call_tool" "$setup_unit" "$request_path" "$request_unit"; do
    stat_path "$path" || { echo "Missing setup layer path: $path." >&2; exit 1; }
  done
  require_root_mode "$profile_file" 644
  require_root_mode "$sidecar" 755
  require_root_mode "$wrapper" 755
  require_root_mode "$request_helper" 755
  require_root_mode "$request_cleanup" 755
  require_root_mode "$start_helper" 755
  require_root_mode "$cleanup_helper" 755
  require_root_mode "$status_tool" 755
  require_root_mode "$status_cli" 644
  require_root_mode "$call_tool" 755
  require_root_mode "$setup_unit" 644
  require_root_mode "$request_path" 644
  require_root_mode "$request_unit" 644
  [[ "$(read_file "$profile_file")" == "orange-pi-zero-2w" ]] || { echo "Orange setup profile is not fixed." >&2; exit 1; }
  grep -qF 'ALLOWED_ORIGINS = frozenset(("http://192.168.42.1", "http://192.168.42.1:80"))' <(read_file "$sidecar") || { echo "Setup origins are not exact." >&2; exit 1; }
  grep -qF 'ipaddress.ip_network("192.168.42.0/24")' <(read_file "$sidecar") || { echo "Setup client network is not exact." >&2; exit 1; }
  grep -qF 'PUBLIC_DIR = "/run/octessera-setup-status"' <(read_file "$status_tool") || { echo "Setup public status path is not fixed." >&2; exit 1; }
  grep -qF 'RECEIPT_DIR' <(read_file "$status_tool") || { echo "Setup receipts are not staged." >&2; exit 1; }
  grep -q '^octessera-runtime:' <(read_file etc/passwd) || { echo "Orange setup runtime account is not staged." >&2; exit 1; }
  grep -q '^octessera-runtime:' <(read_file etc/group) || { echo "Orange setup runtime group is not staged." >&2; exit 1; }
  grep -qF 'MAX_BODY = 16384' <(read_file "$sidecar") || { echo "Setup body limit is not fixed." >&2; exit 1; }
  grep -qF 'Transfer-Encoding' <(read_file "$sidecar") || { echo "Setup transfer encoding is not rejected." >&2; exit 1; }
  grep -qF 'content_type != "application/json"' <(read_file "$sidecar") || { echo "Setup content type is not fixed." >&2; exit 1; }
  grep -qF 'interface=wlan0' <(read_file "$wrapper") || { echo "Setup wrapper interface is not fixed." >&2; exit 1; }
  grep -qF "/sys/class/net/\$interface/address" <(read_file "$wrapper") || { echo "Setup wrapper MAC path is not fixed." >&2; exit 1; }
  grep -qF 'PathExists=/run/octessera/setup-portal.request' <(read_file "$request_path") || { echo "Setup request path watches the wrong path." >&2; exit 1; }
  grep -qF 'RuntimeDirectory=octessera-setup' <(read_file "$setup_unit") || { echo "Setup runtime directory is not exact." >&2; exit 1; }
  grep -qF ' -/run/octessera-setup-control -/run/octessera-setup-status' <(read_file "$setup_unit") || { echo "Optional setup status paths are not namespace-safe." >&2; exit 1; }
  grep -qF 'RuntimeDirectoryMode=0700' <(read_file "$setup_unit") || { echo "Setup nonce runtime directory is not private." >&2; exit 1; }
  grep -qF 'RuntimeMaxSec=1800s' <(read_file "$setup_unit") || { echo "Setup runtime timeout is not fixed." >&2; exit 1; }
  if printf '%s\n' "$(read_file "$sidecar")" "$(read_file "$wrapper")" "$(read_file "$request_helper")" | grep -Eiq 'OCTESSERA_SETUP|setup-force|BEGIN (RSA|OPENSSH|PRIVATE) KEY|wpa_passphrase|ssid=|psk='; then
    echo "Setup layer contains secret, connection, or persistent-force material." >&2
    exit 1
  fi
  for path in \
    var/lib/octessera/setup-complete \
    var/lib/octessera/setup-force \
    var/lib/octessera/setup-finalize-failed \
    run/octessera/setup-portal.request \
    run/octessera-setup/nonce \
    run/octessera-setup-control \
    run/octessera-setup-status; do
    reject_path "$path"
  done
  octessera_require_image_symlink etc/systemd/system/multi-user.target.wants/octessera-setup-request.path ../octessera-setup-request.path
  require_absent_setup_path etc/systemd/system/multi-user.target.wants/octessera-setup.service
}

require_orange_constructor_policy() {
  local profile_file=etc/octessera/setup-profile
  local locale_file=etc/default/locale
  [[ "$(read_file "$profile_file")" == "orange-pi-zero-2w" ]] || { echo "Orange setup profile is not fixed." >&2; exit 1; }
  require_root_mode "$locale_file" 644
  [[ "$(read_file "$locale_file")" == $'LANG=C.UTF-8\nLANGUAGE=en\nLC_MESSAGES=C.UTF-8' ]] || { echo "Orange default locale is not exact." >&2; exit 1; }
  if stat_path home/octessera/.bashrc && grep -Eq '^[[:space:]]*(export[[:space:]]+)?(LANG|LANGUAGE|LC_[[:alnum:]_]+)[[:space:]]*=' <(read_file home/octessera/.bashrc); then
    echo "Orange appliance user profile overrides the default locale." >&2
    exit 1
  fi
  require_absent_setup_path etc/systemd/system/multi-user.target.wants/dnsmasq.service
  require_absent_setup_path etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service
  require_absent_setup_path etc/systemd/system/network-online.target.wants/NetworkManager-wait-online.service
  octessera_require_image_symlink etc/systemd/system/ssh.service /dev/null
  octessera_require_image_symlink etc/systemd/system/ssh.socket /dev/null
  require_absent_setup_path etc/systemd/system/multi-user.target.wants/ssh.service
  require_absent_setup_path etc/systemd/system/sockets.target.wants/ssh.socket
  octessera_require_image_symlink etc/systemd/system/serial-getty@ttyS0.service /dev/null
  for unit in NetworkManager.service dnsmasq.service systemd-networkd-wait-online.service NetworkManager-wait-online.service; do
    if ! stat_path "etc/systemd/system/$unit" && ! stat_path "lib/systemd/system/$unit" && ! stat_path "usr/lib/systemd/system/$unit"; then
      echo "Missing required systemd unit: $unit." >&2
      exit 1
    fi
  done
}
