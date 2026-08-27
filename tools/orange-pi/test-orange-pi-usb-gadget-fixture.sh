#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
CANONICAL_SCRIPT=$SCRIPT_DIR/orange-pi-usb-gadget.sh
DEPLOYED_SCRIPT=$SCRIPT_DIR/../../userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget
export DEPLOYED_SCRIPT
SCRIPT=$CANONICAL_SCRIPT
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/octessera-orange-gadget.XXXXXX")
FAKE_BIN=$TEST_ROOT/bin
cleanup() {
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT HUP INT TERM
mkdir "$FAKE_BIN"
cat > "$FAKE_BIN/mkdir" <<'EOF'
#!/bin/sh
set -eu
/usr/bin/mkdir "$@"
for argument in "$@"; do
    if [ "$argument" = "${FAKE_GADGET:-}" ] && [ -n "${FAKE_UDC_FUNCTION:-}" ]; then
        /usr/bin/mkdir "$argument/configs" "$argument/functions" "$argument/strings" \
            "$argument/os_desc" "$argument/webusb"
        /usr/bin/ln -s "$FAKE_UDC_FUNCTION" "$argument/UDC"
        if [ "${FAKE_BIND_WRITE_FAILURE:-0}" = 1 ]; then
            chmod a-w "$FAKE_UDC_FUNCTION"
        fi
    fi
    if [ "$argument" = "${FAKE_UAC2:-}" ]; then
        : > "$argument/function_name"
        : > "$argument/p_chmask"
        : > "$argument/p_ssize"
        : > "$argument/p_srate"
        : > "$argument/c_chmask"
    fi
    if [ "$argument" = "${FAKE_MIDI:-}" ]; then
        : > "$argument/id"
        if [ "${FAKE_MIDI_ID_ONLY:-0}" != 1 ]; then
            : > "$argument/function_name"
            if [ "${FAKE_MIDI_WRITE_FAILURE:-0}" = 1 ]; then
                /usr/bin/mkdir "$argument/interface_string"
            else
                : > "$argument/interface_string"
            fi
        fi
    fi
done
EOF
chmod +x "$FAKE_BIN/mkdir"
cat > "$FAKE_BIN/ln" <<'EOF'
#!/bin/sh
set -eu
if [ "${FAKE_PREBIND_FAILURE:-0}" = 1 ]; then
    printf 'fake pre-bind failure\n' >&2
    exit 94
fi
if [ "${FAKE_BIND_ORDER:-0}" = 1 ] && [ "$#" -eq 3 ] && [ "$1" = -s ]; then
    case "$3" in
        */configs/c.1/*.usb0)
            bound=$(cat "${FAKE_GADGET:?}/UDC" 2>/dev/null || true)
            [ -z "$bound" ] || {
                printf 'fake configfs link created after UDC bind\n' >&2
                exit 92
            }
            ;;
    esac
fi
exec /usr/bin/ln "$@"
EOF
chmod +x "$FAKE_BIN/ln"
cat > "$FAKE_BIN/cat" <<'EOF'
#!/bin/sh
set -eu
if [ "$#" -eq 1 ] && [ "$1" = "${FAKE_BOUND_UDC_GADGET:-}/UDC" ]; then
    printf '%s\n' "${FAKE_EXPECTED_UDC:?}"
    exit 0
fi
exec /usr/bin/cat "$@"
EOF
chmod +x "$FAKE_BIN/cat"
cat > "$FAKE_BIN/cmp" <<'EOF'
#!/bin/sh
set -eu
if [ "$#" -eq 3 ] && [ "$1" = -s ] && [ "$2" = - ] && \
    [ "$3" = "${FAKE_MIDI:-}/interface_string" ]; then
    if [ "${FAKE_MIDI_SHORT_READBACK:-0}" = 1 ]; then
        : > "$3"
    elif [ "${FAKE_MIDI_MODIFIED_READBACK:-0}" = 1 ]; then
        printf '%s' 'Octessera MIDZ' > "$3"
    elif [ "${FAKE_MIDI_POST_BIND_MISMATCH:-0}" = 1 ] && \
        [ -n "$(cat "${FAKE_GADGET:?}/UDC" 2>/dev/null || true)" ]; then
        printf '%s' 'Octessera MIDZ' > "$3"
        if [ "${FAKE_ROLLBACK_UNBIND_FAILURE:-0}" = 1 ]; then
            chmod a-w "${FAKE_UDC_FUNCTION:?}"
        fi
    fi
fi
exec /usr/bin/cmp "$@"
EOF
chmod +x "$FAKE_BIN/cmp"
cat > "$FAKE_BIN/rmdir" <<'EOF'
#!/bin/sh
set -eu
if [ "$#" -eq 2 ] && [ "$1" = "--" ]; then
    rmdir_path=$2
else
    rmdir_path=$1
fi
if [ "${FAKE_RMDIR_ERROR:-0}" = 1 ]; then
    printf 'fake rmdir real error\n' >&2
    exit 93
fi
case "$rmdir_path" in
    */octessera-orange-pi/configs|*/octessera-orange-pi/functions|*/octessera-orange-pi/strings|*/octessera-orange-pi/os_desc|*/octessera-orange-pi/webusb|*/configs/c.1/strings)
        printf 'fake configfs default group refuses direct rmdir\n' >&2
        exit 95
        ;;
    */configs/c.1)
        /usr/bin/rmdir "$rmdir_path/strings" 2>/dev/null || true
        ;;
