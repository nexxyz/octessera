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

test -f "$library"
if grep -qE '/dev/disk/by-label|mmcblk' "$helper"; then
  echo 'SD helper contains a device-index or by-label fallback.' >&2
  exit 1
fi
grep -qF "[ \"\$dev\" != \"\$root\" ]" "$library"
grep -qF "[ -z \"\$root_pk\" ] || [ \"\$dev_pk\" != \"\$root_pk\" ]" "$library"
grep -qFx 'Environment=OCTESSERA_SD_OWNER=octessera-runtime' "$service"
grep -qFx 'ACTION=="add|change", SUBSYSTEM=="block", ENV{ID_PATH}=="platform-5011000.spi-cs-1", ENV{ID_FS_TYPE}=="vfat|exfat", TAG+="systemd", ENV{SYSTEMD_WANTS}+="octessera-orange-sd-card.service"' "$rule"
grep -qFx 'Environment=OCTESSERA_SD_OWNER=pi' "$pi_service"
grep -qFx 'ACTION=="add|change", SUBSYSTEM=="block", ENV{ID_PATH}=="platform-3f204000.spi-cs-1", ENV{ID_FS_TYPE}=="vfat|exfat", TAG+="systemd", ENV{SYSTEMD_WANTS}+="octessera-sd-card.service"' "$pi_rule"

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
readlink() {
  case "$2" in
    /sys/class/block/sdb1|/sys/class/block/sdc1) printf '%s\n' "/sys/devices/platform/soc/5011000.spi/spi_master/spi1/spi1.1/mmc_host/mmc2/mmc2:0000/block/sdb/${2##*/}" ;;
    /sys/class/block/sde1) printf '%s\n' "/sys/devices/platform/soc/3f204000.spi/spi_master/spi0/spi0.1/mmc_host/mmc2/mmc2:0000/block/sde/sde1" ;;
    /sys/class/block/sdb|/sys/class/block/loop0) printf '%s
' "/sys/devices/platform/soc/5011000.spi/spi_master/spi1/spi1.1/mmc_host/mmc2/mmc2:0000/block/${2##*/}" ;;
    /sys/class/block/loop1) printf '%s
' "/sys/devices/platform/soc/5200000.usb/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/loop1" ;;
    /sys/class/block/sdd1) printf '%s\n' "/sys/devices/platform/soc/5200000.usb/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/sdd/sdd1" ;;
    *) command readlink "$@" ;;
  esac
}

probe_devices=""
probe_types=""
probe() { probe_devices="$1"; probe_types="$2"; }
blkid() {
  case "$*" in
    '-o device') [ -z "$probe_devices" ] || printf '%s\n' "${probe_devices// /$'\n'}" ;;
    *'-s TYPE'*)
      local args="$*"
      for entry in $probe_types; do
        [ "${entry%%=*}" = "${args##* }" ] && printf '%s\n' "${entry#*=}"
      done
      return 0
      ;;
  esac
}
expect_no_sd2() {
  local expected="$1" message="$2"
  if configured_device; then
    echo "$message" >&2
    exit 1
  else
    test "$?" = "$expected"
  fi
}

probe /dev/sdb1 /dev/sdb1=vfat
test "$(configured_device)" = /dev/sdb1
probe /dev/sde1 /dev/sde1=exfat
test "$(configured_device)" = /dev/sde1
probe /dev/sdd1 /dev/sdd1=vfat
expect_no_sd2 1 'SD helper took a USB FAT drive as SD2.'
probe '/dev/sdd1 /dev/sdb1' '/dev/sdd1=vfat /dev/sdb1=vfat'
test "$(configured_device)" = /dev/sdb1
probe /dev/sdb1 /dev/sdb1=ext4
expect_no_sd2 1 'SD helper took a card without a FAT32/exFAT partition as SD2.'
probe '/dev/sdb1 /dev/sdc1' '/dev/sdb1=vfat /dev/sdc1=ext4'
test "$(configured_device)" = /dev/sdb1
probe '/dev/sdb1 /dev/sdc1' '/dev/sdb1=vfat /dev/sdc1=exfat'
expect_no_sd2 2 'SD helper picked one of two FAT32/exFAT partitions.'
probe '' ''
expect_no_sd2 1 'SD helper found an SD2 card in an empty slot.'

probe /dev/sdb /dev/sdb=vfat
test "$(configured_device)" = /dev/sdb
test "$(mount_options_for /dev/sdb1)" = uid=octessera-runtime,gid=octessera-runtime,umask=002,nosuid,nodev,noexec

root_device() { printf '%s
' /dev/mmcblk0p2; }
root_parent_disk() { printf '%s
' mmcblk0; }
lsblk() {
  case "$*" in
    *'TYPE /dev/loop0'*|*'TYPE /dev/loop1'*) printf '%s
' disk ;;
    *'TYPE /dev/loop2'*) printf '%s
' part ;;
    *'PKNAME /dev/loop2'*) printf '%s
' loop0 ;;
  esac
}
safe_partition /dev/loop0 || { echo 'SD helper refused a whole-disk FAT card in the SD2 slot.' >&2; exit 1; }
if safe_partition /dev/loop1; then
  echo 'SD helper accepted a whole disk outside the SD2 slot.' >&2
  exit 1
fi
safe_partition /dev/loop2 || { echo 'SD helper refused an SD2 partition.' >&2; exit 1; }
unset -f root_device root_parent_disk lsblk

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
probe /dev/sdb1 /dev/sdb1=vfat
SD_DISKSEQ_STATE="$work/diskseq"
card_sequence=7
disk_sequence() { printf '%s
' "$card_sequence"; }
if mounted_card_was_replaced /dev/sdb1; then
  echo 'SD helper treated an unrecorded mount as a removed card.' >&2
  exit 1
fi
mount_card
test -e "$mount_marker"
test -d "$OCTESSERA_SD_MOUNT/octessera/samples"
test -d "$OCTESSERA_SD_MOUNT/octessera/saves"
test "$(cat "$SD_DISKSEQ_STATE")" = 7

lazy_unmounts=0
mounted_device() { printf '%s
' /dev/sdb1; }
umount() { test "$1" = -l && lazy_unmounts=$((lazy_unmounts + 1)) && rm -f "$mount_marker"; }
mount_card
test "$lazy_unmounts" = 0
card_sequence=8
mount_card
test "$lazy_unmounts" = 1
test -e "$mount_marker"
test "$(cat "$SD_DISKSEQ_STATE")" = 8

echo 'Octessera shared SD helper, service, udev, safety, SD2-slot, whole-disk, filesystem, USB-drive, two-partition, mount, replaced-card, and folder tests passed.'
