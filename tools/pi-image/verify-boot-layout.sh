#!/bin/bash

require_octessera_boot_config() {
    local boot_root="$1"
    local image_root="$2"
    if grep -q 'octessera additions' "$boot_root/config.txt" 2>/dev/null ||
        grep -q 'octessera additions' "$image_root/boot/firmware/config.txt" 2>/dev/null; then
        return
    fi
    echo "Sanitation check failed: missing octessera boot config marker" >&2
    return 1
}

require_octessera_boot_overlay() {
    local boot_root="$1"
    local image_root="$2"
    local relative=overlays/i2s-dac-no20.dtbo
    local path
    for path in \
        "$boot_root/octessera/$relative" \
        "$image_root/boot/firmware/octessera/$relative" \
        "$boot_root/$relative" \
        "$image_root/boot/firmware/$relative"; do
        if [ -f "$path" ]; then
            return
        fi
    done
    echo "Sanitation check failed: missing i2s-dac-no20 boot overlay" >&2
    return 1
}
