#!/bin/bash
set -Eeuo pipefail

run_id='__RUN_ID__'
live_seconds='__LIVE_SECONDS__'
artifact_hash='__ARTIFACT_HASH__'
source_commit='__SOURCE_COMMIT__'
root='__REMOTE_ROOT__'
service=octessera.service
candidate_unit="octessera-autoaux-$run_id.service"
candidate_ready="/run/octessera/autoaux-ready-$run_id.json"
store_dir="/var/lib/octessera/study-stores/octessera-study-$run_id.service"
installed_binary=/usr/local/bin/octessera-pi
installed_binary_realpath=
default_store=/home/pi/presets
evidence="$root/evidence"
candidate_attempted=0
candidate_stop_failed=0
service_stopped=0
restore_status=0
original_binary_hash=
original_default_hash=
original_recovery_hash=
original_recovery_present=0
samples_dir=
working_directory=
journal_since=0

mkdir -p "$evidence"
exec > >(tee -a "$evidence/host.log") 2>&1
die() { printf 'study_error=%s\n' "$*" >&2; exit 1; }
sudo_n() { sudo -n "$@"; }
service_value() { sudo_n systemctl show "$service" --property="$1" --value; }
hash_store() { sudo_n sha256sum -- "$1" | awk '{print $1}'; }
process_wake_trace() { sudo_n sh -c 'tr "\000" "\n" < "/proc/$1/environ" | grep -Fx "OCTESSERA_WAKE_TRACE=1"' sh "$1"; }
process_environment() { sudo_n sh -c 'tr "\000" "\n" < "/proc/$1/environ"' sh "$1"; }
readiness_matches() {
  sudo_n python3 - "$1" "$2" "$3" <<'PY'
import json,sys
try:
 d=json.load(open(sys.argv[1]))
 ready_at=d.get('ready_at_unix_ms')
 ok=type(d.get('schema_version')) is int and d['schema_version']==1 and d.get('kind')=='octessera_candidate_readiness' and d.get('status')=='ready' and d.get('board_profile')=='raspberry-pi-zero-2w' and type(d.get('pid')) is int and str(d['pid'])==sys.argv[2] and d.get('systemd_invocation_id')==sys.argv[3] and isinstance(d.get('package_version'),str) and bool(d['package_version']) and type(ready_at) is int and ready_at>0
except Exception: ok=False
raise SystemExit(0 if ok else 1)
PY
}
verify_original_store() {
  [ "$(hash_store "$default_file" 2>/dev/null || true)" = "$original_default_hash" ] || return 1
  if [ "$original_recovery_present" -eq 1 ]; then
    [ "$(hash_store "$recovery_file" 2>/dev/null || true)" = "$original_recovery_hash" ] || return 1
  else
    sudo_n test ! -e "$recovery_file" || return 1
  fi
}

