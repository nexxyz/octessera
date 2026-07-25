#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
spi_dts="$root/userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts"
spi_env_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh"
spi_validation_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh"
boot_dtb_helper="$root/userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
inspect_mode_helper="$root/tools/armbian-image/inspect-mode.sh"
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
  "$root/tools/armbian-image/inspect-output-images.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
  "$root/userpatches/overlay/etc/sudoers.d/octessera-update"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-guard.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"
  "$root/userpatches/overlay/etc/systemd/system/octessera-setup.service"
  "$spi_dts"
  "$spi_env_helper"
  "$spi_validation_helper"
  "$boot_dtb_helper"
  "$inspect_mode_helper"
  "$spi_fixture"
)

bash -n "$root/userpatches/customize-image.sh"
bash -n "$root/tools/armbian-image/inspect-built-image.sh"
bash -n "$root/tools/armbian-image/inspect-output-images.sh"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
bash -n "$spi_env_helper"
bash -n "$spi_validation_helper"
bash -n "$boot_dtb_helper"
bash -n "$inspect_mode_helper"
python3 -m py_compile "$root/tools/device-update/updater_protocol.py" "$root/tools/device-update/updater_state.py" "$root/tools/device-update/updater_assets.py" "$root/tools/device-update/updater_guard.py" "$root/tools/device-update/updater_cli.py"

for file in "${required_files[@]}"; do
  [[ -f "$file" ]] || { echo "Missing required setup file: $file" >&2; exit 1; }
done
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh
source "$spi_env_helper"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh
source "$spi_validation_helper"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
source "$boot_dtb_helper"
# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$inspect_mode_helper"

env_test_work="$(mktemp -d)"
run_env_case() {
  local name="$1"
  local expected_status="$2"
  local input="$3"
  local expected_output="$4"
  local input_file="$env_test_work/$name.in"
  local output_file="$env_test_work/$name.out"
  local actual_status
  printf '%s' "$input" > "$input_file"
  if octessera_armbian_env_update "$input_file" "$output_file" octessera-h618-spi1-cs0 i2c1-pi 2>"$input_file.stderr"; then
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
grep -q 'artifact_kind == "diagnostic-only"' "$root/userpatches/customize-image.sh" || { echo "Orange image payloads must be diagnostic-only." >&2; exit 1; }
grep -q 'runtime_ready == false' "$root/userpatches/customize-image.sh" || { echo "Orange image payloads must be runtime-disabled." >&2; exit 1; }
grep -q 'enable_runtime' "$root/userpatches/customize-image.sh" || { echo "Orange image payloads must reject runtime enablement." >&2; exit 1; }
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
grep -Eq '^[[:space:]]*spi-max-frequency = <1000000>;$' "$spi_dts" || { echo "SPI1 overlay must cap the device at 1 MHz." >&2; exit 1; }
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

command -v dtc >/dev/null 2>&1 || { echo "dtc is required for Armbian overlay validation." >&2; exit 1; }
command -v fdtoverlay >/dev/null 2>&1 || { echo "fdtoverlay is required for Armbian overlay validation." >&2; exit 1; }
command -v fdtget >/dev/null 2>&1 || { echo "fdtget is required for Armbian overlay validation." >&2; exit 1; }
dt_work="$(mktemp -d)"
cleanup_validation() {
  rm -rf "${dt_work:-}" "${env_test_work:-}" "${dtb_test_work:-}" "${work:-}"
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
octessera_run_strict_diagnostic "$dt_work" inspect_merged_spi_fixture dtc -I dtb -O dts -o "$dt_work/h618-spi-merged.dts" "$dt_work/h618-spi-merged.dtb"
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
grep -Eq 'spi-max-frequency = (<0x0*f4240>|<1000000>);' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay must cap the device at 1 MHz." >&2; exit 1; }
grep -q 'pinctrl-names = "default";' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing its pinctrl name." >&2; exit 1; }
grep -q 'pinctrl-0 =' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing its pinctrl group." >&2; exit 1; }
grep -Eq '#address-cells = <0x0*1>;' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing one address cell." >&2; exit 1; }
grep -Eq '#size-cells = <0x0+>;' "$dt_work/$spi_overlay_name.dts" || { echo "Compiled SPI1 overlay is missing zero size cells." >&2; exit 1; }

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "$root/userpatches/customize-image.sh" "$root/tools/armbian-image/inspect-built-image.sh" "$root/tools/armbian-image/inspect-mode.sh" "$root/tools/armbian-image/inspect-output-images.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh" "$root/userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh" "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect" "$root/userpatches/overlay/usr/local/sbin/octessera-update" "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard" "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery" "$0"
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
if find "$root/userpatches/overlay" -type f \( -name 'octessera.service' -o -name 'octessera-pi' \) | grep -q .; then
  echo "Orange image overlay must not carry a runtime service or binary." >&2
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
  if grep -RInE '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|usb[_-]?gadget|g_mass_storage|wpa_passphrase|BEGIN OPENSSH PRIVATE KEY|BEGIN RSA PRIVATE KEY|BEGIN PRIVATE KEY|default_password|changeme|raspberry)' "$path"; then
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
if [[ -n "$payload_url" ]]; then
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
