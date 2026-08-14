#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
SCRIPT=$SCRIPT_DIR/stage4-octessera/files/root/usr/local/sbin/octessera-usb-gadget
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/octessera-rpi-gadget.XXXXXX")
FAKE_BIN=$TEST_ROOT/bin

cleanup() { rm -rf "$TEST_ROOT"; }
trap cleanup EXIT

mkdir "$FAKE_BIN"

cat > "$FAKE_BIN/modprobe" <<'EOF'
#!/bin/sh
if [ -n "${FAKE_MODPROBE_CALLS:-}" ]; then
    printf '%s\n' "$$" >> "$FAKE_MODPROBE_CALLS"
fi
if [ -n "${FAKE_HOLD_STARTED:-}" ] && [ ! -e "$FAKE_HOLD_STARTED" ]; then
    : > "$FAKE_HOLD_STARTED"
    while [ ! -e "$FAKE_HOLD_RELEASE" ]; do sleep 0.01; done
fi
exit 0
EOF
chmod +x "$FAKE_BIN/modprobe"
cat > "$FAKE_BIN/mountpoint" <<'EOF'
#!/bin/sh
if [ "$2" = "${FAKE_CONFIGFS_ROOT:-}" ]; then
    exit 0
fi
if [ "$2" = "${FAKE_SD_MOUNT:-}" ] && [ -e "${FAKE_MOUNT_MARKER:-}" ]; then
    exit 0
fi
exit 1
EOF
chmod +x "$FAKE_BIN/mountpoint"
cat > "$FAKE_BIN/logger" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$FAKE_BIN/logger"
cat > "$FAKE_BIN/aplay" <<'EOF'
#!/bin/sh
printf 'card 0: Octessera USB audio\n'
EOF
chmod +x "$FAKE_BIN/aplay"
cat > "$FAKE_BIN/cat" <<'EOF'
#!/bin/sh
set -eu
if [ "$#" -eq 1 ] && [ "$1" = "${FAKE_GADGET:-}/UDC" ]; then
    if [ "${FAKE_POST_BIND_MISMATCH:-0}" = 1 ] && [ ! -e "${FAKE_POST_BIND_MARKER:-}" ]; then
        : > "$FAKE_POST_BIND_MARKER"
        printf 'wrong-udc\n'
        exit 0
    fi
    if [ "${FAKE_REQUIRE_LINKS:-0}" = 1 ]; then
        test -L "${FAKE_GADGET}/configs/c.1/midi.usb0"
        if [ "${FAKE_REQUIRE_AUDIO:-0}" = 1 ]; then
            test -L "${FAKE_GADGET}/configs/c.1/uac2.usb0"
        fi
    fi
    if [ -d "$1" ] && [ -n "${FAKE_BOUND_UDC:-}" ]; then
        printf '%s\n' "$FAKE_BOUND_UDC"
        exit 0
    fi
    if [ -d "$1" ]; then
        exit 0
    fi
