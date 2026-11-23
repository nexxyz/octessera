#!/usr/bin/env bash

octessera_armbian_usb_quirks_call=$'\t\tadd_usb_storage_quirks &'
octessera_armbian_usb_quirks_disabled=$'\t\t: octessera keeps /boot/armbianEnv.txt unchanged at boot, add_usb_storage_quirks disabled'

octessera_armbian_usb_quirks_rewrite_disabled() {
  local script="$1"
  [[ -f "$script" && ! -L "$script" ]] || { echo "Missing Armbian hardware optimization script: $script" >&2; return 1; }
  [[ "$(grep -cxF "$octessera_armbian_usb_quirks_disabled" "$script")" == 1 ]] || { echo "Armbian USB storage quirks boot rewrite is not disabled in $script." >&2; return 1; }
  ! grep -qxF "$octessera_armbian_usb_quirks_call" "$script" || { echo "Armbian USB storage quirks boot rewrite still runs in $script." >&2; return 1; }
  [[ "$(grep -cxF 'add_usb_storage_quirks() {' "$script")" == 1 ]] || { echo "Armbian hardware optimization script lost add_usb_storage_quirks: $script" >&2; return 1; }
}

octessera_disable_armbian_usb_quirks_rewrite() {
  local script="$1"
  local script_tmp
  [[ -f "$script" && ! -L "$script" ]] || { echo "Missing Armbian hardware optimization script: $script" >&2; return 1; }
  if [[ "$(grep -cxF 'add_usb_storage_quirks() {' "$script")" != 1 \
      || "$(grep -cxF "$octessera_armbian_usb_quirks_call" "$script")" != 1 \
      || "$(grep -cE '^[^#]*add_usb_storage_quirks' "$script")" != 2 ]]; then
    echo "Armbian hardware optimization script changed shape; review its add_usb_storage_quirks step before building: $script" >&2
    return 1
  fi
  script_tmp="$(mktemp "$script.XXXXXX")" || return 1
  awk -v call="$octessera_armbian_usb_quirks_call" -v disabled="$octessera_armbian_usb_quirks_disabled" \
    '$0 == call { print disabled; next } { print }' "$script" > "$script_tmp" || { rm -f "$script_tmp"; return 1; }
  if ! chmod --reference="$script" "$script_tmp" || ! chown --reference="$script" "$script_tmp"; then rm -f "$script_tmp"; return 1; fi
  octessera_armbian_usb_quirks_rewrite_disabled "$script_tmp" || { rm -f "$script_tmp"; return 1; }
  bash -n "$script_tmp" || { rm -f "$script_tmp"; echo "Disabling add_usb_storage_quirks broke $script." >&2; return 1; }
  mv -f -- "$script_tmp" "$script"
}

octessera_armbian_usb_quirks_default='0x2537:0x1066:u,0x2537:0x1068:u'

octessera_write_armbian_usb_quirks() {
  local env_file="$1"
  local env_tmp
  [[ -f "$env_file" && ! -L "$env_file" ]] || { echo "Missing Armbian boot configuration: $env_file" >&2; return 1; }
  if grep -q '^usbstoragequirks=' "$env_file"; then
    [[ "$(grep -c '^usbstoragequirks=' "$env_file")" == 1 ]] || { echo "Armbian boot configuration repeats usbstoragequirks: $env_file" >&2; return 1; }
    return 0
  fi
  env_tmp="$(mktemp "$env_file.usb-quirks.XXXXXX")" || return 1
  { awk '{ print }' "$env_file" && printf 'usbstoragequirks=%s\n' "$octessera_armbian_usb_quirks_default"; } > "$env_tmp" || { rm -f "$env_tmp"; return 1; }
  if ! chmod --reference="$env_file" "$env_tmp" || ! chown --reference="$env_file" "$env_tmp"; then rm -f "$env_tmp"; return 1; fi
  sync "$env_tmp" && mv -f -- "$env_tmp" "$env_file"
}
