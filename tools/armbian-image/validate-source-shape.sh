#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
required_files=(
  "$root/tools/armbian-image/stage-canonical-welcome.sh"
  "$root/tools/armbian-image/stage-device-config.py"
  "$root/userpatches/overlay/etc/profile.d/octessera-welcome.sh"
  "$root/tools/armbian-image/resolve-armbian-extensions.sh"
  "$root/tools/armbian-image/inspect-built-image.sh"
  "$root/tools/armbian-image/inspect-runtime.sh"
  "$root/tools/armbian-image/inspect-account-ssh.sh"
  "$root/tools/armbian-image/inspect-network.sh"
  "$root/tools/armbian-image/inspect-device-tree.sh"
  "$root/tools/armbian-image/inspect-runtime-contracts.sh"
  "$root/tools/armbian-image/inspect-runtime-account.sh"
  "$root/tools/armbian-image/inspect-runtime-service.sh"
  "$root/tools/armbian-image/inspect-runtime-udev.sh"
  "$root/tools/armbian-image/inspect-runtime-device-apply.sh"
  "$root/tools/armbian-image/inspect-runtime-oled.sh"
  "$root/tools/armbian-image/inspect-runtime-mode.sh"
  "$root/tools/armbian-image/inspect-mode.sh"
  "$root/tools/armbian-image/inspect-path.sh"
  "$root/tools/armbian-image/authorized-key-paths.sh"
  "$root/tools/armbian-image/inspect-output-images.sh"
  "$root/tools/armbian-image/setup-layer-proof.sh"
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-request-cleanup"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-start"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-cleanup"
  "$root/tools/armbian-image/stage-musical-assets.sh"
  "$root/tools/armbian-image/test-musical-assets.sh"
  "$root/tools/armbian-image/test-image-sanitization.sh"
  "$root/tools/armbian-image/test-inspector.sh"
  "$root/tools/armbian-image/test-inspector-account.sh"
  "$root/tools/armbian-image/test-inspector-device-tree.sh"
  "$root/tools/armbian-image/test-inspector-fixture.sh"
  "$root/tools/armbian-image/test-inspector-network.sh"
  "$root/tools/armbian-image/test-inspector-runtime.sh"
  "$root/tools/armbian-image/test-image-mode.sh"
  "$root/tools/armbian-image/test-orange-runtime-service.sh"
  "$root/tools/armbian-image/test-orange-alsa-sequencer.sh"
  "$root/tools/armbian-image/test-orange-kernel-package.sh"
  "$root/tools/armbian-image/validate-orange-kernel-package.sh"
  "$root/tools/armbian-image/test-orange-image-proof.sh"
  "$root/tools/armbian-image/test-validation-runner.sh"
  "$root/tools/armbian-image/test-validation-negative-fixtures.sh"
  "$root/tools/armbian-image/validation-assertions.sh"
  "$root/tools/armbian-image/verify-orange-image.sh"
  "$root/tools/armbian-image/orange_image_mount.py"
  "$root/tools/armbian-image/orange_boot_selection.py"
  "$root/tools/armbian-image/verify-orange-image.py"
  "$root/tools/armbian-image/test-orange-image-proof.py"
  "$root/tools/armbian-image/test_orange_image_proof_boot.py"
  "$root/tools/armbian-image/test_orange_image_proof_image.py"
  "$root/tools/armbian-image/test_orange_image_proof_runtime.py"
  "$root/tools/armbian-image/test_orange_image_proof_security.py"
  "$root/tools/armbian-image/test_orange_image_proof_source.py"
  "$root/tools/armbian-image/test_orange_image_proof_support.py"
  "$root/tools/armbian-image/orange_trusted_parent_proof.py"
  "$root/tools/armbian-image/test-orange-trusted-proof.py"
  "$root/tools/armbian-image/orange_initramfs.py"
  "$root/tools/armbian-image/orange_phase5_proof.py"
  "$root/tools/armbian-image/test-orange-boot-splash-hook.sh"
  "$root/tools/armbian-image/test-orange-oled-suspend.sh"
  "$root/tools/armbian-image/test-orange-oled-suspend.py"
  "$root/tools/armbian-image/test_orange_oled_logo.py"
  "$root/tools/armbian-image/test_orange_oled_off.py"
  "$root/tools/armbian-image/test_orange_oled_readiness.py"
  "$root/tools/armbian-image/test_orange_oled_handoff.py"
  "$root/tools/armbian-image/test_orange_oled_lifecycle.py"
  "$root/tools/armbian-image/test-orange-runtime-identity.py"
  "$root/tools/armbian-image/test-orange-construction.py"
  "$root/tools/armbian-image/test-orange-updater.py"
  "$root/tools/armbian-image/test-orange-update-broker.py"
  "$root/tools/device-update/test_updater_layout.py"
  "$root/tools/armbian-image/test-build-armbian-action.sh"
  "$root/tools/armbian-image/test-release-workflow.sh"
  "$root/tools/armbian-image/fixtures/python313-initramfs-closure-files.txt"
  "$root/tools/armbian-image/fixtures/python313-initramfs-closure/imports.py"
  "$root/tools/armbian-image/fixtures/python313-initramfs-closure/collections/__init__.py"
  "$root/tools/armbian-image/fixtures/python313-initramfs-closure/_collections_abc.py"
  "$root/tools/armbian-image/test-device-config.py"
  "$root/tools/armbian-image/test-orange-device-apply.py"
  "$root/tools/armbian-image/test-setup-layer.sh"
  "$root/tools/armbian-image/test_setup_sidecar.py"
  "$root/tools/armbian-image/test-setup-request.py"
  "$root/tools/armbian-image/test-setup-http.py"
  "$root/tools/armbian-image/test-setup-flow.py"
  "$root/tools/armbian-image/test-setup-state.py"
  "$root/tools/orange-pi/input-routing-provision.sh"
  "$root/tools/orange-pi/orange-pi-usb-gadget.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-electrical.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-fixture.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-function.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-host-enumeration.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-passive.sh"
  "$root/tools/pi-image/test-wifi-foundation.sh"
  "$root/tools/pi-image/test-usb-gadget.sh"
  "$root/tools/pi-image/test-usb-gadget-electrical.sh"
  "$root/tools/pi-image/test-usb-gadget-fixture.sh"
  "$root/tools/pi-image/test-usb-gadget-gadget.sh"
  "$root/tools/pi-image/test-usb-gadget-host.sh"
  "$root/tools/pi-image/test-usb-gadget-layout.sh"
  "$root/tools/pi-image/test-rpi-boot-splash.sh"
  "$root/tools/pi-image/test-rpi-boot-services.sh"
  "$root/tools/pi-image/test-rpi-initramfs-proof.py"
  "$root/tools/pi-image/test-sanitized-image-boot-layout.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-boot.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-fixture.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-layout.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-sanitization.sh"
  "$root/tools/pi-image/test-board-profile.py"
  "$root/tools/pi-image/test-welcome.py"
  "$root/tools/pi-image/verify-boot-layout.sh"
  "$root/tools/pi-image/verify-sanitized-image.sh"
  "$root/tools/pi-image/verify-trusted-parent-v0.7.5.sh"
  "$root/tools/pi-image/verify-rpi-samples.py"
  "$root/tools/pi-image/test-rpi-kernel-image.sh"
  "$root/tools/pi-image/test-rpi-kernel-image.py"
  "$root/tools/pi-image/test-boot-layer-contract.py"
  "$root/tools/pi-image/verify-rpi-kernel-image.py"
  "$root/tools/pi-image/test-rpi-kernel-mount.py"
  "$root/tools/pi-image/rpi_kernel_image_mount.py"
  "$root/tools/pi-image/rpi_kernel_boot_proof.py"
  "$root/tools/pi-image/rpi_kernel_payload_proof.py"
  "$root/tools/pi-image/rpi_kernel_stock_recovery.py"
  "$root/tools/pi-image/rpi_initramfs_proof.py"
  "$root/tools/pi-image/install-rpi-kernel.py"
  "$root/tools/pi-image/rpi_initramfs_fixture.py"
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar"
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request"
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status.py"
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status-cli.py"
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-call.py"
  "$root/userpatches/customize-image.sh"
  "$root/userpatches/extensions/octessera_midi.sh"
  "$root/userpatches/extensions/octessera_image_sanitize.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation"
  "$root/userpatches/overlay/etc/systemd/system/octessera-wifi-foundation.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
  "$root/userpatches/overlay/etc/sudoers.d/octessera-update"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-guard.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update.socket"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update@.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"
  "$root/userpatches/overlay/etc/systemd/system/octessera-setup.service"
  "$root/userpatches/overlay/etc/octessera/image-contract.json"
  "$root/userpatches/overlay/etc/systemd/system/octessera.service"
  "$root/userpatches/overlay/etc/modules-load.d/octessera-orange-usb-gadget.conf"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-usb-gadget.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket"
  "$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend"
  "$root/userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot"
  "$root/userpatches/overlay/usr/local/lib/octessera/device_config.py"
  "$root/userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh"
  "$root/userpatches/overlay/usr/local/lib/octessera/diagnostic-payload.sh"
  "$root/userpatches/overlay/usr/local/lib/octessera/orange-sample-assets.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
  "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
  "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-status.py"
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-status-cli.py"
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-call.py"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-request"
  "$root/userpatches/overlay/etc/octessera/setup-profile"
  "$root/userpatches/overlay/etc/udev/rules.d/70-octessera-orange-runtime.rules"
  "$root/userpatches/overlay/usr/local/share/octessera-setup-ui/app.js"
  "$root/userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-mark.svg"
  "$root/userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-wordmark.svg"
  "$root/resources/image-construction/boot-layers/orange-pi-zero-2w.json"
  "$root/resources/image-construction/boot-layers/raspberry-pi-zero-2w.json"
  "$root/resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json"
  "$root/tools/armbian-image/fixtures/h618-spi-base.dts"
)

