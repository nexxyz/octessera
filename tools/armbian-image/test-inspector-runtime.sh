#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/armbian-image/test-inspector-fixture.sh
source "$script_dir/test-inspector-fixture.sh"
module_dir="$script_dir"
# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$module_dir/inspect-mode.sh"
# shellcheck source=tools/armbian-image/inspect-runtime.sh
source "$module_dir/inspect-runtime.sh"

assert_runtime_owned_mode_status() {
  local expected="$1"
  shift
  local actual
  if (octessera_require_owned_mode "$@"); then actual=0; else actual=$?; fi
  [[ "$actual" == "$expected" ]] || { echo "Expected runtime ownership status $expected, got $actual." >&2; exit 1; }
}
target="$fake_image"
export DEBUGFS_CASE=runtime-owner-valid
assert_runtime_owned_mode_status 0 var/lib/octessera/presets 990:990 755
assert_runtime_owned_mode_status 0 var/lib/octessera/presets 990:990 0755
export DEBUGFS_CASE=runtime-owner-wrong-owner
assert_runtime_owned_mode_status 1 var/lib/octessera/presets 990:990 755
export DEBUGFS_CASE=runtime-owner-wrong-mode
assert_runtime_owned_mode_status 1 var/lib/octessera/presets 990:990 755
runtime_directory="$work/runtime-owned"
mkdir -p "$runtime_directory/var/lib/octessera/presets"
chmod 0755 "$runtime_directory/var/lib/octessera/presets"
directory_owner="$(stat -c '%u:%g' "$runtime_directory/var/lib/octessera/presets")"
target="$runtime_directory"
assert_runtime_owned_mode_status 0 var/lib/octessera/presets "$directory_owner" 755
chmod 0700 "$runtime_directory/var/lib/octessera/presets"
assert_runtime_owned_mode_status 1 var/lib/octessera/presets "$directory_owner" 755
assert_runtime_owned_mode_status 1 var/lib/octessera/presets "$directory_owner" 07555
target="$fake_image"
stat_path() { octessera_stat_path "$target" "$1"; }
export DEBUGFS_CASE=variable-whitespace
assert_status 0 octessera_require_real_directory opt/octessera
assert_status 0 octessera_require_runtime_entry_set opt/octessera/releases/1.2.3
# shellcheck disable=SC2218
octessera_require_image_symlink opt/octessera/current /opt/octessera/releases/1.2.3
runtime_contract="$root/userpatches/overlay/etc/octessera/image-contract.json"
runtime_contract_hash="$(sha256sum "$runtime_contract" | awk '{ print $1 }')"
device_apply_socket_unit="$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket"
device_apply_service_unit="$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service"
device_config_validator="$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
device_apply_helper="$root/userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot"
pi_default="$root/config/generated/pi/default.json"
mkdir -p "$fake_image/etc/systemd/system/sockets.target.wants"
ln -s ../octessera-device-apply-reboot.socket "$fake_image/etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket"
runtime_rejected_paths=()
read_file() {
  case "$1" in
    etc/octessera/image-contract.json) cat -- "$runtime_contract" ;;
    etc/systemd/system/octessera-device-apply-reboot.socket) cat -- "$device_apply_socket_unit" ;;
    etc/systemd/system/octessera-device-apply-reboot@.service) cat -- "$device_apply_service_unit" ;;
    etc/passwd) printf '%s\n' 'octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin' ;;
    usr/local/lib/octessera/device_config.py) cat -- "$device_config_validator" ;;
    usr/local/sbin/octessera-device-apply-reboot) cat -- "$device_apply_helper" ;;
    usr/share/octessera/defaults/pi-default.json) cat -- "$pi_default" ;;
    *) return 1 ;;
  esac
}
require_root_mode() { :; }
hash_path() { [[ "$1" == etc/octessera/image-contract.json ]] && printf '%s\n' "$runtime_contract_hash"; }
reject_path() { runtime_rejected_paths+=("$1"); }
octessera_require_orange_boot_service() { :; }
octessera_require_orange_shutdown_service() { :; }
octessera_require_orange_suspend_service() { :; }
octessera_require_real_directory() { :; }
octessera_require_owned_mode() { :; }
profile_metadata=$'OCTESSERA_IMAGE_MODE=diagnostic\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\nOCTESSERA_IMAGE_CONTRACT_SHA256='"$runtime_contract_hash"$'\nOCTESSERA_RUNTIME_VERSION=none\nOCTESSERA_RUNTIME_BINARY_SHA256=none\nOCTESSERA_RUNTIME_MANIFEST_SHA256=none\nOCTESSERA_RUNTIME_METADATA_SHA256=none'
octessera_inspect_runtime_mode "$profile_metadata" diagnostic
[[ "${runtime_rejected_paths[*]}" == 'etc/systemd/system/octessera.service etc/systemd/system/multi-user.target.wants/octessera.service usr/local/bin/octessera-pi opt/octessera/current opt/octessera/releases' ]] || { echo 'Diagnostic inspector did not reject every runtime path.' >&2; exit 1; }

