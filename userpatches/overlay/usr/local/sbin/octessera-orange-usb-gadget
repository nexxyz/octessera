#!/bin/sh

set -eu

GADGET_NAME=octessera-orange-pi
CONFIGFS_ROOT=/sys/kernel/config
UDC_ROOT=/sys/class/udc
REQUIRED_UDC=musb-hdrc.4.auto
LOCK_FILE=/run/lock/octessera-orange-usb-gadget.lock
CONFIG=/var/lib/octessera/presets/default.json
DEVICE_CONFIG_VALIDATOR=${OCTESSERA_DEVICE_CONFIG_VALIDATOR:-/usr/local/lib/octessera/device_config.py}
ACTION=
MODE=
UDC=$REQUIRED_UDC

usage() {
    cat <<EOF
Usage:
  $0 setup [--config <path>] [options]
  $0 teardown [options]

Options:
  --config <path>         Persisted device config (default: /var/lib/octessera/presets/default.json)
  --configfs-root <path>  Configfs mount root (default: /sys/kernel/config)
  --udc-root <path>       UDC sysfs root (default: /sys/class/udc)
  --lock-file <path>      Lifecycle lock path (default: /run/lock/octessera-orange-usb-gadget.lock)
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
        --config)
            require_value "$@"
            CONFIG=$2
            shift 2
            ;;
        --config=*)
            CONFIG=${1#*=}
            [ -n "$CONFIG" ] || die "empty value for --config"
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
        --lock-file)
            require_value "$@"
            LOCK_FILE=$2
            shift 2
            ;;
        --lock-file=*)
            LOCK_FILE=${1#*=}
            [ -n "$LOCK_FILE" ] || die "empty value for --lock-file"
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

[ "$UDC" = "$REQUIRED_UDC" ] || die "unexpected UDC configuration"

GADGET_ROOT=$CONFIGFS_ROOT/usb_gadget
GADGET=$GADGET_ROOT/$GADGET_NAME
UDC_PATH=$UDC_ROOT/$UDC

if [ "$ACTION" = teardown ]; then
    [ "$CONFIG" = /var/lib/octessera/presets/default.json ] || die "--config is valid only for setup"
fi

read_device_config() {
    state=$(python3 "$DEVICE_CONFIG_VALIDATOR" "$CONFIG") || die "invalid persisted device config"
    AUDIO=${state%% *}
    MIDI=${state#* }
    case "$AUDIO$MIDI" in
        00) MODE=none ;;
        01) MODE=midi ;;
        10) MODE=uac2 ;;
        11) MODE=combined ;;
        *) die "device config validator returned invalid state" ;;
    esac
}

require_configfs() {
    [ -d "$GADGET_ROOT" ] || die "configfs USB gadget directory is missing: $GADGET_ROOT"
}

require_udc() {
    [ "$UDC" = "$REQUIRED_UDC" ] || die "refusing non-Orange UDC: $UDC"
    [ -e "$UDC_PATH" ] || die "UDC was not found: $UDC"
}

read_bound_udc() {
    [ -r "$1/UDC" ] || return 0
    cat "$1/UDC" 2>/dev/null
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

verify_midi_interface_string() {
    path=$1
    size=$(wc -c < "$path" | tr -d '[:space:]')
    [ "$size" = 14 ] || die "MIDI interface_string size mismatch: $path"
    if ! printf '%s' 'Octessera MIDI' | cmp -s - "$path"; then
        die "MIDI interface_string readback mismatch: $path"
    fi
}

write_midi_interface_string() {
    path=$1
    [ -e "$path" ] || die "required MIDI interface_string attribute is missing: $path"
    if ! printf '%s' 'Octessera MIDI' > "$path"; then
        die "could not write MIDI interface_string: $path"
    fi
    verify_midi_interface_string "$path"
}

product_name() {
    case "$MODE" in
        combined)
            printf 'Octessera Audio + MIDI\n'
            ;;
        midi)
            printf 'Octessera MIDI\n'
            ;;
        uac2)
            printf 'Octessera Line In\n'
            ;;
        *)
            die "unsupported mode: $MODE"
            ;;
    esac
}

configuration_name() {
    case "$MODE" in
        none) printf 'Octessera USB disabled\n' ;;
        *) printf 'Octessera Orange Pi %s\n' "$MODE" ;;
    esac
}

with_lifecycle_lock() {
    command -v flock >/dev/null 2>&1 || die "flock is required for gadget lifecycle safety"
    if ! exec 9>"$LOCK_FILE"; then
        die "could not open lifecycle lock: $LOCK_FILE"
    fi
    if ! flock -n 9; then
        exec 9>&-
        die "gadget lifecycle operation is already in progress"
    fi
    "$@"
    status=$?
    flock -u 9 || true
    exec 9>&-
    return "$status"
}

