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
if OCTESSERA_USB_ROLE_BOOT_ROOT="$root" "$helper" is-gadget; then
    printf '%s\n' 'competing dwc2 directive was accepted' >&2
    exit 1
fi

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

printf '%s\n' 'Raspberry USB role helper tests passed.'
