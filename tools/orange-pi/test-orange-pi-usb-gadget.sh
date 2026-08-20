#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
for module in \
    test-orange-pi-usb-gadget-host-enumeration.sh \
    test-orange-pi-usb-gadget-function.sh \
    test-orange-pi-usb-gadget-passive.sh \
    test-orange-pi-usb-gadget-electrical.sh; do
    sh "$SCRIPT_DIR/$module"
done

printf 'Orange Pi USB gadget fake-configfs tests passed\n'
