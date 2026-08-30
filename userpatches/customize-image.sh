#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

overlay_dir=/tmp/overlay

armbian_board="$(awk -F= '$1 == "BOARD" { print $2; exit }' /etc/armbian-release 2>/dev/null || true)"
if [[ "$armbian_board" != orangepizero2w ]]; then
  echo "Refusing Orange Pi device-tree customization for board: ${armbian_board:-unknown}." >&2
  exit 1
fi
if [[ ! -d "$overlay_dir" ]]; then
  echo "Expected Armbian userpatches overlay at $overlay_dir." >&2
  exit 1
fi
image_mode_helper="$overlay_dir/usr/local/lib/octessera/orange-image-mode.sh"
diagnostic_payload_helper="$overlay_dir/usr/local/lib/octessera/diagnostic-payload.sh"
sample_assets_helper="$overlay_dir/usr/local/lib/octessera/orange-sample-assets.sh"
[[ -f "$image_mode_helper" && ! -L "$image_mode_helper" ]] || { echo "Missing Orange image mode helper." >&2; exit 1; }
[[ -f "$diagnostic_payload_helper" && ! -L "$diagnostic_payload_helper" ]] || { echo "Missing diagnostic payload helper." >&2; exit 1; }
[[ -f "$sample_assets_helper" && ! -L "$sample_assets_helper" ]] || { echo "Missing Orange sample asset helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh
source "$image_mode_helper"
# shellcheck source=userpatches/overlay/usr/local/lib/octessera/diagnostic-payload.sh
source "$diagnostic_payload_helper"
# shellcheck disable=SC1090
source "$sample_assets_helper"
orange_runtime_assets_helper="$overlay_dir/usr/local/lib/octessera/orange-runtime-assets-install.sh"
[[ -f "$orange_runtime_assets_helper" && ! -L "$orange_runtime_assets_helper" ]] || { echo "Missing Orange runtime asset installer." >&2; exit 1; }
# shellcheck disable=SC1090
source "$orange_runtime_assets_helper"
spi_dts="$overlay_dir/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts"
input_routing_dts="$overlay_dir/usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts"
audio_dts="$overlay_dir/usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts"
octessera_validate_orange_runtime_assets "$overlay_dir" || exit 1
install -d -m 0755 /etc/octessera /usr/local/sbin /usr/local/lib/octessera /var/lib/octessera/samples /var/lib/octessera/presets
rm -f /var/lib/octessera/setup-complete /var/lib/octessera/setup-force /var/lib/octessera/setup-finalize-failed
rm -f /run/octessera/setup-portal.request /run/octessera-setup-request/inbox/start /run/octessera-setup-status/current.json
rm -rf /run/octessera-setup /run/octessera-setup-control /run/octessera-setup-status /run/octessera-setup-queue /run/octessera-setup-request

apt-get update
apt-get install -y --no-install-recommends ca-certificates coreutils curl device-tree-compiler tar xz-utils jq gpiod alsa-utils i2c-tools network-manager dnsmasq wireless-tools iw iproute2 python3-minimal initramfs-tools openssh-server sudo unzip util-linux psmisc
octessera_load_image_contract "$overlay_dir"
if [[ "$OCTESSERA_IMAGE_MODE" == production && ( -n "${OCTESSERA_PAYLOAD_URL:-}" || -n "${OCTESSERA_PAYLOAD_SHA256:-}" ) ]]; then
  echo "Production Orange images do not accept payload URLs or payload hashes." >&2
  exit 1
fi

mapfile -t kernel_configs < <(find -P /boot -maxdepth 1 -type f -name 'config-*' -print | LC_ALL=C sort)
[[ "${#kernel_configs[@]}" == 1 ]] || { echo "Expected exactly one Orange kernel config in /boot." >&2; exit 1; }
kernel_config="${kernel_configs[0]}"
kernel_config_value() {
  local symbol="$1"
  local matches
  matches="$(grep -E "^${symbol}=(y|m)$" "$kernel_config" || true)"
  [[ "$(printf '%s\n' "$matches" | awk 'NF { count++ } END { print count + 0 }')" == 1 ]] || {
    echo "Orange kernel config must contain exactly one enabled ${symbol}." >&2
    return 1
  }
  printf '%s\n' "${matches#*=}"
}
mmc_core_value="$(kernel_config_value CONFIG_MMC)"
mmc_block_value="$(kernel_config_value CONFIG_MMC_BLOCK)"
mmc_spi_value="$(kernel_config_value CONFIG_MMC_SPI)"
[[ "$mmc_core_value" == y && "$mmc_block_value" == y ]] || { echo "Orange kernel must build MMC core and block support in." >&2; exit 1; }
sd_modules_load_file=/etc/modules-load.d/octessera-orange-sd-card.conf
rm -f "$sd_modules_load_file"
if [[ "$mmc_spi_value" == m ]]; then
  printf '%s\n' mmc_spi > "$sd_modules_load_file"
  chown root:root "$sd_modules_load_file"
  chmod 0644 "$sd_modules_load_file"
fi

wifi_connect_artifact_dir="$overlay_dir/usr/local/share/octessera/wifi-connect"
wifi_connect_expected_sha256=4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627
wifi_connect_patch_sha256=c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6
for wifi_connect_file in wifi-connect wifi-connect.metadata.json cargo-metadata.json LICENSE THIRD-PARTY-NOTICES.md; do
  [[ -f "$wifi_connect_artifact_dir/$wifi_connect_file" && ! -L "$wifi_connect_artifact_dir/$wifi_connect_file" ]] || { echo "Missing locally supplied patched wifi-connect artifact: $wifi_connect_file" >&2; exit 1; }
done
echo "$wifi_connect_expected_sha256  $wifi_connect_artifact_dir/wifi-connect" | sha256sum -c -
[[ "$(jq -er '.binary_sha256' "$wifi_connect_artifact_dir/wifi-connect.metadata.json")" == "$wifi_connect_expected_sha256" ]] || { echo "Patched wifi-connect metadata has the wrong binary SHA-256." >&2; exit 1; }
[[ "$(jq -er '.patch_sha256' "$wifi_connect_artifact_dir/wifi-connect.metadata.json")" == "$wifi_connect_patch_sha256" ]] || { echo "Patched wifi-connect metadata has the wrong patch SHA-256." >&2; exit 1; }
[[ "$(jq -er '.target' "$wifi_connect_artifact_dir/wifi-connect.metadata.json")" == aarch64-unknown-linux-gnu ]] || { echo "Patched wifi-connect metadata has the wrong target." >&2; exit 1; }
install -D -m 0755 "$wifi_connect_artifact_dir/wifi-connect" /usr/local/bin/wifi-connect
for wifi_connect_doc in LICENSE THIRD-PARTY-NOTICES.md wifi-connect.metadata.json cargo-metadata.json; do
  install -D -m 0644 "$wifi_connect_artifact_dir/$wifi_connect_doc" "/usr/local/share/doc/octessera/wifi-connect/$wifi_connect_doc"
done

install_overlay_file() {
  local src="$1"
  local dest="$2"
  local mode="$3"
  [[ -f "$overlay_dir/$src" ]] || return 0
  install -D -m "$mode" -o root -g root "$overlay_dir/$src" "$dest"
}

notice_tree="$overlay_dir/usr/share/doc/octessera"
[[ -d "$notice_tree" && ! -L "$notice_tree" ]] || { echo "Missing exactly pre-staged canonical Orange legal notice tree; run tools/legal/stage_notices.py first." >&2; exit 1; }
while IFS= read -r -d '' legal_file; do
  legal_relative="${legal_file#"$overlay_dir/usr/share/doc/octessera/"}"
  install -D -m 0644 -o root -g root "$legal_file" "/usr/share/doc/octessera/$legal_relative"
done < <(find -P "$overlay_dir/usr/share/doc/octessera" -type f -print0)

if grep -RInE --exclude=wifi-connect '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|g_mass_storage)' "$overlay_dir"; then
  echo "Refusing Raspberry Pi-specific overlay content." >&2
  exit 1
fi
[[ -f "$overlay_dir/etc/octessera/armbian-image.txt" ]] || { echo "Missing Octessera Armbian marker overlay." >&2; exit 1; }
[[ -f "$overlay_dir/usr/local/sbin/octessera-armbian-diagnostics" ]] || { echo "Missing Octessera Armbian diagnostics overlay." >&2; exit 1; }
env_token_helper="$overlay_dir/usr/local/share/octessera/device-tree/armbian-env-token.sh"
[[ -f "$env_token_helper" ]] || { echo "Missing Armbian environment token helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh
source "$env_token_helper"
spi_validation_helper="$overlay_dir/usr/local/share/octessera/device-tree/spi-overlay-validation.sh"
[[ -f "$spi_validation_helper" ]] || { echo "Missing SPI overlay validation helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh
source "$spi_validation_helper"
input_routing_validation_helper="$overlay_dir/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh"
[[ -f "$input_routing_validation_helper" ]] || { echo "Missing input-routing overlay validation helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh
source "$input_routing_validation_helper"
audio_validation_helper="$overlay_dir/usr/local/share/octessera/device-tree/orange-ahub-overlay-validation.sh"
[[ -f "$audio_validation_helper" ]] || { echo "Missing Orange Pi AHUB0 audio validation helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/orange-ahub-overlay-validation.sh
source "$audio_validation_helper"
input_routing_boot_helper="$overlay_dir/usr/local/share/octessera/device-tree/input-routing-boot-config.sh"
[[ -f "$input_routing_boot_helper" ]] || { echo "Missing input-routing boot configuration helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh
source "$input_routing_boot_helper"
boot_dtb_helper="$overlay_dir/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
[[ -f "$boot_dtb_helper" ]] || { echo "Missing boot DTB selection helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
source "$boot_dtb_helper"

spi_overlay_name=octessera-h618-spi1-oled-sd2
spi_user_overlay_assignment='user_overlays=octessera-h618-spi1-oled-sd2'
spi_user_overlay_token="${spi_user_overlay_assignment#*=}"
input_routing_overlay_name=octessera-h618-input-routing
input_routing_user_overlay_token="$input_routing_overlay_name"
audio_overlay_name=octessera-ahub0-pcm5102
audio_user_overlay_token="$audio_overlay_name"
spi_overlay_dir=/boot/overlay-user
spi_dtbo="$spi_overlay_dir/$spi_overlay_name.dtbo"
spi_dts_image=/usr/local/share/octessera/device-tree/$spi_overlay_name.dts
input_routing_dtbo="$spi_overlay_dir/$input_routing_overlay_name.dtbo"
input_routing_dts_image=/usr/local/share/octessera/device-tree/$input_routing_overlay_name.dts
audio_dtbo="$spi_overlay_dir/$audio_overlay_name.dtbo"
audio_dts_image=/usr/local/share/octessera/device-tree/$audio_overlay_name.dts
spi_work="$(mktemp -d)"
input_routing_work="$(mktemp -d)"
audio_work="$(mktemp -d)"
spi_dtbo_tmp=
spi_dts_tmp=
input_routing_dtbo_tmp=
input_routing_dts_tmp=
audio_dtbo_tmp=
audio_dts_tmp=
armbian_env_tmp=
boot_args_tmp=
extlinux_tmp=
work=
cleanup() {
  rm -rf "${spi_work:-}" "${input_routing_work:-}" "${audio_work:-}" "${work:-}"
  rm -f "${spi_dtbo_tmp:-}" "${spi_dts_tmp:-}" "${input_routing_dtbo_tmp:-}" "${input_routing_dts_tmp:-}" "${audio_dtbo_tmp:-}" "${audio_dts_tmp:-}" "${armbian_env_tmp:-}" "${boot_args_tmp:-}" "${extlinux_tmp:-}"
}
trap cleanup EXIT
install -d -m 0755 "$spi_overlay_dir"
armbian_env=/boot/armbianEnv.txt
[[ -f "$armbian_env" ]] || { echo "Missing Armbian boot configuration: $armbian_env." >&2; exit 1; }

if ! spi_base_dtb="$(octessera_resolve_boot_dtb /)"; then
  exit 1
fi
[[ -n "$spi_base_dtb" ]] || { echo "Missing exact H618 Orange Pi Zero 2W base DTB." >&2; exit 1; }
[[ "$(basename "$(dirname "$spi_base_dtb")")" == allwinner ]] || { echo "Selected H618 DTB is not in an allwinner DTB tree." >&2; exit 1; }
selected_dtb_tree="$(dirname "$spi_base_dtb")"
stock_i2c1_dtbo="$selected_dtb_tree/overlay/sun50i-h616-i2c1-pi.dtbo"
[[ -f "$stock_i2c1_dtbo" && ! -L "$stock_i2c1_dtbo" ]] || { echo "Missing regular stock i2c1-pi DTBO beside the selected H618 DTB." >&2; exit 1; }
command -v dtc >/dev/null 2>&1 || { echo "dtc is required for SPI overlay installation." >&2; exit 1; }
command -v fdtoverlay >/dev/null 2>&1 || { echo "fdtoverlay is required for SPI overlay installation." >&2; exit 1; }
command -v fdtget >/dev/null 2>&1 || { echo "fdtget is required for SPI overlay installation." >&2; exit 1; }

spi_dtbo_tmp="$(mktemp "$spi_overlay_dir/.${spi_overlay_name}.dtbo.XXXXXX")"
octessera_run_strict_diagnostic "$spi_work" compile_spi_overlay dtc -@ -I dts -O dtb -o "$spi_dtbo_tmp" "$spi_dts" || exit 1
octessera_run_strict_diagnostic "$spi_work" inspect_spi_overlay dtc -I dtb -O dts -o "$spi_work/$spi_overlay_name.dts" "$spi_dtbo_tmp" || exit 1
stock_i2c1_merged_dtb="$spi_work/stock-i2c1-merged.dtb"
octessera_run_strict_diagnostic "$spi_work" merge_stock_i2c1_overlay fdtoverlay -i "$spi_base_dtb" -o "$stock_i2c1_merged_dtb" "$stock_i2c1_dtbo" || exit 1
octessera_run_dtc_inspection "$spi_work" inspect_stock_i2c1_overlay dtc -q -I dtb -O dts -o "$spi_work/stock-i2c1-merged.dts" "$stock_i2c1_merged_dtb" || exit 1
if ! octessera_assert_orange_preserved_peripherals "$spi_base_dtb" "$stock_i2c1_merged_dtb" "Orange Pi stock i2c1-pi composition"; then
  echo "Orange Pi stock i2c1-pi composition assertions failed." >&2
  exit 1
fi
spi_merged_dtb="$spi_work/$spi_overlay_name-merged.dtb"
octessera_run_strict_diagnostic "$spi_work" merge_spi_overlay fdtoverlay -i "$stock_i2c1_merged_dtb" -o "$spi_merged_dtb" "$spi_dtbo_tmp" || exit 1
octessera_run_dtc_inspection "$spi_work" inspect_merged_spi_overlay dtc -q -I dtb -O dts -o "$spi_work/$spi_overlay_name-merged.dts" "$spi_merged_dtb" || exit 1

spi1_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi1)"
spi1_pins_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi1_pins)"
spi1_cs0_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi1_cs0_pin)"
spi1_cs1_path="$(fdtget -t s "$spi_merged_dtb" /__symbols__ spi1_cs1_pin || true)"
spi0_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi0)"
i2c1_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ i2c1)"
[[ -n "$spi1_path" && -n "$spi1_pins_path" && -n "$spi1_cs0_path" && -n "$spi1_cs1_path" && -n "$spi0_path" && -n "$i2c1_path" ]] || { echo "H618 base DTB is missing required bus symbols or the local SPI1 CS1 group." >&2; exit 1; }
if ! octessera_assert_spi1_merge "$stock_i2c1_merged_dtb" "$spi_merged_dtb" "$spi1_path" "$spi1_pins_path" "$spi1_cs0_path" "$spi1_cs1_path" "$spi0_path" "$i2c1_path" "Orange Pi"; then
  echo "Orange Pi SPI1 merge assertions failed." >&2
  exit 1
fi

input_routing_dtbo_tmp="$(mktemp "$spi_overlay_dir/.${input_routing_overlay_name}.dtbo.XXXXXX")"
octessera_run_strict_diagnostic "$input_routing_work" compile_input_routing_overlay dtc -@ -I dts -O dtb -o "$input_routing_dtbo_tmp" "$input_routing_dts" || exit 1
octessera_run_strict_diagnostic "$input_routing_work" inspect_input_routing_overlay dtc -I dtb -O dts -o "$input_routing_work/$input_routing_overlay_name.dts" "$input_routing_dtbo_tmp" || exit 1
input_routing_merged_dtb="$input_routing_work/$input_routing_overlay_name-merged.dtb"
octessera_run_strict_diagnostic "$input_routing_work" merge_input_routing_overlay fdtoverlay -i "$spi_merged_dtb" -o "$input_routing_merged_dtb" "$input_routing_dtbo_tmp" || exit 1
octessera_run_dtc_inspection "$input_routing_work" inspect_merged_input_routing_overlay dtc -q -I dtb -O dts -o "$input_routing_work/$input_routing_overlay_name-merged.dts" "$input_routing_merged_dtb" || exit 1
uart0_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ uart0)"
pio_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ pio)"
[[ -n "$uart0_path" && -n "$pio_path" ]] || { echo "H618 base DTB is missing UART0 or pinctrl symbols." >&2; exit 1; }
if ! octessera_assert_input_routing_merge "$spi_merged_dtb" "$input_routing_merged_dtb" "$uart0_path" "$pio_path" /chosen "Orange Pi"; then
  echo "Orange Pi input-routing merge assertions failed." >&2
  exit 1
