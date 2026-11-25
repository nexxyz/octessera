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
grep -qFx 'ACTION=="remove", SUBSYSTEM=="block", ENV{ID_PATH}=="platform-5011000.spi-cs-1", RUN+="/bin/systemctl --no-block start octessera-orange-sd-card.service"' "$rule"
grep -qFx 'ACTION=="remove", SUBSYSTEM=="block", ENV{ID_PATH}=="platform-3f204000.spi-cs-1", RUN+="/bin/systemctl --no-block start octessera-sd-card.service"' "$pi_rule"
grep -qF ':-/run/octessera-sd-card.needs-repair}' "$library"
grep -qF ':-/run/octessera-sd-card.repaired}' "$library"
grep -qF 'const REPAIR_MARKER_PATH: &str = "/run/octessera-sd-card.repaired";' "$root/apps/pi-zero/src/sd_card_watch.rs"

export OCTESSERA_SD_MOUNT="$work/mount"
export OCTESSERA_SD_OWNER=octessera-runtime
SD_MOUNT="$OCTESSERA_SD_MOUNT"
SD_OWNER="$OCTESSERA_SD_OWNER"
STORAGE_STATE="$work/storage.state"
SD_REPAIR_STATE="$work/needs-repair"
SD_REPAIRED_STATE="$work/repaired"
: "$SD_MOUNT" "$SD_OWNER" "$STORAGE_STATE" "$SD_REPAIR_STATE" "$SD_REPAIRED_STATE"
# shellcheck source=tools/storage/octessera-sd-card-lib.sh
source "$library"

canonical_block() { printf '%s\n' "$1"; }
transfer_active() { return 1; }
with_sd_lock() { "$@"; }
logger() { :; }
boot_sector_byte() { printf '0\n'; }
readlink() {
  case "$2" in
    /sys/class/block/sdb1|/sys/class/block/sdc1) printf '%s\n' "/sys/devices/platform/soc/5011000.spi/spi_master/spi1/spi1.1/mmc_host/mmc2/mmc2:0000/block/sdb/${2##*/}" ;;
    /sys/class/block/sde1) printf '%s\n' "/sys/devices/platform/soc/3f204000.spi/spi_master/spi0/spi0.1/mmc_host/mmc2/mmc2:0000/block/sde/sde1" ;;
    /sys/class/block/sdb|/sys/class/block/loop0) printf '%s\n' "/sys/devices/platform/soc/5011000.spi/spi_master/spi1/spi1.1/mmc_host/mmc2/mmc2:0000/block/${2##*/}" ;;
    /sys/class/block/loop1) printf '%s\n' "/sys/devices/platform/soc/5200000.usb/usb1/1-1/1-1:1.0/host0/target0:0:0/0:0:0:0/block/loop1" ;;
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
test "$(mount_options_for /dev/sdb1)" = ro,uid=octessera-runtime,gid=octessera-runtime,umask=002,nosuid,nodev,noexec

root_device() { printf '%s\n' /dev/mmcblk0p2; }
root_parent_disk() { printf '%s\n' mmcblk0; }
lsblk() {
  case "$*" in
    *'TYPE /dev/loop0'*|*'TYPE /dev/loop1'*) printf '%s\n' disk ;;
    *'TYPE /dev/loop2'*) printf '%s\n' part ;;
    *'PKNAME /dev/loop2'*) printf '%s\n' loop0 ;;
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
disk_sequence() { printf '%s\n' "$card_sequence"; }
if mounted_card_was_replaced /dev/sdb1; then
  echo 'SD helper treated an unrecorded mount as a removed card.' >&2
  exit 1
fi
mount_card
test -e "$mount_marker"
test -d "$OCTESSERA_SD_MOUNT/octessera/samples"
test ! -e "$OCTESSERA_SD_MOUNT/octessera/saves"
test "$(cat "$SD_DISKSEQ_STATE")" = 7

lazy_unmounts=0
mounted_device() { printf '%s\n' /dev/sdb1; }
umount() { test "$1" = -l && lazy_unmounts=$((lazy_unmounts + 1)) && rm -f "$mount_marker"; }
mount_card
test "$lazy_unmounts" = 0
card_sequence=8
mount_card
test "$lazy_unmounts" = 1
test -e "$mount_marker"
test "$(cat "$SD_DISKSEQ_STATE")" = 8

