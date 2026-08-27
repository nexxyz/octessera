#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/pi-image/test-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-usb-gadget-fixture.sh"

for outputs in 100 010 001 110 101 011 111; do
    config="$TEST_ROOT/audio-$outputs.json"
    usb=${outputs#?}; usb=${usb%?}
    root="$TEST_ROOT/audio-set-$outputs"
    new_fake_configfs "$root"
    run_setup "$root" "$config" > "$root/setup.log"
    if [ "$usb" = 1 ]; then test -L "$root/config/usb_gadget/octessera/configs/c.1/uac2.usb0"; else test ! -e "$root/config/usb_gadget/octessera/functions/uac2.usb0"; fi
    run_teardown "$root" > "$root/teardown.log"
done
for config in audio-zero.json audio-extra.json audio-legacy-only.json audio-mixed.json audio-malformed.json; do
    root="$TEST_ROOT/reject-${config%.json}"
    new_fake_configfs "$root"
    assert_failed run_setup "$root" "$TEST_ROOT/$config"
    test ! -e "$root/config/usb_gadget/octessera"
done
