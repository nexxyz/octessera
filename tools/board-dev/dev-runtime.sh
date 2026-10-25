#!/usr/bin/env bash
# Runs on the board. The deploy tool prepends ACTION (preflight, activate or restore),
# BOARD_PROFILE and, for activate, HASH. The development binary lives at one fixed path and
# octessera.service runs it through one ExecStart drop-in; restore removes both.
set -euo pipefail

service=octessera.service
drop=/etc/systemd/system/octessera.service.d/90-octessera-dev.conf
dev_dir=/opt/octessera/dev
dev=$dev_dir/octessera-pi
staged=/tmp/octessera-pi-dev

die() { printf 'BLOCKER: %s\n' "$*" >&2; exit 1; }
property() { systemctl show "$service" --property="$1" --value; }
root() { sudo -n "$@"; }

wait_running() {
  local expected=$1 pid _
  for _ in $(seq 1 20); do
    sleep 1
    pid=$(property MainPID)
    [[ "$(systemctl is-active "$service")" == active && "$pid" != 0 ]] || continue
    [[ "$(root readlink -f -- "/proc/$pid/exe" || :)" == "$expected" ]] || continue
    sleep 3
    [[ "$(property MainPID)" == "$pid" && "$(systemctl is-active "$service")" == active ]] && return 0
  done
  return 1
}

case "$BOARD_PROFILE" in
  raspberry-pi-zero-2w) model='Raspberry Pi Zero 2 W' ;;
  orange-pi-zero-2w) model='OrangePi Zero 2W' ;;
  *) die 'unknown board profile' ;;
esac
[[ "$(tr -d '\0' < /proc/device-tree/model)" == "$model"* ]] || die "this board is not a $model"
[[ "$(property FragmentPath)" == /etc/systemd/system/octessera.service ]] || die 'unexpected octessera.service unit'
[[ "$(property Environment)" == *"OCTESSERA_EXPECTED_BOARD_PROFILE=$BOARD_PROFILE"* ]] || die "service does not expect $BOARD_PROFILE"
[[ "$(systemctl is-enabled "$service")" == enabled ]] || die 'service is not enabled'
root true || die 'passwordless sudo is required'

case "$ACTION" in
  preflight)
    rm -f -- "$staged"
    ;;
  activate)
    trap 'rm -f -- "$staged"; root rm -f -- "$dev.new"' EXIT
    [[ "$HASH" =~ ^[0-9a-f]{64}$ && -f "$staged" && ! -L "$staged" ]] || die 'uploaded binary is missing'
    root install -d -o root -g root -m 0755 "$dev_dir" "${drop%/*}"
    root install -o root -g root -m 0755 -- "$staged" "$dev.new"
    [[ "$(sha256sum -- "$dev.new" | cut -d' ' -f1)" == "$HASH" ]] || die 'installed binary does not match the uploaded hash'
    root mv -f -- "$dev.new" "$dev"
    printf '[Service]\nExecStart=\nExecStart=%s\n' "$dev" | root tee "$drop" >/dev/null
    root systemctl daemon-reload
    [[ "$(property ExecStart)" == *"path=$dev "* ]] || die 'service does not run the development binary'
    root systemctl reset-failed "$service" || :
    root systemctl restart "$service"
    wait_running "$dev" || die "service did not stay up on $dev; check the journal or run -Restore"
    printf 'Development runtime active: %s (PID %s). Listen before calling it good.\n' "$dev" "$(property MainPID)"
    ;;
  restore)
    if [[ ! -e "$drop" ]]; then
      printf 'No development override is installed.\n'
      exit 0
    fi
    root rm -f -- "$drop"
    root systemctl daemon-reload
    root systemctl reset-failed "$service" || :
    root systemctl restart "$service"
    installed=$(readlink -f -- /usr/local/bin/octessera-pi)
    wait_running "$installed" || die 'installed runtime did not come back up; inspect the service now'
    root rm -rf -- "$dev_dir"
    printf 'Restored installed runtime: %s\n' "$installed"
    ;;
  *) die 'unknown action' ;;
esac
