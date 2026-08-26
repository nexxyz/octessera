#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
device_tree_root="$root/userpatches/overlay/usr/local/share/octessera/device-tree"
spi_dts="$device_tree_root/octessera-h618-spi1-oled-sd2.dts"
input_dts="$device_tree_root/octessera-h618-input-routing.dts"
audio_dts="$device_tree_root/octessera-ahub0-pcm5102.dts"
spi_fixture="$root/tools/armbian-image/fixtures/h618-spi-base.dts"
spi_name=octessera-h618-spi1-oled-sd2
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=tools/armbian-image/inspect-path.sh
source "$root/tools/armbian-image/inspect-path.sh"
source "$root/tools/armbian-image/inspect-mode.sh"

for command in dtc fdtoverlay fdtget; do
  command -v "$command" >/dev/null 2>&1 || { echo "$command is required for Armbian device-tree validation." >&2; exit 1; }
done

# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh
source "$device_tree_root/armbian-env-token.sh"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh
source "$device_tree_root/spi-overlay-validation.sh"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh
source "$device_tree_root/input-routing-overlay-validation.sh"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh
source "$device_tree_root/input-routing-boot-config.sh"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
source "$device_tree_root/boot-dtb-selection.sh"
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/orange-ahub-overlay-validation.sh
source "$device_tree_root/orange-ahub-overlay-validation.sh"

env_work="$work/env"
mkdir -p "$env_work"
run_env_case() {
  local name="$1" expected="$2" input="$3" output="$4" extra="${5:-octessera-h618-input-routing}"
  local actual input_file="$env_work/$name.in" output_file="$env_work/$name.out"
  printf '%s' "$input" > "$input_file"
  if octessera_armbian_env_update "$input_file" "$output_file" octessera-h618-spi1-oled-sd2 i2c1-pi "$extra" octessera-ahub0-pcm5102 1 2>"$input_file.stderr"; then actual=0; else actual=$?; fi
  [[ "$actual" == "$expected" ]] || { echo "Unexpected status for Armbian environment case $name." >&2; exit 1; }
  if [[ "$expected" == 0 ]]; then printf '%s' "$output" > "$input_file.expected"; cmp "$input_file.expected" "$output_file"; fi
}
run_env_case no_assign 0 $'keep=one\n' $'keep=one\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102\noverlays=i2c1-pi\n'
run_env_case existing_tokens 0 $'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102\n' $'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102\n'
run_env_case extra_user_token 2 $'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102 extra\n' ''
run_env_case extra_overlay_token 2 $'overlays=i2c1-pi spidev1_0\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102\n' ''
run_env_case missing_audio_token 2 $'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing\n' ''
run_env_case duplicate_audio_token 2 $'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102 octessera-ahub0-pcm5102\n' ''
run_env_case wrong_user_order 2 $'overlays=i2c1-pi\nuser_overlays=octessera-ahub0-pcm5102 octessera-h618-spi1-oled-sd2 octessera-h618-input-routing\n' ''
run_env_case duplicate_user 2 $'user_overlays=foo\nuser_overlays=bar\n' ''
run_env_case duplicate_token 2 $'user_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-spi1-oled-sd2\n' ''
run_env_case commented_assignment 2 $'# user_overlays=user-overlay\n' ''
run_env_case inline_comment 2 $'user_overlays=foo # comment\n' ''
run_env_case malformed_assignment 2 $'user_overlays = foo\n' ''
run_env_case duplicate_i2c 2 $'overlays=i2c1-pi\noverlays=foo\n' ''
run_env_case commented_i2c 2 $'# overlays=i2c1-pi\n' ''
run_env_case malformed_i2c 2 $'overlays = foo\n' ''
run_env_case add_input_routing 2 $'user_overlays=octessera-h618-spi1-oled-sd2\noverlays=i2c1-pi\n' '' octessera-h618-input-routing
run_env_case duplicate_input_routing 2 $'user_overlays=octessera-h618-input-routing octessera-h618-input-routing\n' '' octessera-h618-input-routing