fi

audio_dtbo_tmp="$(mktemp "$spi_overlay_dir/.${audio_overlay_name}.dtbo.XXXXXX")"
octessera_run_strict_diagnostic "$audio_work" compile_audio_overlay dtc -@ -I dts -O dtb -o "$audio_dtbo_tmp" "$audio_dts" || exit 1
octessera_run_strict_diagnostic "$audio_work" inspect_audio_overlay dtc -q -I dtb -O dts -o "$audio_work/$audio_overlay_name.dts" "$audio_dtbo_tmp" || exit 1
production_spi_input_dtb="$audio_work/$audio_overlay_name-spi-input-merged.dtb"
production_merged_dtb="$audio_work/$audio_overlay_name-production-merged.dtb"
cp -f -- "$input_routing_merged_dtb" "$production_spi_input_dtb"
octessera_run_strict_diagnostic "$audio_work" merge_production_user_overlays fdtoverlay -i "$production_spi_input_dtb" -o "$production_merged_dtb" "$audio_dtbo_tmp" || exit 1
octessera_run_dtc_inspection "$audio_work" inspect_production_user_overlays dtc -q -I dtb -O dts -o "$audio_work/$audio_overlay_name-production-merged.dts" "$production_merged_dtb" || exit 1
if ! octessera_assert_spi1_merge "$stock_i2c1_merged_dtb" "$production_merged_dtb" "$spi1_path" "$spi1_pins_path" "$spi1_cs0_path" "$spi1_cs1_path" "$spi0_path" "$i2c1_path" "Orange Pi production composition"; then
  echo "Orange Pi SPI1 production composition assertions failed." >&2
  exit 1
