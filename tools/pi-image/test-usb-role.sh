#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
helper="$script_dir/stage4-octessera/files/root/usr/local/sbin/octessera-usb-role"

if ! command -v python3 >/dev/null 2>&1 || [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' 'USB role helper tests skipped; root and python3 are required.'
    exit 0
fi

root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT
mkdir -p "$root/boot/firmware"
config="$root/boot/firmware/config.txt"

write_config() {
    printf '%s\n' '# fixture' '[all]' 'dtoverlay=dwc2,dr_mode=peripheral' 'dtparam=spi=on' > "$config"
    chmod 0640 "$config"
    chown 0:0 "$config"
}

write_stock_config() {
    cp "$script_dir/fixtures/stock-boot-config.txt" "$config"
    chmod 0640 "$config"
    chown 0:0 "$config"
}

managed_role() {
    awk '
        /^\[all\]$/ { section = "all"; next }
        /^\[[^]]+\]$/ { section = "other"; next }
        section == "all" && /^dtoverlay=dwc2,dr_mode=/ { print; exit }
    ' "$config"
}

model_scope() {
    awk '
        /^\[cm5\]$/ { in_cm5 = 1 }
        in_cm5 && /^\[all\]$/ { exit }
        in_cm5 { print }
    ' "$config"
}

prepare_python_failure_injector() {
    mkdir -p "$root/python" "$root/bin"
    cat > "$root/python/sitecustomize.py" <<'PY'
import os


failure = os.environ.get("OCTESSERA_USB_ROLE_TEST_POST_REPLACE_FAILURE")
committed = False
directory_fds = set()
original_close = os.close
original_fsync = os.fsync
original_open = os.open
original_replace = os.replace


def replace(source, destination):
    global committed
    result = original_replace(source, destination)
    committed = True
    return result


def open_directory(path, flags, mode=0o777, *, dir_fd=None):
    if committed and failure == "open" and flags & getattr(os, "O_DIRECTORY", 0):
        raise OSError("injected post-replace directory open failure")
    descriptor = original_open(path, flags, mode, dir_fd=dir_fd)
    if flags & getattr(os, "O_DIRECTORY", 0):
        directory_fds.add(descriptor)
    return descriptor


def fsync(descriptor):
    if committed and failure == "fsync" and descriptor in directory_fds:
        raise OSError("injected post-replace directory fsync failure")
    return original_fsync(descriptor)


def close(descriptor):
    if committed and failure == "close" and descriptor in directory_fds:
        raise OSError("injected post-replace directory close failure")
    return original_close(descriptor)


os.close = close
os.fsync = fsync
os.open = open_directory
os.replace = replace
PY
    cat > "$root/bin/python3" <<'SH'
#!/bin/sh
exec /usr/bin/python3 "$@"
SH
    chmod +x "$root/bin/python3"
}

assert_gadget() {
    OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" is-gadget
}

assert_host() {
    if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" is-gadget; then
        printf '%s\n' 'host role was reported as gadget' >&2
        exit 1
    fi
}

write_config
assert_gadget
OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" host
assert_host
grep -qxF 'dtoverlay=dwc2,dr_mode=host' "$config"
grep -qxF '# fixture' "$config"
grep -qxF '[all]' "$config"
grep -qxF 'dtparam=spi=on' "$config"
[ "$(stat -c '%u:%g:%a:%h' "$config")" = 0:0:640:1 ]
OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" gadget
assert_gadget
[ "$(stat -c '%u:%g:%a:%h' "$config")" = 0:0:640:1 ]

write_stock_config
stock_model_scope="$(model_scope)"
assert_gadget
[ "$(managed_role)" = 'dtoverlay=dwc2,dr_mode=peripheral' ]
OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" host
[ "$(managed_role)" = 'dtoverlay=dwc2,dr_mode=host' ]
[ "$(model_scope)" = "$stock_model_scope" ]
OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" gadget
[ "$(managed_role)" = 'dtoverlay=dwc2,dr_mode=peripheral' ]
[ "$(model_scope)" = "$stock_model_scope" ]

prepare_python_failure_injector
for failure in open fsync close; do
    write_config
    OCTESSERA_USB_ROLE_BOOT_ROOT="$root" \
        OCTESSERA_USB_ROLE_TEST_POST_REPLACE_FAILURE="$failure" \
        PYTHONPATH="$root/python" PATH="$root/bin:$PATH" "$helper" host
    grep -qxF 'dtoverlay=dwc2,dr_mode=host' "$config"
done

write_config
printf '%s\n' 'dtoverlay=dwc2,dr_mode=host' >> "$config"
duplicate_before="$(cat "$config")"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" is-gadget; then
    printf '%s\n' 'duplicate role lines were accepted' >&2
    exit 1
fi
[ "$(cat "$config")" = "$duplicate_before" ]

write_config
printf '%s\n' 'DEV=/dev/loop0' 'WAS_MOUNTED=0' > "$root/storage.state"
chmod 0600 "$root/storage.state"
role_before="$(cat "$config")"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" OCTESSERA_USB_STORAGE_STATE="$root/storage.state" "$helper" host; then
    printf '%s\n' 'active SD2 transfer permitted host apply' >&2
    exit 1
fi
[ "$(cat "$config")" = "$role_before" ]

write_config
printf '%s\n' 'DEV=/dev/loop0' > "$root/storage.state"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" OCTESSERA_USB_STORAGE_STATE="$root/storage.state" "$helper" host; then
    printf '%s\n' 'malformed SD2 state permitted host apply' >&2
    exit 1
fi

