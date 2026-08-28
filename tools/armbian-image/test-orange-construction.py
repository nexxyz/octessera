#!/usr/bin/env python3
import copy
import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "resources/image-construction/boot-layers/orange-pi-zero-2w.json"
SETUP_CONTRACT_PATH = ROOT / "resources/image-mutations/orange-pi-zero-2w-setup.json"
SOURCE_BOUND_PROOF_SOURCES = {
    "tools/armbian-image/verify-orange-image.py",
    "tools/armbian-image/orange_boot_contract.py",
    "tools/armbian-image/orange_first_boot_contract.py",
    "tools/armbian-image/orange_boot_inventory.py",
    "tools/armbian-image/orange_boot_selection.py",
    "tools/armbian-image/orange_image_mount.py",
    "tools/armbian-image/orange_initramfs.py",
    "tools/armbian-image/orange_phase5_proof.py",
    "tools/armbian-image/orange_audio_proof.py",
    "tools/armbian-image/orange_sd_card_proof.py",
    "tools/armbian-image/orange_trusted_parent_proof.py",
    "tools/armbian-image/test_orange_oled_logo.py",
    "tools/armbian-image/verify_runtime_account.py",
    "userpatches/overlay/usr/local/share/octessera/device-tree/orange-ahub-overlay-validation.sh",
    "tools/storage/octessera-sd-card",
    "tools/storage/octessera-sd-card-lib.sh",
    "tools/storage/octessera-orange-storage",
    "tools/storage/octessera-orange-storage-control",
    "userpatches/overlay/usr/local/lib/octessera/octessera-sd-card-lib.sh",
    "userpatches/overlay/usr/local/sbin/octessera-orange-storage",
    "userpatches/overlay/usr/local/sbin/octessera-orange-storage-control",
    "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control.socket",
    "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control@.service",
    "userpatches/overlay/etc/systemd/system/octessera-orange-sd-card.service",
    "userpatches/overlay/etc/udev/rules.d/99-octessera-orange-sd-card.rules",
    "tools/kernel-patches/orange-midi-interface-manifest.json",
}
contract = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
setup_contract = json.loads(SETUP_CONTRACT_PATH.read_text(encoding="utf-8"))


def exact(value, keys):
    assert set(value) == set(keys)


def validate_source_inputs(document, root):
    paths = set()
    for item in document["exact_inputs"]:
        exact(item, ["path", "sha256", "size", "mode"])
        assert item["path"] not in paths
        assert not Path(item["path"]).is_absolute() and ".." not in Path(item["path"]).parts
        paths.add(item["path"])
        source = root / item["path"]
        assert source.is_file() and not source.is_symlink(), source
        assert hashlib.sha256(source.read_bytes()).hexdigest() == item["sha256"], source
        assert source.stat().st_size == item["size"], source
        assert item["mode"] in {420, 493}
    assert SOURCE_BOUND_PROOF_SOURCES <= paths


exact(contract, ["schema_version", "proof_mode", "contract_kind", "construction_kind", "board_profile", "constructor_required", "trusted_parent_finalization", "mutation_authority", "regeneration_required", "expected_changes", "exact_inputs", "managed_outputs", "notice_bundle", "terminal_invariants", "uart_invariants", "enabled_sysinit_wants", "device_dependencies", "required_builtin_kernel_config_lines", "selected_initramfs", "mounted_proof", "proofs"])
assert contract["schema_version"] == 1
assert contract["proof_mode"] == "phase5-constructor"
assert contract["contract_kind"] == "constructor-required"
assert contract["construction_kind"] == "orange-boot-layer"
assert contract["board_profile"] == "orange-pi-zero-2w"
assert contract["constructor_required"] is True
assert contract["trusted_parent_finalization"] == "forbidden"
assert contract["regeneration_required"] == ["initramfs", "python_closure"]
assert contract["expected_changes"] == {"packages": [], "accounts": [], "kernel": False, "firmware": False}

validate_source_inputs(contract, ROOT)

for path in sorted(SOURCE_BOUND_PROOF_SOURCES):
    altered = copy.deepcopy(contract)
    source = next(item for item in altered["exact_inputs"] if item["path"] == path)
    source["sha256"] = "0" * 64
    try:
        validate_source_inputs(altered, ROOT)
    except AssertionError:
        pass
    else:
        raise AssertionError(f"tampered Orange proof source was accepted: {path}")

    altered = copy.deepcopy(contract)
    altered["exact_inputs"] = [item for item in altered["exact_inputs"] if item["path"] != path]
    try:
        validate_source_inputs(altered, ROOT)
    except AssertionError:
        pass
    else:
        raise AssertionError(f"missing Orange proof source was accepted: {path}")

