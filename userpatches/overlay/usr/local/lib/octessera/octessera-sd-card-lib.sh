#!/bin/sh
set -eu

ROOT_OCTESSERA=octessera
SAMPLES_SUBDIR=$ROOT_OCTESSERA/samples
SAVES_SUBDIR=$ROOT_OCTESSERA/saves
LOG=octessera-sd-card

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
  [ "$(lsblk -ndo TYPE "$dev" 2>/dev/null || true)" = part ] || return 1
  root="$(canonical_block "$(root_device)")"
  [ "$dev" != "$root" ] || return 1
  root_pk="$(root_parent_disk)"
  dev_pk="$(parent_disk "$dev")"
  [ -n "$dev_pk" ] || return 1
  [ -z "$root_pk" ] || [ "$dev_pk" != "$root_pk" ] || return 1
}

configured_device() {
  devices="$(blkid -t LABEL=OCTESSERA_SD -o device 2>/dev/null || true)"
  count="$(printf '%s\n' "$devices" | awk 'NF { count++ } END { print count + 0 }')"
  case "$count" in
    0) return 1 ;;
    1) canonical_block "$devices" ;;
    *)
      log "refusing duplicate OCTESSERA_SD labels"
      return 2
      ;;
  esac
}

prepare_folders() {
  mkdir -p "$SD_MOUNT/$SAMPLES_SUBDIR" "$SD_MOUNT/$SAVES_SUBDIR"
  fstype="$(findmnt -nro FSTYPE --mountpoint "$SD_MOUNT" 2>/dev/null || true)"
  case "$fstype" in
    vfat|exfat|fat|msdos)
      chown "$SD_OWNER:$SD_OWNER" "$SD_MOUNT/$ROOT_OCTESSERA" "$SD_MOUNT/$SAMPLES_SUBDIR" "$SD_MOUNT/$SAVES_SUBDIR" 2>/dev/null || true
      ;;
    *)
      chown "$SD_OWNER:$SD_OWNER" "$SD_MOUNT/$ROOT_OCTESSERA" "$SD_MOUNT/$SAMPLES_SUBDIR" "$SD_MOUNT/$SAVES_SUBDIR"
      ;;
  esac
}

transfer_active() { [ -e "$STORAGE_STATE" ]; }

mount_options_for() {
  fstype="$(blkid -o value -s TYPE "$1" 2>/dev/null || true)"
  case "$fstype" in
    vfat|exfat|fat|msdos) printf 'uid=%s,gid=%s,umask=002' "$SD_OWNER" "$SD_OWNER" ;;
    *) printf '' ;;
  esac
}

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
    options="$(mount_options_for "$dev")"
    if [ -n "$options" ]; then
      mount -o "$options" "$dev" "$SD_MOUNT"
    else
      mount "$dev" "$SD_MOUNT"
    fi
  fi
  prepare_folders
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
      if mountpoint -q "$SD_MOUNT"; then
        log "$SD_MOUNT is mounted but no OCTESSERA_SD card is present"
        return 1
      fi
      log "no OCTESSERA_SD card found"
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
    prepare_folders
    return 0
  fi
  remount_card "$dev"
  log "mounted OLED SD card $dev at $SD_MOUNT"
}
