#!/usr/bin/env bash
set -euo pipefail

stage_root="${1:?stage root is required}"
rootfs_root="${2:?rootfs root is required}"
source_media="$stage_root/home/pi/samples"
source_metadata="$stage_root/usr/share/octessera/samples"
target_media="$rootfs_root/home/pi/samples"
target_metadata="$rootfs_root/usr/share/octessera/samples"

for path in "$source_media" "$source_metadata"; do
  [[ -d "$path" && ! -L "$path" ]] || { echo "Missing staged Raspberry sample directory: $path" >&2; exit 1; }
done
for relative in sample-manifest.tsv ATTRIBUTIONS.tsv upstream/LICENSE upstream/README.txt; do
  path="$source_metadata/$relative"
  [[ -f "$path" && ! -L "$path" ]] || { echo "Missing staged Raspberry sample metadata: $relative" >&2; exit 1; }
done
if find -P "$source_media" -mindepth 1 \( -type l -o -type p -o -type s -o -type c -o -type b \) -print -quit | grep -q .; then
  echo "Staged Raspberry sample tree contains an unsafe entry" >&2
  exit 1
fi

rm -rf -- "$target_media" "$target_metadata"
install -d -m 0755 "$target_media" "$target_metadata"
while IFS= read -r -d '' path; do
  install -D -m 0644 "$path" "$target_media/${path#"$source_media/"}"
done < <(find -P "$source_media" -mindepth 1 -type f -print0)
install -d -m 0755 "$target_media/sd-card"

install -D -m 0644 "$source_metadata/sample-manifest.tsv" "$target_metadata/sample-manifest.tsv"
install -D -m 0644 "$source_metadata/ATTRIBUTIONS.tsv" "$target_metadata/ATTRIBUTIONS.tsv"
install -D -m 0644 "$source_metadata/upstream/LICENSE" "$target_metadata/upstream/LICENSE"
install -D -m 0644 "$source_metadata/upstream/README.txt" "$target_metadata/upstream/README.txt"
