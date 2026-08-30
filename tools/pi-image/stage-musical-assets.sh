#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stage_root="${1:-$root/tools/pi-image/stage4-octessera/files/root}"

python3 "$root/tools/samples/sample_library.py" \
  --repository-root "$root" \
  --media-destination "$stage_root/home/pi/samples" \
  --metadata-destination "$stage_root/usr/share/octessera/samples" \
  --manifest-destination "$stage_root/usr/share/octessera/samples/MANIFEST.tsv"
mkdir -p "$stage_root/home/pi/samples/sd-card"
printf 'Staged complete Raspberry sample library under %s/home/pi/samples\n' "$stage_root"
