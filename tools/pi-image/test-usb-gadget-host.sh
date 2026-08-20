#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/pi-image/test-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-usb-gadget-fixture.sh"

root=$TEST_ROOT/storage-round-trip
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
gadget=$root/config/usb_gadget/octessera
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
test -L "$gadget/configs/c.1/mass_storage.usb0"
test ! -e "$gadget/configs/c.1/midi.usb0"
test ! -e "$gadget/configs/c.1/uac2.usb0"
test ! -e "$root/mounted"
grep -q '^DEV=/dev/loop0$' "$root/storage.state"
grep -q '^WAS_MOUNTED=1$' "$root/storage.state"
run_storage_action "$root" storage-stop "$COMBINED_CONFIG" > "$root/stop-storage.log"
test -L "$gadget/configs/c.1/midi.usb0"
test -L "$gadget/configs/c.1/uac2.usb0"
assert_file_value fake-udc "$gadget/UDC"
test -e "$root/mounted"
test ! -e "$root/storage.state"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/storage-naming-restore-failure
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
assert_failed run_storage_action "$root" storage-stop "$COMBINED_CONFIG" FAKE_MIDI_ID_ONLY=1
test -e "$root/mounted"
test ! -e "$root/storage.state"
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/storage-unbind-failure
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
gadget=$root/config/usb_gadget/octessera
rm "$gadget/UDC"
mkdir "$gadget/UDC"
assert_failed run_storage_action "$root" storage-stop "$COMBINED_CONFIG" FAKE_BOUND_UDC=fake-udc
test ! -e "$root/mounted"
test -e "$root/storage.state"
test -L "$gadget/configs/c.1/mass_storage.usb0"
test -d "$gadget/functions/mass_storage.usb0"
rm -rf "$gadget"

root=$TEST_ROOT/storage-cleanup-failure
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
gadget=$root/config/usb_gadget/octessera
assert_failed run_storage_action "$root" storage-stop "$COMBINED_CONFIG" FAKE_RMDIR_FAILURE=1
test ! -e "$root/mounted"
test -e "$root/storage.state"
test -d "$gadget/functions/mass_storage.usb0"
rm -rf "$gadget"
