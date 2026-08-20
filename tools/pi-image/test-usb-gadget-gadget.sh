#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/pi-image/test-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-usb-gadget-fixture.sh"

root=$TEST_ROOT/patched-midi
new_fake_configfs "$root"
run_setup "$root" "$MIDI_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
test -L "$gadget/configs/c.1/midi.usb0"
test ! -e "$gadget/functions/uac2.usb0"
test -f "$gadget/functions/midi.usb0/interface_string"
assert_file_value 'Octessera MIDI' "$gadget/strings/0x409/product"
assert_file_value 'Octessera MIDI' "$gadget/functions/midi.usb0/id"
assert_file_value 'Octessera MIDI' "$gadget/functions/midi.usb0/function_name"
expected=$root/expected-interface-string
printf '%s' 'Octessera MIDI' > "$expected"
cmp -s "$expected" "$gadget/functions/midi.usb0/interface_string"
assert_file_value fake-udc "$gadget/UDC"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/sysfs-lf-readback
new_fake_configfs "$root"
observed=$root/underlying-interface-write
run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=lf FAKE_READBACK_OBSERVED="$observed" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
expected=$root/expected-interface-string-lf
printf '%s\n' 'Octessera MIDI' > "$expected"
cmp -s "$expected" "$gadget/functions/midi.usb0/interface_string"
expected=$root/expected-underlying-interface-write
printf '%s' 'Octessera MIDI' > "$expected"
cmp -s "$expected" "$observed"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/patched-combined
new_fake_configfs "$root"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
test -L "$gadget/configs/c.1/midi.usb0"
test -L "$gadget/configs/c.1/uac2.usb0"
assert_file_value 'Octessera Audio + MIDI' "$gadget/strings/0x409/product"
assert_file_value 3 "$gadget/functions/uac2.usb0/p_chmask"
assert_file_value 2 "$gadget/functions/uac2.usb0/p_ssize"
assert_file_value 44100 "$gadget/functions/uac2.usb0/p_srate"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/unpatched-id-only
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_MIDI_ID_ONLY=1
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/write-failure
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_MIDI_WRITE_FAILURE=1
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/wrong-readback
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=fail
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/short-readback
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=short
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/modified-readback
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=modified
test ! -e "$root/config/usb_gadget/octessera"

for mode in crlf internal two-lf spaces long; do
    root=$TEST_ROOT/malformed-readback-$mode
    new_fake_configfs "$root"
    assert_failed run_setup "$root" "$MIDI_CONFIG" "FAKE_READBACK_MODE=$mode"
    test ! -e "$root/config/usb_gadget/octessera"
done

root=$TEST_ROOT/bind-failure
new_fake_configfs "$root"
assert_failed run_setup "$root" "$COMBINED_CONFIG" FAKE_BIND_FAILURE=1
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/links-before-bind
new_fake_configfs "$root"
run_setup "$root" "$COMBINED_CONFIG" FAKE_REQUIRE_LINKS=1 FAKE_REQUIRE_AUDIO=1 > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
test -L "$gadget/configs/c.1/midi.usb0"
test -L "$gadget/configs/c.1/uac2.usb0"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/post-bind-mismatch
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_POST_BIND_MISMATCH=1 FAKE_POST_BIND_MARKER="$root/post-bind-mismatch"
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/unbind-failure
new_fake_configfs "$root"
run_setup "$root" "$MIDI_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
rm "$gadget/UDC"
mkdir "$gadget/UDC"
assert_failed run_teardown "$root" FAKE_BOUND_UDC=fake-udc
test -d "$gadget"
test -d "$gadget/functions/midi.usb0"
test -L "$gadget/configs/c.1/midi.usb0"
rm -rf "$gadget"

root=$TEST_ROOT/cleanup-failure
new_fake_configfs "$root"
run_setup "$root" "$MIDI_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
assert_failed run_teardown "$root" FAKE_RMDIR_FAILURE=1
test -d "$gadget/functions/midi.usb0"
test "$(cat "$gadget/UDC")" = ''
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"