fi
if ! octessera_assert_input_routing_merge "$spi_merged_dtb" "$production_merged_dtb" "$uart0_path" "$pio_path" /chosen "Orange Pi production composition"; then
  echo "Orange Pi input-routing production composition assertions failed." >&2
  exit 1
fi
if ! octessera_assert_orange_audio_merge "$production_spi_input_dtb" "$production_merged_dtb" "Orange Pi production composition"; then
  echo "Orange Pi AHUB0 production composition assertions failed." >&2
  exit 1
fi
if ! octessera_assert_orange_preserved_peripherals "$spi_base_dtb" "$production_merged_dtb" "Orange Pi production composition"; then
  echo "Orange Pi preserved-peripheral production composition assertions failed." >&2
  exit 1
fi

install -d -m 0755 "$(dirname "$spi_dts_image")"
spi_dts_tmp="$(mktemp "${spi_dts_image}.XXXXXX")"
install -m 0644 -o root -g root "$spi_dts" "$spi_dts_tmp"
mv -f -- "$spi_dts_tmp" "$spi_dts_image"
spi_dts_tmp=
chmod 0644 "$spi_dtbo_tmp"
chown root:root "$spi_dtbo_tmp"
mv -f -- "$spi_dtbo_tmp" "$spi_dtbo"
spi_dtbo_tmp=

