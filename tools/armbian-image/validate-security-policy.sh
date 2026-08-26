#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
customize="$root/userpatches/customize-image.sh"
overlay="$root/userpatches/overlay"
runtime_assets="$overlay/usr/local/lib/octessera/orange-runtime-assets-install.sh"
raspberry="$root/tools/pi-image/stage4-octessera/files/root"
workflow="$root/.github/workflows/armbian-image.yml"
image_mode="${OCTESSERA_IMAGE_MODE:?OCTESSERA_IMAGE_MODE must be set for Armbian validation}"

grep -q 'wifi_connect_artifact_dir=.*usr/local/share/octessera/wifi-connect' "$customize"
grep -q 'wifi_connect_expected_sha256=929a5b937a771a0e4f96446242af217c61118aedaaaa053aff75af61151c6acc' "$customize"
grep -q 'wifi_connect_patch_sha256=3481ef27637c5c4a176b59f74af4e2c232f6c67de8399eaf705fe6431ffc8939' "$customize"
octessera_reject_file_match 'Orange image must not download upstream wifi-connect.' -qE 'wifi-connect-aarch64-unknown-linux-gnu\.tar\.gz|github\.com/balena-os/wifi-connect/releases|curl[[:space:]].*wifi-connect|(^|[[:space:]])tar[[:space:]].*wifi-connect' "$customize"
grep -q 'network-manager.*dnsmasq.*wireless-tools.*iw' "$customize"
grep -q 'install_overlay_file usr/local/sbin/octessera-wifi-foundation' "$customize"
grep -q 'install_overlay_file etc/systemd/system/octessera-wifi-foundation.service' "$customize"
octessera_reject_file_match 'Orange image must not enable the inactive Wi-Fi unit.' -q 'enable.*octessera-wifi-foundation' "$customize"
grep -qF 'systemctl enable octessera-update-recovery.service >/dev/null' "$customize"
octessera_reject_file_match 'Image customization must not start update recovery in the chroot.' -qE 'systemctl[[:space:]]+enable[[:space:]]+--now[[:space:]]+octessera-update-recovery\.service' "$customize"
grep -qF 'rm -f /root/.ssh/authorized_keys /home/octessera/.ssh/authorized_keys' "$customize"
octessera_reject_file_match 'Image customization must not read or print authorized key contents.' -qE '(cat|read|printf|echo|grep|sha256sum).*authorized_keys|authorized_keys.*(cat|read|printf|echo|grep|sha256sum)' "$customize"
grep -q 'OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w' "$customize"
grep -q 'armbian_board.*orangepizero2w' "$customize"
grep -q 'device-tree-compiler' "$customize"
grep -q 'psmisc' "$customize"
grep -q 'dtc -@ -I dts -O dtb' "$customize"
grep -q 'fdtoverlay' "$customize"
grep -q '/boot/overlay-user' "$customize"
grep -q 'user_overlays=octessera-h618-spi1-oled-sd2' "$customize"
for symbol in CONFIG_MMC CONFIG_MMC_BLOCK CONFIG_MMC_SPI; do
  grep -q "kernel_config_value $symbol" "$customize"
done
grep -q 'printf.*mmc_spi.*sd_modules_load_file' "$customize"
grep -q 'octessera-ahub0-pcm5102' "$customize"
grep -qF "mv -f -- \"\$spi_dtbo_tmp\" \"\$spi_dtbo\"" "$customize"
grep -qF "mv -f -- \"\$armbian_env_tmp\" \"\$armbian_env\"" "$customize"
for metadata_key in OCTESSERA_SPI1_OLED_SD2_DTS_SHA256 OCTESSERA_SPI1_OLED_SD2_DTBO_SHA256 OCTESSERA_INPUT_ROUTING_DTS_SHA256 OCTESSERA_INPUT_ROUTING_DTBO_SHA256 OCTESSERA_AHUB0_PCM5102_DTS_SHA256 OCTESSERA_AHUB0_PCM5102_DTBO_SHA256; do
  grep -q "$metadata_key" "$customize"
