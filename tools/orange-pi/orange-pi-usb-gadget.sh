#!/bin/sh

set -eu

GADGET_NAME=octessera-orange-pi
CONFIGFS_ROOT=/sys/kernel/config
UDC_ROOT=/sys/class/udc
ACTION=
MODE=
UDC=

usage() {
    cat <<EOF
Usage:
  $0 setup --udc <name> --mode <midi|uac2|combined> [options]
  $0 teardown --udc <name> [options]

Options:
  --configfs-root <path>  Configfs mount root (default: /sys/kernel/config)
  --udc-root <path>       UDC sysfs root (default: /sys/class/udc)
  -h, --help              Show this help
EOF
}

die() {
    printf 'orange-pi-usb-gadget: %s\n' "$1" >&2
    exit 1
}

require_value() {
    [ "$#" -ge 2 ] || die "missing value for $1"
    [ -n "$2" ] || die "empty value for $1"
}

[ "$#" -ge 1 ] || { usage >&2; exit 2; }
ACTION=$1
shift

case "$ACTION" in
    setup|teardown)
        ;;
    -h|--help)
        usage
        exit 0
        ;;
    *)
        printf 'unknown action: %s\n' "$ACTION" >&2
        usage >&2
        exit 2
        ;;
esac

while [ "$#" -gt 0 ]; do
    case "$1" in
        --udc)
            require_value "$@"
            UDC=$2
            shift 2
            ;;
        --udc=*)
            UDC=${1#*=}
            [ -n "$UDC" ] || die "empty value for --udc"
            shift
            ;;
        --mode)
            require_value "$@"
            MODE=$2
            shift 2
            ;;
        --mode=*)
            MODE=${1#*=}
            [ -n "$MODE" ] || die "empty value for --mode"
            shift
            ;;
        --configfs-root)
            require_value "$@"
            CONFIGFS_ROOT=$2
            shift 2
            ;;
        --configfs-root=*)
            CONFIGFS_ROOT=${1#*=}
            [ -n "$CONFIGFS_ROOT" ] || die "empty value for --configfs-root"
            shift
            ;;
        --udc-root)
            require_value "$@"
            UDC_ROOT=$2
            shift 2
            ;;
        --udc-root=*)
            UDC_ROOT=${1#*=}
            [ -n "$UDC_ROOT" ] || die "empty value for --udc-root"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

[ -n "$UDC" ] || die "--udc is required; automatic UDC selection is disabled"
case "$UDC" in
    */*|.|..)
        die "invalid UDC name: $UDC"
        ;;
esac

GADGET_ROOT=$CONFIGFS_ROOT/usb_gadget
GADGET=$GADGET_ROOT/$GADGET_NAME
UDC_PATH=$UDC_ROOT/$UDC

if [ "$ACTION" = setup ]; then
    [ -n "$MODE" ] || die "--mode is required for setup"
    case "$MODE" in
        midi|uac2|combined)
            ;;
        *)
            die "unsupported mode: $MODE"
            ;;
    esac
else
    [ -z "$MODE" ] || die "--mode is valid only for setup"
fi

require_configfs() {
    [ -d "$GADGET_ROOT" ] || die "configfs USB gadget directory is missing: $GADGET_ROOT"
}

require_udc() {
    [ -e "$UDC_PATH" ] || die "UDC was not found: $UDC"
}

read_bound_udc() {
    [ -r "$1/UDC" ] || return 0
    cat "$1/UDC" 2>/dev/null || true
}

refuse_existing_gadgets() {
    for candidate in "$GADGET_ROOT"/*; do
        [ -d "$candidate" ] || continue
        die "refusing existing gadget: $candidate"
    done
}

refuse_prebound_udc() {
    bound_function=$UDC_PATH/function
    if [ -r "$bound_function" ] && [ -n "$(cat "$bound_function" 2>/dev/null || true)" ]; then
        die "refusing pre-bound UDC: $UDC"
    fi

    for candidate in "$GADGET_ROOT"/*; do
        [ -d "$candidate" ] || continue
        bound=$(read_bound_udc "$candidate")
        [ "$bound" != "$UDC" ] || die "refusing UDC already bound by: $candidate"
    done
}

write_attribute() {
    printf '%s\n' "$2" > "$1"
}

write_optional_attribute() {
    [ -e "$1" ] || return 0
    write_attribute "$1" "$2"
}

unbind_gadget() {
    [ -e "$GADGET/UDC" ] || return 0
    printf '\n' > "$GADGET/UDC"
}

remove_link() {
    [ -e "$1" ] || [ -L "$1" ] || return 0
    rm -f -- "$1"
}

remove_directory() {
    [ -d "$1" ] || return 0
    rmdir -- "$1" || die "could not remove non-empty configfs directory: $1"
}

remove_gadget_tree() {
    remove_link "$GADGET/configs/c.1/midi.usb0"
    remove_link "$GADGET/configs/c.1/uac2.usb0"
    remove_directory "$GADGET/functions/midi.usb0"
    remove_directory "$GADGET/functions/uac2.usb0"
    remove_directory "$GADGET/configs/c.1/strings/0x409"
    remove_directory "$GADGET/configs/c.1/strings"
    remove_directory "$GADGET/configs/c.1"
    remove_directory "$GADGET/strings/0x409"
    remove_directory "$GADGET/functions"
    remove_directory "$GADGET/configs"
    remove_directory "$GADGET/strings"
    remove_directory "$GADGET"
}

rollback_setup() {
    status=$1
    trap - EXIT
    unbind_gadget 2>/dev/null || true
    remove_link "$GADGET/configs/c.1/midi.usb0" 2>/dev/null || true
    remove_link "$GADGET/configs/c.1/uac2.usb0" 2>/dev/null || true
    rmdir "$GADGET/functions/midi.usb0" 2>/dev/null || true
    rmdir "$GADGET/functions/uac2.usb0" 2>/dev/null || true
    rmdir "$GADGET/configs/c.1/strings/0x409" 2>/dev/null || true
    rmdir "$GADGET/configs/c.1/strings" 2>/dev/null || true
    rmdir "$GADGET/configs/c.1" 2>/dev/null || true
    rmdir "$GADGET/strings/0x409" 2>/dev/null || true
    rmdir "$GADGET/functions" 2>/dev/null || true
    rmdir "$GADGET/configs" 2>/dev/null || true
    rmdir "$GADGET/strings" 2>/dev/null || true
    rmdir "$GADGET" 2>/dev/null || true
    exit "$status"
}

create_midi_function() {
    mkdir "$GADGET/functions/midi.usb0"
    write_optional_attribute "$GADGET/functions/midi.usb0/id" "Octessera MIDI"
    write_optional_attribute "$GADGET/functions/midi.usb0/function_name" "Octessera MIDI"
    write_optional_attribute "$GADGET/functions/midi.usb0/interface_string" "Octessera MIDI"
    ln -s "$GADGET/functions/midi.usb0" "$GADGET/configs/c.1/midi.usb0"
}

create_uac2_function() {
    mkdir "$GADGET/functions/uac2.usb0"
    write_optional_attribute "$GADGET/functions/uac2.usb0/function_name" "Octessera Audio"
    write_attribute "$GADGET/functions/uac2.usb0/p_chmask" 3
    write_attribute "$GADGET/functions/uac2.usb0/p_ssize" 2
    write_attribute "$GADGET/functions/uac2.usb0/p_srate" 44100
    write_optional_attribute "$GADGET/functions/uac2.usb0/c_chmask" 0
    ln -s "$GADGET/functions/uac2.usb0" "$GADGET/configs/c.1/uac2.usb0"
}

setup_gadget() {
    require_configfs
    require_udc
    refuse_existing_gadgets
    refuse_prebound_udc

    mkdir "$GADGET"
    trap 'rollback_setup "$?"' EXIT

    mkdir -p "$GADGET/configs" "$GADGET/functions" "$GADGET/strings"
    mkdir -p "$GADGET/strings/0x409" "$GADGET/configs/c.1/strings/0x409"
    write_attribute "$GADGET/idVendor" 0x1d6b
    write_attribute "$GADGET/idProduct" 0x0104
    write_attribute "$GADGET/bcdUSB" 0x0200
    write_attribute "$GADGET/bcdDevice" 0x0100
    write_attribute "$GADGET/strings/0x409/manufacturer" Octessera
    write_attribute "$GADGET/strings/0x409/product" "Octessera Orange Pi $MODE"
    write_attribute "$GADGET/strings/0x409/serialnumber" octessera-orange-pi
    write_attribute "$GADGET/configs/c.1/MaxPower" 250
    write_attribute "$GADGET/configs/c.1/strings/0x409/configuration" "Octessera Orange Pi $MODE"

    case "$MODE" in
        midi)
            create_midi_function
            ;;
        uac2)
            create_uac2_function
            ;;
        combined)
            create_midi_function
            create_uac2_function
            ;;
    esac

    printf '%s\n' "$UDC" > "$GADGET/UDC"
    trap - EXIT
    printf 'configured Orange Pi USB gadget mode=%s udc=%s\n' "$MODE" "$UDC"
}

teardown_gadget() {
    require_configfs
    [ -d "$GADGET" ] || exit 0
    [ -r "$GADGET/UDC" ] || die "gadget is missing its UDC attribute: $GADGET"
    bound=$(read_bound_udc "$GADGET")
    [ -z "$bound" ] || [ "$bound" = "$UDC" ] || die "gadget is bound to a different UDC: $bound"
    unbind_gadget
    remove_gadget_tree
    printf 'removed Orange Pi USB gadget udc=%s\n' "$UDC"
}

case "$ACTION" in
    setup)
        setup_gadget
        ;;
    teardown)
        teardown_gadget
        ;;
esac