probe '' ''
printf '%s\n' /dev/sdb1 > "$SD_REPAIR_STATE"
printf '%s\n' '/dev/sdb1 8' > "$SD_REPAIRED_STATE"
mount_card
test "$lazy_unmounts" = 2
test ! -e "$mount_marker"
test ! -e "$SD_REPAIR_STATE"
test ! -e "$SD_REPAIRED_STATE"
mount_card
test "$lazy_unmounts" = 2

touch "$mount_marker"
transfer_active() { return 0; }
mount_card
test "$lazy_unmounts" = 2
test -e "$mount_marker"
transfer_active() { return 1; }
rm -f "$mount_marker"
probe /dev/sdb1 /dev/sdb1=vfat

fsck_bin="$work/fsck-bin"
mkdir -p "$fsck_bin"
for checker in fsck.vfat fsck.exfat; do
  cat > "$fsck_bin/$checker" <<EOF
#!/bin/sh
printf '%s %s\n' "$checker" "\$*" >> "$work/fsck.log"
[ "\${FAKE_FSCK_IGNORE_TERM:-0}" = 1 ] && trap '' TERM
sleep "\${FAKE_FSCK_SLEEP:-0}"
echo "Automatically removing dirty bit."
[ "\${FAKE_FSCK_KEEP_DIRTY:-0}" = 1 ] || printf '0\n' > "$work/sector"
exit "\${FAKE_FSCK_STATUS:-1}"
EOF
  chmod +x "$fsck_bin/$checker"
done
PATH="$fsck_bin:$PATH"
fat16_size=0
boot_sector_byte() {
  case "$2" in
    22|23) printf '%s\n' "$fat16_size" ;;
    *) cat "$work/sector" ;;
  esac
}
blkid() {
  case "$*" in
    *'-s TYPE'*) printf '%s\n' "$card_type" ;;
  esac
}
mount() {
  printf '%s\n' "$*" >> "$work/mount.log"
  case "$*" in
    *remount,ro*) [ "${FAKE_REMOUNT_RO_FAIL:-0}" = 0 ] || return 1 ;;
    *remount*) ;;
    *) touch "$mount_marker" ;;
  esac
  [ "${FAKE_MOUNT_FAIL:-0}" = 0 ] || { rm -f "$mount_marker"; return 32; }
}
fresh_mount() {
  rm -rf "$mount_marker" "$work/fsck.log" "$work/mount.log" "$OCTESSERA_SD_MOUNT/octessera"
  card_type="$1"
  printf '%s\n' "$2" > "$work/sector"
}

rm -f "$SD_REPAIRED_STATE"
fresh_mount vfat 0
remount_card /dev/sdb1
test -e "$mount_marker"
test ! -e "$work/fsck.log"
test ! -e "$SD_REPAIRED_STATE"
test "$(head -n 1 "$work/mount.log")" = "-o ro,uid=octessera-runtime,gid=octessera-runtime,umask=002,nosuid,nodev,noexec /dev/sdb1 $OCTESSERA_SD_MOUNT"
test "$(sed -n 2,3p "$work/mount.log")" = "-o remount,rw $OCTESSERA_SD_MOUNT
-o remount,ro $OCTESSERA_SD_MOUNT"
test -d "$OCTESSERA_SD_MOUNT/octessera/samples"
rm -f "$mount_marker" "$work/mount.log"
remount_card /dev/sdb1
test "$(wc -l < "$work/mount.log")" = 1

fresh_mount vfat 1
remount_card /dev/sdb1
test -e "$mount_marker"
grep -qx 'fsck.vfat -a /dev/sdb1' "$work/fsck.log"
grep -q "^/dev/sdb1 $card_sequence [0-9][0-9]*$" "$SD_REPAIRED_STATE"
first_repair="$(cat "$SD_REPAIRED_STATE")"
fresh_mount vfat 1
remount_card /dev/sdb1
[ "$(cat "$SD_REPAIRED_STATE")" != "$first_repair" ] || { echo 'A second repair of the same card left the same repair record.' >&2; exit 1; }

