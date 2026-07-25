#!/usr/bin/env bash

set -u

OUTPUT_DIR="/tmp/octessera-opi-bringup"
WITH_SUDO_CHECKS=0
FAILURES=0
WARNINGS=0

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

while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-dir)
      [ "$#" -ge 2 ] || { echo "missing value for --output-dir" >&2; exit 2; }
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --with-sudo-checks)
      WITH_SUDO_CHECKS=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

mkdir -p "$OUTPUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
LOG_FILE="$OUTPUT_DIR/opi-bringup-$STAMP.log"
printf '%s\n' "$LOG_FILE" > "$OUTPUT_DIR/latest-log-path"
exec > >(tee -a "$LOG_FILE") 2>&1

USB_CONFIG_RE='CONFIGFS_FS|USB_LIBCOMPOSITE|USB_CONFIGFS|USB_F_UAC2|USB_F_MIDI'

section() {
  printf '\n== %s ==\n' "$1"
}

run() {
  label="$1"
  command="$2"
  section "$label"
  printf '$ %s\n' "$command"
  bash -lc "$command"
  status="$?"
  printf '[exit %s]\n' "$status"
  return 0
}

run_optional() {
  label="$1"
  command="$2"
  if bash -lc "command -v ${command%% *} >/dev/null 2>&1"; then
    run "$label" "$command"
  else
    section "$label"
    printf 'missing command: %s\n' "${command%% *}"
  fi
}

sudo_available() {
  command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1
}

run_sudo() {
  label="$1"
  command="$2"
  section "$label"
  if ! sudo_available; then
    echo "sudo -n is not available; skipping"
    return 0
  fi
  printf '$ sudo %s\n' "$command"
  sudo -n bash -lc "$command"
  status="$?"
  printf '[exit %s]\n' "$status"
}

record_failure() {
  FAILURES=$((FAILURES + 1))
  printf 'FAIL: %s\n' "$1"
}

record_warning() {
  WARNINGS=$((WARNINGS + 1))
  printf 'WARN: %s\n' "$1"
}

validate_static_gates() {
  section "bring-up gates"
  if [ ! -f /boot/armbianEnv.txt ]; then
    record_warning "missing /boot/armbianEnv.txt; confirm this is the intended Armbian image"
  fi
  if [ ! -d /sys/class/udc ]; then
    record_failure "missing /sys/class/udc; USB gadget/device mode is not available"
  elif [ -z "$(ls -A /sys/class/udc 2>/dev/null)" ]; then
    record_failure "empty /sys/class/udc; USB gadget/device mode is not ready"
  else
    printf 'UDC candidates:\n'
    ls -1 /sys/class/udc
  fi
  command -v gpioinfo >/dev/null 2>&1 || record_warning "gpioinfo missing; install gpiod for gpiochip line mapping"
  command -v aplay >/dev/null 2>&1 || record_warning "aplay missing; install alsa-utils for audio checks"
  command -v i2cdetect >/dev/null 2>&1 || record_warning "i2cdetect missing; install i2c-tools for I2C adapter checks"
}

section "octessera Orange Pi bring-up probe"
printf 'LOG_FILE=%s\n' "$LOG_FILE"
printf 'WITH_SUDO_CHECKS=%s\n' "$WITH_SUDO_CHECKS"

run "identity" "date -Is; hostname; whoami; id; pwd; uptime"
run "os release" "cat /etc/os-release 2>/dev/null || true; cat /etc/armbian-release 2>/dev/null || true"
run "kernel and model" "uname -a; cat /proc/device-tree/model 2>/dev/null || true; tr '\0' '\n' </proc/device-tree/compatible 2>/dev/null || true"
run "boot config" "cat /boot/armbianEnv.txt 2>/dev/null || true; cat /boot/extlinux/extlinux.conf 2>/dev/null || true"
run "overlay files" "ls -la /boot/overlay-user 2>/dev/null || true; ls -la /boot/dtb/overlay /boot/dtb/*/overlay 2>/dev/null || true"
run "device nodes" "ls -l /dev/i2c-* /dev/spidev* /dev/gpiochip* /dev/snd/* 2>/dev/null || true"
run "usb udc and roles" "ls -la /sys/class/udc 2>/dev/null || true; find /sys -maxdepth 6 -type f \( -name dr_mode -o -name role -o -name mode \) -print -exec cat {} \; 2>/dev/null | head -n 200"
run "configfs state" "mount | grep configfs || true; ls -la /sys/kernel/config/usb_gadget 2>/dev/null || true"
run "kernel USB gadget config" "zgrep -E '$USB_CONFIG_RE' /proc/config.gz 2>/dev/null || true; grep -E '$USB_CONFIG_RE' /boot/config-\$(uname -r) 2>/dev/null || true"
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

validate_static_gates

if [ "$WITH_SUDO_CHECKS" -eq 1 ]; then
  run_sudo "sudo libcomposite/configfs check" "modprobe libcomposite >/dev/null 2>&1 || true; mountpoint -q /sys/kernel/config || mount -t configfs none /sys/kernel/config; ls -la /sys/class/udc; ls -la /sys/kernel/config/usb_gadget 2>/dev/null || true"
  run_sudo "sudo gadget function modules" "for m in usb_f_midi usb_f_uac2; do modprobe \$m >/dev/null 2>&1 || true; modinfo \$m 2>/dev/null | sed -n '1,8p'; done"
fi

section "operator reminders"
cat <<'EOF'
- Do not attach the Octessera PCB until power/header compatibility is checked with the board revision in hand.
- Empty /sys/class/udc is a failed USB gadget gate for Orange Pi bring-up.
- I2S on the Raspberry Pi physical pins is not proven by the desk docs yet.
- Use Armbian /boot/armbianEnv.txt overlays, not Raspberry Pi config.txt dtoverlay names.
- GPIO mapping must use gpiochip lines; do not use Raspberry Pi BCM numbering.
- USB gadget tests belong to orange-pi-usb-gadget.sh with an explicit UDC; this probe never binds a gadget.
EOF

section "done"
printf 'LOG_FILE=%s\n' "$LOG_FILE"
printf 'WARNINGS=%s\n' "$WARNINGS"
printf 'FAILURES=%s\n' "$FAILURES"

if [ "$FAILURES" -gt 0 ]; then
  exit 1
fi