install -d -m 0755 "$(dirname "$input_routing_dts_image")"
input_routing_dts_tmp="$(mktemp "${input_routing_dts_image}.XXXXXX")"
install -m 0644 -o root -g root "$input_routing_dts" "$input_routing_dts_tmp"
mv -f -- "$input_routing_dts_tmp" "$input_routing_dts_image"
input_routing_dts_tmp=
chmod 0644 "$input_routing_dtbo_tmp"
chown root:root "$input_routing_dtbo_tmp"
mv -f -- "$input_routing_dtbo_tmp" "$input_routing_dtbo"
input_routing_dtbo_tmp=

install -d -m 0755 "$(dirname "$audio_dts_image")"
audio_dts_tmp="$(mktemp "${audio_dts_image}.XXXXXX")"
install -m 0644 -o root -g root "$audio_dts" "$audio_dts_tmp"
mv -f -- "$audio_dts_tmp" "$audio_dts_image"
audio_dts_tmp=
chmod 0644 "$audio_dtbo_tmp"
chown root:root "$audio_dtbo_tmp"
mv -f -- "$audio_dtbo_tmp" "$audio_dtbo"
audio_dtbo_tmp=

armbian_env_tmp="$(mktemp "${armbian_env}.XXXXXX")"
if ! octessera_armbian_env_update "$armbian_env" "$armbian_env_tmp" "$spi_user_overlay_token" i2c1-pi "$input_routing_user_overlay_token" "$audio_user_overlay_token" 1; then
  echo "Refusing malformed or ambiguous Armbian overlay configuration." >&2
  exit 1
