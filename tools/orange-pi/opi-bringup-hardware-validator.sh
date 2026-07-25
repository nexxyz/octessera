#!/usr/bin/env bash

EXPECTED_I2C_CONTROLLER_SUFFIX="/i2c@5002400"
EXPECTED_SPI_CONTROLLER_SUFFIX="/spi@5011000/spidev@0"
EXPECTED_GPIO_LABEL="300b000.pinctrl"
EXPECTED_GPIO_RESET_OFFSET="76"
EXPECTED_GPIO_DC_OFFSET="270"

require_path() {
  local path="$1"
  local label="$2"

  if [ ! -e "$path" ]; then
    record_failure "$label is missing: $path"
  fi
}

require_controller() {
  local path="$1"
  local expected="$2"
  local label="$3"
  local resolved

  if [ ! -e "$path" ]; then
    record_failure "$label controller evidence is missing: $path"
    return 0
  fi
  resolved="$(readlink -f -- "$path" 2>/dev/null)"
  case "$resolved" in
    *"$expected") ;;
    *) record_failure "$label does not resolve to $expected (found: ${resolved:-unresolved})" ;;
  esac
}

check_gpio_line() {
  local gpioinfo_output="$1"
  local line_number="$2"
  local matching_lines
  local line_count

  matching_lines="$(printf '%s\n' "$gpioinfo_output" | awk -v expected_line="$line_number" '
    $1 == "line" {
      offset = $2
      sub(/:$/, "", offset)
      if (offset == expected_line) {
        print
      }
    }
  ')"
  if [ -z "$matching_lines" ]; then
    record_failure "GPIO offset $line_number is missing from gpioinfo evidence"
    return 0
  fi
  line_count="$(printf '%s\n' "$matching_lines" | awk 'END { print NR }')"
  if [ "$line_count" -ne 1 ]; then
    record_failure "GPIO offset $line_number occurs $line_count times in gpioinfo evidence"
  elif printf '%s\n' "$matching_lines" | grep -Eq "^[[:space:]]*line[[:space:]]+${line_number}[[:space:]]*:[[:space:]]*unnamed[[:space:]]+input[[:space:]]+consumer=[^[:space:]]+[[:space:]]*$"; then
    record_failure "GPIO offset $line_number is owned: $matching_lines"
  elif ! printf '%s\n' "$matching_lines" | grep -Eq "^[[:space:]]*line[[:space:]]+${line_number}[[:space:]]*:[[:space:]]*unnamed[[:space:]]+input[[:space:]]*$"; then
    record_failure "GPIO offset $line_number ownership is unknown: $matching_lines"
  fi
}

GPIO_CHIP_NAME=""
privileged_gpiodetect() { sudo -n gpiodetect 2>&1; }
privileged_gpioinfo() { sudo -n gpioinfo -c "$1" 2>&1; }
check_gpio_evidence() {
  local detect_status
  local detect_output
  local gpioinfo_status
  local gpioinfo_output
  local gpiochip_name

  GPIO_CHIP_NAME=""
  if ! sudo_available; then
    record_failure "GPIO evidence is unproven: passwordless sudo is unavailable"
    return 0
  fi
  if ! tool_available gpiodetect; then
    return 0
  fi
  detect_output="$(privileged_gpiodetect)"
  detect_status="$?"
  printf '\n-- gpiodetect evidence --\n%s\n[exit %s]\n' "$detect_output" "$detect_status"
  if [ "$detect_status" -ne 0 ]; then
    record_failure "gpiodetect failed with exit $detect_status"
    return 0
  fi
  if ! gpiochip_name="$(printf '%s\n' "$detect_output" | awk -v expected_label="$EXPECTED_GPIO_LABEL" '
    $2 == "[" expected_label "]" {
      match_count++
      if (match_count == 1) {
        print $1
      }
    }
    END { exit(match_count == 1 ? 0 : 1) }
  ')"; then
    record_failure "canonical GPIO controller label is missing or not unique: $EXPECTED_GPIO_LABEL"
    return 0
  fi
  GPIO_CHIP_NAME="$gpiochip_name"

  if ! tool_available gpioinfo; then
    return 0
  fi
  gpioinfo_output="$(privileged_gpioinfo "$GPIO_CHIP_NAME")"
  gpioinfo_status="$?"
  printf '\n-- gpioinfo -c %s evidence --\n%s\n[exit %s]\n' "$GPIO_CHIP_NAME" "$gpioinfo_output" "$gpioinfo_status"
  if [ "$gpioinfo_status" -ne 0 ]; then
    record_failure "gpioinfo failed for $GPIO_CHIP_NAME with exit $gpioinfo_status"
    return 0
  fi
  check_gpio_line "$gpioinfo_output" "$EXPECTED_GPIO_RESET_OFFSET"
  check_gpio_line "$gpioinfo_output" "$EXPECTED_GPIO_DC_OFFSET"
}

check_required_tools() {
  local tool
  local -a required_tools=(gpiodetect gpioinfo i2cdetect aplay fuser pgrep systemctl)

  for tool in "${required_tools[@]}"; do
    if ! tool_available "$tool"; then
      record_failure "required evidence tool is missing: $tool"
    fi
  done
}