printf '%s\n' 'extraargs=root=UUID=abc console=ttyS0,115200n8 quiet' 'keep=one' > "$env_work/boot.in"
octessera_remove_uart0_console_args "$env_work/boot.in" "$env_work/boot.out"
printf '%s\n' 'extraargs=root=UUID=abc quiet' 'keep=one' > "$env_work/boot.expected"
cmp "$env_work/boot.expected" "$env_work/boot.out"
octessera_assert_no_uart0_console_args "$env_work/boot.out"
printf '%s\n' 'verbosity=1' 'console=both' > "$env_work/console.in"
octessera_set_armbian_display_console "$env_work/console.in" "$env_work/console.out"
printf '%s\n' 'verbosity=1' 'console=display' > "$env_work/console.expected"
cmp "$env_work/console.expected" "$env_work/console.out"
printf '%s\n' 'verbosity=1' > "$env_work/console-missing.in"
octessera_set_armbian_display_console "$env_work/console-missing.in" "$env_work/console-missing.out"
grep -qxF 'console=display' "$env_work/console-missing.out"
printf '%s\n' 'console=display' 'console=display' > "$env_work/console-duplicate.in"
if octessera_set_armbian_display_console "$env_work/console-duplicate.in" "$env_work/console-duplicate.out"; then
  echo 'Armbian console helper accepted duplicate console assignments.' >&2
  exit 1
fi
printf '%s\n' '  APPEND console=ttyS0,115200n8 root=UUID=abc' > "$env_work/append.in"
octessera_remove_uart0_console_args "$env_work/append.in" "$env_work/append.out"
grep -qxF '  APPEND root=UUID=abc' "$env_work/append.out"
octessera_assert_no_uart0_console_args "$env_work/append.out"
[[ "$(octessera_normalize_fdt_numbers '00000008 0x0000000a deadbeef')" == '8 10 3735928559' ]]
if octessera_normalize_fdt_numbers not-a-number >/dev/null 2>&1; then echo 'FDT numeric normalization accepted invalid input.' >&2; exit 1; fi
[[ "$(octessera_debugfs_mode 'Inode: 1 Type: regular Mode: 0644 Flags: 0x0')" == 0644 ]]
[[ "$(octessera_debugfs_mode 'Inode: 2 Type: regular Mode: 0100755 Flags: 0x0')" == 0755 ]]
[[ "$(octessera_debugfs_mode 'Inode: 3 Type: regular Mode: 0104755 Flags: 0x0')" == 4755 ]]

grep -q 'pins = "PH6", "PH7", "PH8";' "$spi_fixture"
grep -q 'pins = "PH5";' "$spi_fixture"
grep -q 'function = "spi1";' "$spi_fixture"
grep -q 'fdtget -t bx' "$device_tree_root/spi-overlay-validation.sh"
octessera_reject_file_match 'Unchanged-property checks must not use bare fdtget -t b.' -qE 'fdtget -t b[[:space:]]' "$device_tree_root/spi-overlay-validation.sh"
boot_dtb_helper="$device_tree_root/boot-dtb-selection.sh"
grep -q 'fdtfile' "$boot_dtb_helper"
grep -q 'sun50i-h618-orangepi-zero2w.dtb' "$boot_dtb_helper"
octessera_reject_file_match 'Image customization must not infer the base DTB from uname.' -q 'uname -r' "$root/userpatches/customize-image.sh"
grep -q '/boot/overlay-user' "$root/userpatches/customize-image.sh"
grep -q 'user_overlays=octessera-h618-spi1-oled-sd2' "$root/userpatches/customize-image.sh"

