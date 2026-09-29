#!/usr/bin/env bash
set -euo pipefail

source_script=$(dirname -- "${BASH_SOURCE[0]}")/deploy-opi-fast-remote.sh
fixture=$(mktemp -d /tmp/octessera-opi-deploy-test.XXXXXX)
trap 'rm -rf -- "$fixture"' EXIT
export FIXTURE="$fixture"
mkdir -p "$fixture/etc/systemd/system" "$fixture/usr/local/bin" "$fixture/opt/octessera/releases/0.8.7" "$fixture/run/octessera"
printf 'OrangePi Zero 2W\0' > "$fixture/model"
printf 'ExecStart=%s/usr/local/bin/octessera-pi\n' "$fixture" > "$fixture/etc/systemd/system/octessera.service"
touch "$fixture/opt/octessera/releases/0.8.7/octessera-pi"
ln -s "$fixture/opt/octessera/releases/0.8.7" "$fixture/opt/octessera/current"
ln -s "$fixture/opt/octessera/current/octessera-pi" "$fixture/usr/local/bin/octessera-pi"

sed -e "s@/etc/systemd/system@$fixture/etc/systemd/system@g" \
  -e "s@/usr/local/bin@$fixture/usr/local/bin@g" \
  -e "s@/opt/octessera@$fixture/opt/octessera@g" \
  -e "s@/run/octessera@$fixture/run/octessera@g" \
  "$source_script" > "$fixture/payload.sh"