restore_service() {
  local active=unknown enabled=unknown pid=0 invocation= ready_valid=0 deadline candidate_state=not-found candidate_activity=not-found candidate_main_pid=0 stop_status=0 candidate_safe=1 current_binary_hash current_exec trace
  if [ "$candidate_attempted" -eq 1 ]; then
    if ! candidate_state="$(sudo_n systemctl show "$candidate_unit" --property=LoadState --value 2>/dev/null)"; then
      candidate_state=unknown
      candidate_safe=0
      restore_status=1
    fi
    if [ "$candidate_stop_failed" -eq 1 ]; then
      candidate_safe=0
      restore_status=1
    fi
    case "$candidate_state" in
      not-found) ;;
      loaded)
        sudo_n systemctl stop "$candidate_unit" || stop_status=$?
        candidate_activity="$(sudo_n systemctl is-active "$candidate_unit" 2>/dev/null || true)"
        candidate_main_pid="$(sudo_n systemctl show "$candidate_unit" --property=MainPID --value 2>/dev/null || printf 0)"
        if [ "$stop_status" -ne 0 ] || [[ "$candidate_activity" != inactive && "$candidate_activity" != failed ]] || [ "$candidate_main_pid" != 0 ]; then
          candidate_safe=0
          restore_status=1
        fi
        ;;
      *)
        candidate_safe=0
        restore_status=1
        ;;
    esac
  fi
  if [ "$service_stopped" -eq 1 ] && [ "$candidate_safe" -eq 1 ]; then
    if ! verify_original_store; then
      restore_status=1
      candidate_safe=0
    fi
  fi
  if [ "$service_stopped" -eq 1 ] && [ "$candidate_safe" -eq 1 ]; then
    sudo_n systemctl start "$service" || restore_status=1
    deadline=$(( $(date +%s) + 30 ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
      active="$(sudo_n systemctl is-active "$service" 2>/dev/null || true)"
      enabled="$(sudo_n systemctl is-enabled "$service" 2>/dev/null || true)"
      pid="$(service_value MainPID 2>/dev/null || printf 0)"
      invocation="$(service_value InvocationID 2>/dev/null || true)"
      if [ "$active" = active ] && [ "$enabled" = enabled ] && [[ "$pid" =~ ^[1-9][0-9]*$ ]] && [ -n "$invocation" ] && [ "$pid" != "$initial_pid" ] && [ "$invocation" != "$initial_invocation" ] && readiness_matches /run/octessera/candidate-ready.json "$pid" "$invocation"; then
        ready_valid=1
        break
      fi
      sleep 1
    done
    if [ "$ready_valid" -eq 1 ]; then
      active="$(sudo_n systemctl is-active "$service" 2>/dev/null || true)"
      enabled="$(sudo_n systemctl is-enabled "$service" 2>/dev/null || true)"
      final_pid="$(service_value MainPID 2>/dev/null || printf 0)"
      final_invocation="$(service_value InvocationID 2>/dev/null || true)"
      if [ "$active" = active ] && [ "$enabled" = enabled ] && [ "$final_pid" = "$pid" ] && [ "$final_invocation" = "$invocation" ] && readiness_matches /run/octessera/candidate-ready.json "$final_pid" "$final_invocation"; then
        pid="$final_pid"
        invocation="$final_invocation"
      else
        ready_valid=0
      fi
    fi
    [ "$active" = active ] && [ "$enabled" = enabled ] && [ "$ready_valid" -eq 1 ] || restore_status=1
    [ "$(sudo_n readlink -f -- "/proc/$pid/exe" 2>/dev/null || true)" = "$installed_binary_realpath" ] || restore_status=1
    process_wake_trace "$pid" >/dev/null 2>&1 || restore_status=1
    verify_original_store || restore_status=1
    service_stopped=0
  fi
  current_binary_hash="$(sudo_n sha256sum -- "$installed_binary" 2>/dev/null | awk '{print $1}' || true)"
  current_exec="$(service_value ExecStart 2>/dev/null || true)"
  trace="$(service_value Environment 2>/dev/null | tr ' ' '\n' | grep -Fx 'OCTESSERA_WAKE_TRACE=1' || true)"
  if [ -n "$original_binary_hash" ]; then
    [ "$current_binary_hash" = "$original_binary_hash" ] || restore_status=1
    [[ "$current_exec" == *"path=$installed_binary"* ]] || restore_status=1
    [ "$trace" = OCTESSERA_WAKE_TRACE=1 ] || restore_status=1
  fi
  printf 'restore_status=%s candidate_state=%s candidate_stop_status=%s service_active=%s service_enabled=%s readiness_valid=%s installed_pid=%s installed_invocation=%s installed_binary_sha256=%s wake_trace=%s\n' "$restore_status" "$candidate_activity" "$stop_status" "$active" "$enabled" "$ready_valid" "$pid" "$invocation" "$current_binary_hash" "$trace"
  [ "$restore_status" -eq 0 ]
}

on_exit() {
  local status=$?
  trap - EXIT
  set +e
  restore_service
  local restore=$?
  if [ "$status" -eq 0 ] && [ "$restore" -eq 0 ]; then sudo_n rm -rf -- "$store_dir" || restore=1; fi
  if [ "$restore" -ne 0 ]; then status=70; fi
  sudo_n journalctl -u "$candidate_unit" --no-pager --lines=5000 -o short-monotonic > "$evidence/candidate-journal.log" 2>&1
  sudo_n journalctl -k --since "@$journal_since" --no-pager --lines=2000 -o short-monotonic > "$evidence/kernel-audio-journal.log" 2>&1
  sudo_n journalctl -u "$service" --since "@$journal_since" --no-pager --lines=2000 -o short-monotonic > "$evidence/service-journal.log" 2>&1
  cat "$evidence/candidate-journal.log" >> "$evidence/candidate-stderr-and-journal.log" 2>/dev/null || true
  grep -Ei 'error|failed|underrun|xrun|underflow|overrun' "$evidence/candidate-stderr-and-journal.log" > "$evidence/visible-candidate-errors.txt" || true
  grep -Ei 'error|failed|underrun|xrun|underflow|overrun' "$evidence/kernel-audio-journal.log" > "$evidence/visible-kernel-audio-errors.txt" || true
  grep -Ei 'startup|steady|error|failed|underrun|xrun|underflow|overrun' "$evidence/candidate-stderr-and-journal.log" "$evidence/kernel-audio-journal.log" > "$evidence/visible-startup-steady-audio-lines.txt" || true
  printf 'study_exit=%s restoration_exit=%s\n' "$status" "$restore" >> "$evidence/host.log"
  exit "$status"
}
trap on_exit EXIT

[[ "$run_id" =~ ^[0-9a-f]{32}$ ]] || die 'invalid run identity'
[[ "$artifact_hash" =~ ^[0-9a-f]{64}$ ]] || die 'invalid artifact hash'
[[ "$source_commit" =~ ^[0-9a-f]{40}$ ]] || die 'invalid source identity'
[[ "$live_seconds" =~ ^[0-9]+$ ]] || die 'invalid duration'
sudo_n -v || die 'non-interactive sudo is unavailable'
test "$(sudo_n systemctl is-active "$service")" = active || die 'installed service is not active'
test "$(sudo_n systemctl is-enabled "$service")" = enabled || die 'installed service is not enabled'
test "$(service_value User)" = pi || die 'managed service user is not pi'
test "$(service_value ExecStart | sed -n 's/.*path=\([^ ;]*\).*/\1/p')" = "$installed_binary" || die 'managed executable differs from expected installed binary'
test -x "$installed_binary" || die 'installed executable is missing'
installed_binary_realpath="$(sudo_n readlink -f -- "$installed_binary")"
test -n "$installed_binary_realpath" || die 'installed executable path cannot be resolved'
service_environment="$(service_value Environment)"
test "$(printf '%s\n' "$service_environment" | tr ' ' '\n' | grep -Fx 'OCTESSERA_WAKE_TRACE=1' || true)" = OCTESSERA_WAKE_TRACE=1 || die 'wake trace is not armed'
test "$(printf '%s\n' "$service_environment" | tr ' ' '\n' | grep -Fx 'OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w' || true)" = OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w || die 'installed service is not configured for the Raspberry Pi Zero 2W'
installed_pid="$(service_value MainPID)"
[[ "$installed_pid" =~ ^[1-9][0-9]*$ ]] || die 'installed service PID is not live'
installed_environment="$(process_environment "$installed_pid")"
configured_store="$(printf '%s\n' "$installed_environment" | sed -n 's/^OCTESSERA_PI_STORE_DIR=//p')"
test "$configured_store" = /home/pi/presets || die 'installed service store is not the expected /home/pi/presets'
default_store="$configured_store"
[[ "$installed_pid" =~ ^[1-9][0-9]*$ ]] && process_wake_trace "$installed_pid" >/dev/null || die 'installed service process wake trace is not armed'
installed_invocation="$(service_value InvocationID)"
test -n "$installed_invocation" || die 'installed service invocation identity is missing'
readiness_matches /run/octessera/candidate-ready.json "$installed_pid" "$installed_invocation" || die 'installed service readiness does not identify its active process'
test "$(sudo_n readlink -f -- "/proc/$installed_pid/exe")" = "$installed_binary_realpath" || die 'installed service process does not execute the expected binary'
initial_pid="$installed_pid"
initial_invocation="$installed_invocation"
original_binary_hash="$(hash_store "$installed_binary")"
samples_dir="$(printf '%s\n' "$installed_environment" | sed -n 's/^OCTESSERA_PI_SAMPLES_DIR=//p')"
working_directory="$(service_value WorkingDirectory)"
test -n "$samples_dir" || die 'managed service does not define OCTESSERA_PI_SAMPLES_DIR'
test -n "$working_directory" && test -d "$working_directory" || die 'managed service working directory is missing'
default_file="$default_store/default.json"
recovery_file="$default_store/recovery-save.json"
test -d "$default_store" || die "normal runtime store directory is missing: $default_store"
[ ! -L "$default_store" ] || die "normal runtime store directory is a symlink: $default_store"
test -f "$default_file" && [ ! -L "$default_file" ] || die 'normal runtime default store is missing or a symlink'
if sudo_n find "$default_store" -type l -print -quit | grep -q .; then die 'normal runtime store contains a symlink'; fi
original_default_hash="$(hash_store "$default_file")"
if sudo_n test -e "$recovery_file"; then
sudo_n test -f "$recovery_file" && sudo_n test ! -L "$recovery_file" || die 'recovery store is not a regular file'
  original_recovery_present=1
  original_recovery_hash="$(hash_store "$recovery_file")"
fi
sudo_n test ! -e "$store_dir" || die 'unique study store path already exists'
study_parent="$(dirname "$store_dir")"
sudo_n test -d /var/lib/octessera && sudo_n test ! -L /var/lib/octessera || die 'Octessera data directory is missing or a symlink'
sudo_n test ! -L "$study_parent" || die 'study store parent is a symlink'
sudo_n install -d -o root -g root -m 0755 "$study_parent"
sudo_n test -d "$study_parent" && sudo_n test ! -L "$study_parent" || die 'study store parent could not be safely created'
candidate_source="$(python3 - "$root/octessera-pi.metadata.json" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
expected={'schema_version':1,'board_profile':'raspberry-pi-zero-2w','binary':'octessera-pi','arch':'aarch64-unknown-linux-gnu','cargo_feature':'hardware-raspberry-pi-zero-2w'}
if any(d.get(k)!=v for k,v in expected.items()): raise SystemExit('candidate board metadata mismatch')
if not __import__('re').fullmatch(r'[0-9a-f]{40}',d.get('source_commit','')): raise SystemExit('candidate source metadata is malformed')
if not __import__('re').fullmatch(r'[0-9a-f]{64}',d.get('binary_sha256','')): raise SystemExit('candidate binary metadata is malformed')
print(d['source_commit'])
PY
)"
candidate_sha="$(sha256sum "$root/octessera-pi" | awk '{print $1}')"
candidate_metadata_sha="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["binary_sha256"])' "$root/octessera-pi.metadata.json")"
test "$candidate_source" = "$source_commit" && test "$candidate_sha" = "$artifact_hash" && test "$candidate_metadata_sha" = "$artifact_hash" || die 'candidate metadata/hash mismatch'
cp -- "$root/octessera-pi.metadata.json" "$evidence/candidate.metadata.json"
test -f "$root/octessera-pi" && [ ! -L "$root/octessera-pi" ] || die 'staged candidate is not a regular file'
chmod 0755 -- "$root/octessera-pi"
sudo_n -u pi test -x "$root/octessera-pi" || die 'staged candidate is not executable by runtime user pi'
test "$(sha256sum "$root/octessera-pi" | awk '{print $1}')" = "$artifact_hash" || die 'candidate hash changed while setting executable mode'
sudo_n install -d -o pi -g pi -m 0750 "$store_dir"
sudo_n cp -a -- "$default_store/." "$store_dir/"
sudo_n chown -R pi:pi "$store_dir"
if sudo_n find "$store_dir" -type l -print -quit | grep -q .; then die 'isolated store clone contains a symlink'; fi
sudo_n test -f "$store_dir/default.json" && sudo_n test ! -L "$store_dir/default.json" || die 'isolated default store is not a regular file'
sudo_n -u pi python3 - "$store_dir/default.json" <<'PY'
import json,sys
p=sys.argv[1]
with open(p, encoding='utf-8') as f: data=json.load(f)
runtime=data.get('runtimeConfig')
if not isinstance(runtime,dict): raise SystemExit('default runtimeConfig is missing')
midi=runtime.get('midi')
if not isinstance(midi,dict) or midi.get('syncMode')!='internal': raise SystemExit('default runtime is not configured for internal audio')
runtime['dimTimerSeconds']=0
runtime['screenSleepSeconds']=0
runtime['autoSaveDefault']=True
with open(p,'w',encoding='utf-8') as f: json.dump(data,f,separators=(',',':'))
PY
sudo_n test "$(hash_store "$default_file")" = "$original_default_hash" || die 'original default store drifted during clone setup'
sudo_n test "$(hash_store "$installed_binary")" = "$original_binary_hash" || die 'installed binary changed during preflight'
journal_since="$(date +%s)"
service_stopped=1
sudo_n systemctl stop "$service"
candidate_attempted=1
sudo_n install -d -o pi -g pi -m 0755 /run/octessera
sudo_n systemd-run --unit="$candidate_unit" --property=Type=simple --property=User=pi --property=Group=pi --property=WorkingDirectory="$working_directory" --property=Restart=no --property=RuntimeMaxSec="$((live_seconds + 120))" --property="SupplementaryGroups=tty input" --property="AmbientCapabilities=CAP_SYS_NICE CAP_SYS_TTY_CONFIG" --property="CapabilityBoundingSet=CAP_SYS_NICE CAP_SETUID CAP_SETGID CAP_SYS_TTY_CONFIG" --property=NoNewPrivileges=no --property=TTYPath=/dev/tty1 --property=TTYReset=yes --setenv=OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w --setenv=OCTESSERA_OLED_BOOT_HANDOFF=v1 --setenv=OCTESSERA_PI_TIMING_AUTOAUX=1 --setenv=OCTESSERA_PI_UI_PROFILE=1 --setenv=OCTESSERA_PI_TIMING_KEEP_AWAKE=1 --setenv="OCTESSERA_PI_STORE_DIR=$store_dir" --setenv="OCTESSERA_PI_SAMPLES_DIR=$samples_dir" --setenv="OCTESSERA_CANDIDATE_HEALTH_PATH=$candidate_ready" --setenv=OCTESSERA_WAKE_TRACE=1 "$root/octessera-pi"
deadline=$(( $(date +%s) + 30 ))
candidate_pid=
candidate_invocation=
candidate_ready_valid=0
while [ "$(date +%s)" -lt "$deadline" ]; do
  sudo_n systemctl is-active --quiet "$candidate_unit" || die 'candidate exited before readiness'
  candidate_pid="$(sudo_n systemctl show "$candidate_unit" --property=MainPID --value 2>/dev/null || true)"
  candidate_invocation="$(sudo_n systemctl show "$candidate_unit" --property=InvocationID --value 2>/dev/null || true)"
  if readiness_matches "$candidate_ready" "$candidate_pid" "$candidate_invocation"; then candidate_ready_valid=1; break; fi
  sleep 1
