#!/usr/bin/env bash
set -euo pipefail

service=octessera.service
drop_dir=/etc/systemd/system/octessera.service.d
drop="$drop_dir/90-octessera-dev.conf"
installed=/usr/local/bin/octessera-pi
current=/opt/octessera/current/octessera-pi

die() { printf 'BLOCKER: %s\n' "$*" >&2; exit 1; }
property() { systemctl show "$service" --property="$1" --value; }
root() { sudo -n "$@"; }

installed_exe() {
  [[ -L "$installed" && "$(readlink -- "$installed")" == "$current" ]] || die 'installed executable link is not canonical'
  [[ -L /opt/octessera/current ]] || die 'current release link is missing'
  local release
  release=$(readlink -f -- /opt/octessera/current)
  [[ "$release" =~ ^/opt/octessera/releases/[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'current release is not canonical'
  local path
  path=$(readlink -f -- "$installed")
  [[ "$path" == "$release/octessera-pi" && -f "$path" && ! -L "$path" ]] || die 'installed binary is not in the current release'
  printf '%s\n' "$path"
}

unit_baseline() {
  cmp -s /proc/device-tree/model <(printf 'OrangePi Zero 2W\0') || die 'wrong physical board model'
  [[ ! -L /etc/systemd/system/octessera.service && ! -L "$drop_dir" ]] || die 'unsafe service path'
  [[ ! -e "$drop_dir" ]] || {
    [[ -d "$drop_dir" && "$(stat -c '%U:%G %a' "$drop_dir")" == 'root:root 755' ]] || die 'unsafe service drop-in directory'
  }
  [[ "$(property FragmentPath)" == /etc/systemd/system/octessera.service ]] || die 'unexpected service fragment'
  [[ "$(property Type)" == simple && "$(property User)" == octessera-runtime && "$(property Group)" == octessera-runtime ]] || die 'unexpected service identity'
  [[ "$(property Environment)" == *OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w* ]] || die 'wrong board profile in service'
  [[ -z "$(property WorkingDirectory)" || "$(property WorkingDirectory)" == / ]] || die 'unexpected working directory'
  grep -Fxq 'ExecStart=/usr/local/bin/octessera-pi' /etc/systemd/system/octessera.service || die 'installed service ExecStart changed'
  [[ "$(property ExecStart)" == *"path=$installed "* ]] || die 'effective ExecStart is not the installed binary'
  [[ "$(property DropInPaths)" == '' ]] || die 'foreign service drop-in exists'
  [[ ! -e "$drop" && ! -L "$drop" ]] || die 'development drop-in already exists'
  [[ "$(systemctl is-enabled "$service")" == enabled && "$(systemctl is-active "$service")" == active ]] || die 'service is not active and enabled'
  [[ "$(property NRestarts)" == 0 ]] || die 'service already has unexpected restarts'
  [[ "$(property MainPID)" =~ ^[1-9][0-9]*$ ]] || die 'service has no MainPID'
  root true || die 'passwordless sudo is required'
  [[ "$(root readlink -f -- "/proc/$(property MainPID)/exe")" == "$(installed_exe)" ]] || die 'running executable is not the installed release'
}

expected_drop() {
  printf '[Service]\nExecStart=\nExecStart=/opt/octessera/dev/%s/octessera-pi\n' "$1"
}

check_drop() {
  [[ -f "$drop" && ! -L "$drop" ]] || die 'development drop-in is missing or unsafe'
  [[ "$(stat -c '%U:%G %a' -- "$drop")" == 'root:root 644' ]] || die 'development drop-in ownership or mode changed'
  [[ "$(property DropInPaths)" == "$drop" ]] || die 'foreign or missing service drop-in'
  local sha
  sha=$(sed -n 's|^ExecStart=/opt/octessera/dev/\([0-9a-f]\{40\}\)/octessera-pi$|\1|p' "$drop")
  [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || die 'development drop-in is not recognized'
  expected_drop "$sha" | cmp -s - "$drop" || die 'development drop-in has changed'
  printf '%s\n' "$sha"
}

marker_ready() {
  local pid=$1 invocation=$2
  [[ -r /run/octessera/candidate-ready.json ]] || return 1
  python3 - /run/octessera/candidate-ready.json "$pid" "$invocation" <<'PY'
import json, sys
try:
    with open(sys.argv[1], encoding='utf-8') as file:
        marker = json.load(file)
    assert type(marker) is dict
    assert type(marker.get('schema_version')) is int and marker['schema_version'] == 1
    assert marker.get('kind') == 'octessera_candidate_readiness'
    assert marker.get('status') == 'ready'
    assert marker.get('board_profile') == 'orange-pi-zero-2w'
    assert type(marker.get('pid')) is int and marker['pid'] == int(sys.argv[2])
    assert sys.argv[3] and marker.get('systemd_invocation_id') == sys.argv[3]
except (OSError, ValueError, KeyError, AssertionError):
    sys.exit(1)
PY
}

running_ready() {
  local expected=$1 pid=$2 invocation=$3 baseline_restarts=$4
  [[ "$(systemctl is-active "$service")" == active && "$(systemctl is-enabled "$service")" == enabled &&
    "$(property MainPID)" == "$pid" && "$(property InvocationID)" == "$invocation" &&
    "$(property NRestarts)" == "$baseline_restarts" &&
    "$(root readlink -f -- "/proc/$pid/exe" 2>/dev/null || :)" == "$expected" ]] &&
    marker_ready "$pid" "$invocation"
}

wait_exe() {
  local expected=$1 prior=$2 baseline_restarts=$3 pid invocation i
  for ((i=0; i<20; i++)); do
    pid=$(property MainPID)
    invocation=$(property InvocationID)
    if [[ "$pid" =~ ^[1-9][0-9]*$ && "$pid" != "$prior" && -n "$invocation" ]] &&
      running_ready "$expected" "$pid" "$invocation" "$baseline_restarts"; then
      sleep 3
      running_ready "$expected" "$pid" "$invocation" "$baseline_restarts" && return 0
    fi
    sleep 1
  done
  return 1
}

warnings() {
  local since=$1
  local lines
  printf 'Current-boot service journal warnings since activation (not an audio-cleanliness verdict):\n'
  if ! lines=$(root journalctl -b -u "$service" --since "@$since" --no-pager -o cat); then
    printf 'WARNING: journal inspection unavailable; check manually.\n' >&2
    return
  fi
  printf '%s\n' "$lines" | grep -Ei 'panic|error|failed|xrun|underrun|POLLERR' || :
}

[[ "$ACTION" == restore || "$ACTION" == cleanup || "$ACTION" == preflight || "$ACTION" == activate ]] || die 'unknown action'
if [[ "$ACTION" == cleanup ]]; then
  [[ "$STAGE" =~ ^/tmp/octessera-dev-[0-9a-f]{32}$ ]] || die 'unsafe temporary path'
  rm -f -- "$STAGE/octessera-pi" "$STAGE/octessera-pi.metadata.json"
  rmdir -- "$STAGE" 2>/dev/null || :
  exit 0
fi

if [[ "$ACTION" == restore ]]; then
  cmp -s /proc/device-tree/model <(printf 'OrangePi Zero 2W\0') || die 'wrong physical board model'
  [[ ! -L /etc/systemd/system/octessera.service && -d "$drop_dir" && ! -L "$drop_dir" &&
    "$(stat -c '%U:%G %a' "$drop_dir")" == 'root:root 755' ]] || die 'unsafe service path'
  [[ "$(property FragmentPath)" == /etc/systemd/system/octessera.service && "$(property Type)" == simple &&
    "$(property User)" == octessera-runtime && "$(property Group)" == octessera-runtime &&
    "$(property Environment)" == *OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w* ]] || die 'unexpected Orange service identity'
  old=$(installed_exe)
  sha=$(check_drop)
  dev=/opt/octessera/dev/$sha/octessera-pi
  [[ "$(property ExecStart)" == *"path=$dev "* ]] || die 'effective ExecStart is not the known development binary'
  root true || die 'passwordless sudo is required'
  prior=$(property MainPID)
  state=$(systemctl is-active "$service" || :)
  [[ "$(systemctl is-enabled "$service")" == enabled ]] || die 'development service is not enabled'
  if [[ "$state" == active ]]; then
    [[ "$prior" =~ ^[1-9][0-9]*$ && "$(root readlink -f -- "/proc/$prior/exe")" == "$dev" ]] || die 'development process executable changed'
  else
    [[ ( "$state" == failed || "$state" == inactive ) && "$prior" == 0 ]] || die 'unexpected development service state'
  fi
  root rm -- "$drop"
  root systemctl daemon-reload
  root systemctl reset-failed "$service" || die 'RESTORE FAILED: could not clear failed service state'
  restarts=$(property NRestarts)
  root systemctl restart "$service" || die 'RESTORE FAILED: installed service restart failed'
  wait_exe "$old" "$prior" "$restarts" || die "RESTORE FAILED: installed executable did not become ready; inspect service immediately"
  [[ "$(property ExecStart)" == *"path=$installed "* && -z "$(property DropInPaths)" ]] || die 'RESTORE FAILED: service override remains'
  printf 'Restored installed Orange runtime: %s\n' "$old"
  exit 0
fi

[[ "$SOURCE" =~ ^[0-9a-f]{40}$ && "$HASH" =~ ^[0-9a-f]{64}$ && "$STAGE" =~ ^/tmp/octessera-dev-[0-9a-f]{32}$ ]] || die 'unsafe deployment identity'
if [[ "$ACTION" == preflight ]]; then
  unit_baseline
  [[ ! -e "$STAGE" && ! -L "$STAGE" ]] || die 'temporary staging path already exists'
  mkdir -m 0700 -- "$STAGE"
  exit 0
fi

unit_baseline
old=$(installed_exe)
prior=$(property MainPID)
restarts=$(property NRestarts)
[[ -d "$STAGE" && ! -L "$STAGE" ]] || die 'temporary staging directory missing'
[[ -f "$STAGE/octessera-pi" && ! -L "$STAGE/octessera-pi" && -f "$STAGE/octessera-pi.metadata.json" && ! -L "$STAGE/octessera-pi.metadata.json" ]] || die 'staged binary or sidecar missing'
[[ "$(sha256sum -- "$STAGE/octessera-pi" | cut -d' ' -f1)" == "$HASH" ]] || die 'staged binary hash mismatch'
python3 - "$STAGE/octessera-pi.metadata.json" "$SOURCE" "$HASH" <<'PY' || die 'staged metadata mismatch'
import json, sys
with open(sys.argv[1], 'rb') as file:
    raw = file.read()
metadata = json.loads(raw.decode('utf-8-sig'))
expected = dict(schema_version=2, board_profile='orange-pi-zero-2w', artifact_kind='runtime-candidate',
                runtime_ready=False, binary='octessera-pi', package='octessera-pi',
                arch='aarch64-unknown-linux-gnu', cargo_feature='hardware-orange-pi-zero-2w',
                profile='release', binary_sha256=sys.argv[3], source_commit=sys.argv[2])
assert not raw.startswith(b'\xef\xbb\xbf') and type(metadata) is dict
assert set(metadata) == set(expected)
assert all(type(metadata[key]) is type(value) and metadata[key] == value for key, value in expected.items())
PY
chmod 0755 -- "$STAGE/octessera-pi"
"$STAGE/octessera-pi" --print-build-metadata | cmp -s - "$STAGE/octessera-pi.metadata.json" || die 'binary embedded metadata differs from sidecar'
dest=/opt/octessera/dev/$SOURCE
[[ -d /opt/octessera && ! -L /opt/octessera && "$(stat -c '%U:%G' /opt/octessera)" == root:root ]] || die 'unsafe /opt/octessera root'
[[ ! -e /opt/octessera/dev && ! -L /opt/octessera/dev ]] || {
  [[ -d /opt/octessera/dev && ! -L /opt/octessera/dev && "$(stat -c '%U:%G %a' /opt/octessera/dev)" == 'root:root 755' ]] || die 'unsafe development directory'
}
[[ ! -e "$dest" && ! -L "$dest" ]] || die 'development source directory already exists; refusing overwrite'
root install -d -o root -g root -m 0755 /opt/octessera/dev "$dest"
root install -o root -g root -m 0755 -- "$STAGE/octessera-pi" "$dest/octessera-pi"
root install -o root -g root -m 0644 -- "$STAGE/octessera-pi.metadata.json" "$dest/octessera-pi.metadata.json"
[[ "$(sha256sum -- "$dest/octessera-pi" | cut -d' ' -f1)" == "$HASH" ]] || die 'installed development hash mismatch'
cmp -s "$STAGE/octessera-pi.metadata.json" "$dest/octessera-pi.metadata.json" || die 'installed sidecar changed'
since=$(date +%s)
activated=1
rollback() {
  local status=$?
  if ((status != 0 && activated == 1)); then
    printf 'Activation failed; restoring installed service...\n' >&2
    if [[ -f "$drop" && ! -L "$drop" ]] && expected_drop "$SOURCE" | cmp -s - "$drop"; then
      root rm -- "$drop" || die 'ROLLBACK FAILED: cannot remove own drop-in'
    elif [[ -e "$drop" || -L "$drop" ]]; then
      die 'ROLLBACK FAILED: drop-in changed; manual recovery required'
    fi
    root systemctl daemon-reload || die 'ROLLBACK FAILED: daemon-reload failed'
    root systemctl reset-failed "$service" || die 'ROLLBACK FAILED: could not clear failed service state'
    restarts=$(property NRestarts)
    root systemctl restart "$service" || die 'ROLLBACK FAILED: installed service restart failed'
    wait_exe "$old" "$prior" "$restarts" || die 'ROLLBACK FAILED: installed executable is not ready and active/enabled'
    [[ "$(property ExecStart)" == *"path=$installed "* && -z "$(property DropInPaths)" ]] || die 'ROLLBACK FAILED: override still effective'
    printf 'Installed service restored after activation failure.\n' >&2
  fi
}
trap rollback EXIT
[[ ! -e "$drop" && ! -L "$drop" ]] || die 'drop-in appeared before activation'
root install -d -o root -g root -m 0755 "$drop_dir"
expected_drop "$SOURCE" | root tee "$drop" >/dev/null
root chown root:root "$drop"
root chmod 0644 "$drop"
root systemctl daemon-reload
[[ "$(check_drop)" == "$SOURCE" && "$(property ExecStart)" == *"path=$dest/octessera-pi "* ]] || die 'effective development ExecStart mismatch'
root systemctl restart "$service"
wait_exe "$dest/octessera-pi" "$prior" "$restarts" || die 'development service did not stabilize at the exact binary'
warnings "$since"
activated=0
printf 'Orange development runtime active: %s (PID %s); audio cleanliness requires listening.\n' "$dest/octessera-pi" "$(property MainPID)"
