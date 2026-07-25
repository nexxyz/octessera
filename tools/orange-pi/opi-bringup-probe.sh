#!/usr/bin/env bash

set -u

export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=tools/orange-pi/opi-bringup-validator.sh
source "$SCRIPT_DIR/opi-bringup-validator.sh"

OUTPUT_DIR="/tmp/octessera-opi-bringup"
WITH_SUDO_CHECKS=0

usage() {
  cat <<'EOF'
Usage: opi-bringup-probe.sh [OPTIONS]

Collect first-boot Orange Pi Zero 2W / Armbian bring-up facts for Octessera.
The default mode is read-only and does not bind USB gadget functions.

Options:
  --output-dir <path>          Remote directory for the timestamped log
                               (default: /tmp/octessera-opi-bringup)
  --with-sudo-checks          Try sudo-only module/configfs checks. This may
                               load gadget modules and mount configfs.
  -h, --help                  Show this help.
EOF
}

run() {
  local label="$1"
  local command="$2"
  local status

  section "$label"
  printf '$ %s\n' "$command"
  bash -c "$command"
  status="$?"
  printf '[exit %s]\n' "$status"
  return 0
}

run_optional() {
  local label="$1"
  local command="$2"

  if tool_available "${command%% *}"; then
    run "$label" "$command"
  else
    section "$label"
    printf 'missing command: %s\n' "${command%% *}"
  fi
}

main() {
  local stamp
  local log_file

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --output-dir)
        [ "$#" -ge 2 ] || { echo "missing value for --output-dir" >&2; return 2; }
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --with-sudo-checks)
        WITH_SUDO_CHECKS=1
        shift
        ;;
      -h|--help)
        usage
        return 0
        ;;
      *)
        echo "unknown option: $1" >&2
        usage >&2
        return 2
        ;;
    esac
  done

  if ! mkdir -p "$OUTPUT_DIR"; then
    echo "could not create output directory: $OUTPUT_DIR" >&2
    return 1
  fi
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  log_file="$OUTPUT_DIR/opi-bringup-$stamp.log"
  printf '%s\n' "$log_file" > "$OUTPUT_DIR/latest-log-path"
  exec > >(tee -a "$log_file") 2>&1

  section "octessera Orange Pi bring-up probe"
  printf 'LOG_FILE=%s\n' "$log_file"
  printf 'WITH_SUDO_CHECKS=%s\n' "$WITH_SUDO_CHECKS"

  run "identity" "date -Is; hostname; whoami; id; pwd; uptime"
  run "os release" "cat /etc/os-release 2>/dev/null || true; cat /etc/armbian-release 2>/dev/null || true"
  run "kernel and model" "uname -a; cat /proc/device-tree/model 2>/dev/null || true; tr '\0' '\n' </proc/device-tree/compatible 2>/dev/null || true"
  run "boot config" "cat /boot/armbianEnv.txt 2>/dev/null || true; cat /boot/extlinux/extlinux.conf 2>/dev/null || true"
  run "artifact metadata" "cat /etc/octessera/build-metadata.env 2>/dev/null || true"
  run "overlay files" "ls -la /boot/overlay-user 2>/dev/null || true; ls -la /boot/dtb/overlay /boot/dtb/*/overlay 2>/dev/null || true"
  run "device nodes" "ls -l /dev/i2c-* /dev/spidev* /dev/gpiochip* /dev/snd/* 2>/dev/null || true"
  run "usb udc and roles" "ls -la /sys/class/udc 2>/dev/null || true; find /sys -maxdepth 6 -type f \( -name dr_mode -o -name role -o -name mode \) -print -exec cat {} \; 2>/dev/null | head -n 200"
  run "configfs state" "mount | grep configfs || true; ls -la /sys/kernel/config/usb_gadget 2>/dev/null || true"
  run "kernel USB gadget config" "USB_CONFIG_RE='CONFIGFS_FS|USB_LIBCOMPOSITE|USB_CONFIGFS|USB_F_UAC2|USB_F_MIDI'; zgrep -E \"\$USB_CONFIG_RE\" /proc/config.gz 2>/dev/null || true; grep -E \"\$USB_CONFIG_RE\" /boot/config-\$(uname -r) 2>/dev/null || true"
  run "gadget modules" "lsmod | grep -E 'libcomposite|usb_f_|uac|midi|dwc2|musb|sunxi' || true; for m in libcomposite usb_f_midi usb_f_uac2 g_serial dwc2; do modinfo \$m 2>/dev/null | sed -n '1,8p'; done"
  run "module function directory" "ls -la /lib/modules/\$(uname -r)/kernel/drivers/usb/gadget/function 2>/dev/null || true"
  run_optional "i2c adapters" "i2cdetect -l"
  run_optional "gpio chips" "gpioinfo"
  run_optional "alsa cards" "aplay -l"
  run_optional "alsa pcm names" "aplay -L"
  run_optional "cpu and thermal" "lscpu"
  run "thermal zones" "for z in /sys/class/thermal/thermal_zone*; do [ -e \$z ] || continue; echo \$z; cat \$z/type 2>/dev/null || true; cat \$z/temp 2>/dev/null || true; done"
  run "network" "ip -brief addr; ip route; systemctl is-active ssh 2>/dev/null || true; systemctl is-active NetworkManager 2>/dev/null || true; systemctl is-active systemd-networkd 2>/dev/null || true"
  run "users and groups" "getent passwd orangepi pi root 2>/dev/null || true; getent group audio gpio i2c spi dialout sudo 2>/dev/null || true"

  validate_baseline || true
  run_optional_sudo_checks

  section "operator reminders"
  cat <<'EOF'
- This passive probe does not establish recovery-path, electrical-safety, physical-port, or I2S mapping gates.
- Do not attach the Octessera PCB until power/header compatibility is checked with the board revision in hand.
- Empty or bound /sys/class/udc is a failed USB gadget bring-up gate.
- I2S on the Raspberry Pi physical pins is not proven by the desk docs yet.
- Use Armbian /boot/armbianEnv.txt overlays, not Raspberry Pi config.txt dtoverlay names.
- GPIO mapping must use the canonical gpiochip controller label and offsets; do not use Raspberry Pi BCM numbering.
- USB gadget tests belong to orange-pi-usb-gadget.sh with an explicit controller; this probe never binds a gadget.
EOF

  section "done"
  printf 'LOG_FILE=%s\n' "$log_file"
  printf 'FAILURES=%s\n' "$FAILURES"

  if [ "$FAILURES" -gt 0 ]; then
    return 1
  fi
  return 0
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
