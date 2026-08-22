#!/bin/bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/pi-image/validate-rpi-parent-sudoers.sh
source "$root/tools/pi-image/validate-rpi-parent-sudoers.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/etc/sudoers.d"

printf '%s\n' 'pi ALL=(ALL) NOPASSWD: ALL' > "$work/etc/sudoers.d/010_pi-nopasswd"
chmod 0440 "$work/etc/sudoers.d/010_pi-nopasswd"
chown 0:0 "$work/etc/sudoers.d/010_pi-nopasswd"
octessera_remove_raspberry_parent_sudoers "$work"
[ ! -e "$work/etc/sudoers.d/010_pi-nopasswd" ]

printf '%s\n' 'pi ALL=(ALL) NOPASSWD: /bin/true' > "$work/etc/sudoers.d/010_pi-nopasswd"
chmod 0440 "$work/etc/sudoers.d/010_pi-nopasswd"
chown 0:0 "$work/etc/sudoers.d/010_pi-nopasswd"
if octessera_remove_raspberry_parent_sudoers "$work"; then
    echo 'Unexpected Raspberry parent sudoers content was accepted.' >&2
    exit 1
fi
[ -f "$work/etc/sudoers.d/010_pi-nopasswd" ]

rm -f "$work/etc/sudoers.d/010_pi-nopasswd"
ln -s /etc/sudoers.d/other "$work/etc/sudoers.d/010_pi-nopasswd"
if octessera_remove_raspberry_parent_sudoers "$work"; then
    echo 'Symlinked Raspberry parent sudoers grant was accepted.' >&2
    exit 1
fi

printf '%s\n' 'Raspberry parent sudoers constructor tests passed'
