#!/usr/bin/env bash
# shellcheck disable=SC1091
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
welcome_stager="$root/tools/armbian-image/stage-canonical-welcome.sh"
welcome_overlay="$root/userpatches/overlay/etc/profile.d/octessera-welcome.sh"
welcome_overlay_preexisting=false
if [[ -e "$welcome_overlay" || -L "$welcome_overlay" ]]; then
  welcome_overlay_preexisting=true
fi
"$welcome_stager"
armbian_extensions_resolver="$root/tools/armbian-image/resolve-armbian-extensions.sh"
image_sanitization_test="$root/tools/armbian-image/test-image-sanitization.sh"
inspector_test="$root/tools/armbian-image/test-inspector.sh"
image_sanitization_extension="$root/userpatches/extensions/octessera_image_sanitize.sh"
alsa_sequencer_test="$root/tools/armbian-image/test-orange-alsa-sequencer.sh"
alsa_sequencer_extension="$root/userpatches/extensions/octessera_midi.sh"
alsa_sequencer_modules="$root/userpatches/overlay/etc/modules-load.d/octessera-orange-midi.conf"
orange_kernel_package_test="$root/tools/armbian-image/test-orange-kernel-package.sh"
orange_kernel_package_validator="$root/tools/armbian-image/validate-orange-kernel-package.sh"
orange_image_proof_test="$root/tools/armbian-image/test-orange-image-proof.sh"
orange_image_proof_verifier="$root/tools/armbian-image/verify-orange-image.sh"
orange_image_mount_helper="$root/tools/armbian-image/orange_image_mount.py"
orange_boot_selection_helper="$root/tools/armbian-image/orange_boot_selection.py"
orange_image_proof_python="$root/tools/armbian-image/verify-orange-image.py"
orange_image_proof_fixture="$root/tools/armbian-image/test-orange-image-proof.py"
orange_trusted_proof="$root/tools/armbian-image/orange_trusted_parent_proof.py"
orange_trusted_proof_test="$root/tools/armbian-image/test-orange-trusted-proof.py"
orange_initramfs="$root/tools/armbian-image/orange_initramfs.py"
orange_phase5_proof="$root/tools/armbian-image/orange_phase5_proof.py"
orange_boot_splash_test="$root/tools/armbian-image/test-orange-boot-splash-hook.sh"
orange_oled_logo_test="$root/tools/armbian-image/test_orange_oled_logo.py"
orange_oled_handoff_test="$root/tools/armbian-image/test_orange_oled_handoff.py"
orange_runtime_identity_test="$root/tools/armbian-image/test-orange-runtime-identity.py"
orange_constructor_test="$root/tools/armbian-image/test-orange-construction.py"
orange_constructor="$root/resources/image-construction/boot-layers/orange-pi-zero-2w.json"
orange_boot_neutral_derivation="$root/resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json"
orange_boot_splash_fixture_files="$root/tools/armbian-image/fixtures/python313-initramfs-closure-files.txt"
orange_boot_splash_fixture_imports="$root/tools/armbian-image/fixtures/python313-initramfs-closure/imports.py"
orange_boot_splash_fixture_collections="$root/tools/armbian-image/fixtures/python313-initramfs-closure/collections/__init__.py"
orange_boot_splash_fixture_abc="$root/tools/armbian-image/fixtures/python313-initramfs-closure/_collections_abc.py"
spi_dts="$root/userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts"
input_routing_dts="$root/userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts"
spi_env_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh"
spi_validation_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh"
input_routing_validation_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh"
input_routing_boot_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh"
input_routing_provision="$root/tools/orange-pi/input-routing-provision.sh"
boot_dtb_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
inspect_mode_helper="$root/tools/armbian-image/inspect-mode.sh"
image_mode_helper="$root/userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh"
diagnostic_payload_helper="$root/userpatches/overlay/usr/local/lib/octessera/diagnostic-payload.sh"
runtime_inspector="$root/tools/armbian-image/inspect-runtime.sh"
image_mode_test="$root/tools/armbian-image/test-image-mode.sh"
runtime_service_test="$root/tools/armbian-image/test-orange-runtime-service.sh"
setup_layer_test="$root/tools/armbian-image/test-setup-layer.sh"
setup_sidecar_test="$root/tools/armbian-image/test_setup_sidecar.py"
setup_request_test="$root/tools/armbian-image/test-setup-request.py"
setup_http_test="$root/tools/armbian-image/test-setup-http.py"
setup_flow_test="$root/tools/armbian-image/test-setup-flow.py"
setup_state_test="$root/tools/armbian-image/test-setup-state.py"
setup_layer_proof="$root/tools/armbian-image/setup-layer-proof.sh"
setup_layer_installer="$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"
setup_status="$root/userpatches/overlay/usr/local/lib/octessera/setup-status.py"
setup_status_cli="$root/userpatches/overlay/usr/local/lib/octessera/setup-status-cli.py"
setup_call="$root/userpatches/overlay/usr/local/lib/octessera/setup-call.py"
setup_request="$root/userpatches/overlay/usr/local/sbin/octessera-setup-request"
setup_request_cleanup="$root/userpatches/overlay/usr/local/sbin/octessera-setup-request-cleanup"
setup_start="$root/userpatches/overlay/usr/local/sbin/octessera-setup-start"
setup_cleanup="$root/userpatches/overlay/usr/local/sbin/octessera-setup-cleanup"
setup_profile="$root/userpatches/overlay/etc/octessera/setup-profile"
runtime_udev_rule="$root/userpatches/overlay/etc/udev/rules.d/70-octessera-orange-runtime.rules"
image_verifier="$root/tools/armbian-image/verify-orange-image.py"
runtime_account_verifier="$root/tools/armbian-image/verify_runtime_account.py"
authorized_key_paths_helper="$root/tools/armbian-image/authorized-key-paths.sh"
inspect_path_helper="$root/tools/armbian-image/inspect-path.sh"
spi_fixture="$root/tools/armbian-image/fixtures/h618-spi-base.dts"
spi_overlay_name=octessera-h618-spi1-cs0

if [[ "${ARMBIAN_BOARD+x}" == x && "${ARMBIAN_BOARD}" != orangepizero2w ]]; then
  echo "Armbian image validation accepts only board orangepizero2w." >&2
  exit 1
fi
if [[ "${ARMBIAN_RUN_BUILD:-false}" == true && ! "${ARMBIAN_BUILD_REF:-}" =~ ^[0-9a-fA-F]{40}$ ]]; then
  echo "Qualification builds require a reviewed immutable 40-character Armbian commit SHA." >&2
  exit 1
fi