runtime_binary_hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
runtime_manifest_hash=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
runtime_metadata_hash=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
runtime_root="$work/runtime-root"
mkdir -p "$runtime_root/etc/udev/rules.d"
printf '%s\n' 'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"' > "$runtime_root/etc/udev/rules.d/70-octessera-orange-runtime.rules"
printf '%s\n' 'KERNEL=="wlan*", ACTION=="add", RUN+="/sbin/iw dev %k set power_save off"' > "$runtime_root/etc/udev/rules.d/10-wifi-power-save.rules"
ln -s /dev/null "$runtime_root/etc/udev/rules.d/09-disabled.rules"
target="$runtime_root"
login_defs_fixture=$'TTYPERM 0620\nUID_MIN 1000'
hosts_fixture=$'127.0.0.1 localhost\n127.0.1.1 octessera-opi local-alias # orangepizero2w stays in comments\n::1 localhost ip6-localhost ip6-loopback octessera-opi # orangepizero2w stays in comments\n'
hostname_hash="$(printf '%s\n' 'octessera-opi' | sha256sum | awk '{ print $1 }')"
hostname_no_newline_hash="$(printf '%s' 'octessera-opi' | sha256sum | awk '{ print $1 }')"
hash_path() {
  case "$1" in
    etc/hostname) printf '%s\n' "$hostname_hash" ;;
    opt/octessera/releases/1.2.3/octessera-pi) printf '%s\n' "$runtime_binary_hash" ;;
    opt/octessera/releases/1.2.3/SHA256SUMS) printf '%s\n' "$runtime_manifest_hash" ;;
    opt/octessera/releases/1.2.3/octessera-runtime.json) printf '%s\n' "$runtime_metadata_hash" ;;
    *) return 1 ;;
  esac
}
require_root_mode() { :; }
stat_path() {
  case "$1" in
    etc/systemd/system/octessera-update-guard.service|etc/systemd/system/octessera-update-recovery.service|etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service|usr/local/sbin/octessera-update|usr/local/sbin/octessera-update-broker|usr/local/sbin/octessera-update-guard|usr/local/sbin/octessera-update-recovery|usr/local/lib/octessera/updater_protocol.py|usr/local/lib/octessera/updater_contract.py|usr/local/lib/octessera/updater_state.py|usr/local/lib/octessera/updater_assets.py|usr/local/lib/octessera/updater_guard.py|usr/local/lib/octessera/updater_cli.py|usr/local/lib/octessera/updater_profiles.py|etc/systemd/system/octessera-update.socket|etc/systemd/system/octessera-update@.service|etc/sudoers.d/octessera-update|etc/login.defs|etc/hostname|etc/hosts) return 0 ;;
    *) [[ -e "$target/$1" || -L "$target/$1" ]] ;;
  esac
}
octessera_require_image_contract() { [[ "$1" == production ]]; }
octessera_require_absent_path() { :; }
octessera_require_runtime_entry_set() { :; }
octessera_require_real_directory() { :; }
octessera_require_runtime_elf() { :; }
octessera_require_owned_mode() { :; }
read_file() {
  case "$1" in
    etc/shadow) printf '%s\n' 'octessera-runtime:!:19000:0:99999:7:::' ;;
    etc/passwd) printf '%s\n' 'octessera:x:1000:1000:Octessera:/home/octessera:/bin/bash' 'octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin' ;;
    etc/group) printf '%s\n' 'octessera:x:1000:' 'octessera-runtime:x:990:' 'audio:x:29:octessera-runtime' 'i2c:x:100:octessera-runtime' 'spi:x:999:octessera-runtime' 'gpio:x:997:octessera-runtime' 'tty:x:5:' 'video:x:44:octessera-runtime' ;;
    etc/hostname) printf '%s\n' 'octessera-opi' ;;
    etc/hosts) printf '%s' "$hosts_fixture" ;;
    etc/login.defs) printf '%s\n' "$login_defs_fixture" ;;
    etc/sudoers) printf '%s\n' "$sudoers_fixture" ;;
    etc/udev/rules.d/70-octessera-orange-runtime.rules) printf '%s\n' 'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"' ;;
    etc/systemd/system/octessera.service) cat "$root/userpatches/overlay/etc/systemd/system/octessera.service" ;;
    etc/systemd/system/octessera-device-apply-reboot.socket) cat "$device_apply_socket_unit" ;;
    etc/systemd/system/octessera-device-apply-reboot@.service) cat "$device_apply_service_unit" ;;
    etc/systemd/system/octessera-update.socket) cat "$root/userpatches/overlay/etc/systemd/system/octessera-update.socket" ;;
    etc/sudoers.d/octessera-update) cat "$root/userpatches/overlay/etc/sudoers.d/octessera-update" ;;
    usr/local/lib/octessera/device_config.py) cat "$device_config_validator" ;;
    usr/local/sbin/octessera-device-apply-reboot) cat "$device_apply_helper" ;;
    usr/share/octessera/defaults/pi-default.json) cat "$pi_default" ;;
    opt/octessera/releases/1.2.3/octessera-runtime.json) printf '%s\n' "{\"name\":\"octessera-pi\",\"profile\":\"orange-pi-zero-2w\",\"version\":\"1.2.3\",\"artifact_kind\":\"production-runtime\",\"runtime_ready\":true,\"binary_sha256\":\"$runtime_binary_hash\"}" ;;
    opt/octessera/releases/1.2.3/SHA256SUMS) printf '%s  octessera-pi\n' "$runtime_binary_hash" ;;
    *) return 1 ;;
  esac
}
sudoers_fixture='octessera-runtime ALL=(ALL) NOPASSWD:ALL'
runtime_links=()
octessera_require_image_symlink() { runtime_links+=("$1=$2"); }
profile_metadata=$'OCTESSERA_IMAGE_MODE=production\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=true\nOCTESSERA_RUNTIME_VERSION=1.2.3\nOCTESSERA_RUNTIME_BINARY_SHA256='"$runtime_binary_hash"$'\nOCTESSERA_RUNTIME_MANIFEST_SHA256='"$runtime_manifest_hash"$'\nOCTESSERA_RUNTIME_METADATA_SHA256='"$runtime_metadata_hash"
octessera_inspect_runtime_mode "$profile_metadata" production
[[ "${runtime_links[*]}" == 'etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket=../octessera-device-apply-reboot.socket etc/systemd/system/sockets.target.wants/octessera-update.socket=../octessera-update.socket opt/octessera/current=/opt/octessera/releases/1.2.3 usr/local/bin/octessera-pi=/opt/octessera/current/octessera-pi etc/systemd/system/multi-user.target.wants/octessera.service=../octessera.service' ]] || { echo 'Production inspector did not require the exact symlink chain.' >&2; exit 1; }
hostname_hash="$hostname_no_newline_hash"
if ( octessera_inspect_runtime_mode "$profile_metadata" production ); then
  echo 'Production inspector accepted /etc/hostname without a trailing newline.' >&2
  exit 1
