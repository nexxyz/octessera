#!/usr/bin/env bash
set -euo pipefail

expected_image_mode=diagnostic
mode_selected=false
verification_profile=""
image_dir=""
usage() {
  echo "Usage: $0 --verification-profile full-constructor|legacy-runtime-only|legacy-setup-layer [--mode diagnostic|production] <armbian-output-images-dir>" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --verification-profile)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      [[ -z "$verification_profile" ]] || { echo "verification profile selected more than once." >&2; usage; exit 2; }
      verification_profile="$2"
      shift 2
      ;;
    --mode)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      [[ "$mode_selected" == false ]] || { echo "image mode selected more than once." >&2; usage; exit 2; }
      expected_image_mode="$2"
      mode_selected=true
      shift 2
      ;;
    --*)
      usage
      exit 2
      ;;
    *)
      [[ -z "$image_dir" ]] || { usage; exit 2; }
      image_dir="$1"
      shift
      ;;
  esac
done

case "$verification_profile" in
  full-constructor|legacy-runtime-only|legacy-setup-layer)
    ;;
  "")
    echo "--verification-profile is required." >&2
    usage
    exit 2
    ;;
  *)
    echo "Invalid verification profile: $verification_profile." >&2
    usage
    exit 2
    ;;
esac

if [[ "$expected_image_mode" != diagnostic && "$expected_image_mode" != production ]] || [[ -z "$image_dir" ]]; then
  usage
  exit 2
fi

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
  bash "$root/tools/armbian-image/inspect-built-image.sh" --verification-profile "$verification_profile" --mode "$expected_image_mode" "$rootfs"
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
