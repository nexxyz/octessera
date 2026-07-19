#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <armbian-output-images-dir>" >&2
  exit 2
fi

image_dir="$1"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp_dirs=()

cleanup() {
  rm -rf "${tmp_dirs[@]}"
}
trap cleanup EXIT

[[ -d "$image_dir" ]] || { echo "Missing image output directory: $image_dir" >&2; exit 1; }

inspect_disk_image() {
  local image="$1"
  local work="$2"
  local partition
  local start
  local sectors
  local rootfs

  partition="$(fdisk -l "$image" | awk '$1 ~ /[0-9]+$/ { for (i = 2; i <= NF - 2; i++) if ($i ~ /^[0-9]+$/ && $(i + 1) ~ /^[0-9]+$/ && $(i + 2) ~ /^[0-9]+$/) { print $i " " $(i + 2); exit } }')"
  [[ -n "$partition" ]] || { echo "Could not locate Linux root partition in $image" >&2; exit 1; }
  read -r start sectors <<<"$partition"
  [[ "$start" =~ ^[0-9]+$ && "$sectors" =~ ^[0-9]+$ ]] || { echo "Invalid partition geometry for $image: $partition" >&2; exit 1; }

  rootfs="$work/rootfs.ext4"
  dd if="$image" of="$rootfs" bs=512 skip="$start" count="$sectors" status=none
  bash "$root/tools/armbian-image/inspect-built-image.sh" "$rootfs"
}

found=0
while IFS= read -r -d '' artifact; do
  found=1
  work="$(mktemp -d)"
  tmp_dirs+=("$work")
  case "$artifact" in
    *.img)
      inspect_disk_image "$artifact" "$work"
      ;;
    *.img.xz)
      xz -dc "$artifact" >"$work/image.img"
      inspect_disk_image "$work/image.img" "$work"
      ;;
  esac
  rm -rf "$work"
done < <(find "$image_dir" -maxdepth 1 \( -name '*.img' -o -name '*.img.xz' \) -print0)

[[ "$found" -eq 1 ]] || { echo "No Armbian .img or .img.xz artifacts found under $image_dir" >&2; exit 1; }
