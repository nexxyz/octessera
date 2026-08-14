#!/usr/bin/env bash
set -euo pipefail

expected_image_mode=diagnostic
setup_layer_required=false
if [[ "${1:-}" == --setup-layer ]]; then
  setup_layer_required=true
  shift
fi
if [[ "${1:-}" == --mode ]]; then
  expected_image_mode="${2:-}"
  shift 2
fi
if [[ "$expected_image_mode" != diagnostic && "$expected_image_mode" != production ]]; then
  echo "Usage: $0 [--setup-layer] [--mode diagnostic|production] <rootfs-dir-or-ext4-image>" >&2
  exit 2
fi
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 [--setup-layer] [--mode diagnostic|production] <rootfs-dir-or-ext4-image>" >&2
  exit 2
fi
target="$1"
# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$(dirname "${BASH_SOURCE[0]}")/inspect-mode.sh"
# shellcheck source=tools/armbian-image/authorized-key-paths.sh
source "$(dirname "${BASH_SOURCE[0]}")/authorized-key-paths.sh"
# shellcheck source=tools/armbian-image/inspect-path.sh
source "$(dirname "${BASH_SOURCE[0]}")/inspect-path.sh"
# shellcheck disable=SC1091
source "$(dirname "${BASH_SOURCE[0]}")/setup-layer-proof.sh"
inspect_work="$(mktemp -d)"

