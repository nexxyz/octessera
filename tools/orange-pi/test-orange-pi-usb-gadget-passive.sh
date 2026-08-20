#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=tools/orange-pi/test-orange-pi-usb-gadget-fixture.sh
. "$SCRIPT_DIR/test-orange-pi-usb-gadget-fixture.sh"

test -f "$DEPLOYED_SCRIPT"
cmp -s "$CANONICAL_SCRIPT" "$DEPLOYED_SCRIPT" || {
    printf 'deployed Orange Pi USB gadget is not the canonical implementation\n' >&2
    exit 1
}
for script in "$CANONICAL_SCRIPT" "$DEPLOYED_SCRIPT"; do
    SCRIPT=$script
    suite=$([ "$script" = "$CANONICAL_SCRIPT" ] && printf canonical || printf deployed)
    SUITE_ROOT=$TEST_ROOT/$suite
    root=$SUITE_ROOT/no-gadget
    new_fake_configfs "$root"
    run_setup_command "$root" none > "$root/setup.log"
    test ! -e "$root/config/usb_gadget/octessera-orange-pi"
    root=$SUITE_ROOT/invalid-config
    new_fake_configfs "$root"
    assert_failed run_setup_command "$root" invalid
    root=$SUITE_ROOT/malformed-config
    new_fake_configfs "$root"
    cp "$TEST_ROOT/config-malformed.json" "$TEST_ROOT/config-invalid.json"
    assert_failed run_setup_command "$root" invalid
    root=$SUITE_ROOT/conflict-config
    new_fake_configfs "$root"
    cp "$TEST_ROOT/config-conflict.json" "$TEST_ROOT/config-invalid.json"
    assert_failed run_setup_command "$root" invalid
    root=$SUITE_ROOT/existing
    new_fake_configfs "$root"
    mkdir "$root/config/usb_gadget/octessera-orange-pi"
    assert_failed run_setup "$root" midi
    root=$SUITE_ROOT/prebound
    new_fake_configfs "$root"
    mkdir "$root/config/usb_gadget/other-gadget"
    printf 'musb-hdrc.4.auto\n' > "$root/config/usb_gadget/other-gadget/UDC"
    assert_failed run_setup "$root" midi
    root=$SUITE_ROOT/missing-udc
    mkdir -p "$root/config/usb_gadget" "$root/udc"
    assert_failed sh "$SCRIPT" setup --configfs-root "$root/config" --udc-root "$root/udc" --lock-file "$root/lifecycle.lock" --mode midi
    root=$SUITE_ROOT/mass-storage
    new_fake_configfs "$root"
    assert_failed run_setup "$root" mass-storage
    root=$SUITE_ROOT/mismatch
    new_fake_configfs "$root"
    run_setup "$root" midi > /dev/null
    gadget=$root/config/usb_gadget/octessera-orange-pi
    grep -F -q musb-hdrc.4.auto "$gadget/UDC"
    run_teardown "$root" > /dev/null
    root=$SUITE_ROOT/unexpected-udc
    mkdir -p "$root/config/usb_gadget" "$root/udc/other-udc"
    assert_failed "$SCRIPT" setup --configfs-root "$root/config" --udc-root "$root/udc" --lock-file "$root/lifecycle.lock" --mode midi
    bind_line=$(grep -n "printf .*\"\$UDC\" > \"\$GADGET/UDC\"" "$SCRIPT" | cut -d: -f1)
    link_line=$(grep -n "ln -s .\\\$GADGET/functions" "$SCRIPT" | tail -n 1 | cut -d: -f1)
    test -n "$bind_line" && test -n "$link_line" && test "$bind_line" -gt "$link_line"
done
