#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$script_dir/verify-boot-layout.sh"

fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

reset_fixture() {
    rm -rf "$fixture"
    mkdir -p "$fixture/boot" "$fixture/root"
}

reset_fixture
mkdir -p "$fixture/boot/octessera/overlays"
printf '%s\n' '# octessera additions' > "$fixture/boot/config.txt"
printf '%s\n' 'dtbo' > "$fixture/boot/octessera/overlays/i2s-dac-no20.dtbo"
require_octessera_boot_config "$fixture/boot" "$fixture/root"
require_octessera_boot_overlay "$fixture/boot" "$fixture/root"

reset_fixture
mkdir -p "$fixture/root/boot/firmware/octessera/overlays"
printf '%s\n' '# octessera additions' > "$fixture/root/boot/firmware/config.txt"
printf '%s\n' 'dtbo' > "$fixture/root/boot/firmware/octessera/overlays/i2s-dac-no20.dtbo"
require_octessera_boot_config "$fixture/boot" "$fixture/root"
require_octessera_boot_overlay "$fixture/boot" "$fixture/root"

reset_fixture
if require_octessera_boot_config "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted a missing config marker.' >&2
    exit 1
fi
if require_octessera_boot_overlay "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted a missing overlay.' >&2
    exit 1
fi

printf '%s\n' 'Sanitized image boot layout tests passed'