esac
if [ -n "${FAKE_GADGET:-}" ] && [ -d "$FAKE_GADGET" ]; then
    if [ "${FAKE_UDC_GONE:-0}" = 1 ]; then
        bound=
    else
        bound=$(cat "$FAKE_GADGET/UDC" 2>/dev/null || true)
    fi
    [ -z "$bound" ] || {
        printf 'rmdir attempted while gadget was bound\n' >&2
        exit 91
    }
    if [ -n "${FAKE_UNBIND_MARKER:-}" ] && [ -e "$FAKE_GADGET/UDC" ]; then
        if [ "$(wc -c < "$FAKE_GADGET/UDC" | tr -d ' ')" -eq 1 ]; then
            : > "$FAKE_UNBIND_MARKER"
        fi
    fi
    rm -f "$FAKE_GADGET"/configs/c.1/MaxPower \
        "$FAKE_GADGET"/configs/c.1/strings/0x409/configuration \
        "$FAKE_GADGET"/strings/0x409/manufacturer \
        "$FAKE_GADGET"/strings/0x409/product \
        "$FAKE_GADGET"/strings/0x409/serialnumber \
        "$FAKE_GADGET"/functions/midi.usb0/id \
        "$FAKE_GADGET"/functions/midi.usb0/function_name \
        "$FAKE_GADGET"/functions/uac2.usb0/function_name \
        "$FAKE_GADGET"/functions/uac2.usb0/p_chmask \
        "$FAKE_GADGET"/functions/uac2.usb0/p_ssize \
        "$FAKE_GADGET"/functions/uac2.usb0/p_srate \
        "$FAKE_GADGET"/functions/uac2.usb0/c_chmask
    if [ "${FAKE_MIDI_WRITE_FAILURE:-0}" = 1 ]; then
        rm -rf "$FAKE_GADGET/functions/midi.usb0/interface_string"
    else
        rm -f "$FAKE_GADGET/functions/midi.usb0/interface_string"
    fi
fi
if [ "$#" -eq 2 ] && [ "$1" = "--" ] && [ "$2" = "${FAKE_GADGET:-}" ]; then
    rm -f "$FAKE_GADGET"/idVendor "$FAKE_GADGET"/idProduct \
        "$FAKE_GADGET"/bcdUSB "$FAKE_GADGET"/bcdDevice "$FAKE_GADGET"/UDC \
        "$FAKE_GADGET"/configs/c.1/strings/0x409/configuration
    /usr/bin/rmdir "$FAKE_GADGET"/configs "$FAKE_GADGET"/functions \
        "$FAKE_GADGET"/strings "$FAKE_GADGET"/os_desc "$FAKE_GADGET"/webusb 2>/dev/null || true
fi
exec /usr/bin/rmdir "$@"
EOF
chmod +x "$FAKE_BIN/rmdir"

