#!/usr/bin/env bash
# shellcheck disable=SC2016,SC2251
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
hook="$root/tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/hooks/octessera-boot-splash"
script="$root/tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"

sh -n "$hook" "$script"
[[ "$(sh "$hook" prereqs)" == "" ]]
[[ "$(sh "$script" prereqs)" == "" ]]
for required_line in \
    'runtime_source="$(readlink -f /usr/local/bin/octessera-pi)"' \
    'if [ -z "$runtime_source" ] || [ ! -f "$runtime_source" ] || [ -L "$runtime_source" ] || [ ! -x "$runtime_source" ]; then' \
    'copy_exec "$runtime_source" /usr/local/bin/octessera-pi' \
    'copy_exec /usr/bin/setsid /usr/bin/setsid' \
    'copy_exec /usr/bin/dash /usr/bin/dash' \
    'ln -s dash "$DESTDIR/usr/bin/sh"' \
    'copy_exec /usr/bin/sleep /usr/bin/sleep' \
    'copy_exec /usr/bin/cat /usr/bin/cat' \
    'copy_exec /usr/bin/mv /usr/bin/mv' \
    'copy_exec /usr/bin/chmod /usr/bin/chmod' \
    'copy_exec /usr/bin/chown /usr/bin/chown' \
    'copy_exec /usr/bin/rm /usr/bin/rm' \
    'manual_add_modules spi-bcm2835 || true' \
    'manual_add_modules spidev || true'; do
    grep -qFx "$required_line" "$hook"
done
python3 - "$hook" <<'PY'
import sys
from pathlib import Path

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
commands = ("setsid", "dash", "sleep", "cat", "mv", "chmod", "chown", "rm")
copy_lines = [line.strip() for line in lines if line.strip().startswith("copy_exec /usr/bin/")]
expected = {f"copy_exec /usr/bin/{command} /usr/bin/{command}" for command in commands}
assert len(copy_lines) == len(expected)
assert set(copy_lines) == expected
copies = {
    line.strip(): index
    for index, line in enumerate(lines)
    if line.strip().startswith("copy_exec /usr/bin/")
}
assert set(copies) == expected
assert len(copies) == len(expected)
for command in commands:
    copy = f"copy_exec /usr/bin/{command} /usr/bin/{command}"
    index = copies[copy]
    assert index > 0
    assert lines[index - 1].strip() == f'rm -f "$DESTDIR/usr/bin/{command}"'
dash_index = copies["copy_exec /usr/bin/dash /usr/bin/dash"]
assert lines[dash_index + 1].strip() == 'rm -f "$DESTDIR/usr/bin/sh"'
assert lines[dash_index + 2].strip() == 'ln -s dash "$DESTDIR/usr/bin/sh"'
PY
grep -qFx '    setsid /usr/local/bin/octessera-pi --boot-splash-static >/dev/kmsg 2>&1 &' "$script"
grep -qF 'setsid /bin/sh -c' "$script"
grep -qF 'sleep 3' "$script"
grep -qF 'kill -TERM "-$1"' "$script"
grep -qF 'kill -KILL "-$1"' "$script"
grep -qF 'kill -TERM "-$group_pid"' "$script"
grep -qF 'kill -KILL "-$group_pid"' "$script"
grep -qF 'trap cleanup EXIT' "$script"
grep -qF 'trap interrupt INT TERM HUP' "$script"
octessera_reject_file_match 'Raspberry initramfs splash must not use marker or handoff state.' -qiE 'marker|handoff|OCTESSERA_INITRAMFS' "$script"
octessera_reject_file_match 'Raspberry initramfs splash must not use the removed one-shot command.' -qF -- '--boot-splash-once' "$script"
octessera_reject_file_match 'Raspberry boot service must not use the removed one-shot command.' -qF -- '--boot-splash-once' "$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-boot-splash.service"

command -v setsid >/dev/null 2>&1
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
run="$work/run"
mkdir -p "$run"
fake="$work/octessera-pi"
child_file="$work/child.pid"
cat > "$fake" <<'EOF'
#!/bin/sh
set -eu
if [ "${FAKE_MODE:-success}" = timeout ]; then
    sleep 1000 &
    child="$!"
    printf '%s\n' "$child" > "$FAKE_CHILD_FILE"
    printf '%s\n' "$$" > "$FAKE_PID_FILE"
    trap 'exit 143' INT TERM HUP
    while :; do sleep 1; done
fi
if [ "${FAKE_MODE:-success}" = failure ]; then
    exit 7
fi
exit 0
EOF
chmod 0755 "$fake"
fixture="$work/init-premount"
sed \
    -e "s|/usr/local/bin/octessera-pi|$fake|g" \
    -e "s|/dev/kmsg|$work/kmsg|g" \
    "$script" > "$fixture"
chmod 0755 "$fixture"

run_case() {
    mode="$1"
    FAKE_MODE="$mode" FAKE_CHILD_FILE="$child_file" FAKE_PID_FILE="$work/fake.pid" sh "$fixture"
}

assert_process_absent() {
    local pid="$1" status state
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        if kill -0 "$pid" >/dev/null 2>&1; then
            state="$(ps -o stat= -p "$pid" 2>/dev/null)" || {
                echo "Unable to inspect process state: $pid." >&2
                return 1
            }
            [[ "$state" == Z* ]] && return 0
        else
            status=$?
            [[ "$status" == 1 ]] && return 0
            echo "Unable to inspect process: $pid (kill status $status)." >&2
            return 1
        fi
        sleep 0.05
    done
    echo "Process remained alive: $pid" >&2
    return 1
}

run_case success
[[ ! -e "$run/octessera-initramfs-splash.ready" ]]

run_case timeout
child_pid="$(cat "$child_file")"
sleep 0.1
assert_process_absent "$child_pid"
animator_pid="$(cat "$work/fake.pid")"
assert_process_absent "$animator_pid"

run_case failure

FAKE_MODE=timeout FAKE_CHILD_FILE="$child_file" FAKE_PID_FILE="$work/fake.pid" setsid sh "$fixture" &
parent_pid="$!"
sleep 0.2
kill -TERM "$parent_pid" >/dev/null 2>&1 || true
wait "$parent_pid" || true
child_pid="$(cat "$child_file")"
sleep 0.1
assert_process_absent "$child_pid"

printf '%s\n' 'Raspberry static initramfs splash watchdog and closure tests passed'
