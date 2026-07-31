#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
STAGE_FILES="$STAGE_DIR/files"
ARTIFACT_DIR="$ROOTFS_DIR/var/lib/octessera/rpi-kernel"

: "${ROOTFS_DIR:?pi-gen did not provide ROOTFS_DIR}"
: "${OCTESSERA_KERNEL_PACKAGE:?set OCTESSERA_KERNEL_PACKAGE to the validated arm64 linux-image .deb}"
: "${OCTESSERA_KERNEL_CHECKSUMS:?set OCTESSERA_KERNEL_CHECKSUMS to its exact SHA256SUMS file}"
: "${OCTESSERA_KERNEL_PROVENANCE:?set OCTESSERA_KERNEL_PROVENANCE to the exact package provenance JSON}"

for input in "$OCTESSERA_KERNEL_PACKAGE" "$OCTESSERA_KERNEL_CHECKSUMS" "$OCTESSERA_KERNEL_PROVENANCE"; do
    if [ ! -f "$input" ]; then
        echo "Raspberry kernel stage input is missing: $input" >&2
        exit 2
    fi
done

install -d -m 0755 "$ARTIFACT_DIR" "$ROOTFS_DIR/usr/local/lib/octessera"
package_name="$(basename "$OCTESSERA_KERNEL_PACKAGE")"
install -m 0644 "$OCTESSERA_KERNEL_PACKAGE" "$ARTIFACT_DIR/$package_name"
install -m 0644 "$OCTESSERA_KERNEL_CHECKSUMS" "$ARTIFACT_DIR/SHA256SUMS"
install -m 0644 "$OCTESSERA_KERNEL_PROVENANCE" "$ARTIFACT_DIR/provenance.json"

for helper in install-rpi-kernel.py rpi_kernel_contract.py rpi_kernel_image.py; do
    install -m 0644 "$STAGE_FILES/root/usr/local/lib/octessera/$helper" \
        "$ROOTFS_DIR/usr/local/lib/octessera/$helper"
done
install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/sbin/octessera-finalize-rpi-kernel" \
    "$ROOTFS_DIR/usr/local/sbin/octessera-finalize-rpi-kernel"

python3 "$STAGE_FILES/root/usr/local/lib/octessera/install-rpi-kernel.py" \
    --rootfs "$ROOTFS_DIR" \
    --package "$ARTIFACT_DIR/$package_name" \
    --checksums "$ARTIFACT_DIR/SHA256SUMS" \
    --provenance "$ARTIFACT_DIR/provenance.json"
