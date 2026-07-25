#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

overlay_dir=/tmp/overlay

armbian_board="$(awk -F= '$1 == "BOARD" { print $2; exit }' /etc/armbian-release 2>/dev/null || true)"
if [[ "$armbian_board" != orangepizero2w ]]; then
  echo "Refusing the Orange Pi SPI overlay for board: ${armbian_board:-unknown}." >&2
  exit 1
fi
if [[ ! -d "$overlay_dir" ]]; then
  echo "Expected Armbian userpatches overlay at $overlay_dir." >&2
  exit 1
fi
spi_dts="$overlay_dir/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts"
[[ -f "$spi_dts" ]] || { echo "Missing Orange Pi SPI overlay source." >&2; exit 1; }
install -d -m 0755 /etc/octessera /usr/local/sbin /usr/local/lib/octessera /var/lib/octessera/samples

apt-get update
apt-get install -y --no-install-recommends ca-certificates coreutils curl device-tree-compiler tar xz-utils jq gpiod alsa-utils i2c-tools network-manager dnsmasq wireless-tools iw python3-minimal openssh-server sudo unzip util-linux psmisc

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

if grep -RInE '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|usb[_-]?gadget|g_mass_storage)' "$overlay_dir"; then
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
boot_dtb_helper="$overlay_dir/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
[[ -f "$boot_dtb_helper" ]] || { echo "Missing boot DTB selection helper." >&2; exit 1; }
# shellcheck source=userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh
source "$boot_dtb_helper"