for file in "${required_files[@]}"; do
  [[ -f "$file" ]] || { echo "Missing required setup file: $file" >&2; exit 1; }
done

bash_files=(
  "$root/tools/armbian-image/stage-canonical-welcome.sh"
  "$root/userpatches/customize-image.sh"
  "$root/tools/armbian-image/resolve-armbian-extensions.sh"
  "$root/tools/armbian-image/inspect-built-image.sh"
  "$root/tools/armbian-image/inspect-runtime.sh"
  "$root/tools/armbian-image/inspect-account-ssh.sh"
  "$root/tools/armbian-image/inspect-network.sh"
  "$root/tools/armbian-image/inspect-device-tree.sh"
  "$root/tools/armbian-image/inspect-runtime-contracts.sh"
  "$root/tools/armbian-image/inspect-runtime-account.sh"
  "$root/tools/armbian-image/inspect-runtime-service.sh"
  "$root/tools/armbian-image/inspect-runtime-udev.sh"
  "$root/tools/armbian-image/inspect-runtime-device-apply.sh"
  "$root/tools/armbian-image/inspect-runtime-oled.sh"
  "$root/tools/armbian-image/inspect-runtime-mode.sh"
  "$root/tools/armbian-image/validation-runner.sh"
  "$root/tools/armbian-image/validation-assertions.sh"
  "$root/tools/armbian-image/validate.sh"
  "$root/tools/armbian-image/validate-source-shape.sh"
  "$root/tools/armbian-image/validate-device-tree.sh"
  "$root/tools/armbian-image/validate-security-policy.sh"
  "$root/tools/armbian-image/validate-image-proof.sh"
  "$root/tools/armbian-image/inspect-mode.sh"
  "$root/tools/armbian-image/inspect-path.sh"
  "$root/tools/armbian-image/authorized-key-paths.sh"
  "$root/tools/armbian-image/inspect-output-images.sh"
  "$root/tools/armbian-image/setup-layer-proof.sh"
  "$root/tools/armbian-image/test-setup-layer.sh"
  "$root/tools/armbian-image/test-validation-negative-fixtures.sh"
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-start"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-cleanup"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-request-cleanup"
  "$root/tools/armbian-image/stage-musical-assets.sh"
  "$root/tools/armbian-image/test-musical-assets.sh"
  "$root/tools/armbian-image/test-image-sanitization.sh"
  "$root/tools/armbian-image/test-inspector.sh"
  "$root/tools/armbian-image/test-inspector-account.sh"
  "$root/tools/armbian-image/test-inspector-device-tree.sh"
  "$root/tools/armbian-image/test-inspector-fixture.sh"
  "$root/tools/armbian-image/test-inspector-network.sh"
  "$root/tools/armbian-image/test-inspector-runtime.sh"
  "$root/tools/armbian-image/test-image-mode.sh"
  "$root/tools/armbian-image/test-orange-runtime-service.sh"
  "$root/tools/armbian-image/test-orange-alsa-sequencer.sh"
  "$root/tools/armbian-image/test-orange-kernel-package.sh"
  "$root/tools/armbian-image/validate-orange-kernel-package.sh"
  "$root/tools/armbian-image/test-orange-image-proof.sh"
  "$root/tools/armbian-image/test-build-armbian-action.sh"
  "$root/tools/armbian-image/test-release-workflow.sh"
  "$root/tools/armbian-image/test-validation-runner.sh"
  "$root/tools/armbian-image/verify-orange-image.sh"
  "$root/tools/armbian-image/test-orange-boot-splash-hook.sh"
  "$root/tools/armbian-image/test-orange-oled-suspend.sh"
  "$root/tools/orange-pi/input-routing-provision.sh"
  "$root/tools/orange-pi/orange-pi-usb-gadget.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-electrical.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-fixture.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-function.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-host-enumeration.sh"
  "$root/tools/orange-pi/test-orange-pi-usb-gadget-passive.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget"
  "$root/tools/pi-image/test-wifi-foundation.sh"
  "$root/tools/pi-image/verify-rpi-kernel-image.sh"
  "$root/tools/pi-image/test-usb-gadget.sh"
  "$root/tools/pi-image/test-usb-gadget-electrical.sh"
  "$root/tools/pi-image/test-usb-gadget-fixture.sh"
  "$root/tools/pi-image/test-usb-gadget-gadget.sh"
  "$root/tools/pi-image/test-usb-gadget-host.sh"
  "$root/tools/pi-image/test-usb-gadget-layout.sh"
  "$root/tools/pi-image/test-rpi-boot-splash.sh"
  "$root/tools/pi-image/test-rpi-boot-services.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-boot.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-fixture.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-layout.sh"
  "$root/tools/pi-image/test-sanitized-image-boot-layout-sanitization.sh"
  "$root/tools/pi-image/verify-boot-layout.sh"
  "$root/tools/pi-image/verify-sanitized-image.sh"
  "$root/tools/pi-image/verify-trusted-parent-v0.7.5.sh"
  "$root/userpatches/extensions/octessera_midi.sh"
  "$root/userpatches/extensions/octessera_image_sanitize.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget"
  "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/armbian-env-token.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh"
  "$root/userpatches/overlay/usr/local/share/octessera/device-tree/boot-dtb-selection.sh"
  "$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
  "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
  "$root/userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh"
  "$root/userpatches/overlay/usr/local/lib/octessera/diagnostic-payload.sh"
  "$root/userpatches/overlay/usr/local/lib/octessera/orange-sample-assets.sh"
)
for file in "${bash_files[@]}"; do
  bash -n "$file"
