#!/usr/bin/env bash

set -u

VALIDATOR_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

: "${FAILURES:=0}"
: "${BASELINE_FAILURES:=0}"

section() { printf '\n== %s ==\n' "$1"; }
tool_available() { command -v "$1" >/dev/null 2>&1; }
sudo_available() { tool_available sudo && sudo -n true >/dev/null 2>&1; }

record_failure() {
  FAILURES=$((FAILURES + 1))
  printf 'FAIL: %s\n' "$1"
}

run_sudo() {
  local label="$1"
  local command="$2"
  local status

  section "$label"
  printf '$ sudo %s\n' "$command"
  sudo -n bash -c "$command"
  status="$?"
  printf '[exit %s]\n' "$status"
  if [ "$status" -ne 0 ]; then
    record_failure "sudo check failed: $label (exit $status)"
  fi
  return 0
}

# shellcheck source=tools/orange-pi/opi-bringup-identity-validator.sh
source "$VALIDATOR_DIR/opi-bringup-identity-validator.sh"
# shellcheck source=tools/orange-pi/opi-bringup-hardware-validator.sh
source "$VALIDATOR_DIR/opi-bringup-hardware-validator.sh"

validate_baseline_paths() {
  local armbian_release_path="$1"
  local os_release_path="$2"
  local metadata_path="$3"
  local model_path="$4"
  local machine="$5"
  local boot_config_path="$6"
  local dtbo_path="$7"
  local i2c_node_path="$8"
  local i2c_controller_path="$9"
  local spi_node_path="${10}"
  local spi_controller_path="${11}"
  local gpio_device_root="${12}"
  local udc_root="${13}"
  local configfs_root="${14}"
  local before_failures="$FAILURES"

  section "qualification-critical passive baseline"
  validate_identity_paths \
    "$armbian_release_path" \
    "$os_release_path" \
    "$metadata_path" \
    "$model_path" \
    "$machine" \
    "$boot_config_path"
  validate_hardware_paths \
    "$dtbo_path" \
    "$i2c_node_path" \
    "$i2c_controller_path" \
    "$spi_node_path" \
    "$spi_controller_path" \
    "$gpio_device_root" \
    "$udc_root" \
    "$configfs_root"

  BASELINE_FAILURES=$((FAILURES - before_failures))
  printf 'PASSIVE_BASELINE_FAILURES=%s\n' "$BASELINE_FAILURES"
  [ "$BASELINE_FAILURES" -eq 0 ]
}

validate_baseline() {
  local machine

  machine="$(uname -m 2>/dev/null)"
  validate_baseline_paths \
    /etc/armbian-release \
    /etc/os-release \
    /etc/octessera/build-metadata.env \
    /proc/device-tree/model \
    "$machine" \
    /boot/armbianEnv.txt \
    /boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo \
    /dev/i2c-2 \
    /sys/bus/i2c/devices/i2c-2/of_node \
    /dev/spidev1.0 \
    /sys/bus/spi/devices/spi1.0/of_node \
    /dev \
    /sys/class/udc \
    /sys/kernel/config/usb_gadget
}

run_optional_sudo_checks() {
  if [ "$WITH_SUDO_CHECKS" -ne 1 ]; then
    return 0
  fi
  if [ "$BASELINE_FAILURES" -ne 0 ]; then
    section "sudo checks"
    printf 'suppressed because the passive baseline failed; no sudo side effects were attempted\n'
    return 0
  fi
  if ! sudo_available; then
    record_failure "sudo checks were requested but passwordless sudo is unavailable"
    return 0
  fi
  run_sudo "sudo libcomposite/configfs check" "modprobe libcomposite && (mountpoint -q /sys/kernel/config || mount -t configfs none /sys/kernel/config) && ls -la /sys/class/udc && ls -la /sys/kernel/config/usb_gadget"
  run_sudo "sudo gadget function modules" "modprobe usb_f_midi && modprobe usb_f_uac2 && modinfo usb_f_midi && modinfo usb_f_uac2"
}