spi_overlay_name=octessera-h618-spi1-cs0
spi_user_overlay_assignment='user_overlays=octessera-h618-spi1-cs0'
spi_user_overlay_token="${spi_user_overlay_assignment#*=}"
spi_overlay_dir=/boot/overlay-user
spi_dtbo="$spi_overlay_dir/$spi_overlay_name.dtbo"
spi_dts_image=/usr/local/share/octessera/device-tree/$spi_overlay_name.dts
spi_work="$(mktemp -d)"
spi_dtbo_tmp=
spi_dts_tmp=
armbian_env_tmp=
work=
cleanup() {
  rm -rf "${spi_work:-}" "${work:-}"
  rm -f "${spi_dtbo_tmp:-}" "${spi_dts_tmp:-}" "${armbian_env_tmp:-}"
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
octessera_run_strict_diagnostic "$spi_work" inspect_merged_spi_overlay dtc -I dtb -O dts -o "$spi_work/$spi_overlay_name-merged.dts" "$spi_merged_dtb" || exit 1

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

install -d -m 0755 "$(dirname "$spi_dts_image")"
spi_dts_tmp="$(mktemp "${spi_dts_image}.XXXXXX")"
install -m 0644 -o root -g root "$spi_dts" "$spi_dts_tmp"
mv -f -- "$spi_dts_tmp" "$spi_dts_image"
spi_dts_tmp=
chmod 0644 "$spi_dtbo_tmp"
chown root:root "$spi_dtbo_tmp"
mv -f -- "$spi_dtbo_tmp" "$spi_dtbo"
spi_dtbo_tmp=

armbian_env_tmp="$(mktemp "${armbian_env}.XXXXXX")"
if ! octessera_armbian_env_update "$armbian_env" "$armbian_env_tmp" "$spi_user_overlay_token" i2c1-pi; then
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

spi_dts_sha256="$(sha256sum "$spi_dts_image" | awk '{ print $1 }')"
spi_dtbo_sha256="$(sha256sum "$spi_dtbo" | awk '{ print $1 }')"

for updater_file in \
  usr/local/sbin/octessera-update \
  usr/local/sbin/octessera-update-guard \
  usr/local/sbin/octessera-update-recovery \
  usr/local/lib/octessera/updater_protocol.py \
  usr/local/lib/octessera/updater_state.py \
  usr/local/lib/octessera/updater_assets.py \
  usr/local/lib/octessera/updater_guard.py \
  usr/local/lib/octessera/updater_cli.py \
  etc/systemd/system/octessera-update-guard.service \
  etc/systemd/system/octessera-update-recovery.service; do
  [[ -f "$overlay_dir/$updater_file" ]] || { echo "Missing updater protocol overlay: $updater_file" >&2; exit 1; }
done
install_overlay_file etc/octessera/armbian-image.txt /etc/octessera/armbian-image.txt 0644
install_overlay_file usr/local/sbin/octessera-armbian-diagnostics /usr/local/sbin/octessera-armbian-diagnostics 0755
install_overlay_file usr/local/sbin/octessera-update /usr/local/sbin/octessera-update 0755
install_overlay_file usr/local/sbin/octessera-update-guard /usr/local/sbin/octessera-update-guard 0755
install_overlay_file usr/local/sbin/octessera-update-recovery /usr/local/sbin/octessera-update-recovery 0755
install_overlay_file usr/local/lib/octessera/updater_protocol.py /usr/local/lib/octessera/updater_protocol.py 0644
install_overlay_file usr/local/lib/octessera/updater_state.py /usr/local/lib/octessera/updater_state.py 0644
install_overlay_file usr/local/lib/octessera/updater_assets.py /usr/local/lib/octessera/updater_assets.py 0644
install_overlay_file usr/local/lib/octessera/updater_guard.py /usr/local/lib/octessera/updater_guard.py 0644
install_overlay_file usr/local/lib/octessera/updater_cli.py /usr/local/lib/octessera/updater_cli.py 0644
install_overlay_file usr/local/sbin/octessera-wifi-connect /usr/local/sbin/octessera-wifi-connect 0755
install_overlay_file usr/local/sbin/octessera-setup-sidecar /usr/local/sbin/octessera-setup-sidecar 0755
install_overlay_file etc/systemd/system/octessera-setup.service /etc/systemd/system/octessera-setup.service 0644
install_overlay_file etc/systemd/system/octessera-update-guard.service /etc/systemd/system/octessera-update-guard.service 0644
install_overlay_file etc/systemd/system/octessera-update-recovery.service /etc/systemd/system/octessera-update-recovery.service 0644
install_overlay_file etc/sudoers.d/octessera-update /etc/sudoers.d/octessera-update 0440
if [[ -d "$overlay_dir/usr/local/share/octessera-setup-ui" ]]; then
  cp -a "$overlay_dir/usr/local/share/octessera-setup-ui" /usr/local/share/
fi

if ! id octessera >/dev/null 2>&1; then
  useradd --create-home --shell /bin/bash --groups sudo octessera
fi
passwd -l octessera >/dev/null || true
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
systemctl enable octessera-setup.service >/dev/null
systemctl enable --now octessera-update-recovery.service >/dev/null

cat >/etc/octessera/build-metadata.env <<EOF
OCTESSERA_IMAGE_KIND=armbian
OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w
OCTESSERA_IMAGE_BUILT_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
OCTESSERA_RUNTIME_ENABLED_DEFAULT=false
OCTESSERA_SPI1_CS0_DTS_SHA256=${spi_dts_sha256}
OCTESSERA_SPI1_CS0_DTBO_SHA256=${spi_dtbo_sha256}
EOF

payload_url="${OCTESSERA_PAYLOAD_URL:-}"
payload_sha256="${OCTESSERA_PAYLOAD_SHA256:-}"
if [[ -n "$payload_url" ]]; then
  [[ "$payload_url" == https://* ]] || { echo "OCTESSERA_PAYLOAD_URL must use HTTPS." >&2; exit 1; }
  [[ "$payload_sha256" =~ ^[a-fA-F0-9]{64}$ ]] || { echo "OCTESSERA_PAYLOAD_SHA256 is required." >&2; exit 1; }
  work="$(mktemp -d)"
  curl --fail --location --proto '=https' --tlsv1.2 --output "$work/payload.tar" "$payload_url"
  echo "$payload_sha256  $work/payload.tar" | sha256sum -c -
  tar -tf "$work/payload.tar" | while IFS= read -r entry; do
    case "$entry" in
      /*|..|../*|*/..|*/../*) echo "Unsafe payload path: $entry" >&2; exit 1 ;;
    esac
  done
  tar -tvf "$work/payload.tar" | while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; exit 1 ;;
    esac
  done
  mkdir "$work/extract"
  tar -xf "$work/payload.tar" -C "$work/extract" --no-same-owner --no-same-permissions
  if [[ -f "$work/extract/octessera-payload.json" ]]; then
    jq -e '.name == "octessera-armbian-payload"' "$work/extract/octessera-payload.json" >/dev/null
    install -D -m 0644 "$work/extract/octessera-payload.json" /etc/octessera/payload.json
    jq -e '.artifact_kind == "diagnostic-only" and .runtime_ready == false and (.enable_runtime // false) == false' "$work/extract/octessera-payload.json" >/dev/null || {
      echo "Orange Pi payloads must be explicitly diagnostic-only and runtime-disabled." >&2
      exit 1
    }
    if find "$work/extract" -type f -name octessera-pi -print -quit | grep -q .; then
      echo "Orange Pi diagnostic images reject octessera-pi runtime payloads." >&2
      exit 1
    fi
    install -d -m 0755 /usr/local/lib/octessera/payload-staged
    cp -a "$work/extract/." /usr/local/lib/octessera/payload-staged/
  else
    echo "Payload is missing octessera-payload.json." >&2
    exit 1
  fi
fi

if [[ -n "${PUBLIC_PRESET_CONFIGURATION_URL:-}" ]]; then
  export PRESET_CONFIGURATION="$PUBLIC_PRESET_CONFIGURATION_URL"
fi

apt-get clean
rm -rf /var/lib/apt/lists/*