done

python3 -m py_compile \
  "$root/tools/armbian-image/stage-device-config.py" \
  "$root/tools/device-update/updater_protocol.py" \
  "$root/tools/device-update/updater_contract.py" \
  "$root/tools/device-update/updater_state.py" \
  "$root/tools/device-update/updater_assets.py" \
  "$root/tools/device-update/updater_guard.py" \
  "$root/tools/device-update/updater_cli.py" \
  "$root/tools/device-update/updater_profiles.py" \
  "$root/tools/device-update/test_updater_layout.py" \
  "$root/tools/device-update/octessera-update-broker" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-broker" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-request" \
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-status.py" \
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-status-cli.py" \
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-call.py" \
  "$root/userpatches/overlay/usr/local/lib/octessera/device_config.py" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend" \
  "$root/tools/pi-image/verify-rpi-kernel-image.py" \
  "$root/tools/pi-image/test-rpi-kernel-mount.py" \
  "$root/tools/pi-image/test-rpi-kernel-image.py" \
  "$root/tools/pi-image/rpi_kernel_image_mount.py" \
  "$root/tools/pi-image/rpi_kernel_boot_proof.py" \
  "$root/tools/pi-image/rpi_kernel_payload_proof.py" \
  "$root/tools/pi-image/rpi_kernel_stock_recovery.py" \
  "$root/tools/armbian-image/orange_image_mount.py" \
  "$root/tools/armbian-image/orange_boot_selection.py" \
  "$root/tools/armbian-image/verify-orange-image.py" \
  "$root/tools/armbian-image/test-orange-image-proof.py" \
  "$root/tools/armbian-image/test_orange_image_proof_boot.py" \
  "$root/tools/armbian-image/test_orange_image_proof_image.py" \
  "$root/tools/armbian-image/test_orange_image_proof_runtime.py" \
  "$root/tools/armbian-image/test_orange_image_proof_security.py" \
  "$root/tools/armbian-image/test_orange_image_proof_source.py" \
  "$root/tools/armbian-image/test_orange_image_proof_support.py" \
  "$root/tools/armbian-image/orange_trusted_parent_proof.py" \
  "$root/tools/armbian-image/test-orange-trusted-proof.py" \
  "$root/tools/armbian-image/orange_initramfs.py" \
  "$root/tools/armbian-image/orange_phase5_proof.py" \
  "$root/tools/armbian-image/verify_runtime_account.py" \
  "$root/tools/armbian-image/test-orange-updater.py" \
  "$root/tools/armbian-image/test-orange-update-broker.py" \
  "$root/tools/armbian-image/test_orange_oled_logo.py" \
  "$root/tools/armbian-image/test_orange_oled_off.py" \
  "$root/tools/armbian-image/test_orange_oled_readiness.py" \
  "$root/tools/armbian-image/test_orange_oled_handoff.py" \
  "$root/tools/armbian-image/test_orange_oled_lifecycle.py" \
  "$root/tools/armbian-image/test-orange-runtime-identity.py" \
  "$root/tools/armbian-image/test-orange-construction.py" \
  "$root/tools/armbian-image/test-device-config.py" \
  "$root/tools/armbian-image/test-orange-device-apply.py" \
  "$root/tools/armbian-image/test_setup_sidecar.py" \
  "$root/tools/armbian-image/test-setup-request.py" \
  "$root/tools/armbian-image/test-setup-http.py" \
  "$root/tools/armbian-image/test-setup-flow.py" \
  "$root/tools/armbian-image/test-setup-state.py" \
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar" \
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request" \
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status.py" \
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status-cli.py" \
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-call.py" \
  "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-request" \
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-status.py" \
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-status-cli.py" \
  "$root/userpatches/overlay/usr/local/lib/octessera/setup-call.py" \
  "$root/userpatches/overlay/usr/local/lib/octessera/device_config.py" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py" \
  "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py"

PYTHONDONTWRITEBYTECODE=1 python3 - "$root/.github/workflows/armbian-image.yml" "$root/.github/actions/build-armbian-image/action.yml" <<'PY'
import sys

import yaml

for path in sys.argv[1:]:
    with open(path, encoding="utf-8") as handle:
        yaml.safe_load(handle)
PY

if command -v node >/dev/null 2>&1; then
  node --check "$root/userpatches/overlay/usr/local/share/octessera-setup-ui/app.js"
else
  echo 'Node.js unavailable; setup UI syntax check skipped.' >&2
fi

if command -v actionlint >/dev/null 2>&1; then
  actionlint "$root/.github/workflows/armbian-image.yml"
else
  echo 'actionlint unavailable; workflow syntax check skipped.' >&2
fi

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "${bash_files[@]}"
else
  echo 'shellcheck unavailable; shell syntax lint skipped.' >&2
fi
