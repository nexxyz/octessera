#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/pi-image/test-sanitized-image-boot-layout-fixture.sh
source "$script_dir/test-sanitized-image-boot-layout-fixture.sh"

reset_fixture
mkdir -p "$fixture/root/etc/initramfs-tools/scripts/init-premount" "$fixture/root/opt/octessera/releases/1.2.3" "$fixture/root/usr/local/bin" "$fixture/archive/scripts/init-premount" "$fixture/archive/usr/local/bin" "$fixture/archive/usr/bin"
cp "$script_dir/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
chmod 0755 "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
cp "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$fixture/archive/scripts/init-premount/octessera-boot-splash"
chmod 0755 "$fixture/archive/scripts/init-premount/octessera-boot-splash"
printf '%s\n' 'constructor-runtime-binary' > "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi"
chmod 0755 "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi"
cp "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi" "$fixture/archive/usr/local/bin/octessera-pi"
ln -s /opt/octessera/releases/1.2.3 "$fixture/root/opt/octessera/current"
ln -s /opt/octessera/current/octessera-pi "$fixture/root/usr/local/bin/octessera-pi"
ln -s usr/bin "$fixture/archive/bin"
ln -s dash "$fixture/archive/usr/bin/sh"
for archive_entry in usr/bin/dash usr/bin/setsid usr/bin/sleep usr/bin/cat usr/bin/mv usr/bin/chmod usr/bin/chown usr/bin/rm; do
    mkdir -p "$fixture/archive/${archive_entry%/*}"
    printf '%s\n' "fixture-$archive_entry" > "$fixture/archive/$archive_entry"
    chmod 0755 "$fixture/archive/$archive_entry"
done
mkdir -p "$fixture/archive/lib/modules/fixture/kernel/drivers/spi"
for required_module in spi-bcm2835 spidev; do
    printf '%s\n' "fixture-$required_module" > "$fixture/archive/lib/modules/fixture/kernel/drivers/spi/$required_module.ko"
done
make_initramfs() {
    local output="$fixture/boot/octessera-bound.img"
    mkdir -p "$fixture/boot"
    (cd "$fixture/archive" && find . -print | cpio -o -H newc --quiet | gzip -n > "$output")
}
make_initramfs
mkdir -p "$fixture/boot/octessera" "$fixture/lsinitramfs-bin"
cp "$fixture/boot/octessera-bound.img" "$fixture/boot/octessera/initrd.img-1.2.3"
cat > "$fixture/lsinitramfs-bin/lsinitramfs" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = -l ]; then
    while IFS= read -r entry; do
        case "$entry" in
            bin)
                target="${OCTESSERA_TEST_BIN_TARGET:-usr/bin}"
                printf '%s\n' "lrwxrwxrwx 1 root root ${#target} Jan 1 1970 bin -> $target" ;;
            usr/bin/sh)
                target="${OCTESSERA_TEST_SH_TARGET:-dash}"
                printf '%s\n' "lrwxrwxrwx 1 root root ${#target} Jan 1 1970 usr/bin/sh -> $target" ;;
            bin/*)
                case "${OCTESSERA_TEST_LEGACY_TYPE:-regular}" in
                    symlink) printf '%s\n' "lrwxrwxrwx 1 root root 8 Jan 1 1970 $entry -> usr/bin" ;;
                    directory) printf '%s\n' "drwxr-xr-x 1 root root 0 Jan 1 1970 $entry" ;;
                    device) printf '%s\n' "crw-rw-rw- 1 root root 0 Jan 1 1970 $entry" ;;
                    hardlink) printf '%s\n' "-rwxr-xr-x 2 root root 8 Jan 1 1970 $entry" ;;
                    *) printf '%s\n' "-rwxr-xr-x 1 root root 8 Jan 1 1970 $entry" ;;
                esac ;;
            usr/bin/sleep)
                printf '%s\n' "${OCTESSERA_TEST_ENTRY_MODE:-$(printf '%s' -rwxr-xr-x)} ${OCTESSERA_TEST_ENTRY_LINKS:-1} root root ${OCTESSERA_TEST_ENTRY_SIZE:-$(stat -c '%s' "$OCTESSERA_TEST_INITRAMFS_ARCHIVE_ROOT/$entry")} Jan 1 1970 $entry" ;;
            *)
                size=1
                if [ -e "$OCTESSERA_TEST_INITRAMFS_ARCHIVE_ROOT/$entry" ] && [ ! -L "$OCTESSERA_TEST_INITRAMFS_ARCHIVE_ROOT/$entry" ]; then
                    size="$(stat -c '%s' "$OCTESSERA_TEST_INITRAMFS_ARCHIVE_ROOT/$entry")"
                fi
                printf '%s\n' "-rwxr-xr-x 1 root root $size Jan 1 1970 $entry" ;;
        esac
    done < "$OCTESSERA_TEST_INITRAMFS_LISTING"
else
    cat "$OCTESSERA_TEST_INITRAMFS_LISTING"
fi
EOF
chmod 0755 "$fixture/lsinitramfs-bin/lsinitramfs"
listing="$fixture/initramfs-listing"
cat > "$listing" <<'EOF'
scripts/init-premount/octessera-boot-splash
usr/local/bin/octessera-pi
bin
usr/bin/sh
usr/bin/dash
usr/bin/setsid
usr/bin/sleep
usr/bin/cat
usr/bin/mv
usr/bin/chmod
usr/bin/chown
usr/bin/rm
lib/modules/fixture/kernel/drivers/spi/spi-bcm2835.ko
lib/modules/fixture/kernel/drivers/spi/spidev.ko
EOF
for ((index = 1; index <= 8192; index++)); do
    printf 'usr/lib/fixture-trailing/%04d\n' "$index" >> "$listing"
done
export OCTESSERA_TEST_INITRAMFS_LISTING="$listing"
export OCTESSERA_TEST_INITRAMFS_ARCHIVE_ROOT="$fixture/archive"
export PATH="$fixture/lsinitramfs-bin:$PATH"
require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"

for target_case in wrong absolute escaping cyclic; do
    case "$target_case" in
        wrong) export OCTESSERA_TEST_BIN_TARGET=wrong ;;
        absolute) export OCTESSERA_TEST_BIN_TARGET=/usr/bin ;;
        escaping) export OCTESSERA_TEST_BIN_TARGET=../usr/bin ;;
        cyclic) export OCTESSERA_TEST_BIN_TARGET=bin ;;
    esac
    if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
        echo "Boot layout accepted $target_case bin symlink target." >&2
        exit 1
    fi
    unset OCTESSERA_TEST_BIN_TARGET
done
for target_case in wrong absolute escaping cyclic; do
    case "$target_case" in
        wrong) export OCTESSERA_TEST_SH_TARGET=wrong ;;
        absolute) export OCTESSERA_TEST_SH_TARGET=/bin/sh ;;
        escaping) export OCTESSERA_TEST_SH_TARGET=../sh ;;
        cyclic) export OCTESSERA_TEST_SH_TARGET=sh ;;
    esac
    if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
        echo "Boot layout accepted $target_case usr/bin/sh symlink target." >&2
        exit 1
    fi
    unset OCTESSERA_TEST_SH_TARGET
done
for command_entry in usr/bin/dash usr/bin/setsid usr/bin/sleep usr/bin/cat usr/bin/mv usr/bin/chmod usr/bin/chown usr/bin/rm; do
    altered_listing="$fixture/missing-${command_entry##*/}-listing"
    sed "\|^$command_entry$|d" "$listing" > "$altered_listing"
    export OCTESSERA_TEST_INITRAMFS_LISTING="$altered_listing"
    if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
        echo "Boot layout accepted a missing command target: $command_entry" >&2
        exit 1
    fi
