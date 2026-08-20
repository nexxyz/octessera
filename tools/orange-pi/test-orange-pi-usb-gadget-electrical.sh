#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/orange-pi/test-orange-pi-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-orange-pi-usb-gadget-fixture.sh"

for script in "$CANONICAL_SCRIPT" "$DEPLOYED_SCRIPT"; do
    SCRIPT=$script
    suite=$([ "$script" = "$CANONICAL_SCRIPT" ] && printf canonical || printf deployed)
    SUITE_ROOT=$TEST_ROOT/$suite
    root=$SUITE_ROOT/pre-bind-failure
    new_fake_configfs "$root"
    unbind_marker="$root/unbind-attempted"
    assert_failed run_setup_prebind_failure "$root" "$unbind_marker"
    assert_unbound "$root"
    test ! -e "$unbind_marker"
    root=$SUITE_ROOT/bind-write-failure
    new_fake_configfs "$root"
    unbind_marker="$root/unbind-attempted"
    assert_failed run_setup_bind_failure "$root" "$unbind_marker"
    assert_unbound "$root"
    test ! -e "$unbind_marker"
    root=$SUITE_ROOT/unbind-write-failure
    new_fake_configfs "$root"
    run_setup "$root" combined >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    rm "$gadget/UDC"
    mkdir "$gadget/UDC"
    output=$root/unbind-write-error.log
    if run_teardown_with_unbind_write_error "$root" "$output"; then
        printf 'expected unbind write failure\n' >&2
        exit 1
    fi
    test -d "$gadget"
    test -d "$gadget/configs/c.1"
    grep -F -q musb-hdrc.4.auto "$root/udc/musb-hdrc.4.auto/function"
    test "$(wc -l < "$output" | tr -d ' ')" -eq 1
    test "$(grep -Fc 'Is a directory' "$output")" -eq 1
    root=$SUITE_ROOT/concurrency
    new_fake_configfs "$root"
    lock_ready="$root/lock-ready"
    flock "$root/lifecycle.lock" sh -c "printf ready > \"\$1\"; sleep 1" sh "$lock_ready" &
    holder=$!
    while [ ! -e "$lock_ready" ]; do sleep 0.01; done
    assert_failed run_setup "$root" combined
    wait "$holder"
    run_setup "$root" combined >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    run_teardown "$root" >/dev/null
    test ! -e "$gadget"
    root=$SUITE_ROOT/rebind
    new_fake_configfs "$root"
    run_setup "$root" uac2 >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    test -L "$gadget/configs/c.1/uac2.usb0"
    unbind_marker="$root/unbind-succeeded"
    run_teardown "$root" "$unbind_marker" >/dev/null
    test -f "$unbind_marker"
    test ! -e "$gadget"
    run_setup "$root" combined >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    test -L "$gadget/configs/c.1/uac2.usb0"
    test -L "$gadget/configs/c.1/midi.usb0"
    assert_midi_interface "$gadget/functions/midi.usb0/interface_string"
    run_teardown "$root" >/dev/null
    test ! -e "$gadget"
    run_setup "$root" midi >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    assert_midi_interface "$gadget/functions/midi.usb0/interface_string"
    run_teardown "$root" >/dev/null
    test ! -e "$gadget"
    root=$SUITE_ROOT/disappeared-udc
    new_fake_configfs "$root"
    run_setup "$root" uac2 >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    rm -rf "$root/udc/musb-hdrc.4.auto"
    run_teardown "$root" >/dev/null
    test ! -e "$gadget"
    mkdir -p "$root/udc/musb-hdrc.4.auto"
    run_setup "$root" combined >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    test -L "$gadget/configs/c.1/uac2.usb0"
    test -L "$gadget/configs/c.1/midi.usb0"
    run_teardown "$root" >/dev/null
    test ! -e "$gadget"
    root=$SUITE_ROOT/already-unbound
    new_fake_configfs "$root"
    run_setup "$root" uac2 >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    : > "$gadget/UDC"
    unbind_marker="$root/unbind-succeeded"
    run_teardown "$root" "$unbind_marker" >/dev/null
    test ! -e "$unbind_marker"
    test ! -e "$gadget"
    root=$SUITE_ROOT/stale-partial
    new_fake_configfs "$root"
    gadget=$root/config/usb_gadget/octessera-orange-pi
    mkdir -p "$gadget/configs/c.1/strings/0x409" "$gadget/functions/uac2.usb0" "$gadget/strings/0x409" "$gadget/os_desc" "$gadget/webusb"
    : > "$gadget/UDC"
    ln -s "$gadget/configs/c.1" "$gadget/os_desc/c.1"
    unbind_marker="$root/unbind-attempted"
    run_teardown "$root" "$unbind_marker" >/dev/null
    test ! -e "$unbind_marker"
    test ! -e "$gadget"
    run_setup "$root" combined >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    test -L "$gadget/configs/c.1/uac2.usb0"
    test -L "$gadget/configs/c.1/midi.usb0"
    run_teardown "$root" >/dev/null
    test ! -e "$gadget"
    root=$SUITE_ROOT/unknown-child
    new_fake_configfs "$root"
    run_setup "$root" combined >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    mkdir "$gadget/functions/foreign.usb0"
    assert_failed run_teardown "$root"
    test -d "$gadget"
    test -d "$gadget/functions/foreign.usb0"
    test -z "$(cat "$root/udc/musb-hdrc.4.auto/function")"
    root=$SUITE_ROOT/unbind-error
    new_fake_configfs "$root"
    gadget=$root/config/usb_gadget/octessera-orange-pi
    mkdir "$gadget"
    mkdir "$gadget/UDC"
    assert_failed run_teardown "$root"
    test -d "$gadget"
    root=$SUITE_ROOT/rmdir-error
    new_fake_configfs "$root"
    run_setup "$root" midi >/dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    assert_failed run_teardown_with_rmdir_error "$root"
    test -d "$gadget"
done
