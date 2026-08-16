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
spi_dts="$overlay_dir/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts"
[[ -f "$spi_dts" ]] || { echo "Missing Orange Pi SPI overlay source." >&2; exit 1; }
input_routing_dts="$overlay_dir/usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts"
[[ -f "$input_routing_dts" ]] || { echo "Missing Orange Pi input-routing overlay source." >&2; exit 1; }
midi_modules_file="$overlay_dir/etc/modules-load.d/octessera-orange-midi.conf"
[[ -f "$midi_modules_file" ]] || { echo "Missing Orange ALSA sequencer module-load file." >&2; exit 1; }
install -d -m 0755 /etc/octessera /usr/local/sbin /usr/local/lib/octessera /var/lib/octessera/samples /var/lib/octessera/presets
rm -f /var/lib/octessera/setup-complete /var/lib/octessera/setup-force /var/lib/octessera/setup-finalize-failed
rm -f /run/octessera/setup-portal.request /run/octessera-setup-control/status.json
rm -rf /run/octessera-setup /run/octessera-setup-control /run/octessera-setup-status

apt-get update
apt-get install -y --no-install-recommends ca-certificates coreutils curl device-tree-compiler tar xz-utils jq gpiod alsa-utils i2c-tools network-manager dnsmasq wireless-tools iw python3-minimal initramfs-tools openssh-server sudo unzip util-linux psmisc
octessera_load_image_contract "$overlay_dir"
if [[ "$OCTESSERA_IMAGE_MODE" == production && ( -n "${OCTESSERA_PAYLOAD_URL:-}" || -n "${OCTESSERA_PAYLOAD_SHA256:-}" ) ]]; then
  echo "Production Orange images do not accept payload URLs or payload hashes." >&2
  exit 1
fi

wifi_connect_version=4.11.84
wifi_connect_sha256=413d70e6d1c1366cbe2b32555e8476f3e92878178ed1b9c82205985f055f1936
wifi_connect_url="https://github.com/balena-os/wifi-connect/releases/download/v${wifi_connect_version}/wifi-connect-aarch64-unknown-linux-gnu.tar.gz"
wifi_work="$(mktemp -d)"
curl --fail --location --proto '=https' --tlsv1.2 --output "$wifi_work/wifi-connect.tar.gz" "$wifi_connect_url"
echo "$wifi_connect_sha256  $wifi_work/wifi-connect.tar.gz" | sha256sum -c -
tar -xf "$wifi_work/wifi-connect.tar.gz" -C "$wifi_work"
install -D -m 0755 "$wifi_work/wifi-connect" /usr/local/bin/wifi-connect
install -d -m 0755 /usr/local/share/doc/octessera
cat >/usr/local/share/doc/octessera/wifi-connect.metadata <<EOF
wifi-connect ${wifi_connect_version}
Source: ${wifi_connect_url}
SHA256: ${wifi_connect_sha256}
License: Apache-2.0
EOF
cat >/usr/local/share/doc/octessera/wifi-connect.NOTICE <<'EOF'
wifi-connect is distributed by balena under the Apache License 2.0.
See https://github.com/balena-os/wifi-connect for upstream license text.
EOF
rm -rf "$wifi_work"

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