fat16_size=32
fresh_mount vfat 1
remount_card /dev/sdb1
grep -qx 'fsck.vfat -a /dev/sdb1' "$work/fsck.log"
fresh_mount vfat 0
remount_card /dev/sdb1
test ! -e "$work/fsck.log"
fat16_size=0

fresh_mount exfat 2
remount_card /dev/sdb1
test -e "$mount_marker"
grep -qx 'fsck.exfat -p /dev/sdb1' "$work/fsck.log"

fresh_mount exfat 1
remount_card /dev/sdb1
test ! -e "$work/fsck.log"

rm -f "$SD_REPAIRED_STATE"
fresh_mount vfat 1
export FAKE_FSCK_STATUS=4
if remount_card /dev/sdb1; then
  echo 'SD helper mounted a card its check could not repair.' >&2
  exit 1
fi
test ! -e "$mount_marker"
grep -qx /dev/sdb1 "$SD_REPAIR_STATE"
test ! -e "$SD_REPAIRED_STATE"
unset FAKE_FSCK_STATUS
fresh_mount vfat 1
remount_card /dev/sdb1
test ! -e "$SD_REPAIR_STATE"

fresh_mount vfat 1
export FAKE_FSCK_KEEP_DIRTY=1
if remount_card /dev/sdb1 2>"$work/still-dirty.log"; then
  echo 'SD helper counted a check that left the card dirty as a repair.' >&2
  exit 1
fi
grep -qF 'still marked dirty' "$work/still-dirty.log"
test ! -e "$mount_marker"
unset FAKE_FSCK_KEEP_DIRTY

fresh_mount exfat 2
export FAKE_FSCK_SLEEP=5
SD_FSCK_TIMEOUT=1
if remount_card /dev/sdb1; then
  echo 'SD helper mounted a card whose check timed out.' >&2
  exit 1
fi
test ! -e "$mount_marker"

fresh_mount exfat 2
export FAKE_FSCK_IGNORE_TERM=1 FAKE_FSCK_SLEEP=30
SD_FSCK_KILL_AFTER=1
started="$(date +%s)"
if remount_card /dev/sdb1; then
  echo 'SD helper mounted a card whose check ignored its timeout.' >&2
  exit 1
fi
test $(( $(date +%s) - started )) -lt 10
test ! -e "$mount_marker"
unset FAKE_FSCK_IGNORE_TERM FAKE_FSCK_SLEEP
SD_FSCK_TIMEOUT=60

fresh_mount vfat 0
export FAKE_REMOUNT_RO_FAIL=1
if remount_card /dev/sdb1 2>"$work/remount-ro.log"; then
  echo 'SD helper kept SD2 mounted after it could not return it to read-only.' >&2
  exit 1
fi
grep -qF 'could not return' "$work/remount-ro.log"
test ! -e "$mount_marker"
unset FAKE_REMOUNT_RO_FAIL

fresh_mount vfat 0
export FAKE_MOUNT_FAIL=1
if remount_card /dev/sdb1; then
  echo 'SD helper reported a failed mount as mounted.' >&2
  exit 1
fi
test ! -e "$OCTESSERA_SD_MOUNT/octessera"
if prepare_folders; then
  echo 'SD helper created SD2 folders on an empty mount point.' >&2
  exit 1
fi
test ! -e "$OCTESSERA_SD_MOUNT/octessera"
unset FAKE_MOUNT_FAIL

rm -f "$mount_marker"
if remount_card_after_transfer /dev/octessera-test-pulled-card 2>"$work/pulled.log"; then :; else
  echo 'SD helper failed a transfer stop after the card was pulled.' >&2
  exit 1
fi
grep -qF 'removed during the transfer' "$work/pulled.log"
test ! -e "$mount_marker"

echo 'Octessera shared SD helper, service, udev, safety, SD2-slot, whole-disk, filesystem, USB-drive, two-partition, dirty-card check, read-only mount, failed mount, pulled-transfer, mount, removed-card, replaced-card, and folder tests passed.'