check_udc_evidence() {
  local udc_root="$1"
  local configfs_root="$2"
  local udc_path
  local gadget_path
  local function_value
  local gadget_value
  local udc_count=0

  if [ ! -d "$udc_root" ]; then
    record_failure "UDC class is missing: $udc_root"
    return 0
  fi
  for udc_path in "$udc_root"/*; do
    [ -d "$udc_path" ] || continue
    udc_count=$((udc_count + 1))
    printf 'UDC candidate: %s\n' "${udc_path##*/}"
    if [ ! -r "$udc_path/function" ]; then
      record_failure "cannot establish that UDC is unbound: $udc_path/function"
      continue
    fi
    function_value="$(cat "$udc_path/function" 2>/dev/null)"
    if [ -n "$function_value" ]; then
      record_failure "UDC is already bound: ${udc_path##*/} -> $function_value"
    fi

    if [ -d "$configfs_root" ]; then
      for gadget_path in "$configfs_root"/*; do
        [ -d "$gadget_path" ] || continue
        [ -r "$gadget_path/UDC" ] || continue
        gadget_value="$(cat "$gadget_path/UDC" 2>/dev/null)"
        if [ "$gadget_value" = "${udc_path##*/}" ]; then
          record_failure "configfs gadget is bound to UDC ${udc_path##*/}: ${gadget_path##*/}"
        fi
      done
    fi
  done
  if [ "$udc_count" -eq 0 ]; then
    record_failure "UDC class is empty: $udc_root"
  fi
}

privileged_fuser() { sudo -n fuser -v "$@" 2>&1; }
check_ownership_evidence() {
  local i2c_node_path="$1"
  local spi_node_path="$2"
  local gpio_node_path="$3"
  local service_output
  local service_status
  local process_output
  local process_status
  local owner_output
  local owner_status
  local unit
  local -a owner_paths=("$i2c_node_path" "$spi_node_path")

  if tool_available systemctl; then
    service_output="$(systemctl list-unit-files --type=service --no-legend 2>&1)"
    service_status="$?"
    printf '\n-- service evidence --\n%s\n[exit %s]\n' "$service_output" "$service_status"
    if [ "$service_status" -ne 0 ]; then
      record_failure "could not inspect system services"
    else
      for unit in octessera.service octessera-pi.service octessera-runtime.service; do
        if printf '%s\n' "$service_output" | awk -v expected_unit="$unit" '$1 == expected_unit { found = 1 } END { exit(found ? 0 : 1) }'; then
          record_failure "Octessera runtime service is installed: $unit"
        fi
      done
    fi
  fi

  if tool_available pgrep; then
    process_output="$(pgrep -af '(^|/)(octessera|octessera-pi|octessera-runtime)([[:space:]]|$)' 2>&1)"
    process_status="$?"
    printf '\n-- process evidence --\n%s\n[exit %s]\n' "$process_output" "$process_status"
    case "$process_status" in
      0) record_failure "Octessera runtime process is present: $process_output" ;;
      1) ;;
      *) record_failure "could not inspect Octessera processes" ;;
    esac
  fi

  if ! sudo_available; then
    record_failure "target-device owner absence is unproven: passwordless sudo is unavailable"
    return 0
  fi
  if [ -n "$gpio_node_path" ]; then
    owner_paths+=("$gpio_node_path")
  fi
  owner_output="$(privileged_fuser "${owner_paths[@]}")"
  owner_status="$?"
  printf '\n-- privileged target-device owner evidence --\n%s\n[exit %s]\n' "$owner_output" "$owner_status"
  case "$owner_status" in
    0) record_failure "a process owns a target device: $owner_output" ;;
    1) ;;
    *) record_failure "could not inspect target-device owners (privileged read-only check failed)" ;;
  esac
}

validate_hardware_paths() {
  local dtbo_path="$1"
  local i2c_node_path="$2"
  local i2c_controller_path="$3"
  local spi_node_path="$4"
  local spi_controller_path="$5"
  local gpio_device_root="$6"
  local udc_root="$7"
  local configfs_root="$8"
  local gpio_node_path

  require_path "$dtbo_path" "reviewed SPI1/CS0 overlay"
  require_path "$i2c_node_path" "expected I2C device node"
  require_controller "$i2c_controller_path" "$EXPECTED_I2C_CONTROLLER_SUFFIX" "expected I2C controller"
  require_path "$spi_node_path" "expected SPI device node"
  require_controller "$spi_controller_path" "$EXPECTED_SPI_CONTROLLER_SUFFIX" "expected SPI controller"
  check_required_tools
  check_gpio_evidence
  if [ -n "$GPIO_CHIP_NAME" ]; then
    gpio_node_path="$gpio_device_root/$GPIO_CHIP_NAME"
    require_path "$gpio_node_path" "canonical GPIO device node"
    if [ ! -e "$gpio_node_path" ]; then
      record_failure "GPIO ownership is unproven: canonical GPIO device node is missing: $gpio_node_path"
      gpio_node_path=""
    fi
  else
    record_failure "GPIO ownership is unproven: canonical GPIO device node is unresolved"
    gpio_node_path=""
  fi
  check_udc_evidence "$udc_root" "$configfs_root"
  check_ownership_evidence "$i2c_node_path" "$spi_node_path" "$gpio_node_path"
}