done
test "$candidate_ready_valid" -eq 1 || die 'candidate readiness did not validate before timeout'
[[ "$candidate_pid" =~ ^[1-9][0-9]*$ ]] && [ -n "$candidate_invocation" ] || die 'candidate systemd identity was not observed'
current_candidate_pid="$(sudo_n systemctl show "$candidate_unit" --property=MainPID --value 2>/dev/null || true)"
current_candidate_invocation="$(sudo_n systemctl show "$candidate_unit" --property=InvocationID --value 2>/dev/null || true)"
sudo_n systemctl is-active --quiet "$candidate_unit" && [ "$current_candidate_pid" = "$candidate_pid" ] && [ "$current_candidate_invocation" = "$candidate_invocation" ] && readiness_matches "$candidate_ready" "$current_candidate_pid" "$current_candidate_invocation" || die 'candidate unhealthy or readiness changed after validation'
start_time="$(date +%s)"
while [ "$(( $(date +%s) - start_time ))" -lt "$live_seconds" ]; do
  sudo_n systemctl is-active --quiet "$candidate_unit" || die 'candidate exited during live interval'
  sleep 1
done
sudo_n systemctl is-active --quiet "$candidate_unit" || die 'candidate exited at end of live interval'
sudo_n journalctl -u "$candidate_unit" --no-pager --lines=5000 -o short-monotonic > "$evidence/candidate-stderr-and-journal.log" 2>&1
grep -E 'raspberry-autoaux .*save_revision=[^ ]+ .*save_request=[^ ]+ .*save_elapsed_ms=[0-9]+' "$evidence/candidate-stderr-and-journal.log" > "$evidence/autoaux-save-receipt.log" || die 'identified AutoAux automatic-save receipt is missing'
if grep -F 'raspberry-autoaux-failed:' "$evidence/candidate-stderr-and-journal.log" >/dev/null; then die 'candidate emitted raspberry-autoaux-failed'; fi
if ! sudo_n systemctl stop "$candidate_unit"; then
  candidate_stop_failed=1
  die 'candidate stop failed; installed service will remain stopped'
fi
sudo_n journalctl -u "$candidate_unit" --no-pager --lines=5000 -o short-monotonic > "$evidence/candidate-stderr-and-journal.log" 2>&1
grep -E 'raspberry-autoaux .*save_revision=[^ ]+ .*save_request=[^ ]+ .*save_elapsed_ms=[0-9]+' "$evidence/candidate-stderr-and-journal.log" > "$evidence/autoaux-save-receipt.log" || die 'identified AutoAux automatic-save receipt is missing'
if grep -F 'raspberry-autoaux-failed:' "$evidence/candidate-stderr-and-journal.log" >/dev/null; then die 'candidate emitted raspberry-autoaux-failed'; fi
verify_original_store || die 'original store hash/absence changed during study'
printf 'candidate_board_profile=raspberry-pi-zero-2w candidate_source_commit=%s candidate_sha256=%s live_seconds=%s\n' "$source_commit" "$artifact_hash" "$live_seconds"