construction_inputs = {item["path"]: item for item in contract["exact_inputs"]}
assert construction_inputs["tools/armbian-image/orange_first_boot_contract.py"]["mode"] == 420
assert "CONFIG_SND_SOC_PCM5102A" not in (ROOT / "userpatches/extensions/octessera_audio.sh").read_text(encoding="utf-8")
assert all(any(item["path"] == path for item in contract["managed_outputs"]) for path in (
    "usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts",
    "boot/overlay-user/octessera-ahub0-pcm5102.dtbo",
    "etc/octessera/build-metadata.env",
    "etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf",
))
assert (ROOT / "userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash").is_file()
assert (ROOT / "userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash").is_file()
assert "userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled" not in construction_inputs
assert "userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service" in construction_inputs
assert "userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend" in construction_inputs
assert construction_inputs["userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565"]["size"] == 32768
assert construction_inputs["userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565"]["size"] == 32768
setup_inputs = {item["path"]: item for item in setup_contract["source_inputs"]}
overlap = sorted(set(construction_inputs) & set(setup_inputs))
assert overlap
for path in overlap:
    assert construction_inputs[path]["sha256"] == setup_inputs[path]["sha256"], path
    assert construction_inputs[path]["size"] == setup_inputs[path]["size"], path

for item in contract["managed_outputs"]:
    if item["path"] == "home/octessera/.hushlogin":
        exact(item, ["path", "mode", "owner", "group", "content"])
        assert item == {"path": "home/octessera/.hushlogin", "mode": 420, "owner": "octessera", "group": "octessera", "content": "empty"}
    elif item.get("kind") == "symlink":
        exact(item, ["path", "kind", "target", "uid", "gid"])
        assert item["target"] in {"../octessera-orange-boot-splash.service", "../octessera-orange-oled-suspend.service", "../octessera-device-apply-reboot.socket", "../octessera-update-recovery.service", "../octessera-update.socket", "../octessera-orange-sd-card.service", "../octessera-orange-storage-control.socket", "/dev/null"} and item["uid"] == 0 and item["gid"] == 0
    else:
        exact(item, ["path", "mode", "uid", "gid"])
        assert item["mode"] in {288, 420, 493} and item["uid"] == 0 and item["gid"] == 0

assert all(isinstance(path, str) and path.startswith("tools/armbian-image/") for path in contract["proofs"])
assert "tools/armbian-image/validate.sh" in contract["proofs"]
assert contract["mutation_authority"] == "none"
for path in (
    "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py",
    "userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot",
    "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket",
    "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service",
    "tools/device-update/octessera-update-broker",
    "userpatches/overlay/usr/local/sbin/octessera-update-broker",
    "userpatches/overlay/etc/systemd/system/octessera-update.socket",
    "userpatches/overlay/etc/systemd/system/octessera-update@.service",
    "userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget",
    "userpatches/overlay/etc/systemd/system/octessera-orange-usb-gadget.service",
    "tools/storage/octessera-sd-card",
    "userpatches/overlay/etc/systemd/system/octessera-orange-sd-card.service",
    "userpatches/overlay/etc/udev/rules.d/99-octessera-orange-sd-card.rules",
    "tools/storage/octessera-orange-storage",
    "tools/storage/octessera-orange-storage-control",
    "userpatches/overlay/usr/local/lib/octessera/octessera-sd-card-lib.sh",
    "userpatches/overlay/usr/local/sbin/octessera-orange-storage",
    "userpatches/overlay/usr/local/sbin/octessera-orange-storage-control",
    "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control.socket",
    "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control@.service",
):
    assert any(item["path"] == path for item in contract["exact_inputs"]), path
