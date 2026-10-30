#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/orange-pi/test-orange-pi-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-orange-pi-usb-gadget-fixture.sh"

for script in "$CANONICAL_SCRIPT" "$DEPLOYED_SCRIPT"; do
    SCRIPT=$script
    if [ "$script" = "$CANONICAL_SCRIPT" ]; then suite=canonical; else suite=deployed; fi
    SUITE_ROOT=$TEST_ROOT/$suite
    for mode in midi uac2 combined; do
        root=$SUITE_ROOT/$mode
        new_fake_configfs "$root"
        run_setup "$root" "$mode" > "$root/setup.log"
        gadget=$root/config/usb_gadget/octessera-orange-pi
        test -d "$gadget"
        test -d "$gadget/configs"
        test -d "$gadget/functions"
        test -d "$gadget/strings"
        test -d "$gadget/os_desc"
        test -d "$gadget/webusb"
        test -d "$gadget/configs/c.1/strings"
        assert_failed "$FAKE_BIN/rmdir" -- "$gadget/functions"
        assert_file_value 'Octessera (Orange 1a2f)' "$gadget/strings/0x409/product"
        assert_file_value Octessera "$gadget/strings/0x409/manufacturer"
        assert_file_value octessera-orange-pi "$gadget/strings/0x409/serialnumber"
        assert_file_value 0x1d6b "$gadget/idVendor"
        assert_file_value 0x0104 "$gadget/idProduct"
        assert_file_value 'Octessera (Orange 1a2f)' "$gadget/configs/c.1/strings/0x409/configuration"
        if [ "$mode" = midi ] || [ "$mode" = combined ]; then
            test -d "$gadget/functions/midi.usb0"
            assert_file_value 'Octessera MIDI' "$gadget/functions/midi.usb0/id"
            assert_file_value 'Octessera (Orange 1a2f)' "$gadget/functions/midi.usb0/function_name"
            assert_midi_interface "$gadget/functions/midi.usb0/interface_string"
        else
            test ! -e "$gadget/functions/midi.usb0"
        fi
        if [ "$mode" = uac2 ] || [ "$mode" = combined ]; then
            test -d "$gadget/functions/uac2.usb0"
            assert_file_value 'Octessera (Orange 1a2f)' "$gadget/functions/uac2.usb0/function_name"
        else
            test ! -e "$gadget/functions/uac2.usb0"
        fi
        test ! -e "$gadget/functions/mass_storage.usb0"
        if [ "$mode" = midi ] || [ "$mode" = combined ]; then test -L "$gadget/configs/c.1/midi.usb0"; else test ! -e "$gadget/configs/c.1/midi.usb0"; fi
        if [ "$mode" = uac2 ] || [ "$mode" = combined ]; then test -L "$gadget/configs/c.1/uac2.usb0"; else test ! -e "$gadget/configs/c.1/uac2.usb0"; fi
        grep -F -q musb-hdrc.4.auto "$gadget/UDC"
        run_teardown "$root" > "$root/teardown.log"
        test ! -e "$gadget"
    done
    root=$SUITE_ROOT/machine-id-suffix
    new_fake_configfs "$root"
    rm "$root/net/wlan0/address"
    printf '%s\n' 0123456789abcdef0123456789abbeef > "$root/machine-id"
    run_setup "$root" combined > "$root/setup.log"
    gadget=$root/config/usb_gadget/octessera-orange-pi
    assert_file_value 'Octessera (Orange beef)' "$gadget/strings/0x409/product"
    run_teardown "$root" > "$root/teardown.log"
    test ! -e "$gadget"
done
