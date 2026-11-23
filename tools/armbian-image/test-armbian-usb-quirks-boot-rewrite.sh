#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=userpatches/overlay/usr/local/lib/octessera/armbian-usb-quirks-boot-rewrite.sh
source "$root/userpatches/overlay/usr/local/lib/octessera/armbian-usb-quirks-boot-rewrite.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fail() { echo "Armbian USB quirks boot rewrite test failed: $*" >&2; exit 1; }

write_bsp_fixture() {
    printf '%s\n' \
        '#!/bin/bash' \
        '# add_usb_storage_quirks' \
        'add_usb_storage_quirks() {' \
        '	echo "usbstoragequirks=x" >> /boot/armbianEnv.txt' \
        '} # add_usb_storage_quirks' \
        "case \$1 in" \
        '	*start*)' \
        '		prepare_board &' \
        '		# add usb quirks' \
        '		add_usb_storage_quirks &' \
        '		;;' \
        'esac' > "$1"
    chmod 0755 "$1"
}

bsp="$work/armbian-hardware-optimization"
write_bsp_fixture "$bsp"
octessera_disable_armbian_usb_quirks_rewrite "$bsp" || fail "the current BSP shape was not disabled"
octessera_armbian_usb_quirks_rewrite_disabled "$bsp" || fail "the disabled BSP did not validate"
grep -qxF $'\t\tprepare_board &' "$bsp" || fail "the rest of the start step changed"
[[ "$(stat -c %a "$bsp")" == 755 ]] || fail "the BSP script lost its mode"
bash -n "$bsp"

write_bsp_fixture "$work/unchanged"
octessera_armbian_usb_quirks_rewrite_disabled "$work/unchanged" 2>/dev/null && fail "an untouched BSP validated as disabled"

for broken in missing-call twice renamed; do
    write_bsp_fixture "$work/$broken"
    case "$broken" in
        missing-call) sed -i '/^\t\tadd_usb_storage_quirks &$/d' "$work/$broken" ;;
        twice) printf '\t\tadd_usb_storage_quirks &\n' >> "$work/$broken" ;;
        renamed) sed -i 's/^add_usb_storage_quirks() {$/add_usb_quirks() {/' "$work/$broken" ;;
    esac
    cp "$work/$broken" "$work/$broken.before"
    if octessera_disable_armbian_usb_quirks_rewrite "$work/$broken" 2>"$work/$broken.err"; then
        fail "a $broken BSP shape was accepted"
    fi
    grep -qF 'changed shape' "$work/$broken.err" || fail "a $broken BSP shape was not reported"
    cmp -s "$work/$broken" "$work/$broken.before" || fail "a $broken BSP shape was modified"
done

env_file="$work/armbianEnv.txt"
printf 'verbosity=1\nrootfstype=ext4' > "$env_file"
octessera_write_armbian_usb_quirks "$env_file" || fail "the quirks line was not written"
[[ "$(cat "$env_file")" == $'verbosity=1\nrootfstype=ext4\nusbstoragequirks=0x2537:0x1066:u,0x2537:0x1068:u' ]] || fail "the quirks line was not appended on its own line"
cp "$env_file" "$env_file.before"
octessera_write_armbian_usb_quirks "$env_file" || fail "an existing quirks line was refused"
cmp -s "$env_file" "$env_file.before" || fail "an existing quirks line was rewritten"
printf 'usbstoragequirks=a\nusbstoragequirks=b\n' > "$env_file"
octessera_write_armbian_usb_quirks "$env_file" 2>/dev/null && fail "a repeated quirks line was accepted"

printf 'Armbian USB quirks boot rewrite tests passed\n'
