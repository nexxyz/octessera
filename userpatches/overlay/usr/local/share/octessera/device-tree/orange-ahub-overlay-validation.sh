#!/usr/bin/env bash

octessera_assert_orange_audio_merge() {
  local pre_audio="$1"
  local merged="$2"
  local context="$3"
  local ahub0_path
  local dac_path
  local cpu_path
  local ahub1_path
  local ahub1_mach_path
  local spi0_path
  local i2c1_path
  local uart2_path
  local hdmi_path
  local dma_values
  local dma_tx
  local dma_req_tx
  local dma_rx
  local dma_req_rx
  local cpu_phandle
  local frame_master
  local bitclock_master
  local ahub1_cpu_path
  local ahub0_pinctrl
  local i2s0_pins_path
  local i2s0_dout0_pins_path
  local i2s0_pins_phandle
  local i2s0_dout0_pins_phandle
  local ahub1_plat_phandle
  local ahub1_mach_cpu_dai

  for symbol in ahub1_plat ahub1_mach hdmi codec spi0 i2c1 uart2; do
    local symbol_path
    symbol_path="$(fdtget -t s "$pre_audio" /__symbols__ "$symbol")" || {
      echo "${context} pre-audio DTB is missing the ${symbol} symbol." >&2
      return 1
    }
    [[ -n "$symbol_path" ]] || {
      echo "${context} base DTB has an empty ${symbol} symbol." >&2
      return 1
    }
  done

  ahub0_path="$(fdtget -t s "$merged" /__symbols__ octessera_plat)" || return 1
  dac_path="$(fdtget -t s "$merged" /__symbols__ octessera_dac)" || return 1
  cpu_path="$(fdtget -t s "$merged" /__symbols__ octessera_dac_cpu)" || return 1
  [[ -n "$ahub0_path" && -n "$dac_path" && -n "$cpu_path" ]] || {
    echo "${context} AHUB0 symbols are incomplete." >&2
    return 1
  }

  octessera_require_fdt_string "$merged" "$ahub0_path" compatible allwinner,sunxi-snd-plat-ahub || return 1
  octessera_require_fdt_string "$merged" "$ahub0_path" status okay || return 1
  octessera_require_fdt_numbers "$merged" "$ahub0_path" apb_num 0 || return 1
  octessera_require_fdt_numbers "$merged" "$ahub0_path" tdm_num 0 || return 1
  octessera_require_fdt_numbers "$merged" "$ahub0_path" tx_pin 0 || return 1
  octessera_require_fdt_numbers "$merged" "$ahub0_path" rx_pin 0 || return 1
  ahub0_pinctrl="$(fdtget -t u "$merged" "$ahub0_path" pinctrl-0)" || return 1
  i2s0_pins_path="$(fdtget -t s "$merged" /__symbols__ octessera_i2s0_pins)" || return 1
  i2s0_dout0_pins_path="$(fdtget -t s "$merged" /__symbols__ octessera_i2s0_dout0_pins)" || return 1
  i2s0_pins_phandle="$(fdtget -t u "$merged" "$i2s0_pins_path" phandle)" || return 1
  i2s0_dout0_pins_phandle="$(fdtget -t u "$merged" "$i2s0_dout0_pins_path" phandle)" || return 1
  [[ "$(octessera_normalize_fdt_numbers "$ahub0_pinctrl")" == "$(octessera_normalize_fdt_numbers "$i2s0_pins_phandle $i2s0_dout0_pins_phandle")" ]] || {
    echo "${context} AHUB0 pinctrl does not select the I2S0 and DOUT0 groups." >&2
    return 1
  }
  octessera_require_fdt_strings "$merged" "$i2s0_pins_path" pins 'PI1 PI2' || return 1
  octessera_require_fdt_string "$merged" "$i2s0_pins_path" function i2s0 || return 1
  octessera_require_fdt_strings "$merged" "$i2s0_dout0_pins_path" pins PI3 || return 1
  octessera_require_fdt_string "$merged" "$i2s0_dout0_pins_path" function i2s0_dout0 || return 1
  dma_values="$(fdtget -t x "$merged" "$ahub0_path" dmas)" || return 1
  read -r dma_tx dma_req_tx dma_rx dma_req_rx <<< "$dma_values"
  [[ "$dma_req_tx" == 3 && "$dma_req_rx" == 3 && "$dma_tx" == "$dma_rx" ]] || {
    echo "${context} AHUB0 does not use DMA3 for TX and RX." >&2
    return 1
  }

  octessera_require_fdt_string "$merged" "$dac_path" compatible allwinner,sunxi-snd-mach || return 1
  octessera_require_fdt_string "$merged" "$dac_path" soundcard-mach,name octessera-dac || return 1
  octessera_require_fdt_property "$merged" "$dac_path" soundcard-mach,playback-only || return 1
  octessera_require_fdt_string "$merged" "$dac_path" soundcard-mach,format i2s || return 1
  octessera_require_fdt_numbers "$merged" "$dac_path" soundcard-mach,slot-num 2 || return 1
  octessera_require_fdt_numbers "$merged" "$dac_path" soundcard-mach,slot-width 32 || return 1
  cpu_phandle="$(fdtget -t u "$merged" "$cpu_path" phandle)" || return 1
  frame_master="$(fdtget -t u "$merged" "$dac_path" soundcard-mach,frame-master)" || return 1
  bitclock_master="$(fdtget -t u "$merged" "$dac_path" soundcard-mach,bitclock-master)" || return 1
  [[ "$frame_master" == "$cpu_phandle" && "$bitclock_master" == "$cpu_phandle" ]] || {
    echo "${context} Octessera DAC master links are not CPU-owned." >&2
    return 1
  }
  octessera_require_fdt_numbers "$merged" "$cpu_path" soundcard-mach,pll-fs 4 || return 1
  octessera_require_fdt_numbers "$merged" "$cpu_path" sound-dai "$(fdtget -t u "$merged" "$ahub0_path" phandle)" || return 1
  octessera_require_fdt_property "$merged" "$dac_path/soundcard-mach,codec" sound-dai >/dev/null 2>&1 && {
    echo "${context} dummy codec unexpectedly claims a sound-dai." >&2
    return 1
  }
  for path in "$dac_path" "$cpu_path"; do
    if fdtget "$merged" "$path" soundcard-mach,mclk-fs >/dev/null 2>&1; then
      echo "${context} Octessera DAC claims MCLK." >&2
      return 1
    fi
  done
  if fdtget -l "$merged" / | grep -Fxq pcm5102a; then
    echo "${context} merged tree contains a PCM5102A codec node." >&2
    return 1
  fi

  ahub1_path="$(fdtget -t s "$merged" /__symbols__ ahub1_plat)" || return 1
  ahub1_mach_path="$(fdtget -t s "$merged" /__symbols__ ahub1_mach)" || return 1
  hdmi_path="$(fdtget -t s "$merged" /__symbols__ hdmi)" || return 1
  spi0_path="$(fdtget -t s "$merged" /__symbols__ spi0)" || return 1
  i2c1_path="$(fdtget -t s "$merged" /__symbols__ i2c1)" || return 1
  uart2_path="$(fdtget -t s "$merged" /__symbols__ uart2)" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$ahub1_path" "$context HDMI AHUB1" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$ahub1_mach_path" "$context HDMI machine" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$hdmi_path" "$context HDMI" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$(fdtget -t s "$pre_audio" /__symbols__ codec)" "$context codec" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$spi0_path" "$context SPI0" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$i2c1_path" "$context I2C1" || return 1
  octessera_assert_node_unchanged "$pre_audio" "$merged" "$uart2_path" "$context UART2" || return 1
  octessera_require_fdt_string "$merged" "$ahub1_path" status okay || return 1
  octessera_require_fdt_numbers "$merged" "$ahub1_path" tdm_num 1 || return 1
  octessera_require_fdt_string "$merged" "$ahub1_mach_path" soundcard-mach,name HDMI || return 1
  ahub1_cpu_path="$(fdtget -t s "$merged" /__symbols__ ahub1_cpu)" || return 1
  ahub1_plat_phandle="$(fdtget -t u "$merged" "$ahub1_path" phandle)" || return 1
  ahub1_mach_cpu_dai="$(fdtget -t u "$merged" "$ahub1_cpu_path" sound-dai)" || return 1
  [[ "$ahub1_mach_cpu_dai" == "$ahub1_plat_phandle" ]] || {
    echo "${context} HDMI CPU relation changed." >&2
    return 1
  }
  if fdtget -l "$merged" / | grep -Fxq ahub1_plat || fdtget -l "$merged" / | grep -Fxq ahub1_mach; then
    echo "${context} created root AHUB1 nodes." >&2
    return 1
  fi
}

octessera_assert_orange_preserved_peripherals() {
  local base="$1"
  local merged="$2"
  local context="$3"
  local i2c1_path
  for symbol in spi0 i2c0 uart2 hdmi codec dma ahub1_plat ahub1_mach; do
    local symbol_path
    symbol_path="$(fdtget -t s "$base" /__symbols__ "$symbol")" || {
      echo "${context} base DTB is missing the preserved ${symbol} symbol." >&2
      return 1
    }
    octessera_assert_node_unchanged "$base" "$merged" "$symbol_path" "${context} ${symbol}" || return 1
  done
  i2c1_path="$(fdtget -t s "$merged" /__symbols__ i2c1)" || return 1
  octessera_require_fdt_string "$merged" "$i2c1_path" status okay || return 1
}