fi
chmod --reference="$armbian_env" "$armbian_env_tmp"
chown --reference="$armbian_env" "$armbian_env_tmp"
if cmp -s "$armbian_env" "$armbian_env_tmp"; then
  rm -f "$armbian_env_tmp"
else
  mv -f -- "$armbian_env_tmp" "$armbian_env"
fi
armbian_env_tmp=

display_console_tmp="$(mktemp "${armbian_env}.display-console.XXXXXX")"
if ! octessera_set_armbian_display_console "$armbian_env" "$display_console_tmp"; then
  echo "Refusing malformed or ambiguous Armbian console configuration." >&2
  exit 1
fi
chmod --reference="$armbian_env" "$display_console_tmp"
chown --reference="$armbian_env" "$display_console_tmp"
if cmp -s "$armbian_env" "$display_console_tmp"; then
  rm -f "$display_console_tmp"
else
  mv -f -- "$display_console_tmp" "$armbian_env"
fi
display_console_tmp=

boot_args_tmp="$(mktemp "${armbian_env}.boot-args.XXXXXX")"
if ! octessera_remove_uart0_console_args "$armbian_env" "$boot_args_tmp"; then
  echo "Refusing malformed Armbian boot argument configuration." >&2
  exit 1
fi
octessera_assert_no_uart0_console_args "$boot_args_tmp" || {
  echo "Armbian boot configuration still selects console=ttyS0." >&2
  exit 1
}
chmod --reference="$armbian_env" "$boot_args_tmp"
chown --reference="$armbian_env" "$boot_args_tmp"
if cmp -s "$armbian_env" "$boot_args_tmp"; then
  rm -f "$boot_args_tmp"