done
for invalid_case in zero non-executable non-regular device hardlink oversized; do
    export OCTESSERA_TEST_ENTRY_MODE=-rwxr-xr-x
    export OCTESSERA_TEST_ENTRY_LINKS=1
    export OCTESSERA_TEST_ENTRY_SIZE=8
    case "$invalid_case" in
        zero) export OCTESSERA_TEST_ENTRY_SIZE=0 ;;
        non-executable) export OCTESSERA_TEST_ENTRY_MODE=-rw-r--r-- ;;
        non-regular) export OCTESSERA_TEST_ENTRY_MODE=drwxr-xr-x; export OCTESSERA_TEST_ENTRY_SIZE=0 ;;
        device) export OCTESSERA_TEST_ENTRY_MODE=crw-rw-rw-; export OCTESSERA_TEST_ENTRY_SIZE=0 ;;
        hardlink) export OCTESSERA_TEST_ENTRY_LINKS=2 ;;
        oversized) export OCTESSERA_TEST_ENTRY_SIZE=67108865 ;;
    esac
    export OCTESSERA_TEST_INITRAMFS_LISTING="$listing"
    if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
        echo "Boot layout accepted an invalid command target: $invalid_case" >&2
        exit 1
    fi
    unset OCTESSERA_TEST_ENTRY_MODE OCTESSERA_TEST_ENTRY_LINKS OCTESSERA_TEST_ENTRY_SIZE
done
for legacy_entry in bin/sh bin/dash bin/setsid bin/sleep bin/cat bin/mv bin/chmod bin/chown bin/rm bin/unexpected; do
    for legacy_type in regular symlink directory device hardlink; do
        legacy_listing="$fixture/legacy-${legacy_entry##*/}-$legacy_type-listing"
        cp "$listing" "$legacy_listing"
        printf '%s\n' "$legacy_entry" >> "$legacy_listing"
        export OCTESSERA_TEST_INITRAMFS_LISTING="$legacy_listing"
        export OCTESSERA_TEST_LEGACY_TYPE="$legacy_type"
        if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
            echo "Boot layout accepted a legacy $legacy_entry $legacy_type entry." >&2
            exit 1
        fi
        unset OCTESSERA_TEST_LEGACY_TYPE
    done
done
missing_entry_listing="$fixture/missing-entry-listing"
sed '/^usr\/bin\/setsid$/d' "$listing" > "$missing_entry_listing"
export OCTESSERA_TEST_INITRAMFS_LISTING="$missing_entry_listing"
if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted an initramfs missing a required entry.' >&2
    exit 1
fi
missing_module_listing="$fixture/missing-module-listing"
sed '/spi-bcm2835/d' "$listing" > "$missing_module_listing"
export OCTESSERA_TEST_INITRAMFS_LISTING="$missing_module_listing"
if require_octessera_initramfs_boot_layer "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted an initramfs missing a required module.' >&2
    exit 1
fi