cleanup() {
  rm -rf "$inspect_work"
}
trap cleanup EXIT
read_file() {
  local path="$1"
  local request
  local error_path="$inspect_work/debugfs-read.stderr"
  local error_content
  local content
  local status
  octessera_debugfs_path_argument "$path" >/dev/null || return 2
  if [[ -d "$target" ]]; then
    if cat -- "$target/$path"; then
      return
    fi
    echo "Unable to read image path: $path." >&2
    return 2
  fi
  request="$(octessera_debugfs_cat_request "$path")" || return 2
  if content="$(debugfs -R "$request" "$target" 2>"$error_path")"; then
    status=0
  else
    status=$?
  fi
  error_content="$(cat -- "$error_path")"
  if [[ "$status" != 0 ]] || ! octessera_debugfs_stderr_is_startup_banner "$error_content" || printf '%s\n' "$content" | grep -Eq '(^|:) File not found by ext2_lookup[[:space:]]*$|^cat:'; then
    [[ -z "$error_content" ]] || printf '%s\n' "$error_content" >&2
    echo "Unable to read image path: $path." >&2
    return 2
  fi
  printf '%s' "$content"
}
stat_path() {
  octessera_stat_path "$target" "$1"
}
require_root_mode() {
  local path="$1"
  local mode="$2"
  local expected_mode
  local actual_mode
  octessera_debugfs_path_argument "$path" >/dev/null || {
    echo "Unsafe image path: $path." >&2
    exit 1
  }
  case "$mode" in
    [0-7][0-7][0-7]) expected_mode="0$mode" ;;
    [0-7][0-7][0-7][0-7]) expected_mode="$mode" ;;
    *) echo "Invalid expected mode for $path." >&2; exit 1 ;;
  esac
  if [[ -d "$target" ]]; then
    actual_mode="$(stat -c '%a' "$target/$path")"
    [[ "${#actual_mode}" == 3 ]] && actual_mode="0$actual_mode"
    [[ "$(stat -c '%u:%g' "$target/$path")" == 0:0 && "$actual_mode" == "$expected_mode" ]] || {
      echo "Unsafe updater ownership/mode at $path." >&2
      exit 1
    }
    return
  fi
  local metadata
  local metadata_status
  if metadata="$(octessera_debugfs_stat_metadata "$target" "$path")"; then
    metadata_status=0
  else
    metadata_status=$?
  fi
  if [[ "$metadata_status" != 0 ]]; then
    echo "Unable to inspect image path: $path." >&2
    exit 1
  fi
  printf '%s\n' "$metadata" | grep -Eq 'User: +0 +Group: +0' || {
    echo "Unsafe image ownership at $path." >&2
    exit 1
  }
  if ! actual_mode="$(octessera_debugfs_mode "$metadata")"; then
    echo "Missing image mode at $path." >&2
    exit 1
  fi
  [[ "$actual_mode" == "$expected_mode" ]] || {
    echo "Unsafe image mode at $path." >&2
    exit 1
  }
}
hash_path() {
  local path="$1"
  local dump_path
  local request
  local error_path="$inspect_work/debugfs-dump.stderr"
  local error_content
  local status
  octessera_debugfs_path_argument "$path" >/dev/null || {
    echo "Unsafe image path: $path." >&2
    exit 1
  }
  dump_path="$inspect_work/$(basename "$path")"
  if [[ -d "$target" ]]; then
    sha256sum "$target/$path" | awk '{ print $1 }'
    return
  fi
  rm -f "$dump_path"
  request="$(octessera_debugfs_dump_request "$path" "$dump_path")" || {
    echo "Unable to read image path: $path." >&2
    exit 1
  }
  if debugfs -R "$request" "$target" >/dev/null 2>"$error_path"; then
    status=0
  else
    status=$?
  fi
  error_content="$(cat -- "$error_path")"
  if [[ "$status" != 0 ]] || ! octessera_debugfs_stderr_is_startup_banner "$error_content"; then
    [[ -z "$error_content" ]] || printf '%s\n' "$error_content" >&2
    echo "Unable to read image path: $path." >&2
    exit 1
  fi
  [[ -s "$dump_path" ]] || {
    echo "Unable to read image path: $path." >&2
    exit 1
  }
  sha256sum "$dump_path" | awk '{ print $1 }'
}
# shellcheck source=tools/armbian-image/inspect-runtime.sh
source "$(dirname "${BASH_SOURCE[0]}")/inspect-runtime.sh"
validate_env_tokens() {
  local content="$1"
  local key="$2"
  local required_token="$3"
  printf '%s\n' "$content" | awk -v key="$key" -v required_token="$required_token" '
    function invalid(message) {
      print "Invalid " key " assignment: " message > "/dev/stderr"
      failed = 1
    }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) {
        if (line ~ ("(^|[^_[:alnum:]])" key "[[:space:]]*=")) {
          invalid("commented assignment")
        }
        next
      }
      if (line ~ ("^" key "=")) {
        if (assignments++) {
          invalid("duplicate assignment")
        }
        value = substr(line, length(key) + 2)
        if (value ~ /#/) {
          invalid("comments are not allowed")
        }
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        count = value == "" ? 0 : split(value, values, /[[:space:]]+/)
        for (position = 1; position <= count; position++) {
          token = values[position]
          if (token !~ /^[A-Za-z0-9][A-Za-z0-9_.-]*$/) {
            invalid("invalid token")
          }
          if (seen[token]++) {
            invalid("duplicate token")
          }
          if (token == required_token) {
            found++
          }
        }
        next
      }
      if (line ~ (("(^|[^_[:alnum:]])" key "[[:space:]]*="))) {
        invalid("malformed assignment")
      }
    }
    END {
      if (!assignments) {
        invalid("missing assignment")
      }
      if (found != 1) {
        invalid("required token must occur exactly once")
      }
      exit(failed ? 1 : 0)
    }
  '
}
unit_masked() {
  octessera_unit_masked_path "$target" "$1"
}
require_octessera_account() {
  local passwd_content="$1"
  local account_result
  account_result="$(printf '%s\n' "$passwd_content" | awk -F: '
    $1 == "octessera" { count++; if (NF != 7 || $3 !~ /^[0-9]+$/ || $6 != "/home/octessera" || $7 != "/bin/bash") invalid = 1 }
    END { if (count != 1) print "missing"; else if (invalid) print "unexpected" }
  ')"
  case "$account_result" in
    missing) echo "The image is missing the expected octessera account." >&2; return 1 ;;
    unexpected) echo "The image has an unexpected octessera account." >&2; return 1 ;;
  esac
}
reject_authorized_keys() {
  local passwd_content
  local login_defs_content
  local uid_min
  local key_path
  local stat_status
  local key_paths=(root/.ssh/authorized_keys etc/ssh/authorized_keys etc/dropbear/authorized_keys)
  local derived_key_paths
  if passwd_content="$(read_file etc/passwd)"; then
    :
  else
    echo "Unable to read required image path: etc/passwd." >&2
    exit 1
  fi
  if login_defs_content="$(read_file etc/login.defs)"; then
    :
  else
    echo "Unable to read required image path: etc/login.defs." >&2
    exit 1
  fi
  uid_min="$(octessera_uid_min "$login_defs_content")"
  require_octessera_account "$passwd_content" || exit 1
  key_paths+=(home/octessera/.ssh/authorized_keys)
  derived_key_paths="$(octessera_derive_account_authorized_key_paths "$passwd_content" "$uid_min")" || {
    echo "Built-image inspection cannot authorize an unsupported account home." >&2
    exit 1
  }
  while IFS= read -r key_path; do
    [[ -n "$key_path" ]] || continue
    [[ "$key_path" == home/octessera/.ssh/authorized_keys ]] && continue
    key_paths+=("$key_path")
  done <<< "$derived_key_paths"
  for key_path in "${key_paths[@]}"; do
    if stat_path "$key_path"; then
      echo "Built image must not contain baked authorized keys: $key_path." >&2
      exit 1
    else
      stat_status=$?
      [[ "$stat_status" == 1 ]] || {
        echo "Unable to inspect image path: $key_path." >&2
        exit 1
      }
    fi
  done
}
reject_path() {
  local path="$1"
  local stat_status
  if stat_path "$path"; then
    echo "Diagnostic-only Orange image must not contain runtime path: $path." >&2
    exit 1
  else
    stat_status=$?
    [[ "$stat_status" == 1 ]] || {
      echo "Unable to inspect image path: $path." >&2
      exit 1
    }
  fi
}
require_wifi_foundation() {
  local helper_path=usr/local/sbin/octessera-wifi-foundation
  local unit_path=etc/systemd/system/octessera-wifi-foundation.service
  local binary_path=usr/local/bin/wifi-connect
  for path in "$helper_path" "$unit_path" "$binary_path"; do
    stat_path "$path" || { echo "Missing inactive Wi-Fi foundation path: $path." >&2; exit 1; }
  done
  require_root_mode "$helper_path" 755
  require_root_mode "$unit_path" 644
  require_root_mode "$binary_path" 755
  helper_content="$(read_file "$helper_path")"
  unit_content="$(read_file "$unit_path")"
  printf '%s\n' "$helper_content" | grep -qF -- '--portal-interface wlan0' || { echo "Wi-Fi foundation must fix wlan0." >&2; exit 1; }
  printf '%s\n' "$helper_content" | grep -qF -- '--portal-gateway 192.168.42.1' || { echo "Wi-Fi foundation must fix its gateway." >&2; exit 1; }
  printf '%s\n' "$helper_content" | grep -qF -- '900s' || { echo "Wi-Fi foundation must be bounded." >&2; exit 1; }
  printf '%s\n' "$unit_content" | grep -qFx 'User=root' || { echo "Wi-Fi foundation unit must run as root." >&2; exit 1; }
  printf '%s\n' "$unit_content" | grep -qFx 'Group=root' || { echo "Wi-Fi foundation unit must run as root." >&2; exit 1; }
  printf '%s\n' "$unit_content" | grep -qFx 'ExecStart=/usr/local/sbin/octessera-wifi-foundation' || { echo "Wi-Fi foundation unit has the wrong helper." >&2; exit 1; }
  printf '%s\n' "$unit_content" | grep -qFx 'TimeoutStartSec=905s' || { echo "Wi-Fi foundation unit must be bounded." >&2; exit 1; }
  if printf '%s\n' "$helper_content" "$unit_content" | grep -Eiq 'sidecar|hostname|ssh|password|country|setup[-_ ]?(complete|force)|credential|secret|/sys/class/net|iw[[:space:]]+dev|nmcli.*device|mac|wpa_passphrase|chpasswd|ssid=|psk=|BEGIN (RSA|OPENSSH|PRIVATE) KEY'; then
    echo "Wi-Fi foundation contains forbidden behavior or secret handling." >&2
    exit 1
  fi
}
if shadow="$(read_file etc/shadow)"; then
  :