else
  mv -f -- "$boot_args_tmp" "$armbian_env"
fi
boot_args_tmp=

if [[ -f /boot/extlinux/extlinux.conf ]]; then
  extlinux_tmp="$(mktemp /boot/extlinux/.extlinux.conf.XXXXXX)"
  if ! octessera_remove_uart0_console_args /boot/extlinux/extlinux.conf "$extlinux_tmp"; then
    echo "Refusing malformed Armbian extlinux boot argument configuration." >&2
    exit 1
  fi
  octessera_assert_no_uart0_console_args "$extlinux_tmp" || {
    echo "Armbian extlinux configuration still selects console=ttyS0." >&2
    exit 1
  }
  chmod --reference=/boot/extlinux/extlinux.conf "$extlinux_tmp"
  chown --reference=/boot/extlinux/extlinux.conf "$extlinux_tmp"
  mv -f -- "$extlinux_tmp" /boot/extlinux/extlinux.conf
  extlinux_tmp=
fi

spi_dts_sha256="$(sha256sum "$spi_dts_image" | awk '{ print $1 }')"
spi_dtbo_sha256="$(sha256sum "$spi_dtbo" | awk '{ print $1 }')"
audio_dts_sha256="$(sha256sum "$audio_dts_image" | awk '{ print $1 }')"
audio_dtbo_sha256="$(sha256sum "$audio_dtbo" | awk '{ print $1 }')"

octessera_require_updater_overlay "$overlay_dir"
for wifi_foundation_file in \
  usr/local/sbin/octessera-wifi-foundation \
  etc/systemd/system/octessera-wifi-foundation.service; do
  [[ -f "$overlay_dir/$wifi_foundation_file" ]] || { echo "Missing inactive Wi-Fi foundation overlay: $wifi_foundation_file" >&2; exit 1; }
done
setup_layer_installer="$overlay_dir/usr/local/lib/octessera/setup-image-layer.sh"
[[ -f "$setup_layer_installer" && ! -L "$setup_layer_installer" ]] || { echo "Missing setup image layer installer." >&2; exit 1; }
welcome_overlay="$overlay_dir/etc/profile.d/octessera-welcome.sh"
[[ -f "$welcome_overlay" && ! -L "$welcome_overlay" ]] || { echo "Missing staged canonical welcome file. Run tools/armbian-image/stage-canonical-welcome.sh." >&2; exit 1; }
install -D -m 0644 -o root -g root "$welcome_overlay" /etc/profile.d/octessera-welcome.sh
install_overlay_file etc/octessera/armbian-image.txt /etc/octessera/armbian-image.txt 0644
install_overlay_file etc/octessera/image-contract.json /etc/octessera/image-contract.json 0644
install_overlay_file usr/local/sbin/octessera-armbian-diagnostics /usr/local/sbin/octessera-armbian-diagnostics 0755
install_overlay_file usr/local/sbin/octessera-update /usr/local/sbin/octessera-update 0755
install_overlay_file usr/local/sbin/octessera-update-broker /usr/local/sbin/octessera-update-broker 0755
install_overlay_file usr/local/sbin/octessera-update-guard /usr/local/sbin/octessera-update-guard 0755
install_overlay_file usr/local/sbin/octessera-update-recovery /usr/local/sbin/octessera-update-recovery 0755
install_overlay_file usr/local/lib/octessera/updater_protocol.py /usr/local/lib/octessera/updater_protocol.py 0644
install_overlay_file usr/local/lib/octessera/updater_contract.py /usr/local/lib/octessera/updater_contract.py 0644
install_overlay_file usr/local/lib/octessera/updater_state.py /usr/local/lib/octessera/updater_state.py 0644
install_overlay_file usr/local/lib/octessera/updater_assets.py /usr/local/lib/octessera/updater_assets.py 0644
install_overlay_file usr/local/lib/octessera/updater_guard.py /usr/local/lib/octessera/updater_guard.py 0644
install_overlay_file usr/local/lib/octessera/updater_cli.py /usr/local/lib/octessera/updater_cli.py 0644
install_overlay_file usr/local/lib/octessera/updater_profiles.py /usr/local/lib/octessera/updater_profiles.py 0644
install_overlay_file usr/local/sbin/octessera-wifi-foundation /usr/local/sbin/octessera-wifi-foundation 0755
octessera_install_orange_runtime_assets "$overlay_dir"
install_overlay_file etc/systemd/system/octessera-wifi-foundation.service /etc/systemd/system/octessera-wifi-foundation.service 0644
for musical_asset in \
  usr/share/octessera/defaults/pi-default.json \
  usr/share/octessera/samples/MANIFEST.tsv \
  usr/share/octessera/samples/SOURCE.md \
  usr/share/octessera/samples/upstream/LICENSE; do
  [[ -f "$overlay_dir/$musical_asset" && ! -L "$overlay_dir/$musical_asset" ]] || { echo "Missing staged regular musical asset: $musical_asset. Run tools/armbian-image/stage-musical-assets.sh." >&2; exit 1; }