done

hdmi_rsyslog="$overlay/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf"
expected_hdmi_rsyslog="if (\$msg == \"sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin\") then stop"
[[ -f "$hdmi_rsyslog" && ! -L "$hdmi_rsyslog" ]] || { echo 'Orange HDMI rsyslog drop-in must be a regular source file.' >&2; exit 1; }
[[ "$(cat -- "$hdmi_rsyslog")" == "$expected_hdmi_rsyslog" ]] || { echo 'Orange HDMI rsyslog drop-in content is not exact.' >&2; exit 1; }
grep -qF 'install_overlay_file etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf /etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf 0644' "$runtime_assets"
grep -qF 'octessera_validate_orange_rsyslog_configuration()' "$runtime_assets"
grep -qF "validation_config=\"\$(mktemp /tmp/octessera-rsyslog-validation.XXXXXX)\"" "$runtime_assets"
grep -qF 'global(net.enableDNS="off")' "$runtime_assets"
grep -qF 'include(file="/etc/rsyslog.conf")' "$runtime_assets"
grep -qF "if printf '%s\\n' 'global(net.enableDNS=\"off\")' 'include(file=\"/etc/rsyslog.conf\")' > \"\$validation_config\"; then" "$runtime_assets"
grep -qF "rsyslogd -N1 -f \"\$validation_config\"" "$runtime_assets"
grep -qF 'validation_status=$?' "$runtime_assets"
grep -qF "if rm -f -- \"\$validation_config\"; then" "$runtime_assets"
grep -qF "return \"\$validation_status\"" "$runtime_assets"
octessera_reject_file_match 'Orange rsyslog validation must not use removed rsyslogd -x.' -qF 'rsyslogd -x' "$runtime_assets"
octessera_reject_file_match 'Orange rsyslog validation must not mutate hostname or hosts.' -nE '(^|[^[:alnum:]_])(hostnamectl|hostname[[:space:]]+|/etc/(hosts|hostname))' "$runtime_assets"
octessera_reject_file_match 'Orange HDMI logging mitigation must not change global or journald limits.' -RInE --exclude='00-octessera-orange-hdmi-plugin.conf' --exclude='orange-runtime-assets-install.sh' '(^|/)(rsyslog\.conf|journald[^/]*)$|SystemMaxUse|RuntimeMaxUse|RateLimit|rateLimit|Storage=' "$overlay"
octessera_reject_file_match 'Orange HDMI logging mitigation must not change global or journald limits.' -nE '(install|cat|sed|printf|mv|rm).*(/etc/rsyslog\.conf|journald)|SystemMaxUse|RuntimeMaxUse|RateLimit|rateLimit|Storage=' "$customize"
octessera_reject_file_match 'Orange HDMI logging mitigation must not change global or journald limits.' -nE 'SystemMaxUse|RuntimeMaxUse|RateLimit|rateLimit|Storage=' "$runtime_assets"

input_provision="$root/tools/orange-pi/input-routing-provision.sh"
grep -q 'serial-getty@ttyS0.service' "$input_provision"
grep -q 'input-routing-backups' "$input_provision"
grep -q 'ssh_touched=0' "$input_provision"
gadget="$root/tools/orange-pi/orange-pi-usb-gadget.sh"
grep -q 'musb-hdrc.4.auto' "$gadget"
grep -q 'musb_hdrc' "$overlay/etc/modules-load.d/octessera-orange-usb-gadget.conf"
for line in 'octessera-orange-usb-gadget setup' 'octessera-orange-usb-gadget teardown'; do
  grep -q "$line" "$overlay/etc/systemd/system/octessera-orange-usb-gadget.service"
done

