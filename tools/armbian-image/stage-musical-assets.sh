#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
staging="${1:-$root/userpatches/overlay/usr/share/octessera}"
default_config="$root/config/generated/pi/default.json"
projection="${2:-$root/target/save-documents/pi}"
system_source="$projection/system.json"
patch_source="$projection/default.patch.json"

[[ -f "$default_config" && ! -L "$default_config" ]] || { echo "Missing generated Pi default: $default_config" >&2; exit 1; }
[[ -f "$system_source" && ! -L "$system_source" ]] || { echo "Missing native Pi System projection: $system_source" >&2; exit 1; }
[[ -f "$patch_source" && ! -L "$patch_source" ]] || { echo "Missing native Pi Patch projection: $patch_source" >&2; exit 1; }

default_output="$staging/defaults/pi-default.json"
system_output="$staging/defaults/pi-system.json"
patch_output="$staging/defaults/pi-default.patch.json"
manifest_output="$staging/samples/MANIFEST.tsv"
sample_output_root="$staging/samples/files"
rm -f -- "$default_output" "$system_output" "$patch_output"
rm -rf -- "$staging/samples"
mkdir -p "$(dirname "$system_output")"
install -m 0644 "$default_config" "$default_output"
install -m 0644 "$system_source" "$system_output"
install -m 0644 "$patch_source" "$patch_output"
python3 "$root/tools/samples/sample_library.py" \
  --repository-root "$root" \
  --media-destination "$sample_output_root" \
  --metadata-destination "$staging/samples" \
  --manifest-destination "$manifest_output"
printf 'Staged Pi build metadata input, System/Patch documents, and complete sample library under %s\n' "$staging"