unbind_gadget() {
    [ -e "$GADGET/UDC" ] || return 0
    [ -r "$GADGET/UDC" ] || {
        printf 'orange-pi-usb-gadget: gadget UDC attribute is not readable: %s\n' "$GADGET" >&2
        return 1
    }
    bound=
    if ! bound=$(read_bound_udc "$GADGET"); then
        printf 'orange-pi-usb-gadget: could not read gadget UDC attribute: %s\n' "$GADGET" >&2
        return 1
    fi
    [ -n "$bound" ] || return 0
    [ "$bound" = "$UDC" ] || {
        printf 'orange-pi-usb-gadget: refusing UDC bound to %s\n' "$bound" >&2
        return 1
    }
    [ -e "$UDC_PATH" ] || return 0
    error=
    if error="$(printf '\n' 2>&1 > "$GADGET/UDC")"; then
        return 0
    fi
    [ -e "$UDC_PATH" ] || return 0
    [ -z "$error" ] || printf '%s\n' "$error" >&2
    return 1
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
    remove_link "$GADGET/os_desc/c.1"
    remove_directory "$GADGET/configs/c.1/strings/0x409"
    remove_directory "$GADGET/configs/c.1"
    remove_directory "$GADGET/functions/midi.usb0"
    remove_directory "$GADGET/functions/uac2.usb0"
    remove_directory "$GADGET/strings/0x409"
    remove_directory "$GADGET"
}

rollback_setup() {
    status=$1
    trap - EXIT
    if ! unbind_gadget; then
        exit "$status"
    fi
    remove_link "$GADGET/configs/c.1/midi.usb0" 2>/dev/null || true
    remove_link "$GADGET/configs/c.1/uac2.usb0" 2>/dev/null || true
    remove_link "$GADGET/os_desc/c.1" 2>/dev/null || true
    rmdir -- "$GADGET/configs/c.1/strings/0x409" 2>/dev/null || true
    rmdir -- "$GADGET/configs/c.1" 2>/dev/null || true
    rmdir -- "$GADGET/functions/midi.usb0" 2>/dev/null || true
    rmdir -- "$GADGET/functions/uac2.usb0" 2>/dev/null || true
    rmdir -- "$GADGET/strings/0x409" 2>/dev/null || true
    rmdir -- "$GADGET" 2>/dev/null || true
    exit "$status"
}

create_midi_function() {
    mkdir "$GADGET/functions/midi.usb0"
    write_optional_attribute "$GADGET/functions/midi.usb0/id" "Octessera MIDI"
    write_optional_attribute "$GADGET/functions/midi.usb0/function_name" "Octessera MIDI"
    write_midi_interface_string "$GADGET/functions/midi.usb0/interface_string"
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

setup_gadget_unlocked() {
    require_configfs
    read_device_config
    if [ "$MODE" = none ]; then
        for candidate in "$GADGET_ROOT"/*; do
            [ -d "$candidate" ] || continue
            if [ "$candidate" != "$GADGET" ]; then
                die "refusing existing gadget: $candidate"
            fi
        done
        teardown_gadget_unlocked
        refuse_prebound_udc
        printf 'Orange Pi USB gadget disabled\n'
        return 0
    fi
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
    product_name > "$GADGET/strings/0x409/product"
    write_attribute "$GADGET/strings/0x409/serialnumber" octessera-orange-pi
    write_attribute "$GADGET/configs/c.1/MaxPower" 250
    configuration_name > "$GADGET/configs/c.1/strings/0x409/configuration"

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
    case "$MODE" in
        midi|combined)
            verify_midi_interface_string "$GADGET/functions/midi.usb0/interface_string"
            ;;
    esac
    trap - EXIT
    printf 'configured Orange Pi USB gadget mode=%s udc=%s\n' "$MODE" "$UDC"
}

teardown_gadget_unlocked() {
    require_configfs
    [ -d "$GADGET" ] || exit 0
    [ -e "$GADGET/UDC" ] || [ -L "$GADGET/UDC" ] || die "gadget is missing its UDC attribute: $GADGET"
    unbind_gadget
    remove_gadget_tree
    printf 'removed Orange Pi USB gadget udc=%s\n' "$UDC"
}

case "$ACTION" in
    setup)
        with_lifecycle_lock setup_gadget_unlocked
        ;;
    teardown)
        with_lifecycle_lock teardown_gadget_unlocked
        ;;
esac