boot_hook="$overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
boot_script="$overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
grep -q 'copy_exec /usr/local/sbin/octessera-orange-oled-logo' "$boot_hook"
grep -q 'copy_file asset /usr/share/octessera/oled/octessera-pi-booting.rgb565' "$boot_hook"
grep -q 'copy_file asset /usr/share/octessera/oled/octessera-pi-shutdown.rgb565' "$boot_hook"
grep -q '^ConditionPathExists=/opt/octessera/current$' "$overlay/etc/systemd/system/octessera-orange-boot-splash.service"
octessera_reject_file_match 'Orange initramfs must not copy obsolete SVG assets.' -qE 'octessera-(mark|wordmark)\.svg' "$boot_hook"
grep -q 'setsid /usr/bin/python3 /usr/local/sbin/octessera-orange-oled-logo boot-static' "$boot_script"
octessera_reject_file_match 'Orange initramfs must not execute the renderer through its env shebang.' -q 'setsid /usr/local/sbin/octessera-orange-oled-logo boot-static' "$boot_script"
octessera_reject_file_match 'Orange initramfs must not depend on /usr/bin/env.' -q '/usr/bin/env' "$boot_script"
octessera_reject_file_match 'Orange initramfs must not use marker or animated one-shot coupling.' -qE 'MARKER|write_ready_marker|boot-once' "$boot_script"
grep -q 'octessera-orange-oled-suspend.service' "$runtime_assets"
grep -q 'install_overlay_file etc/systemd/system/octessera-orange-oled-shutdown.service' "$runtime_assets"
grep -q 'systemctl enable octessera-orange-oled-shutdown.service' "$runtime_assets"
octessera_reject_file_match 'Orange OLED lifecycle must use the installed /usr/local/sbin executable.' -RInF '/usr/local/bin/octessera-orange-oled-logo' "$customize" "$overlay/etc/systemd/system/octessera-orange-boot-splash.service"
shutdown_service="$overlay/etc/systemd/system/octessera-orange-oled-shutdown.service"
grep -qFx 'ExecStart=/bin/true' "$shutdown_service"
grep -qFx "ExecStop=/bin/sh -c 'sleep 4; /usr/local/sbin/octessera-orange-oled-logo off || true'" "$shutdown_service"
grep -qFx 'WantedBy=multi-user.target' "$shutdown_service"
octessera_reject_file_match 'Orange shutdown service must not use target choreography or write a shutdown logo.' -qE '^(Before=|WantedBy=(shutdown|reboot|halt)\.target$)|orange-oled-logo (shutdown|boot)' "$shutdown_service"
boot_service="$overlay/etc/systemd/system/octessera-orange-boot-splash.service"
for line in Type=notify NotifyAccess=main Environment=OCTESSERA_OLED_READY_NOTIFY_REQUIRED=1 RuntimeDirectory=octessera-boot DevicePolicy=closed 'DeviceAllow=/dev/spidev1.0 rw' 'DeviceAllow=/dev/gpiochip1 rw'; do
  grep -qFx "$line" "$boot_service"
done
grep -q 'ExecStart=/usr/local/sbin/octessera-orange-oled-logo boot-loop' "$boot_service"
grep -q '^After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target$' "$boot_service"
grep -q '^Before=sysinit.target octessera.service$' "$boot_service"
octessera_reject_file_match 'Orange boot splash must not conflict with runtime.' -q 'Conflicts=octessera.service' "$boot_service"
runtime_service="$overlay/etc/systemd/system/octessera.service"
setup_service="$overlay/etc/systemd/system/octessera-setup.service"
for tree in "$overlay" "$raspberry"; do
  grep -qFx 'NoNewPrivileges=yes' "$tree/etc/systemd/system/octessera.service"
  grep -qFx 'NoNewPrivileges=no' "$tree/etc/systemd/system/octessera-setup.service"
  grep -qFx 'RuntimeMaxSec=670s' "$tree/etc/systemd/system/octessera-setup.service"
  grep -qFx 'TimeoutStopSec=10s' "$tree/etc/systemd/system/octessera-setup.service"
