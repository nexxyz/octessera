#!/usr/bin/env bash

function pre_umount_final_image__999_octessera_scrub_authorized_keys() {
	[[ -n "${SDCARD:-}" && -d "$SDCARD" ]] || return 0
	rm -f "$SDCARD/root/.ssh/authorized_keys" "$SDCARD"/home/*/.ssh/authorized_keys
}