done
install_overlay_file usr/share/octessera/defaults/pi-default.json /usr/share/octessera/defaults/pi-default.json 0644
install_overlay_file usr/share/octessera/samples/MANIFEST.tsv /usr/share/octessera/samples/MANIFEST.tsv 0644
install_overlay_file usr/share/octessera/samples/SOURCE.md /usr/share/octessera/samples/SOURCE.md 0644
install_overlay_file usr/share/octessera/samples/upstream/LICENSE /usr/share/octessera/samples/upstream/LICENSE 0644
install_orange_musical_assets "$overlay_dir" ""
install_overlay_file etc/systemd/system/octessera-setup.service /etc/systemd/system/octessera-setup.service 0644
install_overlay_file etc/systemd/system/octessera-update-guard.service /etc/systemd/system/octessera-update-guard.service 0644
install_overlay_file etc/systemd/system/octessera-update-recovery.service /etc/systemd/system/octessera-update-recovery.service 0644
install_overlay_file etc/systemd/system/octessera-update.socket /etc/systemd/system/octessera-update.socket 0644
install_overlay_file etc/systemd/system/octessera-update@.service /etc/systemd/system/octessera-update@.service 0644
install_overlay_file etc/sudoers.d/octessera-update /etc/sudoers.d/octessera-update 0440
if [[ "$OCTESSERA_IMAGE_MODE" == production ]]; then
  octessera_install_orange_production_assets "$overlay_dir"
fi
bash "$setup_layer_installer" "$overlay_dir"

if ! id octessera >/dev/null 2>&1; then
  useradd --create-home --shell /bin/bash --groups sudo octessera
fi
octessera_configure_runtime_account
passwd -l octessera >/dev/null || true
octessera_record="$(getent passwd octessera)"
IFS=: read -r octessera_user _ octessera_uid octessera_gid _ octessera_home octessera_shell <<< "$octessera_record"
[[ "$octessera_user" == octessera && "$octessera_home" == /home/octessera && "$octessera_shell" == /bin/bash ]] || { echo "Orange runtime account home or shell is not exact." >&2; exit 1; }
[[ -d "$octessera_home" && ! -L "$octessera_home" ]] || { echo "Orange runtime account home is missing or symlinked." >&2; exit 1; }
hushlogin="$octessera_home/.hushlogin"
if [[ -e "$hushlogin" || -L "$hushlogin" ]]; then
  [[ -f "$hushlogin" && ! -L "$hushlogin" && "$(stat -c '%u:%g:%a:%s' "$hushlogin")" == "$octessera_uid:$octessera_gid:644:0" && ! -s "$hushlogin" ]] || { echo "Orange .hushlogin exists with unexpected type, owner, mode, or content." >&2; exit 1; }
else
  install -D -m 0644 -o "$octessera_user" -g "$octessera_user" /dev/null "$hushlogin"
fi
bashrc="$octessera_home/.bashrc"
if [[ -f "$bashrc" && ! -L "$bashrc" ]]; then
  sed -i -E '/^[[:space:]]*(export[[:space:]]+)?(LANG|LANGUAGE|LC_[[:alnum:]_]+)[[:space:]]*=/d' "$bashrc"
fi
rm -f /root/.ssh/authorized_keys /home/octessera/.ssh/authorized_keys
install -d -m 0755 /etc/ssh/sshd_config.d
cat >/etc/ssh/sshd_config.d/10-octessera-setup.conf <<'EOF'
PermitRootLogin no
PasswordAuthentication no
AllowUsers octessera
EOF
systemctl disable ssh.service
systemctl disable ssh.socket
systemctl mask ssh.service
systemctl mask ssh.socket
systemctl mask sshd.service
systemctl mask sshd.socket
for unit in dnsmasq.service systemd-networkd-wait-online.service NetworkManager-wait-online.service; do
  systemctl disable "$unit" >/dev/null 2>&1 || true
done
rm -f \
  /etc/systemd/system/multi-user.target.wants/dnsmasq.service \
  /etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service \
  /etc/systemd/system/network-online.target.wants/NetworkManager-wait-online.service
