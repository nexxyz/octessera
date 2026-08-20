#!/usr/bin/env bash
set -euo pipefail

expected_image_mode=diagnostic
setup_layer_required=false
if [[ "${1:-}" == --setup-layer ]]; then setup_layer_required=true; shift; fi
if [[ "${1:-}" == --mode ]]; then expected_image_mode="${2:-}"; shift 2; fi
if [[ "$expected_image_mode" != diagnostic && "$expected_image_mode" != production ]] || [[ $# -ne 1 ]]; then
  echo "Usage: $0 [--setup-layer] [--mode diagnostic|production] <rootfs-dir-or-ext4-image>" >&2
  exit 2
fi
target="$1"
module_dir="$(dirname "${BASH_SOURCE[0]}")"
spi_source_path=usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts
spi_dtbo_path=boot/overlay-user/octessera-h618-spi1-cs0.dtbo
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$module_dir/validation-assertions.sh"
# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$module_dir/inspect-mode.sh"
# shellcheck source=tools/armbian-image/authorized-key-paths.sh
source "$module_dir/authorized-key-paths.sh"
# shellcheck source=tools/armbian-image/inspect-path.sh
source "$module_dir/inspect-path.sh"
# shellcheck source=tools/armbian-image/setup-layer-proof.sh
source "$module_dir/setup-layer-proof.sh"
# shellcheck source=tools/armbian-image/inspect-account-ssh.sh
source "$module_dir/inspect-account-ssh.sh"
# shellcheck source=tools/armbian-image/inspect-network.sh
source "$module_dir/inspect-network.sh"
# shellcheck source=tools/armbian-image/inspect-device-tree.sh
source "$module_dir/inspect-device-tree.sh"
# shellcheck source=tools/armbian-image/inspect-runtime-contracts.sh
source "$module_dir/inspect-runtime-contracts.sh"
# shellcheck source=tools/armbian-image/inspect-runtime.sh
source "$module_dir/inspect-runtime.sh"

inspect_work="$(mktemp -d)"
cleanup() { rm -rf "$inspect_work"; }
trap cleanup EXIT

read_file() {
  local path="$1" request error_path="$inspect_work/debugfs-read.stderr" error_content content status
  octessera_debugfs_path_argument "$path" >/dev/null || return 2
  if [[ -d "$target" ]]; then
    if cat -- "$target/$path"; then return; fi
    echo "Unable to read image path: $path." >&2
    return 2
  fi
  request="$(octessera_debugfs_cat_request "$path")" || return 2
  if content="$(debugfs -R "$request" "$target" 2>"$error_path")"; then status=0; else status=$?; fi
  error_content="$(cat -- "$error_path")"
  if [[ "$status" != 0 ]] || ! octessera_debugfs_stderr_is_startup_banner "$error_content" || printf '%s\n' "$content" | grep -Eq '(^|:) File not found by ext2_lookup[[:space:]]*$|^cat:'; then
    [[ -z "$error_content" ]] || printf '%s\n' "$error_content" >&2
    echo "Unable to read image path: $path." >&2
    return 2
  fi
  printf '%s' "$content"
}

stat_path() { octessera_stat_path "$target" "$1"; }

require_root_mode() {
  local path="$1" mode="$2" expected_mode actual_mode metadata metadata_status
  octessera_debugfs_path_argument "$path" >/dev/null || { echo "Unsafe image path: $path." >&2; exit 1; }
  case "$mode" in
    [0-7][0-7][0-7]) expected_mode="0$mode" ;;
    [0-7][0-7][0-7][0-7]) expected_mode="$mode" ;;
    *) echo "Invalid expected mode for $path." >&2; exit 1 ;;
  esac
  if [[ -d "$target" ]]; then
    actual_mode="$(stat -c '%a' "$target/$path")"
    [[ "${#actual_mode}" == 3 ]] && actual_mode="0$actual_mode"
    [[ "$(stat -c '%u:%g' "$target/$path")" == 0:0 && "$actual_mode" == "$expected_mode" ]] || { echo "Unsafe updater ownership/mode at $path." >&2; exit 1; }
    return
  fi
  if metadata="$(octessera_debugfs_stat_metadata "$target" "$path")"; then metadata_status=0; else metadata_status=$?; fi
  [[ "$metadata_status" == 0 ]] || { echo "Unable to inspect image path: $path." >&2; exit 1; }
  printf '%s\n' "$metadata" | grep -Eq 'User: +0 +Group: +0' || { echo "Unsafe image ownership at $path." >&2; exit 1; }
  actual_mode="$(octessera_debugfs_mode "$metadata")" || { echo "Missing image mode at $path." >&2; exit 1; }
  [[ "$actual_mode" == "$expected_mode" ]] || { echo "Unsafe image mode at $path." >&2; exit 1; }
}

