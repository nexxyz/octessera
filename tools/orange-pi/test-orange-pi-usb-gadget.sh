#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SCRIPT=$SCRIPT_DIR/orange-pi-usb-gadget.sh
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
    if [ "$argument" = "${FAKE_UAC2:-}" ]; then
        : > "$argument/function_name"
        : > "$argument/p_chmask"
        : > "$argument/p_ssize"
        : > "$argument/p_srate"
        : > "$argument/c_chmask"
    fi
done
EOF
chmod +x "$FAKE_BIN/mkdir"

cat > "$FAKE_BIN/rmdir" <<'EOF'
#!/bin/sh

set -eu

if [ -n "${FAKE_GADGET:-}" ] && [ -d "$FAKE_GADGET" ]; then
    bound=$(cat "$FAKE_GADGET/UDC" 2>/dev/null || true)
    [ -z "$bound" ] || {
        printf 'rmdir attempted while gadget was bound\n' >&2
        exit 91
    }
    rm -f "$FAKE_GADGET"/configs/c.1/MaxPower \
        "$FAKE_GADGET"/configs/c.1/strings/0x409/configuration \
        "$FAKE_GADGET"/strings/0x409/manufacturer \
        "$FAKE_GADGET"/strings/0x409/product \
        "$FAKE_GADGET"/strings/0x409/serialnumber \
        "$FAKE_GADGET"/functions/uac2.usb0/function_name \
        "$FAKE_GADGET"/functions/uac2.usb0/p_chmask \
        "$FAKE_GADGET"/functions/uac2.usb0/p_ssize \
        "$FAKE_GADGET"/functions/uac2.usb0/p_srate \
        "$FAKE_GADGET"/functions/uac2.usb0/c_chmask
fi

if [ "$#" -eq 2 ] && [ "$1" = "--" ] && [ "$2" = "${FAKE_GADGET:-}" ]; then
    rm -f "$FAKE_GADGET"/idVendor "$FAKE_GADGET"/idProduct \
        "$FAKE_GADGET"/bcdUSB "$FAKE_GADGET"/bcdDevice "$FAKE_GADGET"/UDC \
        "$FAKE_GADGET"/configs/c.1/strings/0x409/configuration
fi

exec /usr/bin/rmdir "$@"
EOF
chmod +x "$FAKE_BIN/rmdir"

assert_failed() {
    if "$@" >/dev/null 2>&1; then
        printf 'expected command to fail: %s\n' "$*" >&2
        exit 1
    fi
}

assert_contains() {
    needle=$1
    file=$2
    grep -F -- "$needle" "$file" >/dev/null || {
        printf 'missing [%s] in %s\n' "$needle" "$file" >&2
        exit 1
    }
}

new_fake_configfs() {
    root=$1
    mkdir -p "$root/config/usb_gadget" "$root/udc/fake-udc"
}

run_setup() {
    root=$1
    mode=$2
    FAKE_UAC2=$root/config/usb_gadget/octessera-orange-pi/functions/uac2.usb0 \
        PATH="$FAKE_BIN:$PATH" sh "$SCRIPT" setup \
        --configfs-root "$root/config" \
        --udc-root "$root/udc" \
        --udc fake-udc \
        --mode "$mode"
}

run_teardown() {
    root=$1
    FAKE_GADGET=$root/config/usb_gadget/octessera-orange-pi \
        PATH="$FAKE_BIN:$PATH" sh "$SCRIPT" teardown \
        --configfs-root "$root/config" \
        --udc-root "$root/udc" \
        --udc fake-udc
}

for mode in midi uac2 combined; do
    root=$TEST_ROOT/$mode
    new_fake_configfs "$root"
    run_setup "$root" "$mode" > "$root/setup.log"
    gadget=$root/config/usb_gadget/octessera-orange-pi
    test -d "$gadget"
    test -d "$gadget/functions"
    if [ "$mode" = midi ] || [ "$mode" = combined ]; then
        test -d "$gadget/functions/midi.usb0"
    else
        test ! -e "$gadget/functions/midi.usb0"
    fi
    if [ "$mode" = uac2 ] || [ "$mode" = combined ]; then
        test -d "$gadget/functions/uac2.usb0"
    else
        test ! -e "$gadget/functions/uac2.usb0"
    fi
    test ! -e "$gadget/functions/mass_storage.usb0"
    if [ "$mode" = midi ] || [ "$mode" = combined ]; then
        test -L "$gadget/configs/c.1/midi.usb0"
    else
        test ! -e "$gadget/configs/c.1/midi.usb0"
    fi
    if [ "$mode" = uac2 ] || [ "$mode" = combined ]; then
        test -L "$gadget/configs/c.1/uac2.usb0"
    else
        test ! -e "$gadget/configs/c.1/uac2.usb0"
    fi
    assert_contains fake-udc "$gadget/UDC"
    run_teardown "$root" > "$root/teardown.log"
    test ! -e "$gadget"
done

root=$TEST_ROOT/existing
new_fake_configfs "$root"
mkdir "$root/config/usb_gadget/octessera-orange-pi"
assert_failed run_setup "$root" midi

root=$TEST_ROOT/prebound
new_fake_configfs "$root"
mkdir "$root/config/usb_gadget/other-gadget"
printf 'fake-udc\n' > "$root/config/usb_gadget/other-gadget/UDC"
assert_failed run_setup "$root" midi

root=$TEST_ROOT/missing-udc
new_fake_configfs "$root"
assert_failed sh "$SCRIPT" setup --configfs-root "$root/config" --udc-root "$root/udc" --mode midi

root=$TEST_ROOT/mass-storage
new_fake_configfs "$root"
assert_failed run_setup "$root" mass-storage

root=$TEST_ROOT/mismatch
new_fake_configfs "$root"
run_setup "$root" midi >/dev/null
gadget=$root/config/usb_gadget/octessera-orange-pi
assert_failed "$SCRIPT" teardown --configfs-root "$root/config" --udc-root "$root/udc" --udc another-udc
assert_contains fake-udc "$gadget/UDC"
run_teardown "$root" >/dev/null

bind_line=$(grep -n 'printf .%s. .\$UDC. > .\$GADGET/UDC.' "$SCRIPT" | cut -d: -f1)
link_line=$(grep -n 'ln -s .\$GADGET/functions' "$SCRIPT" | tail -n 1 | cut -d: -f1)
test -n "$bind_line" && test -n "$link_line" && test "$bind_line" -gt "$link_line"

printf 'Orange Pi USB gadget fake-configfs tests passed\n'
