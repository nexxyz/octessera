#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
for module in \
    test-usb-gadget-layout.sh \
    test-usb-gadget-gadget.sh \
    test-usb-gadget-host.sh \
    test-usb-gadget-electrical.sh; do
    sh "$SCRIPT_DIR/$module"
done

printf '%s\n' 'Raspberry USB gadget fake-ConfigFS tests passed.'
