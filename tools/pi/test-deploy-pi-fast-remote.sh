#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HELPER="$SCRIPT_DIR/deploy-pi-fast-remote.sh"
LOCK_HELPER="$SCRIPT_DIR/deploy-pi-fast-lock.py"
WORK="$(mktemp -d /tmp/octessera-fast-deploy-test.XXXXXX)"
BIN_DIR="$WORK/bin"
ROOT="$WORK/root"
PROC_ROOT="$WORK/proc"
BIN_LINK="$WORK/runtime/octessera-pi"
CANDIDATE="$WORK/candidate"
CANDIDATE_METADATA="$WORK/candidate.metadata.json"
LOG="$WORK/systemctl.log"
MODE_FILE="$WORK/mode"
RESTART_FILE="$WORK/restarts"
ACTIVE_FILE="$WORK/active"

cleanup() {
    chmod -R u+w -- "$WORK" 2>/dev/null || true
    rm -rf -- "$WORK"
}
trap cleanup EXIT

fail() {
    echo "remote fast-deploy test failed: $*" >&2
    exit 1
}

assert_equal() {
    [ "$1" = "$2" ] || fail "$3"
}

assert_file_equal() {
    cmp -s "$1" "$2" || fail "$3"
}

write_systemctl_mock() {
    cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mode="$(cat "$OCTESSERA_FAST_TEST_MODE")"
count="$(cat "$OCTESSERA_FAST_TEST_RESTARTS" 2>/dev/null || printf '0')"
printf '%s\n' "$*" >> "$OCTESSERA_FAST_TEST_LOG"
if [ "$1" = restart ]; then
    count=$((count + 1))
    printf '%s\n' "$count" > "$OCTESSERA_FAST_TEST_RESTARTS"
    printf '%s\n' active > "$OCTESSERA_FAST_TEST_ACTIVE"
    rm -f "$OCTESSERA_FAST_TEST_PROC/123/exe"
    ln -s "$(readlink -f "$OCTESSERA_FAST_TEST_ROOT/current/octessera-pi")" "$OCTESSERA_FAST_TEST_PROC/123/exe"
    if [ "$mode" = restartfail ] && [ "$count" -eq 1 ]; then
        exit 1
    fi
fi
if [ "$1" = stop ]; then
    printf '%s\n' inactive > "$OCTESSERA_FAST_TEST_ACTIVE"
fi
if printf ' %s\n' "$*" | grep -q ' status ' && [ "$mode" = statusfail ] && [ "$count" -eq 1 ]; then
    exit 1
fi
if [ "$1" = is-active ]; then
    if [ "$mode" = delayedfail ] && [ "$count" -eq 1 ]; then
        marker="$OCTESSERA_FAST_TEST_WORK/delayed-check"
        if [ -e "$marker" ]; then
            exit 1
        fi
        touch "$marker"
    fi
    [ "$(cat "$OCTESSERA_FAST_TEST_ACTIVE" 2>/dev/null || true)" = active ]
    exit $?
fi
if [ "$1" = show ]; then
    if printf '%s\n' "$*" | grep -q -- '--value'; then
        printf '%s\n' 123
    else
        printf '%s\n' MainPID=123
    fi
fi
EOF
    chmod 0755 "$BIN_DIR/systemctl"
}

write_install_mock() {
    cat > "$BIN_DIR/install" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mode="$(cat "$OCTESSERA_FAST_TEST_MODE")"
if [ "$mode" = installfail ] && printf '%s' "$*" | grep -q -- "$OCTESSERA_FAST_TEST_CANDIDATE"; then
    exit 1
fi
exec /usr/bin/install "$@"
EOF
    chmod 0755 "$BIN_DIR/install"
}

write_fixture() {
    chmod -R u+w -- "$ROOT" 2>/dev/null || true
    rm -rf -- "$ROOT" "$PROC_ROOT" "$BIN_DIR" "$BIN_LINK" "$LOG" "$RESTART_FILE" "$ACTIVE_FILE"
    mkdir -p "$BIN_DIR" "$ROOT/releases/1.0.0" "$ROOT/runtime" "$PROC_ROOT/123" "$(dirname "$BIN_LINK")"
    cat > "$ROOT/releases/1.0.0/octessera-pi" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
    chmod 0755 "$ROOT/releases/1.0.0/octessera-pi"
    printf '%s\n' '{"schema_version":2,"updater_protocol":2,"candidate_health_protocol":1,"tag":"v1.0.0","version":"1.0.0","board_profile":"raspberry-pi-zero-2w","arch":"aarch64-unknown-linux-gnu","binary":"octessera-pi","platforms":["raspberry-pi-zero-2w","linux-aarch64-device"]}' > "$ROOT/releases/1.0.0/update-manifest.json"
    chmod 0444 "$ROOT/releases/1.0.0/update-manifest.json"
    ln -s "$ROOT/releases/1.0.0" "$ROOT/current"
    ln -s "$ROOT/current/octessera-pi" "$BIN_LINK"
    printf '%s\n' '{"schema_version":2,"phase":"committed","current":"1.0.0","previous":null,"updated_at":"2026-01-01T00:00:00Z","release":null,"asset":null}' > "$ROOT/update-state.json"
    printf '%s\n' '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}' > "$ROOT/board-profile.json"
    cat > "$CANDIDATE" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = --print-build-metadata ]; then
    printf '%s\n' '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64","package_version":"0.7.0"}'