assert_failed() {
    if "$@" >/dev/null 2>&1; then printf 'expected command to fail: %s\n' "$*" >&2; exit 1; fi
}
assert_file_value() {
    test "$(cat "$2")" = "$1"
}
assert_midi_interface() {
    file=$1
    test "$(wc -c < "$file" | tr -d '[:space:]')" -eq 14
    printf '%s' 'Octessera MIDI' | cmp -s - "$file"
}
assert_unbound() {
    root=$1
    test ! -e "$root/config/usb_gadget/octessera-orange-pi"
    test -z "$(cat "$root/udc/musb-hdrc.4.auto/function")"
}
new_fake_configfs() {
    root=$1
    mkdir -p "$root/config/usb_gadget" "$root/udc/musb-hdrc.4.auto"
    : > "$root/udc/musb-hdrc.4.auto/function"
}
run_setup_command() {
    root=$1
    mode=$2
    shift 2
    config=$TEST_ROOT/config-$mode.json
    env FAKE_GADGET="$root/config/usb_gadget/octessera-orange-pi" \
        FAKE_UDC_FUNCTION="$root/udc/musb-hdrc.4.auto/function" \
        FAKE_MIDI="$root/config/usb_gadget/octessera-orange-pi/functions/midi.usb0" \
        FAKE_UAC2="$root/config/usb_gadget/octessera-orange-pi/functions/uac2.usb0" \
        FAKE_BIND_ORDER=1 OCTESSERA_DEVICE_CONFIG_VALIDATOR="$SCRIPT_DIR/../pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" PATH="$FAKE_BIN:$PATH" "$@" sh "$SCRIPT" setup \
        --configfs-root "$root/config" --udc-root "$root/udc" \
        --lock-file "$root/lifecycle.lock" --config "$config"
}
run_setup() { run_setup_command "$1" "$2"; }
run_setup_default() {
    root=$1
    env FAKE_GADGET="$root/config/usb_gadget/octessera-orange-pi" \
        FAKE_UDC_FUNCTION="$root/udc/musb-hdrc.4.auto/function" \
        FAKE_MIDI="$root/config/usb_gadget/octessera-orange-pi/functions/midi.usb0" \
        FAKE_UAC2="$root/config/usb_gadget/octessera-orange-pi/functions/uac2.usb0" \
        FAKE_BIND_ORDER=1 OCTESSERA_DEVICE_CONFIG_VALIDATOR="$SCRIPT_DIR/../pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" PATH="$FAKE_BIN:$PATH" sh "$SCRIPT" setup \
        --configfs-root "$root/config" --udc-root "$root/udc" \
        --lock-file "$root/lifecycle.lock" --config "$TEST_ROOT/config-combined.json"
}
run_setup_id_only() { run_setup_command "$1" midi FAKE_MIDI_ID_ONLY=1; }
run_setup_write_failure() { run_setup_command "$1" midi FAKE_MIDI_WRITE_FAILURE=1; }
run_setup_readback_failure() { root=$1; shift; run_setup_command "$root" midi "$@"; }
run_setup_post_bind_mismatch() { root=$1; shift; run_setup_command "$root" combined FAKE_MIDI_POST_BIND_MISMATCH=1 "$@"; }
run_setup_prebind_failure() { root=$1; run_setup_command "$root" combined "FAKE_UNBIND_MARKER=$2" FAKE_PREBIND_FAILURE=1; }
run_setup_bind_failure() { root=$1; run_setup_command "$root" combined "FAKE_UNBIND_MARKER=$2" FAKE_BIND_WRITE_FAILURE=1; }
run_teardown_command() {
    root=$1
    marker=$2
    shift 2
    env FAKE_GADGET="$root/config/usb_gadget/octessera-orange-pi" \
        FAKE_UNBIND_MARKER="$marker" \
        FAKE_UDC_GONE="$([ -e "$root/udc/musb-hdrc.4.auto" ] && printf 0 || printf 1)" \
        PATH="$FAKE_BIN:$PATH" "$@" \
        sh "$SCRIPT" teardown --configfs-root "$root/config" --udc-root "$root/udc" \
        --lock-file "$root/lifecycle.lock"
}
run_teardown() { run_teardown_command "$1" "${2:-}"; }
run_teardown_with_rmdir_error() { run_teardown_command "$1" '' FAKE_RMDIR_ERROR=1; }
run_teardown_with_unbind_write_error() {
    root=$1
    output=$2
    FAKE_BOUND_UDC_GADGET="$root/config/usb_gadget/octessera-orange-pi" \
        FAKE_EXPECTED_UDC=musb-hdrc.4.auto run_teardown "$root" > "$output" 2>&1
}

printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":false,"hdmi":true},"usb":{"midiOutEnabled":true}}}' > "$TEST_ROOT/config-midi.json"
printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":true,"hdmi":false},"usb":{"midiOutEnabled":false}}}' > "$TEST_ROOT/config-uac2.json"
printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":false},"usb":{"midiOutEnabled":true}}}' > "$TEST_ROOT/config-combined.json"
printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false},"usb":{"midiOutEnabled":false}}}' > "$TEST_ROOT/config-none.json"
printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":false,"hdmi":false},"usb":{"midiOutEnabled":true}}}' > "$TEST_ROOT/config-invalid.json"
printf '%s\n' '{' > "$TEST_ROOT/config-malformed.json"
printf '%s\n' '{"runtimeConfig":{"usb":{"audioOut":"usb"}}}' > "$TEST_ROOT/config-legacy-only.json"
printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":false},"usb":{"audioOut":"both"}}}' > "$TEST_ROOT/config-conflict.json"
