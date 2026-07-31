#!/usr/bin/env bash

function custom_kernel_config__octessera_midi() {
	# shellcheck disable=SC2034 # Armbian consumes opts_val after the hook returns.
	opts_val["RT_GROUP_SCHED"]="n"
	opts_m+=("SND_SEQUENCER")
	opts_m+=("SND_RAWMIDI")
	opts_m+=("SND_USB_AUDIO")
}
