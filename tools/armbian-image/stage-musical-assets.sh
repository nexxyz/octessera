#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
staging="${1:-$root/userpatches/overlay/usr/share/octessera}"
default_config="$root/config/generated/pi/default.json"

[[ -f "$default_config" && ! -L "$default_config" ]] || { echo "Missing generated Pi default: $default_config" >&2; exit 1; }

default_output="$staging/defaults/pi-default.json"
manifest_output="$staging/samples/MANIFEST.tsv"
sample_output_root="$staging/samples/files"
rm -f -- "$default_output"
rm -rf -- "$staging/samples"
mkdir -p "$(dirname "$default_output")"
install -m 0644 "$default_config" "$default_output"
python3 "$root/tools/samples/sample_library.py" \
  --repository-root "$root" \
  --media-destination "$sample_output_root" \
  --metadata-destination "$staging/samples" \
  --manifest-destination "$manifest_output"
printf 'Staged canonical Pi default and complete sample library under %s\n' "$staging"