spi_references="$(grep -oE '&[A-Za-z0-9_]+' "$spi_dts" | sort -u)"
expected_spi_references="$(printf '%s\n' '&pio' '&spi1' '&spi1_pins' '&spi1_cs0_pin' '&spi1_cs1_pin' | sort -u)"
[[ "$spi_references" == "$expected_spi_references" ]] || { echo 'SPI1 overlay references unexpected device-tree labels.' >&2; exit 1; }
[[ "$(grep -Ec '^[[:space:]]*spidev@0[[:space:]]*\{' "$spi_dts")" == 1 ]]
grep -Eq '^[[:space:]]*compatible = "rohm,dh2228fv";$' "$spi_dts"
grep -Eq '^[[:space:]]*reg = <0>;$' "$spi_dts"
grep -Eq '^[[:space:]]*spi-max-frequency = <16000000>;$' "$spi_dts"
grep -Eq '^[[:space:]]*mmc@1[[:space:]]*\{$' "$spi_dts"
grep -Eq '^[[:space:]]*compatible = "mmc-spi-slot";$' "$spi_dts"
grep -Eq '^[[:space:]]*reg = <1>;$' "$spi_dts"
grep -Eq '^[[:space:]]*spi-max-frequency = <10000000>;$' "$spi_dts"
grep -Eq '^[[:space:]]*voltage-ranges = <3300 3300>;$' "$spi_dts"
grep -Eq '^[[:space:]]*#address-cells = <1>;$' "$spi_dts"
grep -Eq '^[[:space:]]*#size-cells = <0>;$' "$spi_dts"
octessera_reject_file_match 'SPI1 overlay contains an unrelated bus, runtime, service, or authorization change.' -nEi 'spi0|spi2|gpio|spidev1_0|runtime|systemd|service|authorized|ssh|password|sudo' "$spi_dts"
octessera_reject_file_match 'SPI1 image integration contains an unexpected CS, GPIO, or fallback path.' -nE 'spidev@[1-9]|reg = <[2-9]|target-path|cs-gpios|gpio-' "$spi_dts" "$root/userpatches/customize-image.sh"
input_references="$(grep -oE '&[A-Za-z0-9_]+' "$input_dts" | sort -u)"
expected_input_references="$(printf '%s\n' '&uart0' '&pio' '&octessera_uart0_released' | sort -u)"
[[ "$input_references" == "$expected_input_references" ]]
grep -Eq '^[[:space:]]*pins = "PH0", "PH1";$' "$input_dts"
grep -Eq '^[[:space:]]*function = "gpio_in";$' "$input_dts"
grep -q 'stdout-path = ""' "$input_dts"

dtb_work="$work/dtb"
setup_dtb_root() {
  local image_root="$1"
  mkdir -p "$image_root/boot/dtb-1/allwinner" "$image_root/usr/lib/linux-image-1/allwinner"
  printf '%s\n' fdt-base > "$image_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
  cp "$image_root/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb" "$image_root/usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
  : > "$image_root/boot/armbianEnv.txt"
}
assert_dtb_success() {
  local name="$1" image_root="$2" expected="$3" actual
  if ! actual="$(octessera_resolve_boot_dtb "$image_root" 2>"$dtb_work/$name.stderr")"; then cat "$dtb_work/$name.stderr" >&2; exit 1; fi
  [[ "$actual" == "$expected" ]] || { echo "Unexpected boot DTB for test $name: $actual." >&2; exit 1; }
}
assert_dtb_failure() {
  local name="$1" image_root="$2"
  if octessera_resolve_boot_dtb "$image_root" >"$dtb_work/$name.out" 2>"$dtb_work/$name.stderr"; then echo "Boot DTB test unexpectedly succeeded: $name." >&2; exit 1; fi
}
for name in symlink absolute-extlinux absolute-fdtfile relative-extlinux duplicate-identical conflicting conflicting-config missing; do mkdir -p "$dtb_work/$name"; done
setup_dtb_root "$dtb_work/symlink"
if ln -s dtb-1 "$dtb_work/symlink/boot/dtb" 2>/dev/null; then
  printf '%s\n' 'fdtfile=allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$dtb_work/symlink/boot/armbianEnv.txt"
  assert_dtb_success symlink "$dtb_work/symlink" "$(readlink -f "$dtb_work/symlink/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
fi
setup_dtb_root "$dtb_work/absolute-extlinux"
mkdir -p "$dtb_work/absolute-extlinux/boot/extlinux"
printf '%s\n' 'FDT /boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$dtb_work/absolute-extlinux/boot/extlinux/extlinux.conf"
assert_dtb_success absolute-extlinux "$dtb_work/absolute-extlinux" "$(readlink -f "$dtb_work/absolute-extlinux/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
setup_dtb_root "$dtb_work/absolute-fdtfile"
printf '%s\n' 'fdtfile=/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$dtb_work/absolute-fdtfile/boot/armbianEnv.txt"
assert_dtb_success absolute-fdtfile "$dtb_work/absolute-fdtfile" "$(readlink -f "$dtb_work/absolute-fdtfile/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
setup_dtb_root "$dtb_work/relative-extlinux"
mkdir -p "$dtb_work/relative-extlinux/boot/extlinux"
if ln -s dtb-1 "$dtb_work/relative-extlinux/boot/dtb" 2>/dev/null; then
  printf '%s\n' 'FDT /dtb/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$dtb_work/relative-extlinux/boot/extlinux/extlinux.conf"
  assert_dtb_success relative-extlinux "$dtb_work/relative-extlinux" "$(readlink -f "$dtb_work/relative-extlinux/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
