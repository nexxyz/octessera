#!/bin/bash
set -euo pipefail

octessera_remove_raspberry_parent_sudoers() {
    local root="$1"
    local path="$root/etc/sudoers.d/010_pi-nopasswd"
    local expected='pi ALL=(ALL) NOPASSWD: ALL'
    if [ ! -f "$path" ] || [ -L "$path" ]; then
        echo "Raspberry parent sudoers grant is missing or not a regular file: $path" >&2
        return 1
    fi
    [ "$(stat -c '%u:%g:%a' "$path")" = 0:0:440 ] || { echo "Raspberry parent sudoers grant has unexpected ownership or mode: $path" >&2; return 1; }
    printf '%s\n' "$expected" | cmp -s - "$path" || { echo "Raspberry parent sudoers grant content is not the pinned broad Pi rule: $path" >&2; return 1; }
    rm -f -- "$path"
}
