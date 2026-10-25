#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
orange="$root/userpatches/overlay"
raspberry="$root/tools/pi-image/stage4-octessera/files/root"

check_common() {
  local tree="$1" owner="$2" profile="$3"
  local coordinator="$tree/usr/local/sbin/octessera-setup"
  local config="$tree/usr/local/lib/octessera/setup_config.py"
  local http="$tree/usr/local/lib/octessera/setup_http.py"
  local tmpfiles="$tree/etc/tmpfiles.d/octessera-setup-request.conf"
  local setup_unit="$tree/etc/systemd/system/octessera-setup.service"
  local path_unit="$tree/etc/systemd/system/octessera-setup-request.path"
  for required in "$coordinator" "$config" "$http" "$tmpfiles" "$setup_unit" "$path_unit" "$tree/etc/octessera/setup-profile" "$tree/usr/local/share/octessera-setup-ui/index.html" "$tree/usr/local/share/octessera-setup-ui/js/app.js" "$tree/usr/local/share/octessera-setup-ui/css/styles.css"; do
    [[ -f "$required" && ! -L "$required" ]] || { echo "Missing setup source: $required" >&2; exit 1; }
  done
  grep -qxF "$profile" "$tree/etc/octessera/setup-profile"
  grep -qxF "d /run/octessera-setup-request 0711 root root -" "$tmpfiles"
  grep -qxF "d /run/octessera-setup-request/inbox 0700 $owner $owner -" "$tmpfiles"
  grep -qxF "d /run/octessera-setup-status 0750 root $owner -" "$tmpfiles"
  [[ "$(wc -l < "$tmpfiles")" == 3 ]]
  grep -qFx 'After=systemd-tmpfiles-setup.service' "$path_unit"
  grep -qFx 'PathExists=/run/octessera-setup-request/inbox/start' "$path_unit"
  grep -qFx 'Unit=octessera-setup.service' "$path_unit"
  grep -qFx 'ExecStart=/usr/local/sbin/octessera-setup' "$setup_unit"
  grep -qFx 'ExecStopPost=/usr/local/sbin/octessera-setup --cleanup' "$setup_unit"
  grep -qFx 'RuntimeMaxSec=670s' "$setup_unit"
  grep -qFx 'TimeoutStopSec=10s' "$setup_unit"
  grep -qFx 'KillMode=control-group' "$setup_unit"
  grep -qFx 'User=root' "$setup_unit"
  grep -qFx 'Group=root' "$setup_unit"
  grep -qFx 'NoNewPrivileges=no' "$setup_unit"
  grep -qFx 'ProtectSystem=yes' "$setup_unit"
  if grep -q '^ReadWritePaths=' "$setup_unit"; then exit 1; fi
  grep -qF '/run/octessera-setup-request/inbox' "$tree/etc/systemd/system/octessera.service"
  octessera_reject_file_match 'Setup coordinator must not restart NetworkManager or log secrets.' -Eiq 'systemctl.*restart|print\(|logger|journal|wpa_passphrase|psk=' "$coordinator" "$config" "$http"
  cmp "$orange/usr/local/sbin/octessera-setup" "$raspberry/usr/local/sbin/octessera-setup"
  cmp "$orange/usr/local/lib/octessera/setup_config.py" "$raspberry/usr/local/lib/octessera/setup_config.py"
  cmp "$orange/usr/local/lib/octessera/setup_http.py" "$raspberry/usr/local/lib/octessera/setup_http.py"
}

check_common "$orange" octessera-runtime orange-pi-zero-2w
check_common "$raspberry" pi raspberry-pi-zero-2w

python3 -m py_compile \
  "$orange/usr/local/sbin/octessera-setup" \
  "$orange/usr/local/lib/octessera/setup_config.py" \
  "$orange/usr/local/lib/octessera/setup_http.py" \
  "$raspberry/usr/local/sbin/octessera-setup" \
  "$raspberry/usr/local/lib/octessera/setup_config.py" \
  "$raspberry/usr/local/lib/octessera/setup_http.py"

echo "Setup layer units, runtime directories, secret logging, and mirror tests passed."