hash_path() {
  local path="$1" dump_path request error_path="$inspect_work/debugfs-dump.stderr" error_content status
  dump_path="$inspect_work/$(basename "$path")"
  octessera_debugfs_path_argument "$path" >/dev/null || { echo "Unsafe image path: $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then sha256sum "$target/$path" | awk '{ print $1 }'; return; fi
  rm -f -- "$dump_path"
  request="$(octessera_debugfs_dump_request "$path" "$dump_path")" || { echo "Unable to read image path: $path." >&2; exit 1; }
  if debugfs -R "$request" "$target" >/dev/null 2>"$error_path"; then status=0; else status=$?; fi
  error_content="$(cat -- "$error_path")"
  if [[ "$status" != 0 ]] || ! octessera_debugfs_stderr_is_startup_banner "$error_content"; then
    [[ -z "$error_content" ]] || printf '%s\n' "$error_content" >&2
    echo "Unable to read image path: $path." >&2
    exit 1
  fi
  [[ -s "$dump_path" ]] || { echo "Unable to read image path: $path." >&2; exit 1; }
  sha256sum "$dump_path" | awk '{ print $1 }'
}

reject_path() {
  local path="$1" stat_status
  if stat_path "$path"; then
    echo "Diagnostic-only Orange image must not contain runtime path: $path." >&2
    exit 1
  else
    stat_status=$?
    [[ "$stat_status" == 1 ]] || { echo "Unable to inspect image path: $path." >&2; exit 1; }
  fi
}

unit_masked() { octessera_unit_masked_path "$target" "$1"; }

octessera_require_account_ssh_contract
profile_metadata="$(read_file etc/octessera/build-metadata.env)"
default_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_PI_DEFAULT_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
samples_manifest_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SAMPLES_MANIFEST_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
[[ -n "$default_hash" && -n "$samples_manifest_hash" ]] || { echo 'Armbian image is missing musical asset hashes.' >&2; exit 1; }
printf '%s\n' "$profile_metadata" | grep -q '^OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w$' || { echo 'Armbian image must be labeled orange-pi-zero-2w.' >&2; exit 1; }
reject_path etc/systemd/system/multi-user.target.wants/octessera-wifi-foundation.service
octessera_require_wifi_foundation
if [[ "$setup_layer_required" == true ]]; then require_setup_layer; fi
octessera_inspect_runtime_mode "$profile_metadata" "$expected_image_mode"
octessera_require_device_tree_contract "$profile_metadata"
octessera_require_built_updater_contract

for path in \
  usr/local/sbin/octessera-orange-usb-gadget usr/local/lib/octessera/device_config.py \
  usr/local/sbin/octessera-device-apply-reboot usr/local/sbin/octessera-orange-oled-logo \
  usr/local/sbin/octessera-orange-oled-handoff.py usr/local/sbin/octessera-orange-oled-lifecycle.py \
  usr/local/sbin/octessera-orange-oled-suspend usr/local/sbin/octessera-provision-musical-default \
  etc/modules-load.d/octessera-orange-midi.conf etc/modules-load.d/octessera-orange-usb-gadget.conf \
  etc/systemd/system/octessera-orange-usb-gadget.service etc/systemd/system/octessera-device-apply-reboot.socket \
  etc/systemd/system/octessera-device-apply-reboot@.service etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket \
  etc/systemd/system/octessera-orange-boot-splash.service etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service \
  etc/systemd/system/octessera-orange-oled-shutdown.service etc/systemd/system/multi-user.target.wants/octessera-orange-oled-shutdown.service \
  etc/systemd/system/octessera-orange-oled-suspend.service etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service \
  etc/systemd/system/octessera-provision-musical-default.service usr/share/octessera/oled/octessera-pi-booting.rgb565 \
  usr/share/octessera/oled/octessera-pi-shutdown.rgb565 usr/share/octessera/defaults/pi-default.json \
  usr/share/octessera/samples/sample-manifest.tsv usr/share/octessera/samples/ATTRIBUTIONS.tsv \
  usr/share/octessera/samples/upstream/LICENSE usr/share/octessera/samples/upstream/README.txt; do
  stat_path "$path" || { echo "Missing Orange OS parity path: $path." >&2; exit 1; }
done
for path in usr/local/sbin/octessera-orange-usb-gadget usr/local/lib/octessera/device_config.py usr/local/sbin/octessera-device-apply-reboot usr/local/sbin/octessera-orange-oled-logo usr/local/sbin/octessera-orange-oled-suspend usr/local/sbin/octessera-provision-musical-default; do require_root_mode "$path" 755; done
for path in usr/local/sbin/octessera-orange-oled-handoff.py usr/local/sbin/octessera-orange-oled-lifecycle.py etc/modules-load.d/octessera-orange-midi.conf etc/modules-load.d/octessera-orange-usb-gadget.conf etc/systemd/system/octessera-orange-usb-gadget.service etc/systemd/system/octessera-device-apply-reboot.socket etc/systemd/system/octessera-device-apply-reboot@.service etc/systemd/system/octessera-orange-boot-splash.service etc/systemd/system/octessera-orange-oled-shutdown.service etc/systemd/system/octessera-orange-oled-suspend.service etc/systemd/system/octessera-provision-musical-default.service usr/share/octessera/defaults/pi-default.json usr/share/octessera/samples/sample-manifest.tsv usr/share/octessera/samples/ATTRIBUTIONS.tsv usr/share/octessera/samples/upstream/LICENSE usr/share/octessera/samples/upstream/README.txt; do require_root_mode "$path" 644; done
octessera_require_image_symlink etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket ../octessera-device-apply-reboot.socket /etc/systemd/system/octessera-device-apply-reboot.socket
octessera_require_image_symlink etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service ../octessera-orange-boot-splash.service /etc/systemd/system/octessera-orange-boot-splash.service
octessera_require_image_symlink etc/systemd/system/multi-user.target.wants/octessera-orange-oled-shutdown.service ../octessera-orange-oled-shutdown.service /etc/systemd/system/octessera-orange-oled-shutdown.service
octessera_require_image_symlink etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service ../octessera-orange-oled-suspend.service /etc/systemd/system/octessera-orange-oled-suspend.service
reject_path etc/systemd/system/sleep.target.wants/octessera-orange-oled-suspend.service
reject_path lib/systemd/system-sleep/octessera-orange-oled
reject_path usr/lib/systemd/system-sleep/octessera-orange-oled
for path in usr/share/octessera/defaults/pi-default.json usr/share/octessera/samples/sample-manifest.tsv usr/share/octessera/samples/ATTRIBUTIONS.tsv usr/share/octessera/samples/upstream/LICENSE usr/share/octessera/samples/upstream/README.txt; do require_root_mode "$path" 644; done
reject_path usr/share/octessera/samples/files
[[ "$(hash_path usr/share/octessera/defaults/pi-default.json)" == "$default_hash" ]] || { echo 'Pi default hash mismatch.' >&2; exit 1; }
[[ "$(hash_path usr/share/octessera/samples/sample-manifest.tsv)" == "$samples_manifest_hash" ]] || { echo 'Sample manifest hash mismatch.' >&2; exit 1; }
octessera_validate_sample_tree "$target" "$(read_file usr/share/octessera/samples/sample-manifest.tsv)" "$inspect_work"

gadget_unit="$(read_file etc/systemd/system/octessera-orange-usb-gadget.service)"
alsa_modules="$(read_file etc/modules-load.d/octessera-orange-midi.conf)"
[[ "$alsa_modules" == $'snd_seq\nsnd_seq_midi' ]]
printf '%s\n' "$gadget_unit" | grep -q 'ExecStart=/usr/local/sbin/octessera-orange-usb-gadget setup'
printf '%s\n' "$gadget_unit" | grep -q 'ExecStop=/usr/local/sbin/octessera-orange-usb-gadget teardown'
gadget_script="$(read_file usr/local/sbin/octessera-orange-usb-gadget)"
printf '%s\n' "$gadget_script" | grep -q 'musb-hdrc.4.auto'
octessera_reject_text_match 'Orange USB gadget contains a Raspberry Pi assumption or generic fallback.' "$gadget_script" -Eq 'dwc2|BCM[0-9]|/home/pi|config\.txt'

unit_masked etc/systemd/system/ssh.service || { echo 'ssh.service is not masked in the built image.' >&2; exit 1; }
unit_masked etc/systemd/system/ssh.socket || { echo 'ssh.socket is not masked in the built image.' >&2; exit 1; }
unit_masked etc/systemd/system/serial-getty@ttyS0.service || { echo 'serial-getty@ttyS0.service is not masked in the built image.' >&2; exit 1; }
echo "Built Armbian image inspection passed ($expected_image_mode mode)."
