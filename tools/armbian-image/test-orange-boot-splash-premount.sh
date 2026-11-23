#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
premount_script="$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
initramfs_shell="${OCTESSERA_INITRAMFS_SHELL:-/usr/lib/klibc/bin/sh}"

[[ -x "$initramfs_shell" ]] || { echo "The Orange init-premount test needs the initramfs shell $initramfs_shell (klibc-utils)." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "The Orange init-premount test needs python3." >&2; exit 1; }
command -v setsid >/dev/null 2>&1 || { echo "The Orange init-premount test needs setsid." >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

helper="$work/oled-logo"
cat > "$helper" <<'EOF'
#!/usr/bin/env python3
import os
import sys
import time

with open(os.environ["OCTESSERA_TEST_HELPER_PID"], "w") as handle:
    handle.write(str(os.getpid()))
time.sleep(float(os.environ["OCTESSERA_TEST_HELPER_SECONDS"]))
sys.exit(0)
EOF
chmod 0755 "$helper"

script="$work/premount"
sed \
    -e "s|/usr/local/sbin/octessera-orange-oled-logo|$helper|g" \
    -e "s|/usr/bin/python3|$(command -v python3)|g" \
    -e "s|/usr/bin/setsid|$(command -v setsid)|g" \
    -e "s|/bin/sh -c|$initramfs_shell -c|" \
    -e "s|/dev/kmsg|$work/kmsg|" \
    "$premount_script" > "$script"
grep -qF "$helper boot-static" "$script" || { echo "The init-premount helper invocation changed; update this test." >&2; exit 1; }
grep -qF "$initramfs_shell -c" "$script" || { echo "The init-premount watchdog invocation changed; update this test." >&2; exit 1; }

run_premount() {
    local helper_seconds="$1"
    local limit_seconds="$2"
    local description="$3"
    local started
    local elapsed
    local status
    local helper_pid

    rm -f "$work/helper.pid"
    started="$(date +%s%N)"
    set +e
    OCTESSERA_TEST_HELPER_PID="$work/helper.pid" OCTESSERA_TEST_HELPER_SECONDS="$helper_seconds" \
        timeout --kill-after=2 15 setsid "$initramfs_shell" "$script" > "$work/premount.out" 2>&1
    status=$?
    set -e
    elapsed=$(( ($(date +%s%N) - started) / 1000000 ))
    if [[ "$status" != 0 ]]; then
        echo "Orange init-premount splash $description exited with status $status after ${elapsed} ms." >&2
        cat "$work/premount.out" >&2
        exit 1
    fi
    if (( elapsed > limit_seconds * 1000 )); then
        echo "Orange init-premount splash $description took ${elapsed} ms (limit ${limit_seconds} s)." >&2
        exit 1
    fi
    [[ -s "$work/helper.pid" ]] || { echo "Orange init-premount splash $description never started the helper." >&2; exit 1; }
    helper_pid="$(cat "$work/helper.pid")"
    if kill -0 "$helper_pid" 2>/dev/null; then
        echo "Orange init-premount splash $description left the helper running." >&2
        kill -KILL "$helper_pid" 2>/dev/null || true
        exit 1
    fi
    printf 'Orange init-premount splash %s finished in %s ms\n' "$description" "$elapsed"
}

run_premount 0.2 2 'with a helper faster than the watchdog'
run_premount 30 6 'with a helper the watchdog must stop'

printf 'Orange init-premount splash tests passed\n'
