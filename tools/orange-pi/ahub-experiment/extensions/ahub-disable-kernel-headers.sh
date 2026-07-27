function extension_finish_config__ahub_disable_kernel_headers() {
	[[ "${KERNEL_MAJOR_MINOR}" == "6.12" ]] || exit_with_error "AHUB experiment requires Armbian kernel 6.12"
	declare -g KERNEL_HAS_WORKING_HEADERS="no"
	declare -g INSTALL_HEADERS="no"
	display_alert "Disabling linux-headers" "AHUB deploy-only kernel artifact" "info"
}
