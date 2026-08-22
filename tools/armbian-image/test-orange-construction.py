#!/usr/bin/env python3
import copy
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "resources/image-construction/boot-layers/orange-pi-zero-2w.json"
SETUP_CONTRACT_PATH = ROOT / "resources/image-mutations/orange-pi-zero-2w-setup.json"
SOURCE_BOUND_PROOF_SOURCES = {
    "tools/armbian-image/verify-orange-image.py",
    "tools/armbian-image/orange_boot_contract.py",
    "tools/armbian-image/orange_boot_inventory.py",
    "tools/armbian-image/orange_boot_selection.py",
    "tools/armbian-image/orange_image_mount.py",
    "tools/armbian-image/orange_initramfs.py",
    "tools/armbian-image/orange_phase5_proof.py",
    "tools/armbian-image/orange_audio_proof.py",
    "tools/armbian-image/orange_trusted_parent_proof.py",
    "tools/armbian-image/verify_runtime_account.py",
    "userpatches/overlay/usr/local/share/octessera/device-tree/orange-ahub-overlay-validation.sh",
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
assert "CONFIG_SND_SOC_PCM5102A" not in (ROOT / "userpatches/extensions/octessera_audio.sh").read_text(encoding="utf-8")
assert all(any(item["path"] == path for item in contract["managed_outputs"]) for path in (
    "usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts",
    "boot/overlay-user/octessera-ahub0-pcm5102.dtbo",
    "etc/octessera/build-metadata.env",
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
        assert item["target"] in {"../octessera-orange-boot-splash.service", "../octessera-orange-oled-suspend.service", "../octessera-device-apply-reboot.socket", "../octessera-update-recovery.service", "../octessera-update.socket", "/dev/null"} and item["uid"] == 0 and item["gid"] == 0
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
):
    assert any(item["path"] == path for item in contract["exact_inputs"]), path
validator_inputs = [item for item in contract["exact_inputs"] if item["path"] == "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"]
assert len(validator_inputs) == 1
assert contract["mounted_proof"] == "tools/armbian-image/verify-orange-image.py"
for line in (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8").splitlines():
    if any(path in line for path in ("/etc/motd", "/etc/issue", "/usr/share/doc", "/usr/share/common-licenses", "/usr/share/doc/base-files/copyright")) and "/usr/share/doc/octessera" not in line:
        raise AssertionError("Orange constructor mutates a parent legal path")
customize = (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8")
assert "notice_tree=\"$overlay_dir/usr/share/doc/octessera\"" in customize and "tools/legal/stage_notices.py" in customize and "/usr/share/doc/octessera" in customize
assert "install_orange_musical_assets \"$overlay_dir\" \"\"" in customize
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
    "usr/share/octessera/defaults/pi-default.json",
):
    assert any(item["path"] == path for item in contract["managed_outputs"]), path
assert contract["notice_bundle"] == {"manifest": "resources/legal/notice-bundle.json", "stager": "tools/legal/stage_notices.py", "installed_root": "usr/share/doc/octessera", "installed_outputs": "manifest-files", "proof": "tools/armbian-image/orange_boot_contract.py", "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"]}
assert contract["terminal_invariants"] == {"welcome_path": "etc/profile.d/octessera-welcome.sh", "hushlogin_path": "home/octessera/.hushlogin", "hushlogin_mode": 420, "hushlogin_empty": True, "forbidden_pam_update_motd_overrides": True}
assert contract["uart_invariants"] == {"overlay_name": "octessera-h618-input-routing", "forbidden_console_token": "console=ttyS0", "serial_getty_mask": "etc/systemd/system/serial-getty@ttyS0.service", "uart0_status": "disabled", "stdout_path": ""}
assert contract["enabled_sysinit_wants"] == {"path": "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service", "target": "../octessera-orange-boot-splash.service"}
assert {"path": "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service", "kind": "symlink", "target": "../octessera-orange-oled-suspend.service", "uid": 0, "gid": 0} in contract["managed_outputs"]
assert all("sleep.target.wants/octessera-orange-oled-suspend.service" not in item["path"] for item in contract["managed_outputs"])
service = (ROOT / "userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service").read_text(encoding="utf-8")
assert "RequiredBy=sleep.target" in service and "WantedBy=sleep.target" not in service
assert contract["device_dependencies"] == {"spi_device": "/dev/spidev1.0", "gpio_device": "/dev/gpiochip1", "gpio_label": "300b000.pinctrl", "gpio_offsets": {"reset": 76, "dc": 270}, "udev_rule": "etc/udev/rules.d/70-octessera-orange-runtime.rules"}
assert contract["required_builtin_kernel_config_lines"] == ["CONFIG_SPI_SUN6I=y", "CONFIG_SPI_SPIDEV=y", "CONFIG_PINCTRL_SUNXI=y", "CONFIG_SOUND=y", "CONFIG_SND=y", "CONFIG_SND_SOC=y", "CONFIG_REGMAP_MMIO=y", "CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM=y", "CONFIG_SND_SOC_SUNXI_AHUB=y", "CONFIG_SND_SOC_SUNXI_AHUB_DAM=y", "CONFIG_SND_SOC_SUNXI_MACH=y", "CONFIG_NVMEM_SUNXI_SID=y"]
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