fi
setup_dtb_root "$dtb_work/duplicate-identical"
assert_dtb_success duplicate-identical "$dtb_work/duplicate-identical" "$(readlink -f "$dtb_work/duplicate-identical/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb")"
setup_dtb_root "$dtb_work/conflicting"
printf '%s\n' different > "$dtb_work/conflicting/usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
assert_dtb_failure conflicting "$dtb_work/conflicting"
setup_dtb_root "$dtb_work/conflicting-config"
mkdir -p "$dtb_work/conflicting-config/boot/extlinux"
printf '%s\n' 'fdtfile=/boot/dtb-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$dtb_work/conflicting-config/boot/armbianEnv.txt"
printf '%s\n' 'FDT /usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb' > "$dtb_work/conflicting-config/boot/extlinux/extlinux.conf"
printf '%s\n' different > "$dtb_work/conflicting-config/usr/lib/linux-image-1/allwinner/sun50i-h618-orangepi-zero2w.dtb"
assert_dtb_failure conflicting-config "$dtb_work/conflicting-config"
mkdir -p "$dtb_work/missing/boot"
: > "$dtb_work/missing/boot/armbianEnv.txt"
assert_dtb_failure missing "$dtb_work/missing"

octessera_run_strict_diagnostic "$work" compile_spi_overlay dtc -@ -I dts -O dtb -o "$work/$spi_name.dtbo" "$spi_dts"
octessera_run_strict_diagnostic "$work" inspect_spi_overlay dtc -I dtb -O dts -o "$work/$spi_name.dts" "$work/$spi_name.dtbo"
octessera_run_strict_diagnostic "$work" compile_h618_fixture dtc -@ -I dts -O dtb -o "$work/h618-spi-base.dtb" "$spi_fixture"
octessera_run_strict_diagnostic "$work" merge_spi_fixture fdtoverlay -i "$work/h618-spi-base.dtb" -o "$work/h618-spi-merged.dtb" "$work/$spi_name.dtbo"
octessera_run_dtc_inspection "$work" inspect_merged_spi_fixture dtc -q -I dtb -O dts -o "$work/h618-spi-merged.dts" "$work/h618-spi-merged.dtb"
fixture_spi1_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ spi1)"
fixture_spi1_pins_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ spi1_pins)"
fixture_spi1_cs0_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ spi1_cs0_pin)"
fixture_spi1_cs1_path="$(fdtget -t s "$work/h618-spi-merged.dtb" /__symbols__ spi1_cs1_pin)"
fixture_spi0_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ spi0)"
fixture_i2c1_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ i2c1)"
[[ -n "$fixture_spi1_path" && -n "$fixture_spi1_pins_path" && -n "$fixture_spi1_cs0_path" && -n "$fixture_spi1_cs1_path" && -n "$fixture_spi0_path" && -n "$fixture_i2c1_path" ]]
octessera_assert_spi1_merge "$work/h618-spi-base.dtb" "$work/h618-spi-merged.dtb" "$fixture_spi1_path" "$fixture_spi1_pins_path" "$fixture_spi1_cs0_path" "$fixture_spi1_cs1_path" "$fixture_spi0_path" "$fixture_i2c1_path" fixture
octessera_reject_file_match 'Compiled SPI1 overlay contains an unrelated bus, runtime, service, or authorization change.' -nEi 'spi0|spi2|gpio|spidev1_0|runtime|systemd|service|authorized|ssh|password|sudo' "$work/$spi_name.dts"
fixup_keys="$(awk '/^[[:space:]]*__fixups__[[:space:]]*\{/ { inside=1; next } inside && /^[[:space:]]*};/ { exit } inside && /^[[:space:]]*[A-Za-z0-9_]+[[:space:]]*=/{ line=$0; sub(/^[[:space:]]*/,"",line); sub(/[[:space:]]*=.*/,"",line); print line }' "$work/$spi_name.dts" | sort)"
[[ "$fixup_keys" == "$(printf '%s\n' pio spi1 spi1_cs0_pin spi1_pins | sort)" ]]
grep -Eq '^[[:space:]]*spi1 = "/fragment@1:target:0";$' "$work/$spi_name.dts"
grep -Eq '^[[:space:]]*spi1_pins = "/fragment@1/__overlay__:pinctrl-0:0";$' "$work/$spi_name.dts"
grep -Eq '^[[:space:]]*spi1_cs0_pin = "/fragment@1/__overlay__:pinctrl-0:4";$' "$work/$spi_name.dts"
grep -Eq '^[[:space:]]*__local_fixups__[[:space:]]*\{$' "$work/$spi_name.dts"
grep -Eq '^[[:space:]]*pinctrl-0 = <0x08>;$' "$work/$spi_name.dts"
[[ "$(grep -Ec '^[[:space:]]*spidev@0[[:space:]]*\{' "$work/$spi_name.dts")" == 1 ]]
grep -Eq 'compatible = "rohm,dh2228fv";' "$work/$spi_name.dts"
grep -Eq 'reg = <(0x)?0+>;' "$work/$spi_name.dts"
grep -Eq 'spi-max-frequency = (<0xf42400>|<16000000>);' "$work/$spi_name.dts"
grep -q 'pinctrl-names = "default";' "$work/$spi_name.dts"
grep -q 'pinctrl-0 =' "$work/$spi_name.dts"
grep -Eq '#address-cells = <0x0*1>;' "$work/$spi_name.dts"
grep -Eq '#size-cells = <0x0+>;' "$work/$spi_name.dts"

