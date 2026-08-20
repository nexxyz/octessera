#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/orange-pi/test-orange-pi-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-orange-pi-usb-gadget-fixture.sh"

for script in "$CANONICAL_SCRIPT" "$DEPLOYED_SCRIPT"; do
    SCRIPT=$script
    suite=$([ "$script" = "$CANONICAL_SCRIPT" ] && printf canonical || printf deployed)
    SUITE_ROOT=$TEST_ROOT/$suite
    root=$SUITE_ROOT/midi-id-only
    new_fake_configfs "$root"
    assert_failed run_setup_id_only "$root"
    assert_unbound "$root"
    root=$SUITE_ROOT/midi-write-failure
    new_fake_configfs "$root"
    assert_failed run_setup_write_failure "$root"
    assert_unbound "$root"
    root=$SUITE_ROOT/midi-short-readback
    new_fake_configfs "$root"
    assert_failed run_setup_readback_failure "$root" FAKE_MIDI_SHORT_READBACK=1
    assert_unbound "$root"
    root=$SUITE_ROOT/midi-modified-readback
    new_fake_configfs "$root"
    assert_failed run_setup_readback_failure "$root" FAKE_MIDI_MODIFIED_READBACK=1
    assert_unbound "$root"
    root=$SUITE_ROOT/post-bind-mismatch
    new_fake_configfs "$root"
    unbind_marker="$root/unbind-succeeded"
    assert_failed run_setup_post_bind_mismatch "$root" "FAKE_UNBIND_MARKER=$unbind_marker"
    assert_unbound "$root"
    test -f "$unbind_marker"
    root=$SUITE_ROOT/post-bind-mismatch-unbind-failure
    new_fake_configfs "$root"
    assert_failed run_setup_post_bind_mismatch "$root" FAKE_ROLLBACK_UNBIND_FAILURE=1
    gadget=$root/config/usb_gadget/octessera-orange-pi
    test -d "$gadget" && test -d "$gadget/configs/c.1" && test -L "$gadget/configs/c.1/midi.usb0"
done