fi
hostname_hash="$(printf '%s\n' 'octessera-opi' | sha256sum | awk '{ print $1 }')"
hosts_fixture=$'127.0.0.1 localhost\n127.0.1.1 local-alias\n::1 localhost ip6-localhost ip6-loopback\n'
if ( octessera_inspect_runtime_mode "$profile_metadata" production ); then
  echo 'Production inspector accepted hosts without the target hostname alias.' >&2
  exit 1
fi
touch "$runtime_root/etc/sudoers"
if ( octessera_require_runtime_account "$(read_file etc/passwd)" "$(read_file etc/group)" ); then echo 'Runtime account appeared in sudoers.' >&2; exit 1; fi
sudoers_fixture='octessera ALL=(root) NOPASSWD: /sbin/shutdown'
octessera_require_runtime_account "$(read_file etc/passwd)" "$(read_file etc/group)" >/dev/null
sudoers_fixture='octessera ALL=(ALL) NOPASSWD: ALL'
if ( octessera_require_runtime_account "$(read_file etc/passwd)" "$(read_file etc/group)" ); then echo 'Unrestricted passwordless sudo was accepted.' >&2; exit 1; fi
rm -f "$runtime_root/etc/sudoers"
bad_groups="$(read_file etc/group)"$'\n''sudo:x:27:octessera-runtime'
if ( octessera_require_runtime_account "$(read_file etc/passwd)" "$bad_groups" ); then echo 'Runtime account appeared in the sudo admin group.' >&2; exit 1; fi
