#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
helper="$root/tools/storage/octessera-sd-card"
library="$root/tools/storage/octessera-sd-card-lib.sh"
service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-sd-card.service"
rule="$root/userpatches/overlay/etc/udev/rules.d/99-octessera-orange-sd-card.rules"
pi_service="$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-sd-card.service"
pi_rule="$root/tools/pi-image/stage4-octessera/files/root/etc/udev/rules.d/99-octessera-sd-card.rules"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

grep -qF "SD_MOUNT=\${OCTESSERA_SD_MOUNT:?OCTESSERA_SD_MOUNT must be set}" "$helper"
grep -qF "SD_OWNER=\${OCTESSERA_SD_OWNER:?OCTESSERA_SD_OWNER must be set}" "$helper"
grep -qFx '. /usr/local/lib/octessera/octessera-sd-card-lib.sh' "$helper"
test -f "$library"
if grep -qE '/dev/disk/by-label|mmcblk' "$helper"; then
  echo 'SD helper contains a device-index or by-label fallback.' >&2
  exit 1
fi
grep -qF "[ \"\$dev\" != \"\$root\" ]" "$library"
grep -qF "[ -z \"\$root_pk\" ] || [ \"\$dev_pk\" != \"\$root_pk\" ]" "$library"
grep -qFx 'Environment=OCTESSERA_SD_MOUNT=/var/lib/octessera/samples/sd-card' "$service"
grep -qFx 'Environment=OCTESSERA_SD_OWNER=octessera-runtime' "$service"
grep -qFx 'ExecStart=/usr/local/sbin/octessera-sd-card mount' "$service"
grep -qFx 'ACTION=="add|change", SUBSYSTEM=="block", ENV{DEVTYPE}=="partition", ENV{ID_FS_LABEL}=="OCTESSERA_SD", TAG+="systemd", ENV{SYSTEMD_WANTS}+="octessera-orange-sd-card.service"' "$rule"
grep -qFx 'Environment=OCTESSERA_SD_MOUNT=/home/pi/samples/sd-card' "$pi_service"
grep -qFx 'Environment=OCTESSERA_SD_OWNER=pi' "$pi_service"
grep -qFx 'ExecStart=/usr/local/sbin/octessera-sd-card mount' "$pi_service"
grep -qFx 'ACTION=="add|change", SUBSYSTEM=="block", ENV{DEVTYPE}=="partition", ENV{ID_FS_LABEL}=="OCTESSERA_SD", TAG+="systemd", ENV{SYSTEMD_WANTS}+="octessera-sd-card.service"' "$pi_rule"

export OCTESSERA_SD_MOUNT="$work/mount"
export OCTESSERA_SD_OWNER=octessera-runtime
SD_MOUNT="$OCTESSERA_SD_MOUNT"
SD_OWNER="$OCTESSERA_SD_OWNER"
STORAGE_STATE="$work/storage.state"
: "$SD_MOUNT" "$SD_OWNER" "$STORAGE_STATE"
# shellcheck source=tools/storage/octessera-sd-card-lib.sh
source "$library"

canonical_block() { printf '%s\n' "$1"; }
transfer_active() { return 1; }
with_sd_lock() { "$@"; }
logger() { :; }

blkid() {
  case "$*" in
    *'LABEL=OCTESSERA_SD'*) printf '%s\n' /dev/sdb1 ;;
    *'-s TYPE'*) printf '%s\n' vfat ;;
  esac
}
test "$(configured_device)" = /dev/sdb1
test "$(mount_options_for /dev/sdb1)" = uid=octessera-runtime,gid=octessera-runtime,umask=002

blkid() {
  case "$*" in
    *'LABEL=OCTESSERA_SD'*) printf '%s\n' /dev/sdb1 /dev/sdc1 ;;
  esac
}
if configured_device; then
  echo 'SD helper accepted duplicate OCTESSERA_SD labels.' >&2
  exit 1
else
  test "$?" = 2
fi

findmnt() {
  case "$*" in
    *'-S /dev/sdb1'*) return 0 ;;
    *'FSTYPE'*) printf '%s\n' vfat ;;
  esac
}
if refuse_mount_elsewhere /dev/sdb1; then :; else exit 1; fi
findmnt() {
  case "$*" in
    *'-S /dev/sdb1'*) printf '%s\n' /media/elsewhere ;;
  esac
}
if refuse_mount_elsewhere /dev/sdb1; then
  echo 'SD helper accepted a card mounted elsewhere.' >&2
  exit 1
else
  test "$?" = 1
fi

mount_marker="$work/mounted"
mountpoint() { test "${1:-}" = -q && test -e "$mount_marker"; }
mount() { touch "$mount_marker"; }
safe_partition() { return 0; }
findmnt() {
  case "$*" in
    *'-S /dev/sdb1'*) return 0 ;;
    *'FSTYPE'*) printf '%s\n' vfat ;;
  esac
}
blkid() {
  case "$*" in
    *'LABEL=OCTESSERA_SD'*) printf '%s\n' /dev/sdb1 ;;
    *'-s TYPE'*) printf '%s\n' vfat ;;
  esac
}
mount_card
test -e "$mount_marker"
test -d "$OCTESSERA_SD_MOUNT/octessera/samples"
test -d "$OCTESSERA_SD_MOUNT/octessera/saves"

echo 'Octessera shared SD helper, service, udev, safety, label, mount, and folder tests passed.'