fi
EOF
    chmod 0755 "$CANDIDATE"
    printf '%s\n' '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}' > "$CANDIDATE_METADATA"
    : > "$LOG"
    printf '%s\n' normal > "$MODE_FILE"
    printf '%s\n' 0 > "$RESTART_FILE"
    printf '%s\n' active > "$ACTIVE_FILE"
    write_systemctl_mock
    write_install_mock
}

run_helper() {
    env \
        PATH="$BIN_DIR:$PATH" \
        OCTESSERA_FAST_DEPLOY_BIN_LINK="$BIN_LINK" \
        OCTESSERA_FAST_DEPLOY_PROC_ROOT="$PROC_ROOT" \
        OCTESSERA_FAST_TEST_MODE="$MODE_FILE" \
        OCTESSERA_FAST_TEST_RESTARTS="$RESTART_FILE" \
        OCTESSERA_FAST_TEST_ACTIVE="$ACTIVE_FILE" \
        OCTESSERA_FAST_TEST_PROC="$PROC_ROOT" \
        OCTESSERA_FAST_TEST_ROOT="$ROOT" \
        OCTESSERA_FAST_TEST_WORK="$WORK" \
        OCTESSERA_FAST_TEST_LOG="$LOG" \
        OCTESSERA_FAST_TEST_CANDIDATE="$CANDIDATE" \
        python3 "$LOCK_HELPER" "$ROOT/.update.lock" "$(id -u)" "$HELPER" \
        "$ROOT" octessera.service raspberry-pi-zero-2w 1 octessera-pi \
        aarch64-unknown-linux-gnu aarch64 hardware-raspberry-pi-zero-2w \
        "$CANDIDATE" "$CANDIDATE_METADATA" 0
}

run_failure_case() {
    mode="$1"
    write_fixture
    prior_current="$(readlink "$ROOT/current")"
    prior_binary="$(readlink "$BIN_LINK")"
    cp "$ROOT/update-state.json" "$WORK/state.before"
    cp "$ROOT/board-profile.json" "$WORK/profile.before"
    printf '%s\n' "$mode" > "$MODE_FILE"
    if run_helper > "$WORK/output" 2>&1; then
        fail "$mode unexpectedly succeeded"
    fi
    assert_equal "$(readlink "$ROOT/current")" "$prior_current" "$mode changed current"
    assert_equal "$(readlink "$BIN_LINK")" "$prior_binary" "$mode changed binary link"
    assert_file_equal "$ROOT/update-state.json" "$WORK/state.before" "$mode changed updater state"
    assert_file_equal "$ROOT/board-profile.json" "$WORK/profile.before" "$mode changed board metadata"
    [ "$(find "$ROOT/releases" -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 1 ] || fail "$mode left a staging release"
    if [ "$mode" != installfail ]; then
        grep -q 'restart octessera.service' "$LOG" || fail "$mode did not restart candidate"
        grep -q 'stop octessera.service' "$LOG" || fail "$mode did not stop candidate for rollback"
        [ "$(readlink -f "$PROC_ROOT/123/exe")" = "$ROOT/releases/1.0.0/octessera-pi" ] || fail "$mode did not restore prior executable"
    fi
}

write_fixture
if ! run_helper > "$WORK/success.output" 2>&1; then
    cat "$WORK/success.output" >&2
    fail "successful deployment failed"
fi
new_release="$(basename -- "$(readlink -f "$ROOT/current")")"
case "$new_release" in
    0.0.*) ;;
    *) fail "successful deployment did not use a unique release" ;;
esac
[ "$new_release" != 1.0.0 ] || fail "successful deployment overwrote prior release"
[ -d "$ROOT/releases/$new_release" ] || fail "successful deployment removed release"
[ "$(stat -c '%a' "$ROOT/releases/$new_release")" = 555 ] || fail "successful release directory remained writable"
[ "$(stat -c '%a' "$ROOT/.update.lock")" = 600 ] || fail "deployment lock mode was not 0600"
python3 - "$ROOT" "$new_release" <<'PY'
import json
import sys
from pathlib import Path


root = Path(sys.argv[1])
release = sys.argv[2]
manifest = json.loads((root / "releases" / release / "update-manifest.json").read_text())
state = json.loads((root / "update-state.json").read_text())
assert manifest["version"] == release
assert manifest["tag"] == f"v{release}"
assert manifest["board_profile"] == "raspberry-pi-zero-2w"
assert state["current"] == release
assert state["release"] == manifest
assert state["previous"] == "1.0.0"
PY
grep -q 'Fast deployment activated' "$WORK/success.output" || fail "successful deployment did not report activation"

run_failure_case restartfail
run_failure_case statusfail
run_failure_case delayedfail
run_failure_case installfail

write_fixture
printf '%s\n' pending > "$ROOT/update-transaction.json"
prior_current="$(readlink "$ROOT/current")"
if run_helper > "$WORK/pending.output" 2>&1; then
    fail "pending updater transaction was accepted"
fi
assert_equal "$(readlink "$ROOT/current")" "$prior_current" "pending transaction changed current"
grep -q 'updater transaction' "$WORK/pending.output" || fail "pending refusal was not reported"
rm -f "$ROOT/update-transaction.json"

write_fixture
flock "$ROOT/.update.lock" sleep 2 &
holder=$!
sleep 0.1
if run_helper > "$WORK/lock.output" 2>&1; then
    kill "$holder" 2>/dev/null || true
    fail "busy updater lock was accepted"
else
    lock_status="$?"
fi
wait "$holder" || true
[ "$lock_status" -eq 75 ] || fail "busy lock refusal returned the wrong status"
[ "$(stat -c '%a' "$ROOT/.update.lock")" = 600 ] || fail "busy lock path was not normalized to mode 0600"

echo "remote fast deployment failure-path validation passed"
