#!/bin/sh
set -eu

ROOT_OCTESSERA=octessera
SAMPLES_SUBDIR=$ROOT_OCTESSERA/samples
SAVES_SUBDIR=$ROOT_OCTESSERA/saves
LOG=octessera-sd-card
SD_DISKSEQ_STATE=${SD_DISKSEQ_STATE:-/run/octessera-sd-card.diskseq}
SD_FSCK_TIMEOUT=${SD_FSCK_TIMEOUT:-60}
SD_REPAIR_STATE=${SD_REPAIR_STATE:-/run/octessera-sd-card.needs-repair}

log() { logger -t "$LOG" "$*"; echo "$LOG: $*" >&2; }
canonical_block() { readlink -f "$1"; }
root_device() { findmnt -nro SOURCE /; }
parent_disk() { lsblk -no PKNAME "$1" 2>/dev/null | head -n 1; }

root_parent_disk() {
  root="$(canonical_block "$(root_device)")"
  [ -b "$root" ] || return 0
  root_type="$(lsblk -ndo TYPE "$root" 2>/dev/null || true)"
  if [ "$root_type" = disk ]; then
    printf '%s\n' "${root##*/}"
  else
    parent_disk "$root"
  fi
}

safe_partition() {
  dev="$(canonical_block "$1")"
  [ -b "$dev" ] || return 1
  root="$(canonical_block "$(root_device)")"
  [ "$dev" != "$root" ] || return 1
  root_pk="$(root_parent_disk)"
  case "$(lsblk -ndo TYPE "$dev" 2>/dev/null || true)" in
    part) dev_pk="$(parent_disk "$dev")" ;;
    disk) on_sd2_slot "$dev" || return 1; dev_pk="${dev##*/}" ;;
    *) return 1 ;;
  esac
  [ -n "$dev_pk" ] || return 1
  [ -z "$root_pk" ] || [ "$dev_pk" != "$root_pk" ] || return 1
}

SD2_SLOT_HOSTS='/sys/devices/platform/soc/5011000.spi/spi_master/spi1/spi1.1/mmc_host/ /sys/devices/platform/soc/3f204000.spi/spi_master/spi0/spi0.1/mmc_host/'

on_sd2_slot() {
  slot_path="$(readlink -f "/sys/class/block/${1##*/}" 2>/dev/null || true)"
  for slot_host in $SD2_SLOT_HOSTS; do
    case "$slot_path" in "$slot_host"*) return 0 ;; esac
  done
  return 1
}

configured_device() {
  probed="$(blkid -o device 2>/dev/null || true)"
  devices=""
  for candidate in $probed; do
    candidate="$(canonical_block "$candidate")"
    on_sd2_slot "$candidate" || continue
    case "$(blkid -o value -s TYPE "$candidate" 2>/dev/null || true)" in
      vfat|exfat) devices="$devices$candidate
" ;;
    esac
  done
  count="$(printf '%s\n' "$devices" | awk 'NF { count++ } END { print count + 0 }')"
  case "$count" in
    0) return 1 ;;
    1) printf '%s' "$devices" ;;
    *)
      log "refusing SD2 card with more than one FAT32/exFAT partition"
      return 2
      ;;
  esac
}

prepare_folders() {
  mkdir -p "$SD_MOUNT/$SAMPLES_SUBDIR" "$SD_MOUNT/$SAVES_SUBDIR"
  chown "$SD_OWNER:$SD_OWNER" "$SD_MOUNT/$ROOT_OCTESSERA" "$SD_MOUNT/$SAMPLES_SUBDIR" "$SD_MOUNT/$SAVES_SUBDIR" 2>/dev/null || true
}

transfer_active() { [ -e "$STORAGE_STATE" ]; }

# The kernel gives every newly inserted card a new disk sequence number, which
# tells a mount of a since-removed card apart from a healthy one.
disk_sequence() {
  disk="$(parent_disk "$1")"
  [ -n "$disk" ] || disk="${1##*/}"
  cat "/sys/class/block/$disk/diskseq" 2>/dev/null || true
}
remember_disk_sequence() { disk_sequence "$1" >"$SD_DISKSEQ_STATE"; }
mounted_card_was_replaced() {
  recorded="$(cat "$SD_DISKSEQ_STATE" 2>/dev/null || true)"
  [ -n "$recorded" ] && [ "$recorded" != "$(disk_sequence "$1")" ]
}

mount_options_for() { printf 'uid=%s,gid=%s,umask=002,nosuid,nodev,noexec' "$SD_OWNER" "$SD_OWNER"; }

refuse_mount_elsewhere() {
  dev="$1"
  mounts="$(findmnt -nr -S "$dev" -o TARGET || true)"
  [ -z "$mounts" ] || while IFS= read -r target; do
    [ -z "$target" ] || [ "$target" = "$SD_MOUNT" ] || {
      log "$dev is mounted at $target, not $SD_MOUNT"
      return 1
    }
  done <<EOF
$mounts
EOF
}

mounted_device() {
  mounted_source="$(findmnt -nro SOURCE --mountpoint "$SD_MOUNT" 2>/dev/null || true)"
  [ -n "$mounted_source" ] || return 1
  canonical_block "$mounted_source"
}