inspect_payload_tar() {
  local tar_path="$1"
  tar -tf "$tar_path" | while IFS= read -r entry; do
    case "$entry" in
      /*|..|../*|*/..|*/../*) echo "Unsafe payload path: $entry" >&2; exit 1 ;;
    esac
  done
  tar -tvf "$tar_path" | while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; exit 1 ;;
    esac
  done
}

required_files=(
  "$welcome_stager"
  "$root/userpatches/overlay/etc/profile.d/octessera-welcome.sh"
  "$armbian_extensions_resolver"
  "$image_sanitization_test"
  "$image_sanitization_extension"
  "$orange_boot_splash_test"
  "$alsa_sequencer_test"
  "$alsa_sequencer_extension"
  "$alsa_sequencer_modules"
  "$orange_kernel_package_test"
  "$orange_kernel_package_validator"
  "$orange_image_proof_test"
  "$orange_image_proof_verifier"
  "$orange_image_mount_helper"
  "$orange_boot_selection_helper"
  "$orange_image_proof_python"
  "$orange_image_proof_fixture"
  "$orange_trusted_proof"
  "$orange_trusted_proof_test"
  "$orange_initramfs"
  "$orange_phase5_proof"
  "$orange_boot_splash_fixture_files"
  "$orange_boot_splash_fixture_imports"
  "$orange_boot_splash_fixture_collections"
  "$orange_boot_splash_fixture_abc"
  "$orange_oled_logo_test"
  "$orange_oled_handoff_test"
  "$orange_runtime_identity_test"
  "$orange_constructor_test"
  "$orange_constructor"
  "$orange_boot_neutral_derivation"
  "$root/tools/armbian-image/inspect-output-images.sh"
  "$root/tools/armbian-image/stage-musical-assets.sh"
  "$root/tools/armbian-image/test-musical-assets.sh"
  "$authorized_key_paths_helper"
  "$inspect_path_helper"
  "$inspector_test"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation"
  "$root/userpatches/overlay/etc/systemd/system/octessera-wifi-foundation.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
  "$root/userpatches/overlay/etc/sudoers.d/octessera-update"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-guard.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"
  "$root/userpatches/overlay/etc/systemd/system/octessera-setup.service"
  "$root/userpatches/overlay/etc/octessera/image-contract.json"
  "$root/userpatches/overlay/etc/systemd/system/octessera.service"
  "$root/userpatches/overlay/etc/modules-load.d/octessera-orange-usb-gadget.conf"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-usb-gadget.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
  "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
  "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
  "$root/userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
  "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
  "$spi_dts"
  "$input_routing_dts"
  "$spi_env_helper"
  "$spi_validation_helper"
  "$input_routing_validation_helper"
  "$input_routing_boot_helper"
  "$boot_dtb_helper"
  "$inspect_mode_helper"
  "$image_mode_helper"
  "$diagnostic_payload_helper"
  "$runtime_inspector"
  "$image_mode_test"
  "$runtime_service_test"
  "$setup_layer_test"
  "$setup_sidecar_test"
  "$setup_request_test"
  "$setup_http_test"
  "$setup_flow_test"
  "$setup_state_test"
  "$setup_layer_proof"
  "$setup_layer_installer"
  "$setup_status"
  "$setup_status_cli"
  "$setup_call"
  "$setup_request"
  "$setup_request_cleanup"
  "$setup_start"
  "$setup_cleanup"
  "$setup_profile"
  "$runtime_udev_rule"
  "$image_verifier"
  "$runtime_account_verifier"
  "$spi_fixture"
)

bash -n "$root/userpatches/customize-image.sh"
bash -n "$armbian_extensions_resolver"
bash -n "$image_sanitization_extension"
bash -n "$image_sanitization_test"
bash -n "$orange_boot_splash_test"
bash -n "$alsa_sequencer_extension"
bash -n "$alsa_sequencer_test"
bash -n "$orange_kernel_package_test"
bash -n "$orange_kernel_package_validator"
bash -n "$orange_image_proof_test"
bash -n "$orange_image_proof_verifier"
bash -n "$input_routing_provision"
bash -n "$root/tools/armbian-image/inspect-built-image.sh"
bash -n "$setup_layer_proof"
bash -n "$setup_layer_installer"
bash -n "$setup_start"
bash -n "$setup_cleanup"
bash -n "$setup_request_cleanup"
bash -n "$authorized_key_paths_helper"
bash -n "$inspect_path_helper"
bash -n "$inspector_test"
bash -n "$root/tools/armbian-image/inspect-output-images.sh"
bash -n "$root/tools/armbian-image/stage-musical-assets.sh"
bash -n "$root/tools/armbian-image/test-musical-assets.sh"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation"
python3 -m py_compile "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar" "$setup_request" "$setup_status" "$setup_status_cli" "$setup_call"
python3 -m py_compile "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar" "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request" "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status.py" "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status-cli.py" "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-call.py"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget"
bash -n "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
bash -n "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
bash -n "$root/userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled"
bash -n "$spi_env_helper"
bash -n "$spi_validation_helper"
bash -n "$input_routing_validation_helper"
bash -n "$input_routing_boot_helper"
bash -n "$boot_dtb_helper"
bash -n "$inspect_mode_helper"
bash -n "$image_mode_helper"
bash -n "$diagnostic_payload_helper"
bash -n "$runtime_inspector"
bash -n "$image_mode_test"
bash -n "$runtime_service_test"
python3 -m py_compile "$root/tools/device-update/updater_protocol.py" "$root/tools/device-update/updater_state.py" "$root/tools/device-update/updater_assets.py" "$root/tools/device-update/updater_guard.py" "$root/tools/device-update/updater_cli.py"
python3 -m py_compile "$orange_image_mount_helper" "$orange_boot_selection_helper" "$orange_image_proof_python" "$orange_image_proof_fixture" "$orange_trusted_proof" "$orange_trusted_proof_test" "$orange_initramfs" "$orange_phase5_proof"
python3 -m py_compile "$image_verifier" "$runtime_account_verifier"
python3 -m py_compile "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo" "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py" "$root/tools/armbian-image/test_orange_oled_logo.py" "$root/tools/armbian-image/test_orange_oled_handoff.py" "$orange_runtime_identity_test"
bash "$orange_boot_splash_test"
python3 "$orange_oled_logo_test"
python3 "$orange_oled_handoff_test"
python3 "$orange_runtime_identity_test"
python3 "$orange_constructor_test"
python3 "$orange_trusted_proof_test"
bash "$image_sanitization_test"
bash "$inspector_test"
bash "$image_mode_test"
bash "$runtime_service_test"
bash "$armbian_extensions_resolver" '' | grep -qxF 'octessera_midi octessera_image_sanitize'
bash "$armbian_extensions_resolver" preset-firstrun | grep -qxF 'preset-firstrun octessera_midi octessera_image_sanitize'
bash "$armbian_extensions_resolver" 'preset-firstrun octessera_midi' | grep -qxF 'preset-firstrun octessera_midi octessera_image_sanitize'
bash "$armbian_extensions_resolver" 'preset-firstrun,octessera_midi' | grep -qxF 'preset-firstrun,octessera_midi octessera_image_sanitize'
bash "$armbian_extensions_resolver" 'other-extension preset-firstrun' | grep -qxF 'other-extension preset-firstrun octessera_midi octessera_image_sanitize'
bash "$alsa_sequencer_test"
bash "$orange_kernel_package_test"
bash "$orange_image_proof_test"
bash "$root/tools/armbian-image/test-musical-assets.sh"
bash "$root/tools/pi-image/test-wifi-foundation.sh"
bash "$setup_layer_test"
PYTHONDONTWRITEBYTECODE=1 python3 "$setup_sidecar_test"
PYTHONDONTWRITEBYTECODE=1 python3 "$setup_request_test"
PYTHONDONTWRITEBYTECODE=1 python3 "$setup_http_test"
PYTHONDONTWRITEBYTECODE=1 python3 "$setup_flow_test"
PYTHONDONTWRITEBYTECODE=1 python3 "$setup_state_test"

for file in "${required_files[@]}"; do
  [[ -f "$file" ]] || { echo "Missing required setup file: $file" >&2; exit 1; }
done
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh
source "$spi_env_helper"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh
source "$spi_validation_helper"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh
source "$input_routing_validation_helper"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh
source "$input_routing_boot_helper"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
source "$boot_dtb_helper"
# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$inspect_mode_helper"
# shellcheck source=userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh
source "$image_mode_helper"
octessera_load_image_contract "$root/userpatches/overlay"

env_test_work="$(mktemp -d)"
run_env_case() {
  local name="$1"
  local expected_status="$2"
  local input="$3"
  local expected_output="$4"
  local extra_user_token="${5:-}"
  local input_file="$env_test_work/$name.in"
  local output_file="$env_test_work/$name.out"
  local actual_status
  printf '%s' "$input" > "$input_file"
  if octessera_armbian_env_update "$input_file" "$output_file" octessera-h618-spi1-cs0 i2c1-pi "$extra_user_token" 2>"$input_file.stderr"; then
    actual_status=0
  else
    actual_status=$?
  fi
  [[ "$actual_status" == "$expected_status" ]] || { echo "Unexpected status for Armbian environment case ${name}." >&2; exit 1; }
  if [[ "$expected_status" == 0 ]]; then
    printf '%s' "$expected_output" > "$input_file.expected"
    cmp "$input_file.expected" "$output_file"
  fi
}
run_env_case no_assign 0 $'keep=one\n' $'keep=one\nuser_overlays=octessera-h618-spi1-cs0\noverlays=i2c1-pi\n'
run_env_case existing_tokens 0 $'overlays=i2c1-pi\nuser_overlays=foo octessera-h618-spi1-cs0\n' $'overlays=i2c1-pi\nuser_overlays=foo octessera-h618-spi1-cs0\n'
run_env_case add_tokens 0 $'overlays=foo\nuser_overlays=bar\n' $'overlays=foo i2c1-pi\nuser_overlays=bar octessera-h618-spi1-cs0\n'
run_env_case duplicate_user 2 $'user_overlays=foo\nuser_overlays=bar\n' ''
run_env_case duplicate_token 2 $'user_overlays=octessera-h618-spi1-cs0 octessera-h618-spi1-cs0\n' ''
run_env_case commented_assignment 2 $'# user_overlays=user-overlay\n' ''
run_env_case inline_comment 2 $'user_overlays=foo # comment\n' ''
run_env_case malformed_assignment 2 $'user_overlays = foo\n' ''
run_env_case duplicate_i2c 2 $'overlays=i2c1-pi\noverlays=foo\n' ''
run_env_case commented_i2c 2 $'# overlays=i2c1-pi\n' ''
run_env_case malformed_i2c 2 $'overlays = foo\n' ''
run_env_case add_input_routing 0 $'user_overlays=octessera-h618-spi1-cs0\noverlays=i2c1-pi\n' $'user_overlays=octessera-h618-spi1-cs0 octessera-h618-input-routing\noverlays=i2c1-pi\n' octessera-h618-input-routing
run_env_case duplicate_input_routing 2 $'user_overlays=octessera-h618-input-routing octessera-h618-input-routing\n' '' octessera-h618-input-routing
boot_args_in="$env_test_work/boot-args.in"
boot_args_out="$env_test_work/boot-args.out"
printf '%s\n' 'extraargs=root=UUID=abc console=ttyS0,115200n8 quiet' 'keep=one' > "$boot_args_in"
octessera_remove_uart0_console_args "$boot_args_in" "$boot_args_out"
printf '%s\n' 'extraargs=root=UUID=abc quiet' 'keep=one' > "$boot_args_in.expected"
cmp "$boot_args_in.expected" "$boot_args_out"
octessera_assert_no_uart0_console_args "$boot_args_out"
printf '%s\n' '  APPEND console=ttyS0,115200n8 root=UUID=abc' > "$boot_args_in"
octessera_remove_uart0_console_args "$boot_args_in" "$boot_args_out"
printf '%s\n' '  APPEND root=UUID=abc' > "$boot_args_in.expected"
cmp "$boot_args_in.expected" "$boot_args_out"
octessera_assert_no_uart0_console_args "$boot_args_out"
[[ "$(octessera_normalize_fdt_numbers '00000008 0x0000000a deadbeef')" == '8 10 3735928559' ]] || { echo "FDT numeric normalization failed." >&2; exit 1; }
if octessera_normalize_fdt_numbers 'not-a-number' >/dev/null 2>&1; then
  echo "FDT numeric normalization accepted invalid input." >&2
  exit 1
fi
[[ "$(octessera_debugfs_mode 'Inode: 1 Type: regular Mode: 0644 Flags: 0x0')" == 0644 ]] || { echo "Debugfs 0644 mode parsing failed." >&2; exit 1; }
[[ "$(octessera_debugfs_mode 'Inode: 2 Type: regular Mode: 0100755 Flags: 0x0')" == 0755 ]] || { echo "Debugfs 0755 mode parsing failed." >&2; exit 1; }
if [[ "$(octessera_debugfs_mode 'Inode: 3 Type: regular Mode: 0104755 Flags: 0x0')" != 4755 ]]; then
  echo "Debugfs special-bit mode was not preserved for rejection." >&2
  exit 1
fi

grep -q 'wifi_connect_version=4.11.84' "$root/userpatches/customize-image.sh" || { echo "Missing pinned wifi-connect version." >&2; exit 1; }
grep -q 'wifi_connect_sha256=413d70e6d1c1366cbe2b32555e8476f3e92878178ed1b9c82205985f055f1936' "$root/userpatches/customize-image.sh" || { echo "Missing pinned wifi-connect SHA256." >&2; exit 1; }
grep -q 'network-manager.*dnsmasq.*wireless-tools.*iw' "$root/userpatches/customize-image.sh" || { echo "Orange image must install deliberate Wi-Fi dependencies." >&2; exit 1; }
grep -q 'install_overlay_file usr/local/sbin/octessera-wifi-foundation' "$root/userpatches/customize-image.sh" || { echo "Orange image must install the inactive Wi-Fi helper." >&2; exit 1; }
grep -q 'install_overlay_file etc/systemd/system/octessera-wifi-foundation.service' "$root/userpatches/customize-image.sh" || { echo "Orange image must install the inactive Wi-Fi unit." >&2; exit 1; }
! grep -q 'enable.*octessera-wifi-foundation' "$root/userpatches/customize-image.sh" || { echo "Orange image must not enable the inactive Wi-Fi unit." >&2; exit 1; }
grep -qF 'systemctl enable octessera-update-recovery.service >/dev/null' "$root/userpatches/customize-image.sh" || { echo "Image customization must enable update recovery for the next boot." >&2; exit 1; }
! grep -qE 'systemctl[[:space:]]+enable[[:space:]]+--now[[:space:]]+octessera-update-recovery\.service' "$root/userpatches/customize-image.sh" || { echo "Image customization must not start update recovery in the chroot." >&2; exit 1; }
grep -qF 'rm -f /root/.ssh/authorized_keys /home/octessera/.ssh/authorized_keys' "$root/userpatches/customize-image.sh" || { echo "Image customization must remove baked root and user authorized keys." >&2; exit 1; }
if grep -qE '(cat|read|printf|echo|grep|sha256sum).*authorized_keys|authorized_keys.*(cat|read|printf|echo|grep|sha256sum)' "$root/userpatches/customize-image.sh"; then
  echo "Image customization must not read or print authorized key contents." >&2
  exit 1
fi
grep -q 'OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w' "$root/userpatches/customize-image.sh" || { echo "Missing Orange Pi board profile metadata." >&2; exit 1; }
grep -q 'armbian_board.*orangepizero2w' "$root/userpatches/customize-image.sh" || { echo "Image customization must fail closed for non-Orange Pi boards." >&2; exit 1; }
grep -q 'device-tree-compiler' "$root/userpatches/customize-image.sh" || { echo "Image customization must provide dtc." >&2; exit 1; }
grep -q 'psmisc' "$root/userpatches/customize-image.sh" || { echo "Image customization must provide fuser through psmisc." >&2; exit 1; }
grep -q 'dtc -@ -I dts -O dtb' "$root/userpatches/customize-image.sh" || { echo "Image customization must compile the SPI overlay with symbols." >&2; exit 1; }
grep -q 'fdtoverlay' "$root/userpatches/customize-image.sh" || { echo "Image customization must merge the SPI overlay with the exact base DTB." >&2; exit 1; }
grep -q 'fdtfile' "$boot_dtb_helper" || { echo "Image customization must resolve the boot-selected DTB." >&2; exit 1; }
grep -q 'sun50i-h618-orangepi-zero2w.dtb' "$boot_dtb_helper" || { echo "Image customization must select the exact H618 base DTB." >&2; exit 1; }
! grep -q 'uname -r' "$root/userpatches/customize-image.sh" || { echo "Image customization must not infer the base DTB from uname." >&2; exit 1; }
grep -q '/boot/overlay-user' "$root/userpatches/customize-image.sh" || { echo "Image customization must install the user overlay." >&2; exit 1; }
grep -q 'user_overlays=octessera-h618-spi1-cs0' "$root/userpatches/customize-image.sh" || { echo "Image customization must enable the exact user overlay." >&2; exit 1; }
grep -qF "mv -f -- \"\$spi_dtbo_tmp\" \"\$spi_dtbo\"" "$root/userpatches/customize-image.sh" || { echo "DTBO installation must be atomic." >&2; exit 1; }
grep -qF "mv -f -- \"\$armbian_env_tmp\" \"\$armbian_env\"" "$root/userpatches/customize-image.sh" || { echo "Armbian environment installation must be atomic." >&2; exit 1; }
grep -q 'OCTESSERA_SPI1_CS0_DTS_SHA256' "$root/userpatches/customize-image.sh" || { echo "Image metadata must record the DTS hash." >&2; exit 1; }
grep -q 'OCTESSERA_SPI1_CS0_DTBO_SHA256' "$root/userpatches/customize-image.sh" || { echo "Image metadata must record the DTBO hash." >&2; exit 1; }
grep -q 'OCTESSERA_INPUT_ROUTING_DTS_SHA256' "$root/userpatches/customize-image.sh" || { echo "Image metadata must record the input-routing DTS hash." >&2; exit 1; }
grep -q 'OCTESSERA_INPUT_ROUTING_DTBO_SHA256' "$root/userpatches/customize-image.sh" || { echo "Image metadata must record the input-routing DTBO hash." >&2; exit 1; }
grep -q 'serial-getty@ttyS0.service' "$input_routing_provision" || { echo "Input-routing provisioning must manage the UART0 serial getty." >&2; exit 1; }
grep -q 'input-routing-backups' "$input_routing_provision" || { echo "Input-routing provisioning must retain rollback records." >&2; exit 1; }
grep -q 'ssh_touched=0' "$input_routing_provision" || { echo "Input-routing provisioning must record that SSH was untouched." >&2; exit 1; }
grep -q 'musb-hdrc.4.auto' "$root/tools/orange-pi/orange-pi-usb-gadget.sh" || { echo "Orange USB gadget must use the verified UDC exactly." >&2; exit 1; }
grep -q 'musb_hdrc' "$root/userpatches/overlay/etc/modules-load.d/octessera-orange-usb-gadget.conf" || { echo "Orange USB gadget module lifecycle is incomplete." >&2; exit 1; }
grep -q 'octessera-orange-usb-gadget setup' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-usb-gadget.service" || { echo "Orange USB gadget service is missing setup." >&2; exit 1; }
grep -q 'octessera-orange-usb-gadget teardown' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-usb-gadget.service" || { echo "Orange USB gadget service is missing teardown." >&2; exit 1; }
grep -q 'copy_exec /usr/local/sbin/octessera-orange-oled-logo' "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash" || { echo "Orange initramfs is missing the OLED handoff utility." >&2; exit 1; }
grep -q 'octessera-orange-oled-handoff.py' "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash" || { echo "Orange initramfs is missing the OLED handoff module." >&2; exit 1; }
! grep -q 'gpiodetect' "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash" || { echo "Orange initramfs must not use broad GPIO probing." >&2; exit 1; }
grep -q 'copy_exec /usr/bin/gpioset' "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash" || { echo "Orange initramfs is missing the fixed GPIO setter." >&2; exit 1; }
grep -q 'spi-sun6i' "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash" || { echo "Orange initramfs is missing the H618 SPI module." >&2; exit 1; }
grep -q 'system-sleep/octessera-orange-oled' "$root/userpatches/customize-image.sh" || { echo "Orange sleep OLED handoff is not installed." >&2; exit 1; }
oled_logo="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
oled_handoff="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
if grep -RInF '/usr/local/bin/octessera-orange-oled-logo' \
  "$root/userpatches/customize-image.sh" \
  "$root/userpatches/overlay/etc/initramfs-tools" \
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" \
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service" \
  "$root/userpatches/overlay/lib/systemd/system-sleep"; then
  echo "Orange OLED lifecycle must use the installed /usr/local/sbin executable." >&2
  exit 1
fi
grep -q 'Before=sysinit.target octessera.service' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must hand off before the runtime." >&2; exit 1; }
grep -q 'After=octessera.service' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service" || { echo "Orange shutdown logo must wait for runtime release." >&2; exit 1; }
grep -q 'Type=simple' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must be a persistent simple service." >&2; exit 1; }
grep -q 'ExecStart=/usr/local/sbin/octessera-orange-oled-logo boot-loop' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must use boot-loop." >&2; exit 1; }
grep -q 'RuntimeDirectory=octessera-boot' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must own the handoff runtime directory." >&2; exit 1; }
grep -q '^DevicePolicy=closed$' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must close the device policy." >&2; exit 1; }
grep -q '^DeviceAllow=/dev/spidev1.0 rw$' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must allow only the OLED SPI device." >&2; exit 1; }
grep -q '^DeviceAllow=/dev/gpiochip1 rw$' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must allow only the fixed GPIO chip." >&2; exit 1; }
grep -q '^After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target$' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must wait for fixed device dependencies." >&2; exit 1; }
! grep -q 'Conflicts=octessera.service' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo "Orange boot splash must not conflict with runtime." >&2; exit 1; }
grep -q 'Wants=octessera-orange-boot-splash.service' "$root/userpatches/overlay/etc/systemd/system/octessera.service" || { echo "Orange runtime must want the boot splash." >&2; exit 1; }
grep -q 'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' "$root/userpatches/overlay/etc/systemd/system/octessera.service" || { echo "Orange runtime must select OLED handoff v1." >&2; exit 1; }
grep -qF 'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot' "$root/userpatches/overlay/etc/systemd/system/octessera.service" || { echo "Orange runtime must retain handoff runtime-directory write access." >&2; exit 1; }
python3 - "$oled_logo" "$oled_handoff" "$root" <<'PY'
import importlib.machinery
import importlib.util
from pathlib import Path
import sys

sys.path.insert(0, str(Path(sys.argv[1]).parent))
loader = importlib.machinery.SourceFileLoader("orange_oled_logo", sys.argv[1])
spec = importlib.util.spec_from_loader(loader.name, loader)
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)
repository = Path(sys.argv[3])
assert Path(sys.argv[2]).is_file()
module.MARK_SOURCE = str(repository / "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-mark.svg")
module.WORDMARK_SOURCE = str(repository / "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-wordmark.svg")

assert module.rgb565(128) == 0x8410
source = bytearray(module.WIDTH * module.HEIGHT * 2)

def set_pixel(x, y, value):
    offset = (y * module.WIDTH + x) * 2
    source[offset:offset + 2] = value

set_pixel(0, 0, b"\x12\x34")
set_pixel(module.WIDTH - 1, 0, b"\x56\x78")
set_pixel(0, module.HEIGHT - 1, b"\x9a\xbc")
set_pixel(module.WIDTH - 1, module.HEIGHT - 1, b"\xde\xf0")
rotated = module.rotate_clockwise_rgb565(source)

def pixel(frame, x, y):
    offset = (y * module.WIDTH + x) * 2
    return bytes(frame[offset:offset + 2])

assert pixel(rotated, module.WIDTH - 1, 0) == b"\x12\x34"
assert pixel(rotated, module.WIDTH - 1, module.HEIGHT - 1) == b"\x56\x78"
assert pixel(rotated, 0, 0) == b"\x9a\xbc"
assert pixel(rotated, 0, module.HEIGHT - 1) == b"\xde\xf0"

class FakeOled:
    instance = None

    def __init__(self):
        self.initialized = False
        self.frame_payload = None
        self.display_off = None
        FakeOled.instance = self

    def initialize(self):
        self.initialized = True

    def frame(self, payload):
        self.frame_payload = payload

    def close(self, display_off=True):
        self.display_off = display_off

class FakeHandoff:
    @staticmethod
    def utility_lock(timeout_seconds):
        return FakeHandoff()

    def close(self):
        pass

module.Oled = FakeOled
module.Handoff = FakeHandoff
module.drop_to_runtime = lambda: None
module.render_canvas = lambda canvas, frame=None: b"frame"
module.run("shutdown")
assert FakeOled.instance.initialized
assert FakeOled.instance.frame_payload == b"frame"
assert FakeOled.instance.display_off is False
PY
grep -q 'octessera_install_diagnostic_payload' "$root/userpatches/customize-image.sh" || { echo "Diagnostic payload handling must remain explicit." >&2; exit 1; }
grep -qF '[--setup-layer] [--mode diagnostic|production]' "$root/tools/armbian-image/inspect-built-image.sh" || { echo "Built-image inspection must accept an explicit image mode and optional setup proof." >&2; exit 1; }
grep -qF '[--mode diagnostic|production]' "$root/tools/armbian-image/inspect-output-images.sh" || { echo "Output-image inspection must accept an explicit image mode." >&2; exit 1; }
grep -q 'artifact_kind == "diagnostic-only"' "$diagnostic_payload_helper" || { echo "Orange image payloads must be diagnostic-only." >&2; exit 1; }
grep -q 'runtime_ready == false' "$diagnostic_payload_helper" || { echo "Orange image payloads must be runtime-disabled." >&2; exit 1; }
grep -q 'enable_runtime' "$diagnostic_payload_helper" || { echo "Orange image payloads must reject runtime enablement." >&2; exit 1; }
grep -q '"image_kind"' "$root/userpatches/overlay/etc/octessera/image-contract.json" || { echo "Orange image contract must stage an explicit image_kind." >&2; exit 1; }
grep -q '^USER = "octessera"$' "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar" || { echo "Orange setup must retain the interactive octessera account." >&2; exit 1; }
! grep -q 'octessera-runtime' "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar" || { echo "Orange setup sidecar must not grant runtime-account access." >&2; exit 1; }
for service_line in \
  'ExecStart=/usr/local/bin/octessera-pi' \
  'User=octessera-runtime' \
  'Group=octessera-runtime' \
  'Environment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w' \
  'Environment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets' \
  'Environment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples' \
  'Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json' \
  'RuntimeDirectory=octessera' \
  'NoNewPrivileges=yes' \
  'ProtectSystem=strict' \
  'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot' \
  'PrivateTmp=yes' \
  'ProtectHome=yes' \
  'LimitRTPRIO=70' \
  'LimitMEMLOCK=infinity'; do
  grep -qFx "$service_line" "$root/userpatches/overlay/etc/systemd/system/octessera.service" || { echo "Orange runtime service is missing: $service_line" >&2; exit 1; }
done
! grep -qE '^(AmbientCapabilities|CapabilityBoundingSet)=|LimitRTPRIO=80' "$root/userpatches/overlay/etc/systemd/system/octessera.service" || { echo "Orange runtime service must not grant ambient SYS_NICE or priority 80." >&2; exit 1; }
expected_udev_rule=$'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"'
[[ "$(cat -- "$runtime_udev_rule")" == "$expected_udev_rule" ]] || { echo "Orange runtime udev rule content is not exact." >&2; exit 1; }
! grep -qE '^(PrivateDevices|DevicePolicy)=' "$root/userpatches/overlay/etc/systemd/system/octessera.service" || { echo "Orange runtime service must not block /dev hardware." >&2; exit 1; }
grep -qF 'default: preset-firstrun octessera_midi octessera_image_sanitize' "$root/.github/workflows/armbian-image.yml" || { echo "Orange image workflow default must include mandatory extensions." >&2; exit 1; }
[[ "$(grep -cF "extensions: \${{ inputs.extensions }}" "$root/.github/workflows/armbian-image.yml")" == 2 ]] || { echo "Both Orange image build invocations must preserve caller extensions." >&2; exit 1; }
grep -qF 'resolve-armbian-extensions.sh' "$root/.github/actions/build-armbian-image/action.yml" || { echo "Armbian build action must resolve the mandatory extension." >&2; exit 1; }
grep -qF "ENABLE_EXTENSIONS=\"\$effective_extensions\"" "$root/.github/actions/build-armbian-image/action.yml" || { echo "Effective Orange build options must include the mandatory extension." >&2; exit 1; }
grep -qF 'default: octessera_midi octessera_image_sanitize' "$root/.github/actions/build-armbian-image/action.yml" || { echo "Armbian build action default must include mandatory extensions." >&2; exit 1; }
grep -qF 'octessera_image_sanitize' "$root/.github/actions/build-armbian-image/action.yml" || { echo "Armbian build action must require image sanitization." >&2; exit 1; }
grep -q 'ARMBIAN_BOARD:.*inputs.board' "$root/.github/workflows/armbian-image.yml" || { echo "Workflow validation must receive the board input." >&2; exit 1; }
grep -q 'ARMBIAN_BUILD_REF:.*inputs.armbian_build_ref' "$root/.github/workflows/armbian-image.yml" || { echo "Workflow validation must receive the Armbian ref input." >&2; exit 1; }
grep -q 'OCTESSERA_ARMBIAN_BOARD.*orangepizero2w' "$root/.github/actions/build-armbian-image/action.yml" || { echo "Build action must reject other boards." >&2; exit 1; }
grep -q 'ARMBIAN_BUILD_REF.*40' "$root/.github/actions/build-armbian-image/action.yml" || { echo "Build action must reject mutable Armbian refs." >&2; exit 1; }
grep -q 'spi_source_path=usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts' "$root/tools/armbian-image/inspect-built-image.sh" || { echo "Built-image inspection must check the canonical DTS." >&2; exit 1; }
grep -q 'spi_dtbo_path=boot/overlay-user/octessera-h618-spi1-cs0.dtbo' "$root/tools/armbian-image/inspect-built-image.sh" || { echo "Built-image inspection must check the installed DTBO." >&2; exit 1; }
grep -q 'pins = "PH6", "PH7", "PH8";' "$spi_fixture" || { echo "H618 fixture must define the SPI1 data pins." >&2; exit 1; }
grep -q 'pins = "PH5";' "$spi_fixture" || { echo "H618 fixture must define the SPI1 CS0 pin." >&2; exit 1; }
grep -q 'function = "spi1";' "$spi_fixture" || { echo "H618 fixture must define the SPI1 pin function." >&2; exit 1; }
grep -q 'fdtget -t bx' "$spi_validation_helper" || { echo "Unchanged-property checks must use DTC-compatible byte-plus-base reads." >&2; exit 1; }
if grep -Eq 'fdtget -t b[[:space:]]' "$spi_validation_helper"; then
  echo "Unchanged-property checks must not use bare fdtget -t b." >&2
  exit 1
fi

if grep -nEi 'spi0|spi2|cs1|gpio|spidev1_0|runtime|systemd|service|authorized|ssh|password|sudo' "$spi_dts"; then
  echo "SPI1 overlay contains an unrelated bus, pin, runtime, service, or authorization change." >&2
  exit 1
fi
spi_references="$(grep -oE '&[A-Za-z0-9_]+' "$spi_dts" | sort -u)"
expected_spi_references="$(printf '%s\n' '&spi1' '&spi1_pins' '&spi1_cs0_pin' | sort -u)"
[[ "$spi_references" == "$expected_spi_references" ]] || {
  echo "SPI1 overlay references unexpected device-tree labels." >&2
  exit 1
}
[[ "$(grep -Ec '^[[:space:]]*spidev@0[[:space:]]*\{' "$spi_dts")" == 1 ]] || {
  echo "SPI1 overlay must contain exactly one CS0 child." >&2
  exit 1
}
grep -Eq '^[[:space:]]*compatible = "rohm,dh2228fv";$' "$spi_dts" || { echo "SPI1 overlay has the wrong child compatible." >&2; exit 1; }
grep -Eq '^[[:space:]]*reg = <0>;$' "$spi_dts" || { echo "SPI1 overlay must select CS0." >&2; exit 1; }
grep -Eq '^[[:space:]]*spi-max-frequency = <16000000>;$' "$spi_dts" || { echo "SPI1 overlay must cap the device at 16 MHz." >&2; exit 1; }
grep -Eq '^[[:space:]]*#address-cells = <1>;$' "$spi_dts" || { echo "SPI1 overlay must declare one address cell." >&2; exit 1; }
grep -Eq '^[[:space:]]*#size-cells = <0>;$' "$spi_dts" || { echo "SPI1 overlay must declare zero size cells." >&2; exit 1; }
if grep -nE 'spidev@[1-9]|reg = <[1-9]|target-path|cs-gpios|gpio-' "$spi_dts" "$root/userpatches/customize-image.sh"; then
  echo "SPI1 image integration contains an unexpected CS, GPIO, or fallback path." >&2
  exit 1
fi
if grep -RInE 'spidev1_0|authorized_keys|ssh_host_|BEGIN OPENSSH PRIVATE KEY|BEGIN RSA PRIVATE KEY|BEGIN PRIVATE KEY' "$root/userpatches/overlay/usr/local/share/octessera/device-tree"; then
  echo "SPI1 image integration must not contain stock spidev fallback or authorization material." >&2
  exit 1
fi
input_references="$(grep -oE '&[A-Za-z0-9_]+' "$input_routing_dts" | sort -u)"
expected_input_references="$(printf '%s\n' '&uart0' '&pio' '&octessera_uart0_released' | sort -u)"
[[ "$input_references" == "$expected_input_references" ]] || {
  printf 'Unexpected input-routing overlay references:\n%s\n' "$input_references" >&2
  exit 1
}
grep -Eq '^[[:space:]]*pins = "PH0", "PH1";$' "$input_routing_dts" || { echo "Input-routing overlay must release PH0/PH1." >&2; exit 1; }
grep -Eq '^[[:space:]]*function = "gpio_in";$' "$input_routing_dts" || { echo "Input-routing overlay must select GPIO input mode." >&2; exit 1; }
grep -q 'stdout-path = ""' "$input_routing_dts" || { echo "Input-routing overlay must clear stdout-path." >&2; exit 1; }

command -v dtc >/dev/null 2>&1 || { echo "dtc is required for Armbian overlay validation." >&2; exit 1; }
command -v fdtoverlay >/dev/null 2>&1 || { echo "fdtoverlay is required for Armbian overlay validation." >&2; exit 1; }
command -v fdtget >/dev/null 2>&1 || { echo "fdtget is required for Armbian overlay validation." >&2; exit 1; }
dt_work="$(mktemp -d)"
cleanup_validation() {
  rm -rf "${dt_work:-}" "${env_test_work:-}" "${dtb_test_work:-}" "${work:-}"
  if [[ "$welcome_overlay_preexisting" == false ]]; then
    rm -f "$welcome_overlay"
  fi
}
trap cleanup_validation EXIT
dtb_test_work="$(mktemp -d)"
setup_dtb_test_root() {
  local image_root="$1"
  mkdir -p "$image_root/boot/dtb-1/allwinner" "$image_root/usr/lib/linux-image-1/allwinner"
  printf '%s\n' fdt-base > "$image_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
  cp "$image_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb" "$image_root/usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
  : > "$image_root/boot/armbianEnv.txt"
}
assert_dtb_success() {
  local name="$1"
  local image_root="$2"
  local expected="$3"
  local actual
  if ! actual="$(octessera_resolve_boot_dtb "$image_root" 2>"$dtb_test_work/$name.stderr")"; then
    cat "$dtb_test_work/$name.stderr" >&2
    echo "Boot DTB test failed: $name." >&2
    exit 1
  fi
  [[ "$actual" == "$expected" ]] || { echo "Unexpected boot DTB for test $name: $actual." >&2; exit 1; }
}
assert_dtb_failure() {
  local name="$1"
  local image_root="$2"
  if octessera_resolve_boot_dtb "$image_root" >"$dtb_test_work/$name.out" 2>"$dtb_test_work/$name.stderr"; then
    echo "Boot DTB test unexpectedly succeeded: $name." >&2
    exit 1
  fi
}
symlink_root="$dtb_test_work/symlink"
setup_dtb_test_root "$symlink_root"
if ln -s dtb-1 "$symlink_root/boot/dtb" 2>/dev/null; then
  printf '%s\n' 'fdtfile=allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$symlink_root/boot/armbianEnv.txt"
  assert_dtb_success symlink "$symlink_root" "$(readlink -f "$symlink_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
else
  echo "Boot DTB symlink test skipped: symlinks unavailable." >&2
fi
absolute_extlinux_root="$dtb_test_work/absolute-extlinux"
setup_dtb_test_root "$absolute_extlinux_root"
mkdir -p "$absolute_extlinux_root/boot/extlinux"
printf '%s\n' 'FDT /boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$absolute_extlinux_root/boot/extlinux/extlinux.conf"
assert_dtb_success absolute_extlinux "$absolute_extlinux_root" "$(readlink -f "$absolute_extlinux_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
absolute_fdtfile_root="$dtb_test_work/absolute-fdtfile"
setup_dtb_test_root "$absolute_fdtfile_root"
printf '%s\n' 'fdtfile=/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$absolute_fdtfile_root/boot/armbianEnv.txt"
assert_dtb_success absolute_fdtfile "$absolute_fdtfile_root" "$(readlink -f "$absolute_fdtfile_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
relative_extlinux_root="$dtb_test_work/relative-extlinux"
setup_dtb_test_root "$relative_extlinux_root"
mkdir -p "$relative_extlinux_root/boot/extlinux"
if ln -s dtb-1 "$relative_extlinux_root/boot/dtb" 2>/dev/null; then
  printf '%s\n' 'FDT /dtb/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$relative_extlinux_root/boot/extlinux/extlinux.conf"
  assert_dtb_success relative_extlinux "$relative_extlinux_root" "$(readlink -f "$relative_extlinux_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
else
  echo "Relative extlinux DTB symlink test skipped: symlinks unavailable." >&2
fi
duplicate_root="$dtb_test_work/duplicate-identical"
setup_dtb_test_root "$duplicate_root"
assert_dtb_success duplicate_identical "$duplicate_root" "$(readlink -f "$duplicate_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
conflicting_root="$dtb_test_work/conflicting"
setup_dtb_test_root "$conflicting_root"
printf '%s\n' different > "$conflicting_root/usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
assert_dtb_failure conflicting "$conflicting_root"
conflicting_config_root="$dtb_test_work/conflicting-config"
setup_dtb_test_root "$conflicting_config_root"
mkdir -p "$conflicting_config_root/boot/extlinux"
printf '%s\n' 'fdtfile=/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$conflicting_config_root/boot/armbianEnv.txt"
printf '%s\n' 'FDT /usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$conflicting_config_root/boot/extlinux/extlinux.conf"
printf '%s\n' different > "$conflicting_config_root/usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
assert_dtb_failure conflicting_config "$conflicting_config_root"
missing_root="$dtb_test_work/missing"
mkdir -p "$missing_root/boot"
: > "$missing_root/boot/armbianEnv.txt"
assert_dtb_failure missing "$missing_root"
octessera_run_strict_diagnostic "$dt_work" compile_spi_overlay dtc -@ -I dts -O dtb -o "$dt_work/$spi_overlay_name.dtbo" "$spi_dts"
octessera_run_strict_diagnostic "$dt_work" inspect_spi_overlay dtc -I dtb -O dts -o "$dt_work/$spi_overlay_name.dts" "$dt_work/$spi_overlay_name.dtbo"
octessera_run_strict_diagnostic "$dt_work" compile_h618_fixture dtc -@ -I dts -O dtb -o "$dt_work/h618-spi-base.dtb" "$spi_fixture"
octessera_run_strict_diagnostic "$dt_work" merge_spi_fixture fdtoverlay -i "$dt_work/h618-spi-base.dtb" -o "$dt_work/h618-spi-merged.dtb" "$dt_work/$spi_overlay_name.dtbo"
octessera_run_dtc_inspection "$dt_work" inspect_merged_spi_fixture dtc -q -I dtb -O dts -o "$dt_work/h618-spi-merged.dts" "$dt_work/h618-spi-merged.dtb"
fixture_spi1_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ spi1)"
fixture_spi1_pins_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ spi1_pins)"
fixture_spi1_cs0_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ spi1_cs0_pin)"
fixture_spi0_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ spi0)"
fixture_i2c1_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ i2c1)"
[[ -n "$fixture_spi1_path" && -n "$fixture_spi1_pins_path" && -n "$fixture_spi1_cs0_path" && -n "$fixture_spi0_path" && -n "$fixture_i2c1_path" ]] || { echo "H618 fixture is missing required symbols." >&2; exit 1; }
if ! octessera_assert_spi1_merge "$dt_work/h618-spi-base.dtb" "$dt_work/h618-spi-merged.dtb" "$fixture_spi1_path" "$fixture_spi1_pins_path" "$fixture_spi1_cs0_path" "$fixture_spi0_path" "$fixture_i2c1_path" "fixture"; then
  echo "Fixture SPI1 merge assertions failed." >&2
  exit 1
fi
if grep -nEi 'spi0|spi2|cs1|gpio|spidev1_0|runtime|systemd|service|authorized|ssh|password|sudo' "$dt_work/$spi_overlay_name.dts"; then
  echo "Compiled SPI1 overlay contains an unrelated bus, pin, runtime, service, or authorization change." >&2
  exit 1
fi
fixup_keys="$(awk '
  /^[[:space:]]*__fixups__[[:space:]]*\{/ { inside = 1; next }
  inside && /^[[:space:]]*};/ { exit }
  inside && /^[[:space:]]*[A-Za-z0-9_]+[[:space:]]*=/ {
    line = $0
    sub(/^[[:space:]]*/, "", line)
    sub(/[[:space:]]*=.*/, "", line)
    print line
  }