octessera_run_strict_diagnostic "$work" compile_input_routing_overlay dtc -@ -I dts -O dtb -o "$work/octessera-h618-input-routing.dtbo" "$input_dts"
octessera_run_strict_diagnostic "$work" inspect_input_routing_overlay dtc -I dtb -O dts -o "$work/octessera-h618-input-routing.dts" "$work/octessera-h618-input-routing.dtbo"
octessera_run_strict_diagnostic "$work" merge_input_routing_fixture fdtoverlay -i "$work/h618-spi-base.dtb" -o "$work/h618-input-routing-merged.dtb" "$work/octessera-h618-input-routing.dtbo"
octessera_run_dtc_inspection "$work" inspect_merged_input_routing_fixture dtc -q -I dtb -O dts -o "$work/h618-input-routing-merged.dts" "$work/h618-input-routing-merged.dtb"
fixture_uart0_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ uart0)"
fixture_pio_path="$(fdtget -t s "$work/h618-spi-base.dtb" /__symbols__ pio)"
[[ -n "$fixture_uart0_path" && -n "$fixture_pio_path" ]]
octessera_run_strict_diagnostic "$work" compile_audio_overlay dtc -@ -I dts -O dtb -o "$work/octessera-ahub0-pcm5102.dtbo" "$audio_dts"
octessera_run_strict_diagnostic "$work" compile_audio_fixture dtc -@ -I dts -O dtb -o "$work/h618-audio-base.dtb" "$root/tools/armbian-image/fixtures/h618-orange-ahub-base.dts"
octessera_run_strict_diagnostic "$work" merge_audio_fixture fdtoverlay -i "$work/h618-audio-base.dtb" -o "$work/h618-audio-merged.dtb" "$work/octessera-ahub0-pcm5102.dtbo"
octessera_run_dtc_inspection "$work" inspect_audio_fixture dtc -q -I dtb -O dts -o "$work/h618-audio-merged.dts" "$work/h618-audio-merged.dtb"
grep -q 'soundcard-mach,name = "octessera-dac"' "$work/h618-audio-merged.dts"
octessera_reject_file_match 'Canonical AHUB0 overlay must not claim a PCM5102A codec or MCLK.' -Eiq 'pcm5102a|mclk|sound-dai[[:space:]]*=.*codec' "$audio_dts"
octessera_assert_input_routing_merge "$work/h618-spi-base.dtb" "$work/h618-input-routing-merged.dtb" "$fixture_uart0_path" "$fixture_pio_path" /chosen fixture
