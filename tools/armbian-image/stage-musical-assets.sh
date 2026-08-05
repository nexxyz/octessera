#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
staging="${1:-$root/userpatches/overlay/usr/share/octessera}"
default_config="$root/config/generated/pi/default.json"
sample_root="$root/samples"
license_source="https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE"

[[ -f "$default_config" && ! -L "$default_config" ]] || { echo "Missing generated Pi default: $default_config" >&2; exit 1; }
[[ -d "$sample_root" && ! -L "$sample_root" ]] || { echo "Missing sample library: $sample_root" >&2; exit 1; }

sample_paths_text="$(python3 - "$default_config" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    payload = json.load(stream)

paths = set()
for instrument in payload["runtimeConfig"]["instruments"]:
    for slot in instrument["sample"]["slots"]:
        path = slot.get("path")
        if path:
            paths.add(path.removeprefix("samples/"))
for path in sorted(paths):
    print(path)
PY
)"
if [[ -n "$sample_paths_text" ]]; then
  mapfile -t sample_paths <<< "$sample_paths_text"
else
  sample_paths=()
fi

default_output="$staging/defaults/pi-default.json"
manifest_output="$staging/samples/sample-manifest.tsv"
sample_output_root="$staging/samples/files"
for relative_path in "${sample_paths[@]}"; do
  case "$relative_path" in
    ''|/*|*..*|*\\*|*$'\t'*|*$'\r'*) echo "Invalid referenced sample path: $relative_path" >&2; exit 1 ;;
  esac
  source_path="$sample_root/$relative_path"
  [[ -f "$source_path" && ! -L "$source_path" ]] || { echo "Missing referenced regular sample: $source_path" >&2; exit 1; }
done
rm -f -- "$default_output"
rm -rf -- "$staging/samples"
mkdir -p "$(dirname "$default_output")" "$sample_output_root"
install -m 0644 "$default_config" "$default_output"
{
  printf '# path\tsize\tsha256\tsource\tlicense_source\n'
  for relative_path in "${sample_paths[@]}"; do
    source_path="$sample_root/$relative_path"
    destination_path="$sample_output_root/$relative_path"
    case "$relative_path" in
      "Drum/claps/distkit-clap.wav") source_url="https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/fugue-state-audio/drums/claps/distkit-clap.wav" ;;
      "Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav") source_url="https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/freesound/drums/cymbal/open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav" ;;
      "Drum/kick/Kick2.wav") source_url="https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/microlag/One-Shots/Drums/Kick2.wav" ;;
      *) echo "Missing pinned sample provenance: $relative_path" >&2; exit 1 ;;
    esac
    mkdir -p "$(dirname "$destination_path")"
    install -m 0644 "$source_path" "$destination_path"
    size="$(stat -c '%s' "$source_path")"
    sha256="$(sha256sum "$source_path" | awk '{ print $1 }')"
    printf '%s\t%s\t%s\t%s\t%s\n' "$relative_path" "$size" "$sha256" "$source_url" "$license_source"
  done
} > "$manifest_output"
printf 'Staged canonical Pi default and %s referenced samples under %s\n' "${#sample_paths[@]}" "$staging"