else
  echo "Unable to read required image path: etc/shadow." >&2
  exit 1
fi
shadow_record="$(printf '%s\n' "$shadow" | awk -F: '$1 == "octessera" { count++; hash = $2 } END { print count "\t" hash }')"
IFS=$'\t' read -r shadow_account_count hash <<< "$shadow_record"
[[ "$shadow_account_count" == 1 ]] || { echo "The image is missing the expected octessera shadow account." >&2; exit 1; }
case "$hash" in
  ""|\!*|\**|x) ;;
  *) echo "Octessera user has a usable baked password hash." >&2; exit 1 ;;
esac
if [[ -d "$target" ]]; then
  if find "$target/etc/ssh" -maxdepth 1 -name 'ssh_host_*' | grep -q .; then
    echo "Built image must not contain baked SSH host keys." >&2
    exit 1
  fi
else
  ssh_listing_request="$(octessera_debugfs_ls_request etc/ssh)" || {
    echo "Unable to inspect image path: etc/ssh." >&2
    exit 1
  }
  ssh_listing_error="$inspect_work/debugfs-ssh-list.stderr"
  if ssh_listing="$(debugfs -R "$ssh_listing_request" "$target" 2>"$ssh_listing_error")"; then
    ssh_listing_status=0
  else
    ssh_listing_status=$?
  fi
  ssh_listing_error_content="$(cat -- "$ssh_listing_error")"
  if [[ "$ssh_listing_status" != 0 ]] || ! octessera_debugfs_stderr_is_startup_banner "$ssh_listing_error_content" || printf '%s\n' "$ssh_listing" | grep -Eq '(^ls:|File not found by ext2_lookup)'; then
    [[ -z "$ssh_listing_error_content" ]] || printf '%s\n' "$ssh_listing_error_content" >&2
    echo "Unable to inspect image path: etc/ssh." >&2
    exit 1
  fi
  if printf '%s\n' "$ssh_listing" | grep -q 'ssh_host_'; then
    echo "Built image must not contain baked SSH host keys." >&2
    exit 1
  fi