' "$dt_work/$spi_overlay_name.dts" | sort)"
expected_fixup_keys="$(printf '%s\n' spi1 spi1_cs0_pin spi1_pins | sort)"
[[ "$fixup_keys" == "$expected_fixup_keys" ]] || {
  printf 'Unexpected SPI1 overlay fixups:\n%s\n' "$fixup_keys" >&2
  exit 1
}
grep -Eq '^[[:space:]]*spi1 = "/fragment@0:target:0";$' "$dt_work/$spi_overlay_name.dts" || { echo "SPI1 target fixup is wrong." >&2; exit 1; }
grep -Eq '^[[:space:]]*spi1_pins = "/fragment@0/__overlay__:pinctrl-0:0";$' "$dt_work/$spi_overlay_name.dts" || { echo "SPI1 data pin fixup is wrong." >&2; exit 1; }
grep -Eq '^[[:space:]]*spi1_cs0_pin = "/fragment@0/__overlay__:pinctrl-0:4";$' "$dt_work/$spi_overlay_name.dts" || { echo "SPI1 CS0 pin fixup is wrong." >&2; exit 1; }
! grep -q '__local_fixups__' "$dt_work/$spi_overlay_name.dts" || { echo "SPI1 overlay has unexpected local fixups." >&2; exit 1; }
[[ "$(grep -Ec '^[[:space:]]*spidev@0[[:space:]]*\{' "$dt_work/$spi_overlay_name.dts")" == 1 ]] || { echo "Compiled SPI1 overlay must contain one CS0 child." >&2; exit 1; }
grep -Eq 'compatible = "rohm,dh2228fv";' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay has the wrong compatible." >&2; exit 1; }
grep -Eq 'reg = <(0x)?0+>;' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay must select CS0." >&2; exit 1; }
grep -Eq 'spi-max-frequency = (<0xf42400>|<16000000>);' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay must cap the device at 16 MHz." >&2; exit 1; }
grep -q 'pinctrl-names = "default";' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing its pinctrl name." >&2; exit 1; }
grep -q 'pinctrl-0 =' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing its pinctrl group." >&2; exit 1; }
grep -Eq '#address-cells = <0x0*1>;' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing one address cell." >&2; exit 1; }
grep -Eq '#size-cells = <0x0+>;' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing zero size cells." >&2; exit 1; }
octessera_run_strict_diagnostic "$dt_work" compile_input_routing_overlay dtc -@ -I dts -O dtb -o "$dt_work/octessera-h618-input-routing.dtbo" "$input_routing_dts"
octessera_run_strict_diagnostic "$dt_work" inspect_input_routing_overlay dtc -I dtb -O dts -o "$dt_work/octessera-h618-input-routing.dts" "$dt_work/octessera-h618-input-routing.dtbo"
octessera_run_strict_diagnostic "$dt_work" merge_input_routing_fixture fdtoverlay -i "$dt_work/h618-spi-base.dtb" -o "$dt_work/h618-input-routing-merged.dtb" "$dt_work/octessera-h618-input-routing.dtbo"
octessera_run_dtc_inspection "$dt_work" inspect_merged_input_routing_fixture dtc -q -I dtb -O dts -o "$dt_work/h618-input-routing-merged.dts" "$dt_work/h618-input-routing-merged.dtb"
fixture_uart0_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ uart0)"
fixture_pio_path="$(fdtget -t s "$dt_work/h618-spi-base.dtb" /__symbols__ pio)"
[[ -n "$fixture_uart0_path" && -n "$fixture_pio_path" ]] || { echo "H618 fixture is missing UART0 or pinctrl symbols." >&2; exit 1; }
octessera_assert_input_routing_merge "$dt_work/h618-spi-base.dtb" "$dt_work/h618-input-routing-merged.dtb" "$fixture_uart0_path" "$fixture_pio_path" /chosen fixture

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "$root/userpatches/customize-image.sh" "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash" "$armbian_extensions_resolver" "$image_sanitization_extension" "$image_sanitization_test" "$inspector_test" "$orange_boot_splash_test" "$alsa_sequencer_extension" "$alsa_sequencer_test" "$orange_kernel_package_test" "$orange_kernel_package_validator" "$orange_image_proof_test" "$orange_image_proof_verifier" "$root/tools/armbian-image/inspect-built-image.sh" "$runtime_inspector" "$image_mode_helper" "$diagnostic_payload_helper" "$image_mode_test" "$authorized_key_paths_helper" "$inspect_path_helper" "$root/tools/armbian-image/inspect-mode.sh" "$root/tools/armbian-image/inspect-output-images.sh" "$root/tools/armbian-image/stage-musical-assets.sh" "$root/tools/armbian-image/test-musical-assets.sh" "$root/tools/pi-image/test-wifi-foundation.sh" "$root/tools/orange-pi/input-routing-provision.sh" "$root/tools/orange-pi/orange-pi-usb-gadget.sh" "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget" "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default" "$root/userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh" "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect" "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation" "$root/userpatches/overlay/usr/local/sbin/octessera-update" "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard" "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery" "$0"
  shellcheck "$runtime_service_test"
  shellcheck "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
  shellcheck "$setup_layer_proof" "$setup_layer_test" "$setup_start" "$setup_cleanup" "$setup_request_cleanup"
  shellcheck "$setup_layer_installer"
