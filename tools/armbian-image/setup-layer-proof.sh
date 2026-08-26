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
  local coordinator=usr/local/sbin/octessera-setup
  local config=usr/local/lib/octessera/setup_config.py
  local http=usr/local/lib/octessera/setup_http.py
  local setup_unit=etc/systemd/system/octessera-setup.service
  local request_path=etc/systemd/system/octessera-setup-request.path
  local tmpfiles=etc/tmpfiles.d/octessera-setup-request.conf
  for path in "$profile_file" "$coordinator" "$config" "$http" "$setup_unit" "$request_path" "$tmpfiles"; do
    stat_path "$path" || { echo "Missing setup layer path: $path." >&2; exit 1; }
  done
  require_root_mode "$profile_file" 644
  require_root_mode "$coordinator" 755
  require_root_mode "$config" 644
  require_root_mode "$http" 644
  require_root_mode "$setup_unit" 644
  require_root_mode "$request_path" 644
  require_root_mode "$tmpfiles" 644
  [[ "$(read_file "$profile_file")" == "orange-pi-zero-2w" ]] || { echo "Orange setup profile is not fixed." >&2; exit 1; }
  grep -qFx 'd /run/octessera-setup-request 0711 root root -' <(read_file "$tmpfiles") || { echo "Setup request directory is not exact." >&2; exit 1; }
  grep -qFx 'd /run/octessera-setup-request/inbox 0700 octessera-runtime octessera-runtime -' <(read_file "$tmpfiles") || { echo "Setup request inbox is not exact." >&2; exit 1; }
  grep -qFx 'PathExists=/run/octessera-setup-request/inbox/start' <(read_file "$request_path") || { echo "Setup marker path is not exact." >&2; exit 1; }
  grep -qFx 'Unit=octessera-setup.service' <(read_file "$request_path") || { echo "Setup path activation target is not exact." >&2; exit 1; }
  grep -qFx 'ExecStart=/usr/local/sbin/octessera-setup' <(read_file "$setup_unit") || { echo "Setup coordinator is not direct." >&2; exit 1; }
  grep -qFx 'RuntimeMaxSec=670s' <(read_file "$setup_unit") || { echo "Setup outer runtime limit is not exact." >&2; exit 1; }
  grep -qFx 'TimeoutStopSec=10s' <(read_file "$setup_unit") || { echo "Setup outer stop limit is not exact." >&2; exit 1; }
  grep -qF '/run/octessera-setup-status' <(read_file "$setup_unit") || { echo "Setup status path is not writable." >&2; exit 1; }
  grep -qF 'PORTAL_WINDOW_SECONDS = 600' <(read_file "$coordinator") || { echo "Portal window is not exact." >&2; exit 1; }
  grep -qF 'INTERNAL_APPLY_SECONDS = 60' <(read_file "$coordinator") || { echo "Internal apply window is not exact." >&2; exit 1; }
  grep -qF 'def write_status' <(read_file "$coordinator") || { echo "Atomic status writer is missing." >&2; exit 1; }
  grep -qF 'def consume_request' <(read_file "$coordinator") || { echo "Request marker consumer is missing." >&2; exit 1; }
  grep -qF 'def cleanup_profiles' <(read_file "$coordinator") || { echo "Exact AP cleanup is missing." >&2; exit 1; }
  grep -qF 'setup_config.finalize' <(read_file "$coordinator") || { echo "Finalization domain call is missing." >&2; exit 1; }
  grep -qF 'import setup_http' <(read_file "$coordinator") || { echo "HTTP setup module import is missing." >&2; exit 1; }
  grep -qF 'class SetupHandler' <(read_file "$http") || { echo "HTTP setup handler is missing." >&2; exit 1; }
  grep -qF 'class SetupHTTPServer' <(read_file "$http") || { echo "HTTP setup server is missing." >&2; exit 1; }
  for command in usr/local/bin/wifi-connect usr/bin/python3 usr/sbin/iw usr/bin/nmcli usr/sbin/ip usr/bin/ss; do
    stat_path "$command" || { echo "Missing setup prerequisite command: $command." >&2; exit 1; }
  done
  if printf '%s\n' "$(read_file "$coordinator")" "$(read_file "$config")" "$(read_file "$http")" | grep -Eiq 'setup-status\.py|setup-status-cli\.py|setup-call\.py|sidecar|receipt|active\.json|sequence|attemptId|requestToken|replay|retry|nonce|systemctl.*restart|print\('; then
    echo "Removed setup orchestration or unsafe logging remains." >&2
    exit 1
  fi
  for path in \
    var/lib/octessera/setup-force \
    var/lib/octessera/setup-finalize-failed \
    run/octessera/setup-portal.request \
    run/octessera-setup-request/inbox/start \
    run/octessera-setup-queue \
    run/octessera-setup-control \
    run/octessera-setup; do
    require_absent_setup_path "$path"
  done
  octessera_require_image_symlink etc/systemd/system/multi-user.target.wants/octessera-setup-request.path ../octessera-setup-request.path
  require_absent_setup_path etc/systemd/system/multi-user.target.wants/octessera-setup.service
}

require_orange_constructor_policy() {
  require_absent_setup_path etc/systemd/system/multi-user.target.wants/octessera-setup.service
  require_absent_setup_path etc/systemd/system/multi-user.target.wants/dnsmasq.service
  require_absent_setup_path etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service
  require_absent_setup_path etc/systemd/system/network-online.target.wants/NetworkManager-wait-online.service
}
