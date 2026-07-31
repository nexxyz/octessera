#!/usr/bin/env bash

octessera_require_fdt_property() {
  local image="$1"
  local path="$2"
  local property="$3"
  local properties
  properties="$(fdtget -p "$image" "$path")" || {
    echo "Unable to inspect properties at ${path}." >&2
    return 1
  }
  printf '%s\n' "$properties" | grep -qxF "$property" || {
    echo "Missing ${property} at ${path}." >&2
    return 1
  }
}

octessera_assert_input_routing_merge() {
  local base="$1"
  local merged="$2"
  local uart0_path="$3"
  local pio_path="$4"
  local chosen_path="$5"
  local context="$6"
  local release_path="$pio_path/octessera-uart0-released"
  local release_phandle
  local uart0_pinctrl

  octessera_require_fdt_string "$base" "$uart0_path" status okay || return 1
  octessera_require_fdt_string "$merged" "$uart0_path" status disabled || return 1
  octessera_require_fdt_string "$merged" "$uart0_path" pinctrl-names default || return 1
  octessera_require_fdt_strings "$merged" "$release_path" pins 'PH0 PH1' || return 1
  octessera_require_fdt_string "$merged" "$release_path" function gpio_in || return 1
  octessera_require_fdt_property "$merged" "$release_path" bias-pull-up || return 1
  release_phandle="$(fdtget -t u "$merged" "$release_path" phandle)" || return 1
  uart0_pinctrl="$(fdtget -t u "$merged" "$uart0_path" pinctrl-0)" || return 1
  [[ "$(octessera_normalize_fdt_numbers "$uart0_pinctrl")" == "$(octessera_normalize_fdt_numbers "$release_phandle")" ]] || {
    echo "Merged ${context} UART0 pinctrl does not select the PH0/PH1 release group." >&2
    return 1
  }
  octessera_require_fdt_string "$merged" "$chosen_path" stdout-path "" || return 1
  [[ "$(fdtget -l "$merged" "$pio_path" | grep -Ec '^octessera-uart0-released$')" == 1 ]] || {
    echo "Merged ${context} pinctrl is missing the single input-routing child." >&2
    return 1
  }
}