done
grep -qFx '# dnsmasq needs privilege-transition capabilities to drop from root while serving the setup AP.' "$setup_service"
grep -q 'Wants=octessera-orange-boot-splash.service' "$runtime_service"
grep -q 'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' "$runtime_service"
for line in StartLimitIntervalSec=30s StartLimitBurst=3 Restart=on-failure RestartPreventExitStatus=78 RestartSec=5s; do
  grep -qFx "$line" "$runtime_service"
done
octessera_reject_file_match 'Orange runtime service must not restart always.' -qFx 'Restart=always' "$runtime_service"
grep -qFx 'Requires=octessera-device-apply-reboot.socket' "$runtime_service"
grep -qFx 'Requires=octessera-provision-musical-default.service' "$runtime_service"
grep -qFx 'After=octessera-device-apply-reboot.socket' "$runtime_service"
octessera_reject_file_match 'Orange runtime service has an unapproved failure dependency.' -qE '^(StartLimitAction|OnFailure|Requisite|BindsTo|PartOf)=' "$runtime_service"
[[ "$(grep -c '^Requires=' "$runtime_service")" == 3 ]]
grep -qF 'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot /run/octessera-setup-request/inbox' "$runtime_service"

grep -q 'octessera_install_diagnostic_payload' "$customize"
grep -q 'artifact_kind == "diagnostic-only"' "$overlay/usr/local/lib/octessera/diagnostic-payload.sh"
grep -q 'runtime_ready == false' "$overlay/usr/local/lib/octessera/diagnostic-payload.sh"
grep -q 'enable_runtime' "$overlay/usr/local/lib/octessera/diagnostic-payload.sh"
grep -q '"image_kind"' "$overlay/etc/octessera/image-contract.json"
for line in \
  'ExecStart=/usr/local/bin/octessera-pi' 'User=octessera-runtime' 'Group=octessera-runtime' \
  'Requires=octessera-device-apply-reboot.socket' 'Requires=octessera-provision-musical-default.service' \
  'Environment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w' \
  'Environment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets' \
  'Environment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples' \
  'Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json' \
  'RuntimeDirectory=octessera' 'NoNewPrivileges=yes' 'ProtectSystem=strict' \
  'TTYPath=/dev/tty1' 'TTYReset=yes' 'SupplementaryGroups=audio i2c spi gpio tty video' \
  'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot /run/octessera-setup-request/inbox' 'PrivateTmp=yes' 'ProtectHome=yes' \
  'LimitRTPRIO=70' 'LimitMEMLOCK=infinity' 'AmbientCapabilities=CAP_SYS_TTY_CONFIG' 'CapabilityBoundingSet=CAP_SYS_TTY_CONFIG'; do
  grep -qFx "$line" "$runtime_service"
done
octessera_reject_file_match 'Orange runtime service must not grant priority 80.' -qE 'LimitRTPRIO=80' "$runtime_service"
[[ "$(grep -E '^(AmbientCapabilities|CapabilityBoundingSet)=' "$runtime_service")" == $'AmbientCapabilities=CAP_SYS_TTY_CONFIG\nCapabilityBoundingSet=CAP_SYS_TTY_CONFIG' ]]
octessera_reject_file_match 'Orange runtime service contains a prohibited tty, device, graphics, or forced-mode directive.' -Eiq '^(StandardInput=tty|TTY(VHangup|VTDisallocate|Force|Fail)=|ExecStopPost=|DevicePolicy=|DeviceAllow=)|(^|[^[:alnum:]_])(Xorg|Wayland|Weston|sway|chvt|xrandr|wlr-randr|modetest|video=)([^[:alnum:]_]|$)' "$runtime_service"
expected_udev=$'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"'
[[ "$(cat -- "$overlay/etc/udev/rules.d/70-octessera-orange-runtime.rules")" == "$expected_udev" ]]
octessera_reject_file_match 'Orange runtime service must not block fixed hardware devices.' -qE '^(PrivateDevices|DevicePolicy)=' "$runtime_service"
grep -qF 'default: preset-firstrun octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize' "$workflow"
[[ "$(grep -cF "extensions: \${{ inputs.extensions }}" "$workflow")" == 2 ]]