fi

cmp "$root/tools/device-update/octessera-update" "$root/userpatches/overlay/usr/local/sbin/octessera-update"
cmp "$root/tools/device-update/octessera-update-guard" "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
cmp "$root/tools/device-update/octessera-update-recovery" "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
if grep -Eq 'octessera-update-(guard|recovery)' "$root/userpatches/overlay/etc/sudoers.d/octessera-update"; then
  echo "Updater guard internals must not be present in sudoers." >&2
  exit 1
fi
if grep -q '^ConditionPathExists=' "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"; then
  echo "Updater recovery must run once per boot, not only when a transaction file exists." >&2
  exit 1
fi
if [[ "$OCTESSERA_IMAGE_MODE" == diagnostic ]] && find "$root/userpatches/overlay" -type f -name 'octessera-pi' | grep -q .; then
  echo "Diagnostic Orange image overlay must not carry a runtime binary." >&2
  exit 1
fi

if command -v python3 >/dev/null 2>&1; then
  PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY' "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
compile(path.read_text(encoding="utf-8"), str(path), "exec")
PY
  PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test_setup_sidecar.py"
  python3 - <<'PY' "$root/.github/workflows/armbian-image.yml"
import sys
try:
    import yaml
except Exception:
    sys.exit(0)
with open(sys.argv[1], 'r', encoding='utf-8') as handle:
    yaml.safe_load(handle)