if grep -RInE '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|g_mass_storage)' "$overlay_dir"; then
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
input_routing_boot_helper="$overlay_dir/usr/local/share/octessera/device-tree/input-routing-boot-config.sh"
[[ -f "$input_routing_boot_helper" ]] || { echo "Missing input-routing boot configuration helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh
source "$input_routing_boot_helper"
boot_dtb_helper="$overlay_dir/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
[[ -f "$boot_dtb_helper" ]] || { echo "Missing boot DTB selection helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
source "$boot_dtb_helper"

spi_overlay_name=octessera-h618-spi1-cs0
spi_user_overlay_assignment='user_overlays=octessera-h618-spi1-cs0'
spi_user_overlay_token="${spi_user_overlay_assignment#*=}"
input_routing_overlay_name=octessera-h618-input-routing
input_routing_user_overlay_token="$input_routing_overlay_name"
spi_overlay_dir=/boot/overlay-user
spi_dtbo="$spi_overlay_dir/$spi_overlay_name.dtbo"
spi_dts_image=/usr/local/share/octessera/device-tree/$spi_overlay_name.dts
input_routing_dtbo="$spi_overlay_dir/$input_routing_overlay_name.dtbo"
input_routing_dts_image=/usr/local/share/octessera/device-tree/$input_routing_overlay_name.dts
spi_work="$(mktemp -d)"
input_routing_work="$(mktemp -d)"
spi_dtbo_tmp=
spi_dts_tmp=
input_routing_dtbo_tmp=
input_routing_dts_tmp=
armbian_env_tmp=
boot_args_tmp=
extlinux_tmp=
work=
cleanup() {
  rm -rf "${spi_work:-}" "${input_routing_work:-}" "${work:-}"
  rm -f "${spi_dtbo_tmp:-}" "${spi_dts_tmp:-}" "${input_routing_dtbo_tmp:-}" "${input_routing_dts_tmp:-}" "${armbian_env_tmp:-}" "${boot_args_tmp:-}" "${extlinux_tmp:-}"
}
trap cleanup EXIT
install -d -m 0755 "$spi_overlay_dir"
armbian_env=/boot/armbianEnv.txt
[[ -f "$armbian_env" ]] || { echo "Missing Armbian boot configuration: $armbian_env." >&2; exit 1; }

if ! spi_base_dtb="$(octessera_resolve_boot_dtb /)"; then
  exit 1
fi
[[ -n "$spi_base_dtb" ]] || { echo "Missing exact H618 Orange Pi Zero 2W base DTB." >&2; exit 1; }
command -v dtc >/dev/null 2>&1 || { echo "dtc is required for SPI overlay installation." >&2; exit 1; }
command -v fdtoverlay >/dev/null 2>&1 || { echo "fdtoverlay is required for SPI overlay installation." >&2; exit 1; }
command -v fdtget >/dev/null 2>&1 || { echo "fdtget is required for SPI overlay installation." >&2; exit 1; }

spi_dtbo_tmp="$(mktemp "$spi_overlay_dir/.${spi_overlay_name}.dtbo.XXXXXX")"
octessera_run_strict_diagnostic "$spi_work" compile_spi_overlay dtc -@ -I dts -O dtb -o "$spi_dtbo_tmp" "$spi_dts" || exit 1
octessera_run_strict_diagnostic "$spi_work" inspect_spi_overlay dtc -I dtb -O dts -o "$spi_work/$spi_overlay_name.dts" "$spi_dtbo_tmp" || exit 1
spi_merged_dtb="$spi_work/$spi_overlay_name-merged.dtb"
octessera_run_strict_diagnostic "$spi_work" merge_spi_overlay fdtoverlay -i "$spi_base_dtb" -o "$spi_merged_dtb" "$spi_dtbo_tmp" || exit 1
octessera_run_dtc_inspection "$spi_work" inspect_merged_spi_overlay dtc -q -I dtb -O dts -o "$spi_work/$spi_overlay_name-merged.dts" "$spi_merged_dtb" || exit 1

spi1_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi1)"
spi1_pins_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi1_pins)"
spi1_cs0_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi1_cs0_pin)"
spi0_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ spi0)"
i2c1_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ i2c1)"
[[ -n "$spi1_path" && -n "$spi1_pins_path" && -n "$spi1_cs0_path" && -n "$spi0_path" && -n "$i2c1_path" ]] || { echo "H618 base DTB is missing required bus symbols." >&2; exit 1; }
if ! octessera_assert_spi1_merge "$spi_base_dtb" "$spi_merged_dtb" "$spi1_path" "$spi1_pins_path" "$spi1_cs0_path" "$spi0_path" "$i2c1_path" "Orange Pi"; then
  echo "Orange Pi SPI1 merge assertions failed." >&2
  exit 1
fi