fi
reject_authorized_keys
ssh_config="$(read_file etc/ssh/sshd_config.d/10-octessera-setup.conf)"
printf '%s\n' "$ssh_config" | grep -q '^PermitRootLogin no$' || { echo "Missing PermitRootLogin no." >&2; exit 1; }
printf '%s\n' "$ssh_config" | grep -q '^PasswordAuthentication no$' || { echo "Missing default PasswordAuthentication no." >&2; exit 1; }
printf '%s\n' "$ssh_config" | grep -q '^AllowUsers octessera$' || { echo "Missing AllowUsers octessera." >&2; exit 1; }
profile_metadata="$(read_file etc/octessera/build-metadata.env)"
printf '%s\n' "$profile_metadata" | grep -q '^OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w$' || {
  echo "Armbian image must be labeled orange-pi-zero-2w." >&2
  exit 1
}
default_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_PI_DEFAULT_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
samples_manifest_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SAMPLES_MANIFEST_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
[[ -n "$default_hash" && -n "$samples_manifest_hash" ]] || { echo "Armbian image is missing musical asset hashes." >&2; exit 1; }
reject_path etc/systemd/system/multi-user.target.wants/octessera-wifi-foundation.service
require_wifi_foundation
if [[ "$setup_layer_required" == true ]]; then
  require_setup_layer
fi
octessera_inspect_runtime_mode "$profile_metadata" "$expected_image_mode"

