#!/usr/bin/env bash
# The five helper paths are intentionally supplied dynamically by the provisioning adapter.
# shellcheck disable=SC1091
set -euo pipefail

mode=preflight
overlay_source=
validation_helper=
boot_config_helper=
boot_dtb_helper=
common_validation_helper=
environment_helper=
rollback_id=
backup_root=/var/lib/octessera/input-routing-backups
boot_config=/boot/armbianEnv.txt
overlay_dir=/boot/overlay-user
overlay_name=octessera-h618-input-routing
installed_source=/usr/local/share/octessera/device-tree/$overlay_name.dts
installed_dtbo="$overlay_dir/$overlay_name.dtbo"
state_file=/etc/octessera/orange-input-routing.state

usage() {
  printf '%s\n' "Usage: $0 --preflight --overlay-source PATH --validation-helper PATH --boot-config-helper PATH --boot-dtb-helper PATH --common-validation-helper PATH --environment-helper PATH" \
    "       $0 --apply --overlay-source PATH --validation-helper PATH --boot-config-helper PATH --boot-dtb-helper PATH --common-validation-helper PATH --environment-helper PATH" \
    "       $0 --rollback BACKUP_ID"
}

while (($#)); do
  case "$1" in
    --preflight) mode=preflight ;;
    --apply) mode=apply ;;
    --rollback) mode=rollback; shift; rollback_id="${1:-}" ;;
    --overlay-source) shift; overlay_source="${1:-}" ;;
    --validation-helper) shift; validation_helper="${1:-}" ;;
    --boot-config-helper) shift; boot_config_helper="${1:-}" ;;
    --boot-dtb-helper) shift; boot_dtb_helper="${1:-}" ;;
    --common-validation-helper) shift; common_validation_helper="${1:-}" ;;
    --environment-helper) shift; environment_helper="${1:-}" ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
  shift
done

require_root() {
  [[ "$(id -u)" == 0 ]] || { echo "Input-routing provisioning requires root." >&2; exit 1; }
}

require_file() {
  [[ -f "$1" ]] || { echo "Required input-routing file is missing: $1" >&2; exit 1; }
}

require_orange_board() {
  local board
  board="$(awk -F= '$1 == "BOARD" { print $2; exit }' /etc/armbian-release 2>/dev/null || true)"
  [[ "$board" == orangepizero2w ]] || { echo "Input-routing provisioning requires Armbian board orangepizero2w." >&2; exit 1; }
}

load_helpers() {
  require_file "$common_validation_helper"
  require_file "$environment_helper"
  require_file "$validation_helper"
  require_file "$boot_config_helper"
  require_file "$boot_dtb_helper"
  # shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh
  source "$common_validation_helper"
  # shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh
  source "$environment_helper"
  # shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh
  source "$validation_helper"
  # shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh
  source "$boot_config_helper"
  # shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
  source "$boot_dtb_helper"
}

resolve_dtb() {
  octessera_resolve_boot_dtb /
}

validate_overlay_against_dtb() {
  local base_dtb="$1"
  local dt_work="$2"
  local dtbo="$dt_work/$overlay_name.dtbo"
  local merged="$dt_work/$overlay_name-merged.dtb"
  local uart0_path
  local pio_path
  octessera_run_strict_diagnostic "$dt_work" compile_input_routing_overlay dtc -@ -I dts -O dtb -o "$dtbo" "$overlay_source"
  octessera_run_strict_diagnostic "$dt_work" inspect_input_routing_overlay dtc -I dtb -O dts -o "$dt_work/$overlay_name.dts" "$dtbo"
  octessera_run_strict_diagnostic "$dt_work" merge_input_routing_overlay fdtoverlay -i "$base_dtb" -o "$merged" "$dtbo"
  octessera_run_dtc_inspection "$dt_work" inspect_merged_input_routing_overlay dtc -q -I dtb -O dts -o "$dt_work/$overlay_name-merged.dts" "$merged"
  uart0_path="$(fdtget -t s "$base_dtb" /__symbols__ uart0)"
  pio_path="$(fdtget -t s "$base_dtb" /__symbols__ pio)"
  [[ -n "$uart0_path" && -n "$pio_path" ]] || { echo "Exact H618 DTB lacks UART0 or pinctrl symbols." >&2; exit 1; }
  octessera_assert_input_routing_merge "$base_dtb" "$merged" "$uart0_path" "$pio_path" /chosen "Orange Pi"
  cp -f -- "$dtbo" "$dt_work/validated.dtbo"
}

