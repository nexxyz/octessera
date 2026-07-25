#!/usr/bin/env bash

octessera_run_strict_diagnostic() {
  local work_dir="$1"
  local label="$2"
  shift 2
  local diagnostic="$work_dir/$label.stderr"
  if ! "$@" 2>"$diagnostic"; then
    cat "$diagnostic" >&2
    echo "${label} failed." >&2
    return 1
  fi
  if [[ -s "$diagnostic" ]]; then
    cat "$diagnostic" >&2
    echo "${label} emitted diagnostics." >&2
    return 1
  fi
}

octessera_normalize_fdt_numbers() {
  local values="$1"
  local value
  local decimal
  local -a value_list
  local normalized=
  read -r -a value_list <<< "$values"
  for value in "${value_list[@]}"; do
    if [[ "$value" =~ ^0[xX][0-9a-fA-F]+$ ]]; then
      decimal=$((16#${value:2}))
    elif [[ "$value" =~ ^[0-9]+$ ]]; then
      decimal=$((10#$value))
    elif [[ "$value" =~ ^[0-9a-fA-F]+$ ]]; then
      decimal=$((16#$value))
    else
      echo "Invalid FDT numeric value: $value." >&2
      return 1
    fi
    normalized+="$(printf '%u ' "$decimal")"
  done
  printf '%s' "${normalized% }"
}

octessera_require_fdt_string() {
  local image="$1"
  local path="$2"
  local property="$3"
  local expected="$4"
  local actual
  if ! actual="$(fdtget -t s "$image" "$path" "$property")"; then
    echo "Missing ${property} at ${path}." >&2
    return 1
  fi
  [[ "$actual" == "$expected" ]] || {
    echo "Unexpected ${property} at ${path}: ${actual}." >&2
    return 1
  }
}

octessera_normalize_fdt_strings() {
  printf '%s\n' "$1" | tr '\n' ' ' | awk '{$1 = $1; print}'
}

octessera_require_fdt_strings() {
  local image="$1"
  local path="$2"
  local property="$3"
  local expected="$4"
  local actual
  if ! actual="$(fdtget -t s "$image" "$path" "$property")"; then
    echo "Missing ${property} at ${path}." >&2
    return 1
  fi
  [[ "$(octessera_normalize_fdt_strings "$actual")" == "$(octessera_normalize_fdt_strings "$expected")" ]] || {
    echo "Unexpected ${property} at ${path}: ${actual}." >&2
    return 1
  }
}

octessera_require_fdt_numbers() {
  local image="$1"
  local path="$2"
  local property="$3"
  local expected="$4"
  local actual
  if ! actual="$(fdtget -t u "$image" "$path" "$property")"; then
    echo "Missing ${property} at ${path}." >&2
    return 1
  fi
  [[ "$(octessera_normalize_fdt_numbers "$actual")" == "$(octessera_normalize_fdt_numbers "$expected")" ]] || {
    echo "Unexpected ${property} at ${path}: ${actual}." >&2
    return 1
  }
}

octessera_assert_node_unchanged() {
  local base="$1"
  local merged="$2"
  local path="$3"
  local context="$4"
  local base_properties
  local merged_properties
  local base_children
  local merged_children
  local property
  local base_value
  local merged_value
  if ! base_properties="$(fdtget -p "$base" "$path" | sort)"; then
    echo "Missing base ${context} node ${path}." >&2
    return 1
  fi
  if ! merged_properties="$(fdtget -p "$merged" "$path" | sort)"; then
    echo "Missing merged ${context} node ${path}." >&2
    return 1
  fi
  [[ "$base_properties" == "$merged_properties" ]] || {
    echo "Merged ${context} tree changed properties at ${path}." >&2
    return 1
  }
  if ! base_children="$(fdtget -l "$base" "$path" | sort)"; then
    echo "Unable to read base ${context} children at ${path}." >&2
    return 1
  fi
  if ! merged_children="$(fdtget -l "$merged" "$path" | sort)"; then
    echo "Unable to read merged ${context} children at ${path}." >&2
    return 1
  fi
  [[ "$base_children" == "$merged_children" ]] || {
    echo "Merged ${context} tree changed children at ${path}." >&2
    return 1
  }
  while IFS= read -r property; do
    [[ -n "$property" ]] || continue
    if ! base_value="$(fdtget -t bx "$base" "$path" "$property")"; then
      echo "Unable to read base ${context} property ${path}/${property}." >&2
      return 1
    fi
    if ! merged_value="$(fdtget -t bx "$merged" "$path" "$property")"; then
      echo "Unable to read merged ${context} property ${path}/${property}." >&2
      return 1
    fi
    [[ "$base_value" == "$merged_value" ]] || {
      echo "Merged ${context} tree changed ${path}/${property}." >&2
      return 1
    }
  done <<< "$base_properties"
}

octessera_assert_spi1_merge() {
  local base="$1"
  local merged="$2"
  local spi1_path="$3"
  local spi1_pins_path="$4"
  local spi1_cs0_path="$5"
  local spi0_path="$6"
  local i2c1_path="$7"
  local context="$8"
  local spi1_pins_phandle
  local spi1_cs0_phandle
  local spi1_pinctrl
  local image
  for image in "$base" "$merged"; do
    octessera_require_fdt_strings "$image" "$spi1_pins_path" pins 'PH6 PH7 PH8' || return 1
    octessera_require_fdt_string "$image" "$spi1_pins_path" function spi1 || return 1
    octessera_require_fdt_strings "$image" "$spi1_cs0_path" pins PH5 || return 1
    octessera_require_fdt_string "$image" "$spi1_cs0_path" function spi1 || return 1
  done
  octessera_require_fdt_string "$merged" "$spi1_path" status okay || return 1
  octessera_require_fdt_string "$merged" "$spi1_path" pinctrl-names default || return 1
  octessera_require_fdt_numbers "$merged" "$spi1_path" '#address-cells' 1 || return 1
  octessera_require_fdt_numbers "$merged" "$spi1_path" '#size-cells' 0 || return 1
  spi1_pins_phandle="$(fdtget -t u "$merged" "$spi1_pins_path" phandle)" || return 1
  spi1_cs0_phandle="$(fdtget -t u "$merged" "$spi1_cs0_path" phandle)" || return 1
  spi1_pinctrl="$(fdtget -t u "$merged" "$spi1_path" pinctrl-0)" || return 1
  [[ "$(octessera_normalize_fdt_numbers "$spi1_pinctrl")" == "$(octessera_normalize_fdt_numbers "$spi1_pins_phandle $spi1_cs0_phandle")" ]] || {
    echo "Merged ${context} SPI1 pinctrl does not select the expected data and CS0 groups." >&2
    return 1
  }
  octessera_require_fdt_string "$merged" "$spi1_path/spidev@0" compatible rohm,dh2228fv || return 1
  octessera_require_fdt_numbers "$merged" "$spi1_path/spidev@0" reg 0 || return 1
  octessera_require_fdt_numbers "$merged" "$spi1_path/spidev@0" spi-max-frequency 1000000 || return 1
  [[ "$(fdtget -l "$merged" "$spi1_path")" == spidev@0 ]] || {
    echo "Merged ${context} SPI1 node has an unexpected child set." >&2
    return 1
  }
  octessera_assert_node_unchanged "$base" "$merged" "$spi0_path" "$context" || return 1
  octessera_assert_node_unchanged "$base" "$merged" "$i2c1_path" "$context" || return 1
}