for path in \
  usr/local/sbin/octessera-orange-usb-gadget \
  usr/local/lib/octessera/device_config.py \
  usr/local/sbin/octessera-device-apply-reboot \
  usr/local/sbin/octessera-orange-oled-logo \
  usr/local/sbin/octessera-orange-oled-handoff.py \
  usr/local/sbin/octessera-orange-oled-lifecycle.py \
  usr/local/sbin/octessera-orange-oled-suspend \
  usr/local/sbin/octessera-provision-musical-default \
  etc/modules-load.d/octessera-orange-midi.conf \
  etc/modules-load.d/octessera-orange-usb-gadget.conf \
  etc/systemd/system/octessera-orange-usb-gadget.service \
  etc/systemd/system/octessera-device-apply-reboot.socket \
  etc/systemd/system/octessera-device-apply-reboot@.service \
  etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket \
  etc/systemd/system/octessera-orange-boot-splash.service \
  etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service \
  etc/systemd/system/octessera-orange-oled-shutdown.service \
  etc/systemd/system/octessera-orange-oled-suspend.service \
  etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service \
  etc/systemd/system/octessera-provision-musical-default.service \
  etc/initramfs-tools/hooks/octessera-orange-boot-splash \
  etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash \
  usr/share/octessera/oled/octessera-mark.svg \
  usr/share/octessera/oled/octessera-wordmark.svg \
  usr/share/octessera/defaults/pi-default.json \
  usr/share/octessera/samples/sample-manifest.tsv; do
  stat_path "$path" || { echo "Missing Orange OS parity path: $path." >&2; exit 1; }
done
require_root_mode usr/local/sbin/octessera-orange-usb-gadget 755
require_root_mode usr/local/lib/octessera/device_config.py 644
require_root_mode usr/local/sbin/octessera-device-apply-reboot 755
require_root_mode usr/local/sbin/octessera-orange-oled-logo 755
require_root_mode usr/local/sbin/octessera-orange-oled-handoff.py 644
require_root_mode usr/local/sbin/octessera-orange-oled-lifecycle.py 644
require_root_mode usr/local/sbin/octessera-orange-oled-suspend 755
require_root_mode usr/local/sbin/octessera-provision-musical-default 755
require_root_mode etc/modules-load.d/octessera-orange-midi.conf 644
require_root_mode etc/modules-load.d/octessera-orange-usb-gadget.conf 644
require_root_mode etc/systemd/system/octessera-orange-usb-gadget.service 644
require_root_mode etc/systemd/system/octessera-device-apply-reboot.socket 644
require_root_mode etc/systemd/system/octessera-device-apply-reboot@.service 644
octessera_require_image_symlink etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket ../octessera-device-apply-reboot.socket /etc/systemd/system/octessera-device-apply-reboot.socket
require_root_mode etc/systemd/system/octessera-orange-boot-splash.service 644
octessera_require_image_symlink etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service ../octessera-orange-boot-splash.service /etc/systemd/system/octessera-orange-boot-splash.service
require_root_mode etc/systemd/system/octessera-orange-oled-shutdown.service 644
require_root_mode etc/systemd/system/octessera-orange-oled-suspend.service 644
octessera_require_image_symlink etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service ../octessera-orange-oled-suspend.service /etc/systemd/system/octessera-orange-oled-suspend.service
reject_path etc/systemd/system/sleep.target.wants/octessera-orange-oled-suspend.service
require_root_mode etc/systemd/system/octessera-provision-musical-default.service 644
require_root_mode etc/initramfs-tools/hooks/octessera-orange-boot-splash 755
require_root_mode etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash 755
reject_path lib/systemd/system-sleep/octessera-orange-oled
reject_path usr/lib/systemd/system-sleep/octessera-orange-oled
require_root_mode usr/share/octessera/defaults/pi-default.json 644
require_root_mode usr/share/octessera/samples/sample-manifest.tsv 644
[[ "$(hash_path usr/share/octessera/defaults/pi-default.json)" == "$default_hash" ]] || { echo "Pi default hash mismatch." >&2; exit 1; }
[[ "$(hash_path usr/share/octessera/samples/sample-manifest.tsv)" == "$samples_manifest_hash" ]] || { echo "Sample manifest hash mismatch." >&2; exit 1; }
manifest_content="$(read_file usr/share/octessera/samples/sample-manifest.tsv)"
octessera_validate_sample_tree "$target" "$manifest_content" "$inspect_work" || exit 1

