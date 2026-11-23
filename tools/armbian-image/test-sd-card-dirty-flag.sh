#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
for tool in mkfs.vfat mkfs.exfat fsck.vfat fsck.exfat blkid; do
  command -v "$tool" >/dev/null 2>&1 || { echo "The SD2 dirty-flag test needs $tool (dosfstools, exfatprogs, util-linux)." >&2; exit 1; }
done

work="$(mktemp -d "${TMPDIR:-/tmp}/sd2-dirty.XXXXXX")"
trap 'rm -rf "$work"' EXIT
SD_MOUNT="$work/mount"
SD_OWNER=root
STORAGE_STATE="$work/storage.state"
SD_REPAIR_STATE="$work/needs-repair"
: "$SD_MOUNT" "$SD_OWNER" "$STORAGE_STATE" "$SD_REPAIR_STATE"
# shellcheck source=tools/storage/octessera-sd-card-lib.sh
source "$root/tools/storage/octessera-sd-card-lib.sh"
logger() { :; }

set_byte() { printf '%b' "\\0$(printf '%03o' "$3")" | dd of="$1" bs=1 seek="$2" conv=notrunc status=none; }

check_image() {
  local name="$1" offset="$2" value="$3" image="$work/$1.img"
  shift 3
  truncate -s 64M "$image"
  "$@" "$image" >/dev/null 2>&1
  if card_marked_dirty "$image"; then
    echo "A freshly formatted $name image counted as dirty." >&2
    exit 1
  fi
  set_byte "$image" "$offset" "$value"
  card_marked_dirty "$image" || { echo "A $name image with its dirty flag set at byte $offset was not detected." >&2; exit 1; }
  repair_dirty_card "$image" 2>"$work/$name.log" || { cat "$work/$name.log" >&2; echo "The $name check failed." >&2; exit 1; }
  if card_marked_dirty "$image"; then
    echo "The $name check left the dirty flag set." >&2
    cat "$work/$name.log" >&2
    exit 1
  fi
  printf 'SD2 %s dirty flag detected and cleared\n' "$name"
}

check_image FAT16 37 1 mkfs.vfat -F 16
check_image FAT32 65 1 mkfs.vfat -F 32
check_image exFAT 106 2 mkfs.exfat

printf 'SD2 dirty-flag tests on real FAT16, FAT32 and exFAT images passed\n'