PY
fi

if command -v node >/dev/null 2>&1; then
  node --check "$root/userpatches/overlay/usr/local/share/octessera-setup-ui/app.js"
fi

if command -v actionlint >/dev/null 2>&1; then
  actionlint "$root/.github/workflows/armbian-image.yml"
fi

for path in "$root/userpatches/overlay" "$root/.github/workflows/armbian-image.yml"; do
  if grep -RInE --exclude-dir=doc '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|g_mass_storage|wpa_passphrase|BEGIN OPENSSH PRIVATE KEY|BEGIN RSA PRIVATE KEY|BEGIN PRIVATE KEY|default_password|changeme|raspberry)' "$path"; then
    echo "Forbidden Raspberry Pi assumption or secret-like pattern found under $path" >&2
    exit 1
  fi
done

if find "$root/userpatches/overlay" -path '*/.ssh/authorized_keys' -o -name 'ssh_host_*' | grep -q .; then
  echo "Overlay must not bake SSH keys or authorized keys." >&2
  exit 1
fi

if grep -nE '^      (wifi|wi-fi|password|ssh_key|private_key|authorized_keys|user):' "$root/.github/workflows/armbian-image.yml"; then
  echo "Workflow must not expose raw first-run secret inputs." >&2
  exit 1
fi