gadget_unit="$(read_file etc/systemd/system/octessera-orange-usb-gadget.service)"
alsa_modules="$(read_file etc/modules-load.d/octessera-orange-midi.conf)"
[[ "$alsa_modules" == $'snd_seq\nsnd_seq_midi' ]] || { echo "Orange image has an unexpected ALSA module-load list." >&2; exit 1; }
printf '%s\n' "$gadget_unit" | grep -q 'ExecStart=/usr/local/sbin/octessera-orange-usb-gadget setup' || { echo "Orange gadget service setup is missing." >&2; exit 1; }
printf '%s\n' "$gadget_unit" | grep -q 'ExecStop=/usr/local/sbin/octessera-orange-usb-gadget teardown' || { echo "Orange gadget service teardown is missing." >&2; exit 1; }
printf '%s\n' "$(read_file usr/local/sbin/octessera-orange-usb-gadget)" | grep -q 'musb-hdrc.4.auto' || { echo "Orange gadget does not fail closed on the verified UDC." >&2; exit 1; }
if printf '%s\n' "$(read_file usr/local/sbin/octessera-orange-usb-gadget)" | grep -Eq 'dwc2|BCM[0-9]|/home/pi|config\.txt'; then
  echo "Orange OS parity path contains Raspberry Pi assumptions." >&2
  exit 1
fi

spi_source_path=usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts
spi_dtbo_path=boot/overlay-user/octessera-h618-spi1-cs0.dtbo
input_routing_source_path=usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts
input_routing_dtbo_path=boot/overlay-user/octessera-h618-input-routing.dtbo
armbian_env_path=boot/armbianEnv.txt
for path in "$spi_source_path" "$spi_dtbo_path" "$input_routing_source_path" "$input_routing_dtbo_path" "$armbian_env_path"; do
  stat_path "$path" || { echo "Missing Orange Pi SPI image path: $path." >&2; exit 1; }
done
require_root_mode "$spi_source_path" 644
require_root_mode "$spi_dtbo_path" 644
require_root_mode "$input_routing_source_path" 644
require_root_mode "$input_routing_dtbo_path" 644
require_root_mode "$armbian_env_path" 644
source_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SPI1_CS0_DTS_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
dtbo_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SPI1_CS0_DTBO_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
[[ -n "$source_hash" && -n "$dtbo_hash" ]] || { echo "Armbian image is missing SPI overlay hashes." >&2; exit 1; }
[[ "$(hash_path "$spi_source_path")" == "$source_hash" ]] || { echo "SPI overlay source hash mismatch." >&2; exit 1; }
[[ "$(hash_path "$spi_dtbo_path")" == "$dtbo_hash" ]] || { echo "SPI overlay DTBO hash mismatch." >&2; exit 1; }
input_routing_source_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_INPUT_ROUTING_DTS_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
input_routing_dtbo_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_INPUT_ROUTING_DTBO_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
[[ -n "$input_routing_source_hash" && -n "$input_routing_dtbo_hash" ]] || { echo "Armbian image is missing input-routing overlay hashes." >&2; exit 1; }
[[ "$(hash_path "$input_routing_source_path")" == "$input_routing_source_hash" ]] || { echo "Input-routing overlay source hash mismatch." >&2; exit 1; }
[[ "$(hash_path "$input_routing_dtbo_path")" == "$input_routing_dtbo_hash" ]] || { echo "Input-routing overlay DTBO hash mismatch." >&2; exit 1; }
armbian_env_content="$(read_file "$armbian_env_path")"
validate_env_tokens "$armbian_env_content" overlays i2c1-pi || { echo "Armbian image must claim overlays=i2c1-pi exactly once." >&2; exit 1; }
validate_env_tokens "$armbian_env_content" user_overlays octessera-h618-spi1-cs0 || { echo "Armbian image must claim the SPI user overlay exactly once." >&2; exit 1; }
validate_env_tokens "$armbian_env_content" user_overlays octessera-h618-input-routing || { echo "Armbian image must claim the input-routing user overlay exactly once." >&2; exit 1; }
if printf '%s\n' "$armbian_env_content" | awk '!/^[[:space:]]*#/ && /(^|[[:space:]])console=ttyS0(,|$)/' | grep -q .; then
  echo "Armbian image must not retain console=ttyS0 in armbianEnv.txt." >&2
  exit 1
