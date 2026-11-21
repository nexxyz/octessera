#!/usr/bin/env bash
# Live Show Mode counts powered time, not wall-clock time: drive both boards'
# octessera-show-hold-apply through start, ticks, a reboot, a restart and End
# with stubbed systemd, rfkill and nmcli, and check the timers it asks for.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"
cat >"$work/bin/stub" <<'EOF'
#!/bin/sh
echo "$(basename "$0") $*" >> "$STUB_LOG"
EOF
chmod +x "$work/bin/stub"
for command in nmcli systemctl systemd-run rfkill; do
    cp "$work/bin/stub" "$work/bin/$command"
done
export STUB_LOG="$work/log"
script=""
dir=""

fail() {
    echo "FAIL: $script: $1" >&2
    cat "$STUB_LOG" >&2
    exit 1
}

apply() {
    : >"$STUB_LOG"
    PATH="$work/bin:$PATH" OCTESSERA_SHOW_HOLD_DIR="$dir" sh "$script" "$@"
}

expect() {
    grep -qF -- "$1" "$STUB_LOG" || fail "expected '$1'"
}

show_id() {
    stat -c '%i %Y' "$dir/length"
}

start_show() {
    printf '%s\n' "$1" >"$dir/length"
    touch -d @1000 "$dir/length"
}

check_board() {
    script="$1"
    dir="$work/hold-$2"
    mkdir -p "$dir"

    start_show 172800
    apply
    expect "rfkill block wlan"
    expect "systemctl stop octessera-show-hold-release-*.timer"
    expect "--unit=octessera-show-hold-release-"
    expect "--on-active=300"
    expect "tick 300 $(show_id)"
    if [ -e "$dir/elapsed" ]; then fail "a fresh show banked time"; fi

    apply tick 300 "$(show_id)"
    [ "$(sed -n 2p "$dir/elapsed")" = 300 ] || fail "a tick did not bank its five minutes"

    printf '%s\n%s\n' "$(show_id)" 172600 >"$dir/elapsed"
    apply
    expect "rfkill block wlan"
    expect "--on-active=200"

    apply tick 200 "$(show_id)"
    expect "rfkill unblock wlan"
    if [ -e "$dir/length" ] || [ -e "$dir/elapsed" ]; then fail "an expired show left its files"; fi

    start_show 5000
    local old_show
    old_show="$(show_id)"
    printf '%s\n%s\n' "$old_show" 4000 >"$dir/elapsed"
    touch -d @2000 "$dir/length"
    apply tick 600 "$old_show"
    expect "--on-active=300"

    rm "$dir/length"
    apply
    expect "rfkill unblock wlan"
    if [ -e "$dir/elapsed" ]; then fail "End left banked time behind"; fi
}

check_board "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-show-hold-apply" raspberry
expect "systemctl start octessera-network-health.timer"
check_board "$root/userpatches/overlay/usr/local/sbin/octessera-show-hold-apply" orange
echo "show-hold apply checks passed"