input_routing_dtbo_tmp="$(mktemp "$spi_overlay_dir/.${input_routing_overlay_name}.dtbo.XXXXXX")"
octessera_run_strict_diagnostic "$input_routing_work" compile_input_routing_overlay dtc -@ -I dts -O dtb -o "$input_routing_dtbo_tmp" "$input_routing_dts" || exit 1
octessera_run_strict_diagnostic "$input_routing_work" inspect_input_routing_overlay dtc -I dtb -O dts -o "$input_routing_work/$input_routing_overlay_name.dts" "$input_routing_dtbo_tmp" || exit 1
input_routing_merged_dtb="$input_routing_work/$input_routing_overlay_name-merged.dtb"
octessera_run_strict_diagnostic "$input_routing_work" merge_input_routing_overlay fdtoverlay -i "$spi_base_dtb" -o "$input_routing_merged_dtb" "$input_routing_dtbo_tmp" || exit 1
octessera_run_dtc_inspection "$input_routing_work" inspect_merged_input_routing_overlay dtc -q -I dtb -O dts -o "$input_routing_work/$input_routing_overlay_name-merged.dts" "$input_routing_merged_dtb" || exit 1
uart0_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ uart0)"
pio_path="$(fdtget -t s "$spi_base_dtb" /__symbols__ pio)"
[[ -n "$uart0_path" && -n "$pio_path" ]] || { echo "H618 base DTB is missing UART0 or pinctrl symbols." >&2; exit 1; }
if ! octessera_assert_input_routing_merge "$spi_base_dtb" "$input_routing_merged_dtb" "$uart0_path" "$pio_path" /chosen "Orange Pi"; then
  echo "Orange Pi input-routing merge assertions failed." >&2
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

armbian_env_tmp="$(mktemp "${armbian_env}.XXXXXX")"
if ! octessera_armbian_env_update "$armbian_env" "$armbian_env_tmp" "$spi_user_overlay_token" i2c1-pi "$input_routing_user_overlay_token"; then
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

octessera_require_diagnostic_updater_overlay "$overlay_dir"
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
if [[ "$OCTESSERA_IMAGE_MODE" == diagnostic ]]; then
  install_overlay_file usr/local/sbin/octessera-update /usr/local/sbin/octessera-update 0755
  install_overlay_file usr/local/sbin/octessera-update-guard /usr/local/sbin/octessera-update-guard 0755
  install_overlay_file usr/local/sbin/octessera-update-recovery /usr/local/sbin/octessera-update-recovery 0755
  install_overlay_file usr/local/lib/octessera/updater_protocol.py /usr/local/lib/octessera/updater_protocol.py 0644
  install_overlay_file usr/local/lib/octessera/updater_state.py /usr/local/lib/octessera/updater_state.py 0644
  install_overlay_file usr/local/lib/octessera/updater_assets.py /usr/local/lib/octessera/updater_assets.py 0644
  install_overlay_file usr/local/lib/octessera/updater_guard.py /usr/local/lib/octessera/updater_guard.py 0644
  install_overlay_file usr/local/lib/octessera/updater_cli.py /usr/local/lib/octessera/updater_cli.py 0644