validate_proposed_boot_configuration() {
  local work
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' RETURN
  octessera_remove_uart0_console_args "$boot_config" "$work/armbianEnv.txt"
  octessera_armbian_env_update "$work/armbianEnv.txt" "$work/armbianEnv.with-input-routing.txt" "$overlay_name" i2c1-pi
  octessera_assert_no_uart0_console_args "$work/armbianEnv.with-input-routing.txt"
  require_input_overlay_token "$work/armbianEnv.with-input-routing.txt"
  if [[ -f /boot/extlinux/extlinux.conf ]]; then
    octessera_remove_uart0_console_args /boot/extlinux/extlinux.conf "$work/extlinux.conf"
    octessera_assert_no_uart0_console_args "$work/extlinux.conf"
  fi
  rm -rf "$work"
  trap - RETURN
}

require_input_overlay_token() {
  local config_file="${1:-$boot_config}"
  awk -v token="$overlay_name" '
    /^[[:space:]]*#/ { next }
    /^user_overlays=/ {
      count++
      value = substr($0, length("user_overlays=") + 1)
      split(value, values, /[[:space:]]+/)
      for (position in values) {
        if (values[position] == token) found++
      }
    }
    END { exit(count == 1 && found == 1 ? 0 : 1) }
  ' "$config_file" || {
    echo "Armbian boot configuration does not enable $overlay_name exactly once." >&2
    return 1
  }
}

backup_optional() {
  local source="$1"
  local destination="$2"
  if [[ -e "$source" || -L "$source" ]]; then
    printf '1\n' > "$destination.present"
    cp -a -- "$source" "$destination"
  else
    printf '0\n' > "$destination.present"
  fi
}

restore_optional() {
  local source="$1"
  local destination="$2"
  if [[ "$(cat "$source.present")" == 1 ]]; then
    rm -rf -- "$destination"
    cp -a -- "$source" "$destination"
  else
    rm -rf -- "$destination"
  fi
}

new_backup() {
  local stamp
  local backup_id
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  backup_id="$stamp-$$"
  install -d -m 0750 "$backup_root/$backup_id"
  printf '%s\n' "$backup_id"
}

write_manifest() {
  local backup_dir="$1"
  local backup_id="$2"
  local base_dtb="$3"
  local getty_enabled="$4"
  local getty_active="$5"
  cat > "$backup_dir/manifest.env" <<EOF
schema_version=1
backup_id=$backup_id
board=orangepizero2w
base_dtb=$base_dtb
base_dtb_sha256=$(sha256sum "$base_dtb" | awk '{ print $1 }')
original_armbian_env_sha256=$(sha256sum "$backup_dir/armbianEnv.txt" 2>/dev/null | awk '{ print $1 }' || true)
original_extlinux_sha256=$(sha256sum "$backup_dir/extlinux.conf" 2>/dev/null | awk '{ print $1 }' || true)
input_routing_dts_sha256=$(sha256sum "$overlay_source" | awk '{ print $1 }')
input_routing_dtbo_sha256=$(sha256sum "$backup_dir/validated.dtbo" | awk '{ print $1 }')
serial_getty_enabled=$getty_enabled
serial_getty_active=$getty_active
ssh_touched=0
rollback_command=provision-input-routing.ps1 -RollbackId $backup_id
EOF
}