octessera_reject_file_match 'Forbidden Raspberry Pi assumption or secret-like pattern found in Orange image sources.' -RInE --exclude-dir=doc '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|g_mass_storage|wpa_passphrase|BEGIN OPENSSH PRIVATE KEY|BEGIN RSA PRIVATE KEY|BEGIN PRIVATE KEY|default_password|changeme|raspberry)' "$overlay" "$workflow"
if find "$overlay" -path '*/.ssh/authorized_keys' -o -name 'ssh_host_*' | grep -q .; then
  find_pipeline_status=("${PIPESTATUS[@]}")
else
  find_pipeline_status=("${PIPESTATUS[@]}")
fi
if [[ "${find_pipeline_status[0]}" != 0 ]]; then
  echo "Unable to inspect Overlay SSH paths (find status ${find_pipeline_status[0]})." >&2
  exit 1
fi
if [[ "${find_pipeline_status[1]}" != 0 && "${find_pipeline_status[1]}" != 1 ]]; then
  echo "Unable to complete the Overlay SSH negative scan (grep status ${find_pipeline_status[1]})." >&2
  exit 1
fi
if [[ "${find_pipeline_status[1]}" == 0 ]]; then
  echo 'Overlay must not bake SSH keys or authorized keys.' >&2
  exit 1
fi
octessera_reject_file_match 'Workflow must not expose raw first-run secret inputs.' -nE '^      (wifi|wi-fi|password|ssh_key|private_key|authorized_keys|user):' "$workflow"

inspect_payload_tar() {
  local tar_path="$1" entry
  while IFS= read -r entry; do
    case "$entry" in
      /*|..|../*|*/..|*/../*) echo "Unsafe payload path: $entry" >&2; return 1 ;;
    esac
  done < <(tar -tf "$tar_path")
  while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; return 1 ;;
    esac
  done < <(tar -tvf "$tar_path")
}
payload_url="${PAYLOAD_URL:-${OCTESSERA_PAYLOAD_URL:-}}"
payload_sha256="${PAYLOAD_SHA256:-${OCTESSERA_PAYLOAD_SHA256:-}}"
if [[ "$image_mode" == production && ( -n "$payload_url" || -n "$payload_sha256" ) ]]; then
  echo 'Production Orange images do not accept payload URLs or payload hashes.' >&2
  exit 1
elif [[ -n "$payload_url" ]]; then
  [[ "$payload_url" == https://* ]] || { echo 'Payload URL must use HTTPS.' >&2; exit 1; }
  [[ "$payload_sha256" =~ ^[a-fA-F0-9]{64}$ ]] || { echo 'Payload SHA256 is required and must be 64 hex characters.' >&2; exit 1; }
  payload_work="$(mktemp -d)"
  trap 'rm -rf "$payload_work"' EXIT
  curl --fail --location --proto '=https' --tlsv1.2 --output "$payload_work/payload.tar" "$payload_url"
  echo "$payload_sha256  $payload_work/payload.tar" | sha256sum -c -
  inspect_payload_tar "$payload_work/payload.tar"
elif [[ -n "$payload_sha256" ]]; then
  echo 'Payload URL is required when payload SHA256 is set.' >&2
  exit 1
fi
preset_url="${PUBLIC_PRESET_CONFIGURATION_URL:-}"
if [[ -n "$preset_url" ]]; then
  [[ "$preset_url" == https://* ]] || { echo 'Public PRESET_CONFIGURATION URL must use HTTPS.' >&2; exit 1; }
  case " ${ARMBIAN_EXTENSIONS:-} " in
    *' preset-firstrun '*) ;;
    *) echo 'PRESET_CONFIGURATION requires the preset-firstrun extension.' >&2; exit 1 ;;
  esac
fi