fi
install_overlay_file usr/local/sbin/octessera-wifi-foundation /usr/local/sbin/octessera-wifi-foundation 0755
install_overlay_file usr/local/sbin/octessera-orange-usb-gadget /usr/local/sbin/octessera-orange-usb-gadget 0755
device_config_overlay="$overlay_dir/usr/local/lib/octessera/device_config.py"
[[ -f "$device_config_overlay" && ! -L "$device_config_overlay" ]] || { echo "Missing staged canonical device config. Run tools/armbian-image/stage-device-config.py." >&2; exit 1; }
install_overlay_file usr/local/lib/octessera/device_config.py /usr/local/lib/octessera/device_config.py 0644
install_overlay_file usr/local/sbin/octessera-device-apply-reboot /usr/local/sbin/octessera-device-apply-reboot 0755
install_overlay_file usr/local/sbin/octessera-orange-oled-logo /usr/local/sbin/octessera-orange-oled-logo 0755
install_overlay_file usr/local/sbin/octessera-orange-oled-handoff.py /usr/local/sbin/octessera-orange-oled-handoff.py 0644
install_overlay_file usr/local/sbin/octessera-orange-oled-lifecycle.py /usr/local/sbin/octessera-orange-oled-lifecycle.py 0644
install_overlay_file usr/local/sbin/octessera-orange-oled-suspend /usr/local/sbin/octessera-orange-oled-suspend 0755
install_overlay_file usr/local/sbin/octessera-provision-musical-default /usr/local/sbin/octessera-provision-musical-default 0755
install_overlay_file usr/local/lib/octessera/orange-sample-assets.sh /usr/local/lib/octessera/orange-sample-assets.sh 0644
install_overlay_file etc/modules-load.d/octessera-orange-midi.conf /etc/modules-load.d/octessera-orange-midi.conf 0644
install_overlay_file etc/modules-load.d/octessera-orange-usb-gadget.conf /etc/modules-load.d/octessera-orange-usb-gadget.conf 0644
install_overlay_file etc/systemd/system/octessera-orange-usb-gadget.service /etc/systemd/system/octessera-orange-usb-gadget.service 0644
install_overlay_file etc/systemd/system/octessera-device-apply-reboot.socket /etc/systemd/system/octessera-device-apply-reboot.socket 0644
install_overlay_file etc/systemd/system/octessera-device-apply-reboot@.service /etc/systemd/system/octessera-device-apply-reboot@.service 0644
install_overlay_file etc/systemd/system/octessera-provision-musical-default.service /etc/systemd/system/octessera-provision-musical-default.service 0644
install_overlay_file etc/initramfs-tools/hooks/octessera-orange-boot-splash /etc/initramfs-tools/hooks/octessera-orange-boot-splash 0755
install_overlay_file etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash /etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash 0755
install_overlay_file etc/systemd/system/octessera-orange-boot-splash.service /etc/systemd/system/octessera-orange-boot-splash.service 0644
install_overlay_file etc/systemd/system/octessera-orange-oled-shutdown.service /etc/systemd/system/octessera-orange-oled-shutdown.service 0644
install_overlay_file etc/systemd/system/octessera-orange-oled-suspend.service /etc/systemd/system/octessera-orange-oled-suspend.service 0644
install_overlay_file etc/systemd/system/octessera-wifi-foundation.service /etc/systemd/system/octessera-wifi-foundation.service 0644
install_overlay_file usr/local/share/octessera-setup-ui/octessera-mark.svg /usr/share/octessera/oled/octessera-mark.svg 0644
install_overlay_file usr/local/share/octessera-setup-ui/octessera-wordmark.svg /usr/share/octessera/oled/octessera-wordmark.svg 0644
install_overlay_file usr/local/share/octessera/oled/octessera-pi-booting.rgb565 /usr/share/octessera/oled/octessera-pi-booting.rgb565 0644
install_overlay_file usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565 /usr/share/octessera/oled/octessera-pi-shutdown.rgb565 0644
for musical_asset in \
  usr/share/octessera/defaults/pi-default.json \
  usr/share/octessera/samples/sample-manifest.tsv \
  usr/share/octessera/samples/ATTRIBUTIONS.tsv \
  usr/share/octessera/samples/upstream/LICENSE \
  usr/share/octessera/samples/upstream/README.txt; do
  [[ -f "$overlay_dir/$musical_asset" && ! -L "$overlay_dir/$musical_asset" ]] || { echo "Missing staged regular musical asset: $musical_asset. Run tools/armbian-image/stage-musical-assets.sh." >&2; exit 1; }
done
install_overlay_file usr/share/octessera/defaults/pi-default.json /usr/share/octessera/defaults/pi-default.json 0644
install_overlay_file usr/share/octessera/samples/sample-manifest.tsv /usr/share/octessera/samples/sample-manifest.tsv 0644
install_overlay_file usr/share/octessera/samples/ATTRIBUTIONS.tsv /usr/share/octessera/samples/ATTRIBUTIONS.tsv 0644
install_overlay_file usr/share/octessera/samples/upstream/LICENSE /usr/share/octessera/samples/upstream/LICENSE 0644
install_overlay_file usr/share/octessera/samples/upstream/README.txt /usr/share/octessera/samples/upstream/README.txt 0644
install_orange_musical_assets "$overlay_dir" ""
install_overlay_file etc/systemd/system/octessera-setup.service /etc/systemd/system/octessera-setup.service 0644
if [[ "$OCTESSERA_IMAGE_MODE" == diagnostic ]]; then
  install_overlay_file etc/systemd/system/octessera-update-guard.service /etc/systemd/system/octessera-update-guard.service 0644
  install_overlay_file etc/systemd/system/octessera-update-recovery.service /etc/systemd/system/octessera-update-recovery.service 0644
  install_overlay_file etc/sudoers.d/octessera-update /etc/sudoers.d/octessera-update 0440