apply_changes() {
  local base_dtb="$1"
  local dt_work="$2"
  local backup_id
  local backup_dir
  local getty_enabled
  local getty_active
  local environment_tmp
  local environment_with_overlay_tmp
  local extlinux_tmp
  backup_id="$(new_backup)"
  backup_dir="$backup_root/$backup_id"
  backup_optional "$boot_config" "$backup_dir/armbianEnv.txt"
  backup_optional /boot/extlinux/extlinux.conf "$backup_dir/extlinux.conf"
  backup_optional "$installed_source" "$backup_dir/input-routing.dts"
  backup_optional "$installed_dtbo" "$backup_dir/input-routing.dtbo"
  getty_enabled="$(systemctl is-enabled serial-getty@ttyS0.service 2>/dev/null || true)"
  getty_active="$(systemctl is-active serial-getty@ttyS0.service 2>/dev/null || true)"
  printf '%s\n' "$getty_enabled" > "$backup_dir/serial-getty.enabled"
  printf '%s\n' "$getty_active" > "$backup_dir/serial-getty.active"
  cp -f -- "$dt_work/validated.dtbo" "$backup_dir/validated.dtbo"
  write_manifest "$backup_dir" "$backup_id" "$base_dtb" "$getty_enabled" "$getty_active"
  install -d -m 0755 "$overlay_dir" "$(dirname "$installed_source")" /etc/octessera
  install -m 0644 "$overlay_source" "$installed_source.tmp"
  mv -f -- "$installed_source.tmp" "$installed_source"
  install -m 0644 "$dt_work/validated.dtbo" "$installed_dtbo.tmp"
  mv -f -- "$installed_dtbo.tmp" "$installed_dtbo"
  environment_tmp="$(mktemp "${boot_config}.XXXXXX")"
  octessera_remove_uart0_console_args "$boot_config" "$environment_tmp"
  octessera_assert_no_uart0_console_args "$environment_tmp"
  environment_with_overlay_tmp="$(mktemp "${boot_config}.overlay.XXXXXX")"
  octessera_armbian_env_update "$environment_tmp" "$environment_with_overlay_tmp" "$overlay_name" i2c1-pi
  rm -f "$environment_tmp"
  chmod --reference="$boot_config" "$environment_with_overlay_tmp"
  chown --reference="$boot_config" "$environment_with_overlay_tmp"
  require_input_overlay_token "$environment_with_overlay_tmp"
  mv -f -- "$environment_with_overlay_tmp" "$boot_config"
  if [[ -f /boot/extlinux/extlinux.conf ]]; then
    extlinux_tmp="$(mktemp /boot/extlinux/.extlinux.conf.XXXXXX)"
    octessera_remove_uart0_console_args /boot/extlinux/extlinux.conf "$extlinux_tmp"
    octessera_assert_no_uart0_console_args "$extlinux_tmp"
    chmod --reference=/boot/extlinux/extlinux.conf "$extlinux_tmp"
    chown --reference=/boot/extlinux/extlinux.conf "$extlinux_tmp"
    mv -f -- "$extlinux_tmp" /boot/extlinux/extlinux.conf
  fi
  systemctl disable --now serial-getty@ttyS0.service >/dev/null 2>&1 || true
  systemctl mask serial-getty@ttyS0.service >/dev/null
  systemctl daemon-reload
  printf 'backup_id=%s\ninput_routing_enabled=1\nreboot_required=1\nrollback_command=provision-input-routing.ps1 -RollbackId %s\n' "$backup_id" "$backup_id" > "$state_file"
  chmod 0644 "$state_file"
  echo "Input routing staged without reboot. Backup record: $backup_dir/manifest.env"
}

rollback_changes() {
  local backup_dir="$backup_root/$rollback_id"
  local getty_enabled
  local getty_active
  [[ -n "$rollback_id" && -f "$backup_dir/manifest.env" ]] || { echo "Unknown input-routing backup: $rollback_id" >&2; exit 1; }
  restore_optional "$backup_dir/armbianEnv.txt" "$boot_config"
  restore_optional "$backup_dir/extlinux.conf" /boot/extlinux/extlinux.conf
  restore_optional "$backup_dir/input-routing.dts" "$installed_source"
  restore_optional "$backup_dir/input-routing.dtbo" "$installed_dtbo"
  systemctl unmask serial-getty@ttyS0.service >/dev/null 2>&1 || true
  getty_enabled="$(cat "$backup_dir/serial-getty.enabled")"
  getty_active="$(cat "$backup_dir/serial-getty.active")"
  if [[ "$getty_enabled" == enabled ]]; then systemctl enable serial-getty@ttyS0.service >/dev/null; else systemctl disable serial-getty@ttyS0.service >/dev/null 2>&1 || true; fi
  if [[ "$getty_active" == active ]]; then systemctl start serial-getty@ttyS0.service >/dev/null; else systemctl stop serial-getty@ttyS0.service >/dev/null 2>&1 || true; fi
  systemctl daemon-reload
  rm -f "$state_file"
  printf 'rollback_of=%s\nrolled_back_at=%s\n' "$rollback_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$backup_dir/rollback-record.env"
  echo "Input routing rollback recorded at $backup_dir/rollback-record.env; reboot is still required to restore the running DT."
}

require_root
if [[ "$mode" == rollback ]]; then
  require_orange_board
  rollback_changes
  exit 0
fi
[[ "$mode" == preflight || "$mode" == apply ]] || { usage >&2; exit 2; }
require_orange_board
[[ -f "$boot_config" ]] || { echo "Missing Armbian boot configuration: $boot_config" >&2; exit 1; }
load_helpers
require_file "$overlay_source"
base_dtb="$(resolve_dtb)"
dt_work="$(mktemp -d)"
trap 'rm -rf "$dt_work"' EXIT
validate_overlay_against_dtb "$base_dtb" "$dt_work"
if [[ "$mode" == preflight ]]; then
  validate_proposed_boot_configuration
  echo "Orange input-routing preflight passed against $base_dtb"
  exit 0
fi
apply_changes "$base_dtb" "$dt_work"