rm -f /etc/ssh/ssh_host_*
systemctl disable --now serial-getty@ttyS0.service >/dev/null 2>&1 || true
systemctl mask serial-getty@ttyS0.service >/dev/null 2>&1 || true
systemctl enable octessera-setup-request.path >/dev/null
setup_request_link=/etc/systemd/system/multi-user.target.wants/octessera-setup-request.path
[[ -L "$setup_request_link" ]] || { echo "Setup request path was not enabled as a symlink." >&2; exit 1; }
setup_request_target="$(readlink "$setup_request_link")"
[[ "$setup_request_target" == "/etc/systemd/system/octessera-setup-request.path" || "$setup_request_target" == "../octessera-setup-request.path" ]] || { echo "Setup request path has an unexpected preimage target." >&2; exit 1; }
rm -f "$setup_request_link"
ln -s ../octessera-setup-request.path "$setup_request_link"
[[ "$(readlink "$setup_request_link")" == "../octessera-setup-request.path" ]] || { echo "Setup request path symlink target is not canonical." >&2; exit 1; }
octessera_enable_orange_runtime_services
systemctl enable octessera-update-recovery.service >/dev/null
systemctl enable octessera-update.socket >/dev/null
[[ "$OCTESSERA_IMAGE_MODE" != production ]] || systemctl enable octessera.service >/dev/null
update-initramfs -u

cat >/etc/octessera/build-metadata.env <<EOF
OCTESSERA_IMAGE_KIND=armbian
OCTESSERA_IMAGE_MODE=${OCTESSERA_IMAGE_MODE}
OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w
OCTESSERA_IMAGE_BUILT_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
OCTESSERA_RUNTIME_ENABLED_DEFAULT=${OCTESSERA_RUNTIME_ENABLED_DEFAULT}
OCTESSERA_IMAGE_CONTRACT_SHA256=${OCTESSERA_IMAGE_CONTRACT_SHA256}
OCTESSERA_RUNTIME_VERSION=${OCTESSERA_RUNTIME_VERSION}
OCTESSERA_RUNTIME_BINARY_SHA256=${OCTESSERA_RUNTIME_BINARY_SHA256}
OCTESSERA_RUNTIME_MANIFEST_SHA256=${OCTESSERA_RUNTIME_MANIFEST_SHA256}
OCTESSERA_RUNTIME_METADATA_SHA256=${OCTESSERA_RUNTIME_METADATA_SHA256}
OCTESSERA_SPI1_OLED_SD2_DTS_SHA256=${spi_dts_sha256}
OCTESSERA_SPI1_OLED_SD2_DTBO_SHA256=${spi_dtbo_sha256}
OCTESSERA_INPUT_ROUTING_DTS_SHA256=$(sha256sum "$input_routing_dts_image" | awk '{ print $1 }')
OCTESSERA_INPUT_ROUTING_DTBO_SHA256=$(sha256sum "$input_routing_dtbo" | awk '{ print $1 }')
OCTESSERA_AHUB0_PCM5102_DTS_SHA256=${audio_dts_sha256}
OCTESSERA_AHUB0_PCM5102_DTBO_SHA256=${audio_dtbo_sha256}
OCTESSERA_PI_DEFAULT_SHA256=$(sha256sum /usr/share/octessera/defaults/pi-default.json | awk '{ print $1 }')
OCTESSERA_SAMPLES_MANIFEST_SHA256=$(sha256sum /usr/share/octessera/samples/MANIFEST.tsv | awk '{ print $1 }')
EOF
chown root:root /etc/octessera/build-metadata.env
chmod 0644 /etc/octessera/build-metadata.env

payload_url="${OCTESSERA_PAYLOAD_URL:-}"
payload_sha256="${OCTESSERA_PAYLOAD_SHA256:-}"
if [[ "$OCTESSERA_IMAGE_MODE" == diagnostic ]]; then
  octessera_install_diagnostic_payload "$payload_url" "$payload_sha256"
elif [[ -n "$payload_url" || -n "$payload_sha256" ]]; then
  echo "Production Orange images do not accept payload URLs or payload hashes." >&2
  exit 1
fi

if [[ -n "${PUBLIC_PRESET_CONFIGURATION_URL:-}" ]]; then
  export PRESET_CONFIGURATION="$PUBLIC_PRESET_CONFIGURATION_URL"
fi

if [[ "$OCTESSERA_IMAGE_MODE" == production ]]; then
  rm -f /root/.not_logged_in_yet
  [[ ! -e /root/.not_logged_in_yet && ! -L /root/.not_logged_in_yet ]] || { echo "Orange Armbian onboarding marker remains." >&2; exit 1; }
fi

apt-get clean
rm -rf /var/lib/apt/lists/*
