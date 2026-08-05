#!/usr/bin/env python3
import hashlib
import json
import stat
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "resources/image-construction/boot-layers/orange-pi-zero-2w.json"
contract = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))


def exact(value, keys):
    assert set(value) == set(keys)


exact(contract, ["schema_version", "proof_mode", "contract_kind", "construction_kind", "board_profile", "constructor_required", "trusted_parent_finalization", "mutation_authority", "regeneration_required", "expected_changes", "exact_inputs", "managed_outputs", "notice_bundle", "terminal_invariants", "uart_invariants", "enabled_sysinit_wants", "device_dependencies", "selected_initramfs", "mounted_proof", "proofs"])
assert contract["schema_version"] == 1
assert contract["proof_mode"] == "phase5-constructor"
assert contract["contract_kind"] == "constructor-required"
assert contract["construction_kind"] == "orange-boot-layer"
assert contract["board_profile"] == "orange-pi-zero-2w"
assert contract["constructor_required"] is True
assert contract["trusted_parent_finalization"] == "forbidden"
assert contract["regeneration_required"] == ["initramfs", "python_closure"]
assert contract["expected_changes"] == {"packages": [], "accounts": [], "kernel": False, "firmware": False}

for item in contract["exact_inputs"]:
    exact(item, ["path", "sha256", "size", "mode"])
    source = ROOT / item["path"]
    assert source.is_file() and not source.is_symlink(), source
    assert hashlib.sha256(source.read_bytes()).hexdigest() == item["sha256"], source
    assert source.stat().st_size == item["size"], source
    assert item["mode"] in {420, 493}

for item in contract["managed_outputs"]:
    if item["path"] == "home/octessera/.hushlogin":
        exact(item, ["path", "mode", "owner", "group", "content"])
        assert item == {"path": "home/octessera/.hushlogin", "mode": 420, "owner": "octessera", "group": "octessera", "content": "empty"}
    elif item.get("kind") == "symlink":
        exact(item, ["path", "kind", "target", "uid", "gid"])
        assert item["target"] in {"../octessera-orange-boot-splash.service", "/dev/null"} and item["uid"] == 0 and item["gid"] == 0
    else:
        exact(item, ["path", "mode", "uid", "gid"])
        assert item["mode"] in {420, 493} and item["uid"] == 0 and item["gid"] == 0

assert all(isinstance(path, str) and path.startswith("tools/armbian-image/") for path in contract["proofs"])
assert "tools/armbian-image/validate.sh" in contract["proofs"]
assert contract["mutation_authority"] == "none"
assert contract["mounted_proof"] == "tools/armbian-image/verify-orange-image.py"
for line in (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8").splitlines():
    if any(path in line for path in ("/etc/motd", "/etc/issue", "/usr/share/doc", "/usr/share/common-licenses", "/usr/share/doc/base-files/copyright")) and "/usr/share/doc/octessera" not in line:
        raise AssertionError("Orange constructor mutates a parent legal path")
customize = (ROOT / "userpatches/customize-image.sh").read_text(encoding="utf-8")
assert "notice_tree=\"$overlay_dir/usr/share/doc/octessera\"" in customize and "tools/legal/stage_notices.py" in customize and "/usr/share/doc/octessera" in customize
assert "chown root:root /etc/octessera/build-metadata.env" in customize
assert "chmod 0644 /etc/octessera/build-metadata.env" in customize
assert any(item["path"] == "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh" for item in contract["exact_inputs"])
assert contract["managed_outputs"][0] == {"path": "etc/profile.d/octessera-welcome.sh", "mode": 420, "uid": 0, "gid": 0}
assert contract["notice_bundle"] == {"manifest": "resources/legal/notice-bundle.json", "stager": "tools/legal/stage_notices.py", "installed_root": "usr/share/doc/octessera", "installed_outputs": "manifest-files", "proof": "tools/armbian-image/orange_boot_contract.py", "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"]}
assert contract["terminal_invariants"] == {"welcome_path": "etc/profile.d/octessera-welcome.sh", "hushlogin_path": "home/octessera/.hushlogin", "hushlogin_mode": 420, "hushlogin_empty": True, "forbidden_pam_update_motd_overrides": True}
assert contract["uart_invariants"] == {"overlay_name": "octessera-h618-input-routing", "forbidden_console_token": "console=ttyS0", "serial_getty_mask": "etc/systemd/system/serial-getty@ttyS0.service", "uart0_status": "disabled", "stdout_path": ""}
assert contract["enabled_sysinit_wants"] == {"path": "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service", "target": "../octessera-orange-boot-splash.service"}
assert contract["device_dependencies"] == {"spi_device": "/dev/spidev1.0", "gpio_device": "/dev/gpiochip1", "gpio_label": "300b000.pinctrl", "gpio_offsets": {"reset": 76, "dc": 270}, "udev_rule": "etc/udev/rules.d/70-octessera-orange-runtime.rules"}
assert contract["selected_initramfs"]["forbidden_paths"] == ["usr/bin/gpiodetect"]
print("Orange constructor classification and source digest tests passed")
