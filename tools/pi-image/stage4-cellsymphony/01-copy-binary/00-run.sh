#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
STAGE_FILES="$(cd "$SCRIPT_DIR/.." && pwd)/files"

install -D -m 0755 \
    "$STAGE_FILES/root/usr/local/bin/cellsymphony-pi" \
    "$ROOTFS_DIR/usr/local/bin/cellsymphony-pi"