fi
if [[ "$OCTESSERA_IMAGE_MODE" == production ]]; then
  [[ -f "$overlay_dir/etc/udev/rules.d/70-octessera-orange-runtime.rules" && ! -L "$overlay_dir/etc/udev/rules.d/70-octessera-orange-runtime.rules" ]] || { echo "Missing exact Orange runtime udev rule." >&2; exit 1; }
  install_overlay_file etc/udev/rules.d/70-octessera-orange-runtime.rules /etc/udev/rules.d/70-octessera-orange-runtime.rules 0644
  install_overlay_file etc/systemd/system/octessera.service /etc/systemd/system/octessera.service 0644
  octessera_install_production_runtime "$overlay_dir"
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
rm -f /root/.ssh/authorized_keys /home/octessera/.ssh/authorized_keys
install -d -m 0755 /etc/ssh/sshd_config.d
cat >/etc/ssh/sshd_config.d/10-octessera-setup.conf <<'EOF'
PermitRootLogin no
PasswordAuthentication no
AllowUsers octessera
EOF
systemctl disable ssh.service >/dev/null 2>&1 || true
systemctl mask ssh.service >/dev/null 2>&1 || true
systemctl disable ssh.socket >/dev/null 2>&1 || true
systemctl mask ssh.socket >/dev/null 2>&1 || true
if systemctl list-unit-files sshd.service >/dev/null 2>&1; then
  systemctl disable sshd.service >/dev/null 2>&1 || true
  systemctl mask sshd.service >/dev/null 2>&1 || true
fi
if systemctl list-unit-files sshd.socket >/dev/null 2>&1; then
  systemctl disable sshd.socket >/dev/null 2>&1 || true
  systemctl mask sshd.socket >/dev/null 2>&1 || true
fi
rm -f /etc/ssh/ssh_host_*
systemctl disable --now serial-getty@ttyS0.service >/dev/null 2>&1 || true
systemctl mask serial-getty@ttyS0.service >/dev/null 2>&1 || true
systemctl enable octessera-setup.service >/dev/null
systemctl enable octessera-setup-request.path >/dev/null
systemctl enable octessera-orange-usb-gadget.service >/dev/null
systemctl enable octessera-device-apply-reboot.socket >/dev/null
systemctl enable octessera-provision-musical-default.service >/dev/null
systemctl enable octessera-orange-boot-splash.service >/dev/null
systemctl enable octessera-orange-oled-shutdown.service >/dev/null
systemctl enable octessera-orange-oled-suspend.service >/dev/null
if [[ "$OCTESSERA_IMAGE_MODE" == diagnostic ]]; then
  systemctl enable octessera-update-recovery.service >/dev/null
fi
if [[ "$OCTESSERA_IMAGE_MODE" == production ]]; then
  systemctl enable octessera.service >/dev/null
fi
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
OCTESSERA_SPI1_CS0_DTS_SHA256=${spi_dts_sha256}
OCTESSERA_SPI1_CS0_DTBO_SHA256=${spi_dtbo_sha256}
OCTESSERA_INPUT_ROUTING_DTS_SHA256=$(sha256sum "$input_routing_dts_image" | awk '{ print $1 }')
OCTESSERA_INPUT_ROUTING_DTBO_SHA256=$(sha256sum "$input_routing_dtbo" | awk '{ print $1 }')
OCTESSERA_PI_DEFAULT_SHA256=$(sha256sum /usr/share/octessera/defaults/pi-default.json | awk '{ print $1 }')
OCTESSERA_SAMPLES_MANIFEST_SHA256=$(sha256sum /usr/share/octessera/samples/sample-manifest.tsv | awk '{ print $1 }')
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

apt-get clean
rm -rf /var/lib/apt/lists/*
