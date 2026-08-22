#!/usr/bin/env bash

function custom_kernel_config__octessera_audio() {
	# shellcheck disable=SC2034 # Armbian consumes opts_y after the hook returns.
	opts_y+=("SND_SOC_SUNXI_AHUB")
	opts_y+=("SND_SOC_SUNXI_AHUB_DAM")
	opts_y+=("SND_SOC_SUNXI_MACH")
	opts_y+=("NVMEM_SUNXI_SID")
}
