#!/usr/bin/env bash

function custom_kernel_config__octessera_sd2() {
  # shellcheck disable=SC2034 # Armbian consumes opts_y after the hook returns.
  opts_y+=("MMC")
  opts_y+=("MMC_BLOCK")
  opts_m+=("MMC_SPI")
  opts_m+=("USB_F_MASS_STORAGE")
}
