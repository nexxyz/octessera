#!/usr/bin/env bash
# shellcheck disable=SC2016,SC2251
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
hook="$root/tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/hooks/octessera-boot-splash"
script="$root/tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"

sh -n "$hook" "$script"
[[ "$(sh "$hook" prereqs)" == "" ]]
[[ "$(sh "$script" prereqs)" == "" ]]
for required_line in \
    'copy_exec /usr/local/bin/octessera-pi /usr/local/bin/octessera-pi' \
    'copy_exec /usr/bin/setsid /usr/bin/setsid' \
    'copy_exec /bin/sh /bin/sh' \
    'copy_exec /bin/sleep /bin/sleep' \
    'copy_exec /bin/cat /bin/cat' \
    'copy_exec /bin/mv /bin/mv' \
    'copy_exec /bin/chmod /bin/chmod' \
    'copy_exec /bin/chown /bin/chown' \
    'copy_exec /bin/rm /bin/rm' \
    'manual_add_modules spi-bcm2835 || true' \
    'manual_add_modules spidev || true'; do
    grep -qFx "$required_line" "$hook"
done
grep -qFx '    OCTESSERA_INITRAMFS_BOOT_SPLASH=1 setsid /usr/local/bin/octessera-pi --boot-splash-once >/dev/kmsg 2>&1 &' "$script"
grep -qF 'setsid /bin/sh -c' "$script"
grep -qF 'sleep 3' "$script"
grep -qF 'kill -TERM "-$1"' "$script"
grep -qF 'kill -KILL "-$1"' "$script"
grep -qF 'kill -TERM "-$group_pid"' "$script"
grep -qF 'kill -KILL "-$group_pid"' "$script"
grep -qF 'trap cleanup EXIT' "$script"
grep -qF 'trap interrupt INT TERM HUP' "$script"
grep -qF 'chmod 0644 -- "$marker_tmp"' "$script"
grep -qF 'chown 0:0 -- "$marker_tmp"' "$script"
grep -qF 'mv -f -- "$marker_tmp" /run/octessera-initramfs-splash.ready' "$script"
! grep -qF 'kill "$splash_pid"' "$script"
! grep -qF -- '--boot-splash-once >/dev/kmsg 2>&1 &' "$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-boot-splash.service"

command -v setsid >/dev/null 2>&1
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
run="$work/run"
mkdir -p "$run"
boot_id="$work/boot_id"
printf '%s\n' '01234567-89ab-cdef-0123-456789abcdef' > "$boot_id"
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
    -e "s|/run/|$run/|g" \
    -e "s|/proc/sys/kernel/random/boot_id|$boot_id|g" \
    -e "s|/dev/kmsg|$work/kmsg|g" \
    -e 's#    chown 0:0 -- "$marker_tmp" || return 1#    :#' \
    "$script" > "$fixture"
chmod 0755 "$fixture"

run_case() {
    mode="$1"
    FAKE_MODE="$mode" FAKE_CHILD_FILE="$child_file" FAKE_PID_FILE="$work/fake.pid" sh "$fixture"
}

run_case success
python3 - "$run/octessera-initramfs-splash.ready" <<'PY'
import json
import os
import stat
import sys
from pathlib import Path

path = Path(sys.argv[1])
value = json.loads(path.read_text(encoding="utf-8"))
assert value == {"schema": 1, "bootId": "01234567-89ab-cdef-0123-456789abcdef"}
assert stat.S_IMODE(path.stat().st_mode) == 0o644
if os.geteuid() == 0:
    assert path.stat().st_uid == 0 and path.stat().st_gid == 0
PY
rm -f "$run/octessera-initramfs-splash.ready"

run_case timeout
[[ ! -e "$run/octessera-initramfs-splash.ready" ]]
child_pid="$(cat "$child_file")"
sleep 0.1
! kill -0 "$child_pid" >/dev/null 2>&1
animator_pid="$(cat "$work/fake.pid")"
! kill -0 "$animator_pid" >/dev/null 2>&1

run_case failure
[[ ! -e "$run/octessera-initramfs-splash.ready" ]]

printf '%s\n' '{"schema":1,"bootId":"stale"}' > "$run/octessera-initramfs-splash.ready"
run_case timeout
[[ ! -e "$run/octessera-initramfs-splash.ready" ]]

run_case timeout &
parent_pid="$!"
sleep 0.2
kill -TERM "$parent_pid" >/dev/null 2>&1 || true
wait "$parent_pid" || true
child_pid="$(cat "$child_file")"
sleep 0.1
! kill -0 "$child_pid" >/dev/null 2>&1
[[ ! -e "$run/octessera-initramfs-splash.ready" ]]

printf '%s\n' 'Raspberry initramfs watchdog, process-group, marker, and closure tests passed'