validator_inputs = [item for item in contract["exact_inputs"] if item["path"] == "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"]
assert len(validator_inputs) == 1
assert contract["mounted_proof"] == "tools/armbian-image/verify-orange-image.py"
for line in (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8").splitlines():
    if any(path in line for path in ("/etc/motd", "/etc/issue", "/usr/share/doc", "/usr/share/common-licenses", "/usr/share/doc/base-files/copyright")) and "/usr/share/doc/octessera" not in line:
        raise AssertionError("Orange constructor mutates a parent legal path")
customize = (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8")
runtime_assets = (ROOT / "userpatches/overlay/usr/local/lib/octessera/orange-runtime-assets-install.sh").read_text(encoding="utf-8")
setup_config = (ROOT / "userpatches/overlay/usr/local/lib/octessera/setup_config.py").read_text(encoding="utf-8")
assert 'source "$orange_runtime_assets_helper"' in customize
assert 'octessera_validate_orange_runtime_assets "$overlay_dir"' in customize
assert 'octessera_install_orange_runtime_assets "$overlay_dir"' in customize
assert re.search(r"systemctl\s+(?:stop|mask|disable)(?:\s+--now)?\s+(?:NetworkManager|network-manager)(?:\.service)?(?:\s|$)", customize) is None
assert "NetworkManager.service" not in customize and "network-manager.service" not in customize
assert "PasswordAuthentication no" in customize
assert [
    line.strip()
    for line in customize.splitlines()
    if re.search(r"\bsystemctl\s+(?:disable|mask)\s+(?:ssh|sshd)\.(?:service|socket)\b", line)
] == [
    "systemctl disable ssh.service",
    "systemctl disable ssh.socket",
    "systemctl mask ssh.service",
    "systemctl mask ssh.socket",
    "systemctl mask sshd.service",
    "systemctl mask sshd.socket",
]
assert "systemctl disable sshd.service" not in customize and "systemctl disable sshd.socket" not in customize
assert "|| true" not in "\n".join(
    line for line in customize.splitlines() if re.search(r"\bsystemctl\s+(?:disable|mask)\s+(?:ssh|sshd)\.(?:service|socket)\b", line)
)
assert 'if [[ "$OCTESSERA_IMAGE_MODE" == production ]]; then\n  rm -f /root/.not_logged_in_yet' in customize
assert setup_config.count('invoke(["systemctl", "enable", "--now", "ssh.service"])') == 1
assert (
    '    if mode in ("key", "password"):\n'
    '        invoke(["ssh-keygen", "-A"])\n'
    '        for unit in profile["ssh_units"]:\n'
    '            invoke(["systemctl", "unmask", unit])\n'
    '        invoke(["systemctl", "enable", "--now", "ssh.service"])'
) in setup_config
assert '        invoke(["systemctl", "disable", "--now", "ssh.service"])\n        invoke(["systemctl", "disable", "--now", "ssh.socket"])\n        for unit in profile["ssh_units"]:\n            invoke(["systemctl", "mask", unit])' in setup_config
assert "wifi_connect_artifact_dir=\"$overlay_dir/usr/local/share/octessera/wifi-connect\"" in customize
assert "wifi_connect_expected_sha256=4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627" in customize
assert "wifi_connect_patch_sha256=c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6" in customize
assert "wifi-connect.metadata.json" in customize and "cargo-metadata.json" in customize
assert "THIRD-PARTY-NOTICES.md" in customize
assert "sha256sum -c -" in customize
assert "github.com/balena-os/wifi-connect/releases" not in customize
assert "notice_tree=\"$overlay_dir/usr/share/doc/octessera\"" in customize and "tools/legal/stage_notices.py" in customize and "/usr/share/doc/octessera" in customize
assert "install_orange_musical_assets \"$overlay_dir\" \"\"" in customize
assert "install_overlay_file usr/local/sbin/octessera-sd-card /usr/local/sbin/octessera-sd-card 0755" in runtime_assets
assert "install_overlay_file usr/local/lib/octessera/octessera-sd-card-lib.sh /usr/local/lib/octessera/octessera-sd-card-lib.sh 0644" in runtime_assets
assert "systemctl enable octessera-orange-sd-card.service" in runtime_assets
for source_shape in (
    '[[ -L "$sd_card_link" ]] || { echo "Orange SD service was not enabled as a symlink." >&2; return 1; }',
    '[[ "$sd_card_target" == "/etc/systemd/system/octessera-orange-sd-card.service" || "$sd_card_target" == "../octessera-orange-sd-card.service" ]]',
    'ln -s ../octessera-orange-sd-card.service "$sd_card_link"',
    '[[ -L "$sd_card_link" && "$(readlink "$sd_card_link")" == "../octessera-orange-sd-card.service" ]]',
    '[[ -L "$storage_control_link" ]] || { echo "Orange storage socket was not enabled as a symlink." >&2; return 1; }',
    '[[ "$storage_control_target" == "/etc/systemd/system/octessera-orange-storage-control.socket" || "$storage_control_target" == "../octessera-orange-storage-control.socket" ]]',
    'ln -s ../octessera-orange-storage-control.socket "$storage_control_link"',
    '[[ -L "$storage_control_link" && "$(readlink "$storage_control_link")" == "../octessera-orange-storage-control.socket" ]]',
):
    assert source_shape in runtime_assets
for symbol in ("CONFIG_MMC", "CONFIG_MMC_BLOCK", "CONFIG_MMC_SPI"):
    assert f"kernel_config_value {symbol}" in customize
assert "printf '%s\\n' mmc_spi > \"$sd_modules_load_file\"" in customize
assert 'octessera_run_strict_diagnostic "$audio_work" compile_audio_overlay dtc -@ -I dts -O dtb' in customize
assert 'octessera_run_strict_diagnostic "$audio_work" inspect_audio_overlay dtc -q -I dtb -O dts' in customize
assert 'octessera_run_strict_diagnostic "$audio_work" merge_production_user_overlays fdtoverlay' in customize
assert 'octessera_run_dtc_inspection "$audio_work" inspect_production_user_overlays dtc -q -I dtb -O dts' in customize
assert 'octessera_assert_orange_audio_merge "$production_spi_input_dtb" "$production_merged_dtb"' in customize
provisioner = (ROOT / "userpatches/overlay/usr/local/sbin/octessera-provision-musical-default").read_text(encoding="utf-8")
assert "samples" not in provisioner
inspector = (ROOT / "tools/armbian-image/inspect-path.sh").read_text(encoding="utf-8")
assert "local sample_root=var/lib/octessera/samples" in inspector
assert "chown root:root /etc/octessera/build-metadata.env" in customize
assert "chmod 0644 /etc/octessera/build-metadata.env" in customize
assert construction_inputs["userpatches/overlay/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf"]["mode"] == 420
assert (ROOT / "userpatches/overlay/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf").read_text(encoding="utf-8") == 'if ($msg == "sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin") then stop\n'
assert "install_overlay_file etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf /etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf 0644" in runtime_assets
assert "octessera_validate_orange_rsyslog_configuration()" in runtime_assets
assert 'validation_config="$(mktemp /tmp/octessera-rsyslog-validation.XXXXXX)"' in runtime_assets
assert 'global(net.enableDNS="off")' in runtime_assets
assert 'include(file="/etc/rsyslog.conf")' in runtime_assets
assert 'if printf \'%s\\n\' \'global(net.enableDNS="off")\' \'include(file="/etc/rsyslog.conf")\' > "$validation_config"; then' in runtime_assets
assert 'rsyslogd -N1 -f "$validation_config"' in runtime_assets
assert 'validation_status=$?' in runtime_assets
assert 'if rm -f -- "$validation_config"; then' in runtime_assets
assert 'return "$validation_status"' in runtime_assets
assert "rsyslogd -N1 -f /etc/rsyslog.conf" not in runtime_assets
for forbidden in ("rsyslogd -x", "/etc/hosts", "/etc/hostname", "hostnamectl"):
    assert forbidden not in runtime_assets
assert any(item["path"] == "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh" for item in contract["exact_inputs"])
default_input = next(item for item in contract["exact_inputs"] if item["path"] == "config/generated/pi/default.json")
assert default_input == {"path": "config/generated/pi/default.json", "sha256": "c076628ca5240ff82c63cdaa0886e9bb0828b9e1cd02188498251cc474f018ce", "size": 83596, "mode": 420}
assert contract["managed_outputs"][0] == {"path": "etc/profile.d/octessera-welcome.sh", "mode": 420, "uid": 0, "gid": 0}
for path in (
    "etc/systemd/system/octessera-orange-usb-gadget.service",
    "etc/systemd/system/octessera-device-apply-reboot.socket",
    "etc/systemd/system/octessera-device-apply-reboot@.service",
    "etc/systemd/system/octessera-update.socket",
    "etc/systemd/system/octessera-update@.service",
    "usr/local/sbin/octessera-update-broker",
    "usr/local/lib/octessera/device_config.py",
    "usr/local/sbin/octessera-device-apply-reboot",
    "usr/local/sbin/octessera-orange-storage",
    "usr/local/sbin/octessera-orange-storage-control",
    "etc/systemd/system/octessera-orange-storage-control.socket",
    "etc/systemd/system/octessera-orange-storage-control@.service",
    "usr/share/octessera/defaults/pi-default.json",
):
    assert any(item["path"] == path for item in contract["managed_outputs"]), path
assert contract["notice_bundle"] == {"manifest": "resources/legal/notice-bundle.json", "stager": "tools/legal/stage_notices.py", "installed_root": "usr/share/doc/octessera", "installed_outputs": "manifest-files", "proof": "tools/armbian-image/orange_boot_contract.py", "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"]}
assert contract["terminal_invariants"] == {"welcome_path": "etc/profile.d/octessera-welcome.sh", "hushlogin_path": "home/octessera/.hushlogin", "hushlogin_mode": 420, "hushlogin_empty": True, "forbidden_pam_update_motd_overrides": True, "ssh_masked_units": ["ssh.service", "ssh.socket", "sshd.service", "sshd.socket"], "armbian_onboarding_marker": "root/.not_logged_in_yet", "armbian_firstrun_service": "lib/systemd/system/armbian-firstrun.service", "armbian_firstrun_executable": "usr/lib/armbian/armbian-firstrun", "armbian_firstrun_enablement": "etc/systemd/system/multi-user.target.wants/armbian-firstrun.service", "armbian_firstrun_defaults": "etc/default/armbian-firstrun"}
assert contract["uart_invariants"] == {"overlay_name": "octessera-h618-input-routing", "console_assignment": "console=display", "forbidden_console_token": "console=ttyS0", "serial_getty_mask": "etc/systemd/system/serial-getty@ttyS0.service", "uart0_status": "disabled", "stdout_path": ""}
customize = (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8")
assert "octessera_set_armbian_display_console" in customize
assert "console=display" in (ROOT / "userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-boot-config.sh").read_text(encoding="utf-8")
assert contract["enabled_sysinit_wants"] == {"path": "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service", "target": "../octessera-orange-boot-splash.service"}
assert {"path": "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service", "kind": "symlink", "target": "../octessera-orange-oled-suspend.service", "uid": 0, "gid": 0} in contract["managed_outputs"]
assert all("sleep.target.wants/octessera-orange-oled-suspend.service" not in item["path"] for item in contract["managed_outputs"])
service = (ROOT / "userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service").read_text(encoding="utf-8")
assert "RequiredBy=sleep.target" in service and "WantedBy=sleep.target" not in service
assert contract["device_dependencies"] == {"spi_device": "/dev/spidev1.0", "gpio_device": "/dev/gpiochip1", "gpio_label": "300b000.pinctrl", "gpio_offsets": {"reset": 76, "dc": 270}, "udev_rule": "etc/udev/rules.d/70-octessera-orange-runtime.rules"}
assert contract["required_builtin_kernel_config_lines"] == ["CONFIG_SPI_SUN6I=y", "CONFIG_SPI_SPIDEV=y", "CONFIG_PINCTRL_SUNXI=y", "CONFIG_MMC=y", "CONFIG_MMC_BLOCK=y", "CONFIG_SOUND=y", "CONFIG_SND=y", "CONFIG_SND_SOC=y", "CONFIG_REGMAP_MMIO=y", "CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM=y", "CONFIG_SND_SOC_SUNXI_AHUB=y", "CONFIG_SND_SOC_SUNXI_AHUB_DAM=y", "CONFIG_SND_SOC_SUNXI_MACH=y", "CONFIG_NVMEM_SUNXI_SID=y"]
exact(contract["selected_initramfs"], ["required_paths", "forbidden_paths", "required_tools", "python_files", "required_python_modules", "installed_output_matches"])
assert contract["selected_initramfs"]["required_python_modules"] == ["fcntl", "math", "_json", "_posixsubprocess", "select", "_struct", "zlib"]
assert contract["selected_initramfs"]["forbidden_paths"] == ["usr/bin/gpiodetect", "usr/share/octessera/oled/octessera-mark.svg", "usr/share/octessera/oled/octessera-wordmark.svg"]
assert all(not path.endswith(".svg") for path in contract["selected_initramfs"]["required_paths"])
assert all(not item["initramfs_path"].endswith(".svg") for item in contract["selected_initramfs"]["installed_output_matches"])
assert "usr/share/octessera/oled/octessera-pi-booting.rgb565" in contract["selected_initramfs"]["required_paths"]
assert "usr/share/octessera/oled/octessera-pi-shutdown.rgb565" in contract["selected_initramfs"]["required_paths"]
assert all("system-sleep/octessera-orange-oled" not in item["path"] for item in contract["managed_outputs"])
assert any(item["path"] == "usr/share/octessera/oled/octessera-mark.svg" for item in contract["managed_outputs"])
assert any(item["path"] == "usr/share/octessera/oled/octessera-wordmark.svg" for item in contract["managed_outputs"])
print("Orange constructor classification and source digest tests passed")