cat > "$fixture/fakes.sh" <<'SH'
DROP="$FIXTURE/etc/systemd/system/octessera.service.d/90-octessera-dev.conf"
OLD="$FIXTURE/opt/octessera/releases/0.8.7/octessera-pi"
MARKER="$FIXTURE/run/octessera/candidate-ready.json"
save_state() {
  printf 'STATE=%q\nRUN_PID=%q\nRUN_EXE=%q\nINVOCATION=%q\nRESTARTS=%q\nNEXT_PID=%q\n' \
    "$STATE" "$RUN_PID" "$RUN_EXE" "$INVOCATION" "$RESTARTS" "$NEXT_PID" > "$FIXTURE/state"
}
write_marker() {
  if [[ "$RUN_EXE" == "$OLD" || "${DEV_MARKER:-1}" != 0 ]]; then
    local marker_pid=$RUN_PID marker_invocation=$INVOCATION
    if [[ "$RUN_EXE" != "$OLD" && "${DEV_MARKER:-1}" == stale ]]; then
      marker_pid=900; marker_invocation=inv-900
    fi
    printf '{"schema_version":1,"kind":"octessera_candidate_readiness","status":"ready","board_profile":"orange-pi-zero-2w","pid":%s,"systemd_invocation_id":"%s"}\n' \
      "$marker_pid" "$marker_invocation" > "$MARKER"
  fi
}
cmp() {
  if [[ "$1" == -s && "$2" == /proc/device-tree/model ]]; then
    command cmp -s "$FIXTURE/model" "$3"
  else
    command cmp "$@"
  fi
}
stat() {
  if [[ "$1" == -c ]]; then
    if [[ "$2" == '%U:%G' ]]; then printf 'root:root\n'; return; fi
    if [[ "${*: -1}" == "$DROP" ]]; then printf 'root:root 644\n'; else printf 'root:root 755\n'; fi
  else
    command stat "$@"
  fi
}
readlink() {
  if [[ "${*: -1}" == /proc/*/exe ]]; then
    if [[ "${TEST_PRIVILEGED:-}" != 1 ]]; then printf 'unprivileged proc read denied\n' >> "$FIXTURE/commands"; return 1; fi
    printf 'privileged proc read\n' >> "$FIXTURE/commands"
    source "$FIXTURE/state"
    [[ "$RUN_PID" != 0 ]] || return 1
    printf '%s\n' "$RUN_EXE"
  else
    command readlink "$@"
  fi
}
systemctl() {
  source "$FIXTURE/state"
  printf 'systemctl %s\n' "$*" >> "$FIXTURE/commands"
  case "$1" in
    show)
      local key=${3#--property=}
      case "$key" in
        FragmentPath) printf '%s/etc/systemd/system/octessera.service\n' "$FIXTURE" ;;
        Type) printf 'simple\n' ;;
        User|Group) printf 'octessera-runtime\n' ;;
        Environment) printf 'OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w\n' ;;
        WorkingDirectory) printf '/\n' ;;
        ExecStart)
          local path="$FIXTURE/usr/local/bin/octessera-pi"
          [[ ! -e "$DROP" ]] || path=$(sed -n '3s/^ExecStart=//p' "$DROP")
          printf '{ path=%s ; argv[]=%s ; }\n' "$path" "$path" ;;
        DropInPaths) [[ ! -e "$DROP" ]] || printf '%s\n' "$DROP" ;;
        NRestarts) printf '%s\n' "$RESTARTS" ;;
        MainPID) printf '%s\n' "$RUN_PID" ;;
        InvocationID) printf '%s\n' "$INVOCATION" ;;
        *) return 1 ;;
      esac ;;
    is-enabled) printf 'enabled\n' ;;
    is-active) printf '%s\n' "$STATE"; [[ "$STATE" == active ]] ;;
    daemon-reload) : ;;
    reset-failed) RESTARTS=0; save_state ;;
    restart)
      NEXT_PID=$((NEXT_PID + 1))
      if [[ -e "$DROP" && "${FAIL_DEV:-0}" == 1 ]]; then
        STATE=failed; RUN_PID=0; INVOCATION=''; RESTARTS=2
        rm -f -- "$MARKER"
        save_state
        return 1
      fi
      STATE=active; RUN_PID=$NEXT_PID; INVOCATION="inv-$NEXT_PID"
      RUN_EXE="$OLD"
      [[ ! -e "$DROP" ]] || RUN_EXE=$(sed -n '3s/^ExecStart=//p' "$DROP")
      rm -f -- "$MARKER"
      write_marker
      save_state ;;
    *) return 1 ;;
  esac
}
sudo() {
  [[ "$1" == -n ]] || return 1
  shift
  case "$1" in
    readlink) TEST_PRIVILEGED=1 readlink "${@:2}" ;;
    install)
      shift
      local args=()
      while (($#)); do
        if [[ "$1" == -o || "$1" == -g ]]; then shift 2; else args+=("$1"); shift; fi
      done
      command install "${args[@]}" ;;
    chown) : ;;
    journalctl) : ;;
    *) "$@" ;;
  esac
}
sleep() { :; }
SH

printf 'source "%s/fakes.sh"\n' "$fixture" > "$fixture/entry.sh"
cat "$fixture/payload.sh" >> "$fixture/entry.sh"

fail() { printf 'FAILED: %s\n' "$1" >&2; exit 1; }
reset_service() {
  printf 'STATE=active\nRUN_PID=900\nRUN_EXE=%q\nINVOCATION=inv-900\nRESTARTS=0\nNEXT_PID=900\n' \
    "$fixture/opt/octessera/releases/0.8.7/octessera-pi" > "$fixture/state"
  rm -f -- "$fixture/run/octessera/candidate-ready.json" "$fixture/commands"
}
prepare_stage() {
  local source_sha=$1
  local stage_path="/tmp/octessera-dev-${source_sha:0:32}"
  mkdir -m 0700 -- "$stage_path"
  printf '%s\n' '#!/usr/bin/env bash' "cat -- \"\$0.metadata.json\"" > "$stage_path/octessera-pi"
  chmod 0755 "$stage_path/octessera-pi"
  local hash
  hash=$(sha256sum "$stage_path/octessera-pi" | cut -d ' ' -f1)
  printf '{"schema_version":2,"board_profile":"orange-pi-zero-2w","artifact_kind":"runtime-candidate","runtime_ready":false,"binary":"octessera-pi","package":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-orange-pi-zero-2w","profile":"release","binary_sha256":"%s","source_commit":"%s"}\n' \
    "$hash" "$source_sha" > "$stage_path/octessera-pi.metadata.json"
  STAGE="$stage_path" SOURCE="$source_sha" HASH="$hash"
}
assert_restored() {
  grep -Fxq 'STATE=active' "$fixture/state" || fail 'installed service did not become active'
  grep -Fxq "RUN_EXE=$fixture/opt/octessera/releases/0.8.7/octessera-pi" "$fixture/state" || fail 'installed executable did not return'
  grep -Fxq 'RESTARTS=0' "$fixture/state" || fail 'installed service restarted unexpectedly'
  [[ ! -e "$fixture/etc/systemd/system/octessera.service.d/90-octessera-dev.conf" ]] || fail 'own drop-in was not removed'
  grep -q 'systemctl reset-failed octessera.service' "$fixture/commands" || fail 'start-limit state was not cleared'
  grep -q 'privileged proc read' "$fixture/commands" || fail 'proc exe was not read with sudo'
  if grep -q 'unprivileged proc read denied' "$fixture/commands"; then fail 'proc exe read without sudo'; fi
}

for scenario in start_failure missing_marker stale_marker; do
  reset_service
  case "$scenario" in
    start_failure) source_sha=$(printf 'a%.0s' {1..40}); export FAIL_DEV=1 DEV_MARKER=1 ;;
    missing_marker) source_sha=$(printf 'b%.0s' {1..40}); export FAIL_DEV=0 DEV_MARKER=0 ;;
    stale_marker) source_sha=$(printf 'c%.0s' {1..40}); export FAIL_DEV=0 DEV_MARKER=stale ;;
  esac
  prepare_stage "$source_sha"
  if ACTION=activate SOURCE="$SOURCE" HASH="$HASH" STAGE="$STAGE" bash "$fixture/entry.sh" > "$fixture/$scenario.out" 2>&1; then
    fail "$scenario incorrectly succeeded"
  fi
  assert_restored
  if [[ "$scenario" != start_failure ]]; then
    grep -q 'development service did not stabilize' "$fixture/$scenario.out" || fail "$scenario did not block activation"
  fi
  rm -rf -- "$STAGE"
done

reset_service
source_sha=$(printf 'd%.0s' {1..40})
drop="$fixture/etc/systemd/system/octessera.service.d/90-octessera-dev.conf"
mkdir -p -- "$(dirname -- "$drop")"
printf '[Service]\nExecStart=\nExecStart=%s/opt/octessera/dev/%s/octessera-pi\n' "$fixture" "$source_sha" > "$drop"
printf 'STATE=failed\nRUN_PID=0\nRUN_EXE=""\nINVOCATION=""\nRESTARTS=3\nNEXT_PID=905\n' > "$fixture/state"
if ! ACTION=restore SOURCE='' HASH='' STAGE='' bash "$fixture/entry.sh" > "$fixture/restore.out" 2>&1; then
  fail 'explicit restore from failed candidate did not succeed'
fi
assert_restored
printf 'Orange dev deploy remote start-failure, missing/stale marker, and failed-service restore tests passed\n'
