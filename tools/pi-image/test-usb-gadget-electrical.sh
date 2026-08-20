#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/pi-image/test-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-usb-gadget-fixture.sh"

root=$TEST_ROOT/concurrency
new_fake_configfs "$root"
started=$root/hold-started
release=$root/hold-release
calls=$root/modprobe-calls
run_setup "$root" "$AUDIO_CONFIG" FAKE_HOLD_STARTED="$started" FAKE_HOLD_RELEASE="$release" FAKE_MODPROBE_CALLS="$calls" > "$root/first.log" 2>&1 &
first_pid=$!
while [ ! -e "$started" ]; do sleep 0.01; done
run_setup "$root" "$AUDIO_CONFIG" FAKE_HOLD_STARTED="$started" FAKE_HOLD_RELEASE="$release" FAKE_MODPROBE_CALLS="$calls" > "$root/second.log" 2>&1 &
second_pid=$!
sleep 0.1
test "$(wc -l < "$calls" | tr -d '[:space:]')" -eq 1
: > "$release"
wait "$first_pid"
wait "$second_pid"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$root/config/usb_gadget/octessera"