fi
spi_source_content="$(read_file "$spi_source_path")"
if printf '%s\n' "$spi_source_content" "$armbian_env_content" | grep -q 'spidev1_0'; then
  echo "Built Armbian image must not contain the stock spidev1_0 overlay path." >&2
  exit 1
fi
input_routing_source_content="$(read_file "$input_routing_source_path")"
printf '%s\n' "$input_routing_source_content" | grep -q 'status = "disabled"' || { echo "Input-routing overlay must disable UART0." >&2; exit 1; }
printf '%s\n' "$input_routing_source_content" | grep -q 'pins = "PH0", "PH1"' || { echo "Input-routing overlay must release PH0/PH1." >&2; exit 1; }
printf '%s\n' "$input_routing_source_content" | grep -q 'stdout-path = ""' || { echo "Input-routing overlay must clear stdout-path." >&2; exit 1; }

if [[ "$expected_image_mode" == diagnostic ]]; then
  for path in \
    etc/systemd/system/octessera-update-guard.service \
    etc/systemd/system/octessera-update-recovery.service \
    etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service \
    usr/local/sbin/octessera-update \
    usr/local/sbin/octessera-update-guard \
    usr/local/sbin/octessera-update-recovery \
    usr/local/lib/octessera/updater_protocol.py \
    usr/local/lib/octessera/updater_state.py \
    usr/local/lib/octessera/updater_assets.py \
    usr/local/lib/octessera/updater_guard.py \
    usr/local/lib/octessera/updater_cli.py \
    etc/sudoers.d/octessera-update; do
    stat_path "$path" || { echo "Missing updater protocol path: $path" >&2; exit 1; }
  done
  require_root_mode usr/local/sbin/octessera-update 755
  require_root_mode usr/local/sbin/octessera-update-guard 755
  require_root_mode usr/local/sbin/octessera-update-recovery 755
  require_root_mode usr/local/lib/octessera/updater_protocol.py 644
  require_root_mode usr/local/lib/octessera/updater_state.py 644
  require_root_mode usr/local/lib/octessera/updater_assets.py 644
  require_root_mode usr/local/lib/octessera/updater_guard.py 644
  require_root_mode usr/local/lib/octessera/updater_cli.py 644
  require_root_mode etc/sudoers.d/octessera-update 440

  recovery_unit="$(read_file etc/systemd/system/octessera-update-recovery.service)"
  printf '%s\n' "$recovery_unit" | grep -q '^RemainAfterExit=yes$' || {
    echo "Armbian recovery service is not retained for the boot." >&2
    exit 1
  }
  if printf '%s\n' "$recovery_unit" | grep -q '^ConditionPathExists='; then
    echo "Armbian recovery service must run once per boot, not only for pending transactions." >&2
    exit 1
  fi
  sudoers="$(read_file etc/sudoers.d/octessera-update)"
  if printf '%s\n' "$sudoers" | grep -Eq 'octessera-update-(guard|recovery)'; then
    echo "Armbian sudoers must not expose updater internals." >&2
    exit 1
  fi
fi

unit_masked etc/systemd/system/ssh.service || { echo "ssh.service is not masked in the built image." >&2; exit 1; }
unit_masked etc/systemd/system/ssh.socket || { echo "ssh.socket is not masked in the built image." >&2; exit 1; }
unit_masked etc/systemd/system/serial-getty@ttyS0.service || { echo "serial-getty@ttyS0.service is not masked in the built image." >&2; exit 1; }

echo "Built Armbian image inspection passed ($expected_image_mode mode)."