payload_url="${PAYLOAD_URL:-${OCTESSERA_PAYLOAD_URL:-}}"
payload_sha256="${PAYLOAD_SHA256:-${OCTESSERA_PAYLOAD_SHA256:-}}"
if [[ "$OCTESSERA_IMAGE_MODE" == production && ( -n "$payload_url" || -n "$payload_sha256" ) ]]; then
  echo "Production Orange images do not accept payload URLs or payload hashes." >&2
  exit 1
elif [[ -n "$payload_url" ]]; then
  [[ "$payload_url" == https://* ]] || { echo "Payload URL must use HTTPS." >&2; exit 1; }
  [[ "$payload_sha256" =~ ^[a-fA-F0-9]{64}$ ]] || { echo "Payload SHA256 is required and must be 64 hex characters." >&2; exit 1; }
  work="$(mktemp -d)"
  curl --fail --location --proto '=https' --tlsv1.2 --output "$work/payload.tar" "$payload_url"
  echo "$payload_sha256  $work/payload.tar" | sha256sum -c -
  inspect_payload_tar "$work/payload.tar"
elif [[ -n "$payload_sha256" ]]; then
  echo "Payload URL is required when payload SHA256 is set." >&2
  exit 1
fi

preset_url="${PUBLIC_PRESET_CONFIGURATION_URL:-}"
if [[ -n "$preset_url" ]]; then
  [[ "$preset_url" == https://* ]] || { echo "Public PRESET_CONFIGURATION URL must use HTTPS." >&2; exit 1; }
  case " ${ARMBIAN_EXTENSIONS:-} " in
    *" preset-firstrun "*) ;;
    *) echo "PRESET_CONFIGURATION requires the preset-firstrun extension." >&2; exit 1 ;;
  esac
fi

echo "Armbian image validation passed."