fi
exec /usr/bin/cat "$@"
EOF
chmod +x "$FAKE_BIN/cat"
cat > "$FAKE_BIN/mkdir" <<'EOF'
#!/bin/sh
set -eu
/usr/bin/mkdir "$@"
for argument in "$@"; do
    if [ -n "${FAKE_GADGET:-}" ]; then
        case "$argument" in
            "$FAKE_GADGET"/*)
                [ -e "$FAKE_GADGET/UDC" ] || : > "$FAKE_GADGET/UDC"
                ;;
        esac
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
    if [ "$argument" = "${FAKE_MASS:-}" ]; then
        /usr/bin/mkdir -p "$argument/lun.0"
        : > "$argument/stall"
        : > "$argument/lun.0/ro"
        : > "$argument/lun.0/removable"
        : > "$argument/lun.0/nofua"
        : > "$argument/lun.0/forced_eject"
        : > "$argument/lun.0/file"
    fi
done
EOF
chmod +x "$FAKE_BIN/mkdir"
cat > "$FAKE_BIN/ln" <<'EOF'
#!/bin/sh
set -eu
/usr/bin/ln "$@"
if [ "${FAKE_BIND_FAILURE:-0}" = 1 ] && [ "$#" -eq 3 ] && [ "$3" = "$FAKE_GADGET/configs/c.1/midi.usb0" ]; then
    rm -f "$FAKE_GADGET/UDC"
    /usr/bin/mkdir "$FAKE_GADGET/UDC"
fi
EOF
chmod +x "$FAKE_BIN/ln"

cat > "$FAKE_BIN/cmp" <<'EOF'
#!/bin/sh
set -eu
if [ "$#" -eq 3 ] && [ "$1" = -s ] && [ "$2" = - ] && [ "$3" = "$FAKE_MIDI/interface_string" ]; then
    if [ -n "${FAKE_READBACK_OBSERVED:-}" ] && [ ! -e "$FAKE_READBACK_OBSERVED" ]; then
        cp "$3" "$FAKE_READBACK_OBSERVED"
    fi
    case "${FAKE_READBACK_MODE:-}" in
        short) printf '%s' 'Octessera MID' > "$3" ;;
        modified) printf '%s' 'Octessera MIDX' > "$3" ;;
        lf) printf '%s\n' 'Octessera MIDI' > "$3" ;;
        crlf) printf '%s\r\n' 'Octessera MIDI' > "$3" ;;
        internal) printf '%s\n%s' 'Octessera' 'MIDI' > "$3" ;;
        two-lf) printf '%s\n\n' 'Octessera MIDI' > "$3" ;;
        spaces) printf '%s ' 'Octessera MIDI' > "$3" ;;
        long) printf '%sX' 'Octessera MIDI' > "$3" ;;
        fail) exit 1 ;;
    esac
fi
exec /usr/bin/cmp "$@"
EOF
chmod +x "$FAKE_BIN/cmp"

cat > "$FAKE_BIN/rmdir" <<'EOF'
#!/bin/sh
set -eu
if [ "${FAKE_RMDIR_FAILURE:-0}" = 1 ]; then
    exit 97
fi
if [ -n "${FAKE_GADGET:-}" ]; then
    for path in "$@"; do
        case "$path" in
            "$FAKE_GADGET/configs/c.1/strings"|"$FAKE_GADGET/configs"|"$FAKE_GADGET/strings"|"$FAKE_GADGET/functions")
                printf 'fake ConfigFS default group refuses direct rmdir\n' >&2
                exit 95
                ;;
            "$FAKE_GADGET/functions"/*)
                find "$path" -type f -delete 2>/dev/null || true
                find "$path" -type d -name interface_string -exec /usr/bin/rmdir '{}' \; 2>/dev/null || true
                find "$path" -depth -type d -not -path "$path" -exec /usr/bin/rmdir '{}' \; 2>/dev/null || true
                ;;
            "$FAKE_GADGET/configs/c.1/strings/0x409")
                find "$path" -type f -delete 2>/dev/null || true
                ;;
            "$FAKE_GADGET/configs/c.1")
                find "$path" -maxdepth 1 -type f -delete 2>/dev/null || true
                /usr/bin/rmdir "$path/strings" 2>/dev/null || true
                ;;
            "$FAKE_GADGET/strings/0x409")
                find "$path" -type f -delete 2>/dev/null || true
                ;;
            "$FAKE_GADGET")
                find "$path" -maxdepth 1 -type f -delete 2>/dev/null || true
                if [ -d "$path/UDC" ]; then
                    /usr/bin/rmdir "$path/UDC"
                fi
                /usr/bin/rmdir "$path/configs" "$path/functions" "$path/strings" 2>/dev/null || true
                ;;
        esac
    done
fi
exec /usr/bin/rmdir "$@"
EOF
chmod +x "$FAKE_BIN/rmdir"

cat > "$FAKE_BIN/findmnt" <<'EOF'
#!/bin/sh
set -eu
case " $* " in
    *' -nro SOURCE / '*)
        printf '/dev/fake-root\n'
        ;;
    *"--mountpoint ${FAKE_SD_MOUNT:-} -S"*)
        [ -e "${FAKE_MOUNT_MARKER:-}" ] || exit 1
        ;;
    *"-nro SOURCE --mountpoint ${FAKE_SD_MOUNT:-}"*)
        [ -e "${FAKE_MOUNT_MARKER:-}" ] || exit 1
        printf '%s\n' "${FAKE_STORAGE_DEVICE:-/dev/loop0}"
        ;;
    *"-nr -S ${FAKE_STORAGE_DEVICE:-/dev/loop0} -o TARGET"*)
        exit 1
        ;;
    *)
        exit 1
        ;;
esac
EOF
chmod +x "$FAKE_BIN/findmnt"

cat > "$FAKE_BIN/lsblk" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$FAKE_BIN/lsblk"

cat > "$FAKE_BIN/mount" <<'EOF'
#!/bin/sh
[ "$2" = "${FAKE_SD_MOUNT:-}" ] && : > "$FAKE_MOUNT_MARKER"
exit 0
EOF
chmod +x "$FAKE_BIN/mount"

cat > "$FAKE_BIN/umount" <<'EOF'
#!/bin/sh
[ "$1" = "${FAKE_SD_MOUNT:-}" ] && rm -f "$FAKE_MOUNT_MARKER"
exit 0
EOF
chmod +x "$FAKE_BIN/umount"

cat > "$FAKE_BIN/stat" <<'EOF'
#!/bin/sh
set -eu
if [ "$#" -eq 3 ] && [ "$1" = -c ] && [ "$3" = "${FAKE_STORAGE_CONFIG:-}" -o "$3" = "${FAKE_STORAGE_STATE:-}" ]; then
    case "$2" in
        %u) printf '0\n' ;;
        %a) printf '640\n' ;;
        *) exec /usr/bin/stat "$@" ;;
    esac
    exit 0
fi
exec /usr/bin/stat "$@"
EOF
chmod +x "$FAKE_BIN/stat"
assert_failed() {
    if "$@" >/dev/null 2>&1; then
        printf 'expected command to fail: %s\n' "$*" >&2
        exit 1
    fi
}

assert_file_value() {
    expected=$1
    file=$2
    actual=$(cat "$file")
    [ "$actual" = "$expected" ] || {
        printf 'expected [%s] in %s, got [%s]\n' "$expected" "$file" "$actual" >&2
        exit 1
    }
}

new_fake_configfs() {
    root=$1
    mkdir -p "$root/config/usb_gadget" "$root/udc" "$root/udc-target"; ln -s ../udc-target "$root/udc/fake-udc"
}

run_action() {
    root=$1
    action=$2
    config=$3
    shift 3
    env \
        "FAKE_GADGET=$root/config/usb_gadget/octessera" \
        "FAKE_MIDI=$root/config/usb_gadget/octessera/functions/midi.usb0" \
        "FAKE_UAC2=$root/config/usb_gadget/octessera/functions/uac2.usb0" \
        "FAKE_MASS=$root/config/usb_gadget/octessera/functions/mass_storage.usb0" \
        "FAKE_CONFIGFS_ROOT=$root/config" \
        "FAKE_SD_MOUNT=$root/sd-card" \
        "FAKE_MOUNT_MARKER=$root/mounted" \
        "FAKE_STORAGE_DEVICE=/dev/loop0" \
        "FAKE_STORAGE_CONFIG=$root/usb-storage.conf" \
        "FAKE_STORAGE_STATE=$root/storage.state" \
        "$@" \
        "OCTESSERA_USB_CONFIG=$config" \
        "OCTESSERA_DEVICE_CONFIG_VALIDATOR=$SCRIPT_DIR/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" \
        "OCTESSERA_USB_CONFIGFS_ROOT=$root/config" \
        "OCTESSERA_USB_GADGET_ROOT=$root/config/usb_gadget/octessera" \
        "OCTESSERA_USB_UDC_ROOT=$root/udc" \
        "OCTESSERA_USB_STORAGE_CONFIG=$root/usb-storage.conf" \
        "OCTESSERA_USB_SD_MOUNT=$root/sd-card" \
        "OCTESSERA_USB_STORAGE_STATE=$root/storage.state" \
        "OCTESSERA_USB_LIFECYCLE_LOCK=$root/lifecycle.lock" \
        "PATH=$FAKE_BIN:$PATH" \
        sh "$SCRIPT" "$action"
}

run_setup() {
    root=$1
    config=$2
    shift 2
    run_action "$root" setup "$config" "$@"
}

run_teardown() {
    root=$1
    shift
    run_action "$root" teardown /dev/null "$@"
}

run_storage_action() {
    root=$1
    action=$2
    config=$3
    shift 3
    run_action "$root" "$action" "$config" "$@"
}

write_config() { printf '%s\n' "$2" > "$1"; }
prepare_storage_root() {
    root=$1
    new_fake_configfs "$root"
    write_config "$root/usb-storage.conf" 'BACKING_DEVICE=/dev/loop0'
}

MIDI_CONFIG=$TEST_ROOT/midi.json
COMBINED_CONFIG=$TEST_ROOT/combined.json
AUDIO_CONFIG=$TEST_ROOT/audio.json
write_config "$MIDI_CONFIG" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":true},"usb":{"midiOutEnabled":true}}}'
write_config "$COMBINED_CONFIG" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":false},"usb":{"midiOutEnabled":true}}}'
write_config "$AUDIO_CONFIG" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":false}}}'

write_config "$TEST_ROOT/audio-100.json" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false}}}'
write_config "$TEST_ROOT/audio-010.json" '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":true,"hdmi":false}}}'
write_config "$TEST_ROOT/audio-001.json" '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":false,"hdmi":true}}}'
write_config "$TEST_ROOT/audio-110.json" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":false}}}'
write_config "$TEST_ROOT/audio-101.json" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":true}}}'
write_config "$TEST_ROOT/audio-011.json" '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":true,"hdmi":true}}}'
write_config "$TEST_ROOT/audio-111.json" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":true,"hdmi":true}}}'
write_config "$TEST_ROOT/audio-zero.json" '{"runtimeConfig":{"audioOutputs":{"dac":false,"usb":false,"hdmi":false}}}'
write_config "$TEST_ROOT/audio-extra.json" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false,"extra":false}}}'
write_config "$TEST_ROOT/audio-conflict.json" '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false},"usb":{"audioOut":"usb"}}}'
write_config "$TEST_ROOT/audio-malformed.json" '{'
for outputs in 100 010 001 110 101 011 111; do
    config="$TEST_ROOT/audio-$outputs.json"
    usb=${outputs#?}; usb=${usb%?}
    root="$TEST_ROOT/audio-set-$outputs"
    new_fake_configfs "$root"
    run_setup "$root" "$config" > "$root/setup.log"
    if [ "$usb" = 1 ]; then test -L "$root/config/usb_gadget/octessera/configs/c.1/uac2.usb0"; else test ! -e "$root/config/usb_gadget/octessera/functions/uac2.usb0"; fi
    run_teardown "$root" > "$root/teardown.log"
done
for config in audio-zero.json audio-extra.json audio-conflict.json audio-malformed.json; do
    root="$TEST_ROOT/reject-${config%.json}"
    new_fake_configfs "$root"
    assert_failed run_setup "$root" "$TEST_ROOT/$config"
    test ! -e "$root/config/usb_gadget/octessera"
done

root=$TEST_ROOT/patched-midi
new_fake_configfs "$root"
run_setup "$root" "$MIDI_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
test -L "$gadget/configs/c.1/midi.usb0"
test ! -e "$gadget/functions/uac2.usb0"
test -f "$gadget/functions/midi.usb0/interface_string"
assert_file_value 'Octessera MIDI' "$gadget/strings/0x409/product"
assert_file_value 'Octessera MIDI' "$gadget/functions/midi.usb0/id"
assert_file_value 'Octessera MIDI' "$gadget/functions/midi.usb0/function_name"
expected=$root/expected-interface-string
printf '%s' 'Octessera MIDI' > "$expected"
cmp -s "$expected" "$gadget/functions/midi.usb0/interface_string"
assert_file_value fake-udc "$gadget/UDC"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/sysfs-lf-readback
new_fake_configfs "$root"
observed=$root/underlying-interface-write
run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=lf FAKE_READBACK_OBSERVED="$observed" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
expected=$root/expected-interface-string-lf
printf '%s\n' 'Octessera MIDI' > "$expected"
cmp -s "$expected" "$gadget/functions/midi.usb0/interface_string"
expected=$root/expected-underlying-interface-write
printf '%s' 'Octessera MIDI' > "$expected"
cmp -s "$expected" "$observed"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/patched-combined
new_fake_configfs "$root"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
test -L "$gadget/configs/c.1/midi.usb0"
test -L "$gadget/configs/c.1/uac2.usb0"
assert_file_value 'Octessera Audio + MIDI' "$gadget/strings/0x409/product"
assert_file_value 3 "$gadget/functions/uac2.usb0/p_chmask"
assert_file_value 2 "$gadget/functions/uac2.usb0/p_ssize"
assert_file_value 44100 "$gadget/functions/uac2.usb0/p_srate"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/unpatched-id-only
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_MIDI_ID_ONLY=1
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/write-failure
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_MIDI_WRITE_FAILURE=1
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/wrong-readback
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=fail
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/short-readback
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=short
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/modified-readback
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_READBACK_MODE=modified
test ! -e "$root/config/usb_gadget/octessera"

for mode in crlf internal two-lf spaces long; do
    root=$TEST_ROOT/malformed-readback-$mode
    new_fake_configfs "$root"
    assert_failed run_setup "$root" "$MIDI_CONFIG" "FAKE_READBACK_MODE=$mode"
    test ! -e "$root/config/usb_gadget/octessera"
done

root=$TEST_ROOT/bind-failure
new_fake_configfs "$root"
assert_failed run_setup "$root" "$COMBINED_CONFIG" FAKE_BIND_FAILURE=1
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/links-before-bind
new_fake_configfs "$root"
run_setup "$root" "$COMBINED_CONFIG" FAKE_REQUIRE_LINKS=1 FAKE_REQUIRE_AUDIO=1 > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
test -L "$gadget/configs/c.1/midi.usb0"
test -L "$gadget/configs/c.1/uac2.usb0"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/post-bind-mismatch
new_fake_configfs "$root"
assert_failed run_setup "$root" "$MIDI_CONFIG" FAKE_POST_BIND_MISMATCH=1 FAKE_POST_BIND_MARKER="$root/post-bind-mismatch"
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/unbind-failure
new_fake_configfs "$root"
run_setup "$root" "$MIDI_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
rm "$gadget/UDC"
mkdir "$gadget/UDC"
assert_failed run_teardown "$root" FAKE_BOUND_UDC=fake-udc
test -d "$gadget"
test -d "$gadget/functions/midi.usb0"
test -L "$gadget/configs/c.1/midi.usb0"
rm -rf "$gadget"

root=$TEST_ROOT/cleanup-failure
new_fake_configfs "$root"
run_setup "$root" "$MIDI_CONFIG" > "$root/setup.log"
gadget=$root/config/usb_gadget/octessera
assert_failed run_teardown "$root" FAKE_RMDIR_FAILURE=1
test -d "$gadget/functions/midi.usb0"
test "$(cat "$gadget/UDC")" = ''
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/storage-round-trip
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
gadget=$root/config/usb_gadget/octessera
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
test -L "$gadget/configs/c.1/mass_storage.usb0"
test ! -e "$gadget/configs/c.1/midi.usb0"
test ! -e "$gadget/configs/c.1/uac2.usb0"
test ! -e "$root/mounted"
grep -q '^DEV=/dev/loop0$' "$root/storage.state"
grep -q '^WAS_MOUNTED=1$' "$root/storage.state"
run_storage_action "$root" storage-stop "$COMBINED_CONFIG" > "$root/stop-storage.log"
test -L "$gadget/configs/c.1/midi.usb0"
test -L "$gadget/configs/c.1/uac2.usb0"
assert_file_value fake-udc "$gadget/UDC"
test -e "$root/mounted"
test ! -e "$root/storage.state"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$gadget"

root=$TEST_ROOT/storage-naming-restore-failure
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
assert_failed run_storage_action "$root" storage-stop "$COMBINED_CONFIG" FAKE_MIDI_ID_ONLY=1
test -e "$root/mounted"
test ! -e "$root/storage.state"
test ! -e "$root/config/usb_gadget/octessera"

root=$TEST_ROOT/storage-unbind-failure
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
gadget=$root/config/usb_gadget/octessera
rm "$gadget/UDC"
mkdir "$gadget/UDC"
assert_failed run_storage_action "$root" storage-stop "$COMBINED_CONFIG" FAKE_BOUND_UDC=fake-udc
test ! -e "$root/mounted"
test -e "$root/storage.state"
test -L "$gadget/configs/c.1/mass_storage.usb0"
test -d "$gadget/functions/mass_storage.usb0"
rm -rf "$gadget"

root=$TEST_ROOT/storage-cleanup-failure
prepare_storage_root "$root"
: > "$root/mounted"
run_setup "$root" "$COMBINED_CONFIG" > "$root/setup-normal.log"
run_storage_action "$root" storage-start "$COMBINED_CONFIG" > "$root/setup-storage.log"
gadget=$root/config/usb_gadget/octessera
assert_failed run_storage_action "$root" storage-stop "$COMBINED_CONFIG" FAKE_RMDIR_FAILURE=1
test ! -e "$root/mounted"
test -e "$root/storage.state"
test -d "$gadget/functions/mass_storage.usb0"
rm -rf "$gadget"

root=$TEST_ROOT/concurrency
new_fake_configfs "$root"
started=$root/hold-started
release=$root/hold-release
calls=$root/modprobe-calls
run_setup "$root" "$AUDIO_CONFIG" FAKE_HOLD_STARTED="$started" FAKE_HOLD_RELEASE="$release" FAKE_MODPROBE_CALLS="$calls" > "$root/first.log" 2>&1 &
first_pid=$!
while [ ! -e "$started" ]; do sleep 0.01; done
run_setup "$root" "$AUDIO_CONFIG" FAKE_HOLD_STARTED="$started" FAKE_HOLD_RELEASE="$release" FAKE_MODPROBE_CALLS="$calls" > "$root/second.log" 2>&1 &
second_pid=$!
sleep 0.1
test "$(wc -l < "$calls" | tr -d '[:space:]')" -eq 1
: > "$release"
wait "$first_pid"
wait "$second_pid"
run_teardown "$root" > "$root/teardown.log"
test ! -e "$root/config/usb_gadget/octessera"

printf '%s\n' 'Raspberry USB gadget fake-ConfigFS tests passed.'