unmount_expected() {
  dev="$1"
  if mountpoint -q "$SD_MOUNT"; then
    mounted_dev="$(mounted_device)" || {
      log "could not identify the SD card mounted at $SD_MOUNT"
      return 1
    }
    [ "$mounted_dev" = "$dev" ] || {
      log "$SD_MOUNT is mounted from $mounted_dev, expected $dev"
      return 1
    }
    umount "$SD_MOUNT"
  fi
}

boot_sector_byte() {
  dd if="$1" bs=512 count=1 iflag=direct 2>/dev/null | od -An -tu1 -j "$2" -N1 | tr -d '[:space:]'
}

card_marked_dirty() {
  case "$(blkid -o value -s TYPE "$1" 2>/dev/null || true)" in
    exfat) flag="$(boot_sector_byte "$1" 106)"; mask=2 ;;
    vfat)
      if [ "$(boot_sector_byte "$1" 22)" = 0 ] && [ "$(boot_sector_byte "$1" 23)" = 0 ]; then
        flag="$(boot_sector_byte "$1" 65)"
      else
        flag="$(boot_sector_byte "$1" 37)"
      fi
      mask=1
      ;;
    *) return 1 ;;
  esac
  if [ -z "$flag" ]; then
    log "could not read the dirty flag of $1; mounting it without a check"
    return 1
  fi
  [ $((flag & mask)) -ne 0 ]
}

repair_dirty_card() {
  dev="$1"
  card_marked_dirty "$dev" || return 0
  case "$(blkid -o value -s TYPE "$dev" 2>/dev/null || true)" in
    exfat) checker="fsck.exfat -p" ;;
    *) checker="fsck.vfat -a" ;;
  esac
  log "$dev was not cleanly removed; checking it with $checker"
  # shellcheck disable=SC2086
  if report="$(timeout "$SD_FSCK_TIMEOUT" $checker "$dev" 2>&1)"; then status=0; else status=$?; fi
  printf '%s\n' "$report" | tail -n 4 | while IFS= read -r line; do
    [ -z "$line" ] || log "$checker: $line"
  done
  case "$status" in
    0) log "$dev checked; nothing to repair"; return 0 ;;
    1) log "$dev checked and repaired"; return 0 ;;
    124) log "$dev check timed out after ${SD_FSCK_TIMEOUT}s; not mounting SD2" ;;
    *) log "$dev check failed with status $status; not mounting SD2" ;;
  esac
  printf '%s\n' "$dev" > "$SD_REPAIR_STATE" || log "could not record that $dev needs repair"
  return 1
}

# A transfer runs under a short service timeout, so a card the computer left
# dirty is handed to the SD card service, which owns the check and its budget.
remount_card_after_transfer() {
  dev="$1"
  if ! mountpoint -q "$SD_MOUNT" && card_marked_dirty "$dev"; then
    log "$dev was left dirty by the transfer; $SD_CARD_SERVICE checks and mounts it"
    systemctl start --no-block "$SD_CARD_SERVICE" || log "could not start $SD_CARD_SERVICE"
    return 0
  fi
  remount_card "$dev"
}

remount_card() {
  dev="$1"
  mkdir -p "$SD_MOUNT"
  if mountpoint -q "$SD_MOUNT"; then
    mounted_dev="$(mounted_device)" || return 1
    [ "$mounted_dev" = "$dev" ] || {
      log "$SD_MOUNT is mounted from $mounted_dev, expected $dev"
      return 1
    }
  else
    repair_dirty_card "$dev" || return 1
    mount -o "$(mount_options_for "$dev")" "$dev" "$SD_MOUNT"
  fi
  rm -f -- "$SD_REPAIR_STATE"
  prepare_folders
  remember_disk_sequence "$dev"
}

mount_card() {
  if transfer_active; then
    log "USB SD2 transfer is active; not mounting OLED SD card"
    return 0
  fi
  mkdir -p "$SD_MOUNT"
  if dev="$(configured_device)"; then
    :
  else
    status=$?
    if [ "$status" = 1 ]; then
      rm -f -- "$SD_REPAIR_STATE"
      if mountpoint -q "$SD_MOUNT"; then
        log "SD2 card was removed; lazily unmounting $SD_MOUNT"
        umount -l "$SD_MOUNT"
        return
      fi
      log "no SD2 card found"
      return 0
    fi
    return "$status"
  fi
  safe_partition "$dev" || { log "refusing unsafe SD candidate $dev"; return 1; }
  refuse_mount_elsewhere "$dev" || return 1
  if mountpoint -q "$SD_MOUNT"; then
    mounted_dev="$(mounted_device)" || return 1
    [ "$mounted_dev" = "$dev" ] || {
      log "$SD_MOUNT is mounted from $mounted_dev, expected $dev"
      return 1
    }
    if mounted_card_was_replaced "$dev"; then
      log "$SD_MOUNT still holds a removed card; remounting"
      umount -l "$SD_MOUNT"
    else
      prepare_folders
      remember_disk_sequence "$dev"
      return 0
    fi
  fi
  remount_card "$dev"
  log "mounted OLED SD card $dev at $SD_MOUNT"
}
