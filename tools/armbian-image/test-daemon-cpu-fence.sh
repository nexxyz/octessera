#!/usr/bin/env bash
set -euo pipefail

# System daemons share CPU 0 with the app's runtime thread, off the audio callback (1) and the
# DSP workers (2-3); bluealsa, which encodes audio, stays on 2-3. This pins every drop-in so none can drop out quietly.
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
pi_units="$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system"
orange_units="$root/userpatches/overlay/etc/systemd/system"
fail() { echo "Daemon CPU fence test failed: $*" >&2; exit 1; }

declare -A body=(
  [zero]=$'[Service]\nAllowedCPUs=0'
  [zero_nice]=$'[Service]\nNice=10\nAllowedCPUs=0'
  [bluealsa]=$'[Service]\nNice=10\nCPUAffinity=2 3'
  [slice]=$'[Slice]\nAllowedCPUs=0'
)
common=(
  "systemd-udevd.service:zero" "dbus.service:zero" "cron.service:zero" "systemd-journald.service:zero"
  "NetworkManager.service:zero" "NetworkManager-dispatcher.service:zero" "wpa_supplicant.service:zero"
  "bluetooth.service:zero" "systemd-logind.service:zero" "systemd-timesyncd.service:zero" "ssh.service:zero"
  "bluealsa-aplay.service:zero" "getty@.service:zero" "user.slice:slice" "bluealsa.service:bluealsa"
  "fstrim.service:zero" "logrotate.service:zero" "systemd-tmpfiles-clean.service:zero"
)
pi=("${common[@]}" "polkit.service:zero_nice" "avahi-daemon.service:zero_nice" "ModemManager.service:zero"
  "udisks2.service:zero" "triggerhappy.service:zero")
orange=("${common[@]}" "rsyslog.service:zero" "systemd-resolved.service:zero" "systemd-networkd.service:zero"
  "aw859a-bluetooth.service:zero" "aw859a-wifi.service:zero")

check_board() {
  local name="$1" dir="$2"
  shift 2
  local entry unit kind expected found
  for entry in "$@"; do
    unit="${entry%%:*}"
    kind="${entry#*:}"
    expected="${body[$kind]}"
    [[ -f "$dir/$unit.d/10-octessera-background.conf" ]] || fail "$name has no CPU fence for $unit"
    [[ "$(cat "$dir/$unit.d/10-octessera-background.conf")" == "$expected" ]] || fail "$name $unit fence is not: ${expected//$'\n'/ }"
  done
  while IFS= read -r found; do
    unit="$(basename "$(dirname "$found")")"
    unit="${unit%.d}"
    printf '%s\n' "$@" | grep -q "^$unit:" || fail "$name has an unexpected CPU fence for $unit"
  done < <(find "$dir" -path '*.d/10-octessera-background.conf' -print)
}

check_board Raspberry "$pi_units" "${pi[@]}"
check_board Orange "$orange_units" "${orange[@]}"

installs_every_fence() { grep -qF "for dropin in \"\$$1\"$2/etc/systemd/system/*.d/10-octessera-background.conf; do" "$3"; }
installs_every_fence STAGE_FILES /root "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh" || fail "the Raspberry image does not install every fence"
installs_every_fence IMAGE_ROOT "" "$root/tools/pi/provision/provision.sh" || fail "Raspberry provisioning does not install every fence"
installs_every_fence overlay_dir "" "$root/userpatches/customize-image.sh" || fail "the Orange image does not install every fence"
# Every Octessera unit except the app itself carries the fence in its own [Service] section.
service_on_cpu0() {
  awk '/^\[/ { in_service = ($0 == "[Service]") } in_service && $0 == "AllowedCPUs=0" { found = 1 } END { exit !found }' "$1"
}
for unit in "$pi_units"/octessera*.service "$orange_units"/octessera*.service; do
  [[ "$(basename "$unit")" == octessera.service ]] && continue
  service_on_cpu0 "$unit" || fail "$unit does not set AllowedCPUs=0 in its [Service] section"
done
grep -qxF 'ExecStartPost=-/bin/systemctl set-property --runtime init.scope AllowedCPUs=0' \
  "$pi_units/octessera-performance-governor.service" || fail "the Raspberry does not keep systemd itself on CPU 0"
grep -qxF 'ExecStartPre=+-/bin/systemctl set-property --runtime init.scope AllowedCPUs=0' \
  "$orange_units/octessera.service" || fail "the Orange does not keep systemd itself on CPU 0"

printf 'Daemon CPU fence tests passed for both images\n'