write_config
sed -i 's/^\[all\]$/[pi4]/' "$config"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" is-gadget; then
    printf '%s\n' 'role outside the managed all block was accepted' >&2
    exit 1
fi

write_config
printf '%s\n' '[pi4]' 'dtoverlay=dwc2,dr_mode=host' >> "$config"
before_model_specific="$(cat "$config")"
assert_gadget
[ "$(cat "$config")" = "$before_model_specific" ]

rm -f "$config"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" is-gadget 2>/dev/null; then
    printf '%s\n' 'missing boot config was accepted' >&2
    exit 1
else
    [ "$?" -eq 255 ]
fi

write_config
sed -i 's/dr_mode=peripheral/dr_mode=peripheral # unsafe/' "$config"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" gadget; then
    printf '%s\n' 'malformed role line was accepted' >&2
    exit 1
fi

write_config
ln "$config" "$root/boot/firmware/config-copy.txt"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" host; then
    printf '%s\n' 'hard-linked boot config was accepted' >&2
    exit 1
fi

write_config
chmod 0664 "$config"
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" host; then
    printf '%s\n' 'unsafe boot config mode was accepted' >&2
    exit 1
fi

rm -f "$root/boot/firmware/config-copy.txt"
sys_root="$root/sys-root"
driver="$sys_root/sys/bus/platform/drivers/dwc2"
udc="$sys_root/sys/class/udc"
live_log="$root/live.log"
mkdir -p "$driver" "$udc" "$root/live-bin" "$root/boot/firmware/octessera/overlays"
ln -s ../../../../devices/platform/soc/3f980000.usb "$driver/3f980000.usb"
: > "$root/boot/firmware/octessera/overlays/dwc2.dtbo"
cat > "$root/live-bin/dtoverlay" <<'SH'
#!/bin/sh
echo "dtoverlay $*" >> "$FAKE_LIVE_LOG"
[ "$1" != -r ] || exit 1
[ -z "${FAKE_DTOVERLAY_FAILURE:-}" ] || exit 1
case "$4" in
    dr_mode=peripheral) mkdir -p "$FAKE_UDC/3f980000.usb" ;;
    dr_mode=host) rmdir "$FAKE_UDC/3f980000.usb" ;;
esac
SH
cat > "$root/live-bin/gadget" <<'SH'
#!/bin/sh
echo "gadget $1" >> "$FAKE_LIVE_LOG"
[ -z "${FAKE_GADGET_FAILURE:-}" ]
SH
printf '%s\n' '#!/bin/sh' > "$root/live-bin/logger"
chmod +x "$root/live-bin/dtoverlay" "$root/live-bin/gadget" "$root/live-bin/logger"

run_live() {
    : > "$live_log"
    : > "$driver/unbind"
    : > "$driver/bind"
    env OCTESSERA_USB_ROLE_BOOT_ROOT="$root" OCTESSERA_USB_ROLE_SYS_ROOT="$sys_root" \
        OCTESSERA_USB_GADGET_HELPER="$root/live-bin/gadget" FAKE_LIVE_LOG="$live_log" FAKE_UDC="$udc" \
        PATH="$root/live-bin:$PATH" "$@"
}

overlays="$root/boot/firmware/octessera/overlays"
write_stock_config
mkdir -p "$udc/3f980000.usb"
run_live "$helper" host
[ "$(managed_role)" = 'dtoverlay=dwc2,dr_mode=host' ]
printf '%s\n' 'gadget teardown' 'dtoverlay -r dwc2' "dtoverlay -d $overlays dwc2 dr_mode=host" | cmp -s - "$live_log"
[ "$(cat "$driver/unbind")" = 3f980000.usb ]
[ "$(cat "$driver/bind")" = 3f980000.usb ]
test ! -e "$udc/3f980000.usb"

run_live "$helper" host
[ ! -s "$live_log" ]

run_live "$helper" gadget
[ "$(managed_role)" = 'dtoverlay=dwc2,dr_mode=peripheral' ]
printf '%s\n' 'dtoverlay -r dwc2' "dtoverlay -d $overlays dwc2 dr_mode=peripheral" 'gadget setup' | cmp -s - "$live_log"
[ "$(cat "$driver/bind")" = 3f980000.usb ]
test -d "$udc/3f980000.usb"

run_live "$helper" gadget
printf '%s\n' 'gadget setup' | cmp -s - "$live_log"

if run_live FAKE_GADGET_FAILURE=1 "$helper" gadget; then
    printf '%s\n' 'failed gadget setup reported success' >&2
    exit 1
fi

if run_live FAKE_DTOVERLAY_FAILURE=1 "$helper" host; then
    printf '%s\n' 'failed live overlay reported success' >&2
    exit 1
fi
[ "$(managed_role)" = 'dtoverlay=dwc2,dr_mode=host' ]
[ ! -s "$driver/bind" ]

write_stock_config
mkdir -p "$udc/3f980000.usb"
if run_live FAKE_GADGET_FAILURE=1 "$helper" host; then
    printf '%s\n' 'failed gadget teardown reported success' >&2
    exit 1
fi
[ ! -s "$driver/unbind" ]

ln -s ../../../../devices/platform/soc/3f981000.usb "$driver/3f981000.usb"
write_stock_config
if run_live "$helper" host; then
    printf '%s\n' 'ambiguous dwc2 controller was accepted' >&2
    exit 1
fi
rm "$driver/3f981000.usb" "$driver/3f980000.usb"
write_stock_config
if run_live "$helper" host; then
    printf '%s\n' 'missing dwc2 controller was accepted' >&2
    exit 1
fi
[ ! -s "$live_log" ]

printf '%s\n' 'Raspberry USB role helper tests passed.'
