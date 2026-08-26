#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from rpi_kernel_contract import EXPECTED_FIRMWARE_INITRAMFS, sha256_file  # type: ignore[import-not-found]
from rpi_initramfs_proof import (  # type: ignore[import-not-found]
    compare_rootfs_files,
    extract_regular_files,
    parse_initramfs_listing,
    validate_rootfs_bindings,
    validate_selected_initramfs_contract,
    verify_command_layout,
)
from rpi_kernel_image_mount import ImageProofError

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
KERNEL_TOOLS = REPOSITORY_ROOT / "tools/pi-kernel"
BOOT_LAYER_CONTRACT_PATH = REPOSITORY_ROOT / "resources/image-construction/boot-layers/raspberry-pi-zero-2w.json"
CURRENT_BOOT_SERVICE = "etc/systemd/system/octessera-boot-splash.service"
INITRAMFS_SPLASH_INVOCATION = "setsid /usr/local/bin/octessera-pi --boot-splash-static >/dev/kmsg 2>&1 &"


def load_validator() -> Any:
    path = KERNEL_TOOLS / "validate-rpi-kernel-package.py"
    spec = importlib.util.spec_from_file_location("octessera_rpi_kernel_validator", path)
    if spec is None or spec.loader is None:
        raise ImageProofError(f"cannot load package validator: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_installer() -> Any:
    path = Path(__file__).with_name("install-rpi-kernel.py")
    spec = importlib.util.spec_from_file_location("octessera_rpi_kernel_installer", path)
    if spec is None or spec.loader is None:
        raise ImageProofError(f"cannot load image installer: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def boot_dir(root: Path) -> Path:
    firmware = root / "boot/firmware"
    return firmware if firmware.is_dir() else root / "boot"


def hash_matches(path: Path, expected: str, label: str) -> None:
    if not path.is_file() or sha256_file(path) != expected:
        raise ImageProofError(f"{label} hash does not match package provenance: {path}")


def verify_selectors(config: Path) -> None:
    installer = load_installer()
    try:
        installer.verify_selectors(config, installer.image_contract())
    except (installer.ImageInstallError, OSError) as error:
        raise ImageProofError(f"duplicate or conflicting firmware selectors: {error}") from error


def run_lsinitramfs(path: Path) -> str:
    try:
        result = subprocess.run(["lsinitramfs", "-l", str(path)], capture_output=True, text=True, check=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageProofError(f"cannot inspect initramfs {path} with lsinitramfs") from error
    return result.stdout


def resolve_regular_file(root: Path, path: Path, label: str) -> Path:
    try:
        root_resolved = root.resolve(strict=True)
        selected = path.resolve(strict=True)
        selected.relative_to(root_resolved)
    except (OSError, ValueError) as error:
        raise ImageProofError(f"{label} escapes the image root: {path}") from error
    if not path.is_file() or path.is_symlink():
        raise ImageProofError(f"{label} is not a regular file: {path}")
    return selected


def verify_initramfs(path: Path, run_listing: Callable[[Path], str] = run_lsinitramfs) -> None:
    if not path.is_file() or path.is_symlink():
        raise ImageProofError(f"initramfs is not a regular file: {path}")
    if path.stat().st_size == 0:
        raise ImageProofError(f"initramfs is empty: {path}")
    listing = run_listing(path)
    if not listing.strip():
        raise ImageProofError(f"lsinitramfs returned an empty listing: {path}")


def verify_selected_initramfs(boot: Path, path: Path, run_listing: Callable[[Path], str] = run_lsinitramfs) -> None:
    selected = resolve_regular_file(boot, path, "selected initramfs")
    verify_initramfs(selected, run_listing)


def classify_boot_layer(root: Path, boot_layer_contract_path: Path = BOOT_LAYER_CONTRACT_PATH) -> tuple[str, dict[str, Any] | None]:
    service = root / CURRENT_BOOT_SERVICE
    runtime = root / "etc/systemd/system/octessera.service"
    if not service.is_file() or service.is_symlink():
        raise ImageProofError(f"Raspberry boot service is missing: {service}")
    if not runtime.is_file() or runtime.is_symlink():
        raise ImageProofError(f"Raspberry runtime service is missing: {runtime}")
    try:
        lines = set(service.read_text(encoding="utf-8").splitlines())
        runtime_lines = set(runtime.read_text(encoding="utf-8").splitlines())
    except (OSError, UnicodeDecodeError) as error:
        raise ImageProofError(f"Raspberry boot service output is unreadable: {service}") from error
    if "Type=simple" in lines:
        required = ("ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop", "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1")
        if not all(item in lines for item in required):
            raise ImageProofError("Raspberry constructor boot service is incomplete")
        for item in ("Wants=systemd-udev-settle.service", "After=systemd-modules-load.service systemd-udevd.service systemd-udev-trigger.service systemd-udev-settle.service", "Before=sysinit.target octessera.service", "DevicePolicy=closed", "DeviceAllow=/dev/spidev0.0 rw", "DeviceAllow=/dev/gpiomem rw", "DeviceAllow=/dev/gpiochip0 rw"):
            if item not in lines:
                raise ImageProofError(f"Raspberry constructor boot service is missing {item}")
        if sum(line.startswith("Type=") for line in lines) != 1 or sum(line.startswith("ExecStart=") for line in lines) != 1 or sum(line.startswith("Environment=OCTESSERA_OLED_BOOT_HANDOFF=") for line in lines) != 1:
            raise ImageProofError("Raspberry constructor boot service has duplicate directives")
        for item in ("Wants=octessera-boot-splash.service", "After=octessera-boot-splash.service", "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1"):
            if item not in runtime_lines:
                raise ImageProofError(f"Raspberry constructor runtime service is missing {item}")
        if sum(line.startswith("Environment=OCTESSERA_OLED_BOOT_HANDOFF=") for line in runtime_lines) != 1:
            raise ImageProofError("Raspberry constructor runtime service has an extra OLED handoff environment")
        return "constructor-required", load_boot_layer_contract(boot_layer_contract_path)
    if "Type=oneshot" in lines:
        for item in ("After=systemd-modules-load.service systemd-udevd.service", "Before=sysinit.target octessera.service", "ExecStart=-/usr/local/bin/octessera-pi --boot-splash-once", "TimeoutStartSec=2", "WantedBy=sysinit.target"):
            if item not in lines:
                raise ImageProofError(f"trusted-parent-v0.7.5 boot service is missing {item}")
        if "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1" in lines or "ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop" in lines or "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1" in runtime_lines or "Wants=octessera-boot-splash.service" in runtime_lines or "After=octessera-boot-splash.service" in runtime_lines:
            raise ImageProofError("trusted-parent-v0.7.5 image unexpectedly contains the v1 handoff")
        if sum(line.startswith("Type=") for line in lines) != 1 or sum(line.startswith("ExecStart=") for line in lines) != 1:
            raise ImageProofError("trusted-parent-v0.7.5 boot service has duplicate directives")
        return "trusted-parent-v0.7.5", None
    raise ImageProofError("Raspberry boot service is neither current constructor output nor the v0.7.5 trusted parent")


def load_boot_layer_contract(path: Path = BOOT_LAYER_CONTRACT_PATH) -> dict[str, Any]:
    try:
        contract = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ImageProofError("Raspberry v1 boot-layer contract is unreadable") from error
    if not isinstance(contract, dict):
        raise ImageProofError("Raspberry v1 boot-layer contract is not an object")
    checks = (("schema", "octessera.image-construction/boot-layer/v1"), ("schema_version", 1), ("contract_kind", "boot-layer"), ("board_profile", "raspberry-pi-zero-2w"), ("classification", "constructor-required"), ("trusted_parent_finalization", "forbidden"), ("mutation_authority", "none"), ("selected_initramfs_regeneration", "required"))
    for key, value in checks:
        if contract.get(key) != value:
            raise ImageProofError(f"Raspberry boot-layer {key} is invalid")
    source_inputs = contract.get("source_inputs")
    if not isinstance(source_inputs, list) or not source_inputs:
        raise ImageProofError("Raspberry boot-layer source inputs are empty")
    if sum(source.get("path") == "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" for source in source_inputs if isinstance(source, dict)) != 1:
        raise ImageProofError("Raspberry device config validator source identity is not unique")
    if sum(source.get("path") == "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-usb-gadget" for source in source_inputs if isinstance(source, dict)) != 1:
        raise ImageProofError("Raspberry USB gadget composer source identity is not unique")
    for source in source_inputs:
        if not isinstance(source, dict) or set(source) != {"path", "sha256", "size"}:
            raise ImageProofError("Raspberry boot-layer source input changed")
        source_path = REPOSITORY_ROOT / source["path"]
        hash_matches(source_path, source["sha256"], "Raspberry boot-layer source input")
        if source_path.stat().st_size != source["size"]:
            raise ImageProofError("Raspberry boot-layer source input size changed")
    live_parity = contract.get("live_parity_inputs")
    expected = [{"path": "tools/pi/deploy-pi.sh", "sha256": "54ea212f4fefa218315d3a9a9e982e3cfcc311cea832be339e8733ce6b1179ce", "size": 17245}, {"path": "tools/pi/provision/provision.sh", "sha256": "5309db2d7d66abf221636d48b06b189b538d29ff2095a0999d0238105d00ea03", "size": 14552}]
    if not isinstance(live_parity, list) or live_parity != expected:
        raise ImageProofError("Raspberry live parity inputs changed")
    for source in live_parity:
        source_path = REPOSITORY_ROOT / source["path"]
        hash_matches(source_path, source["sha256"], "Raspberry live parity input")
        if source_path.stat().st_size != source["size"]:
            raise ImageProofError("Raspberry live parity input size changed")
    if contract.get("notice_bundle") != {"manifest": "resources/legal/notice-bundle.json", "stager": "tools/legal/stage_notices.py", "installed_root": "usr/share/doc/octessera", "installed_outputs": "manifest-files", "proof": "tools/pi-image/verify-boot-layout.sh", "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"], "firmware_license_path": None, "firmware_license_gate": "full-constructor"}:
        raise ImageProofError("Raspberry legal notice contract changed")
    selected = contract.get("selected_initramfs")
    if not isinstance(selected, dict) or selected.get("path") != EXPECTED_FIRMWARE_INITRAMFS:
        raise ImageProofError("Raspberry selected initramfs contract is invalid")
    try:
        validate_selected_initramfs_contract(selected)
    except ValueError as error:
        raise ImageProofError(str(error)) from error
    if contract.get("uart_invariants") != {"required_config": ["dtoverlay=disable-bt", "enable_uart=0"], "forbidden_config": ["enable_uart=1"], "required_cmdline": ["console=tty1"], "forbidden_cmdline_prefixes": ["console=serial0", "console=ttyAMA0", "console=ttyS0"], "masks": ["serial-getty@serial0.service", "serial-getty@ttyAMA0.service", "serial-getty@ttyS0.service"], "disabled_services": ["bluetooth.service", "hciuart.service"]}:
        raise ImageProofError("Raspberry UART invariants changed")
    return contract


def verify_managed_boot_outputs(root: Path, contract: dict[str, Any]) -> dict[str, Any] | None:
    outputs = contract.get("managed_outputs")
    if not isinstance(outputs, list) or not outputs:
        raise ImageProofError("Raspberry managed boot outputs are empty")
    identities: dict[str, dict[str, Any]] = {}
    for output in outputs:
        if not isinstance(output, dict):
            raise ImageProofError("Raspberry managed boot output is invalid")
        path = root / output["path"]
        if output["type"] == "symlink":
            if not path.is_symlink() or path.readlink().as_posix() != output["target"]:
                raise ImageProofError(f"managed boot symlink is not exact: {path}")
            continue
        if not path.is_file() or path.is_symlink():
            raise ImageProofError(f"managed boot output is missing: {path}")
        metadata = path.stat()
        if metadata.st_uid != output["uid"] or metadata.st_gid != output["gid"] or metadata.st_mode & 0o7777 != output["mode"]:
            raise ImageProofError(f"managed boot output metadata changed: {path}")
        if output["classification"] == "device-config-validator":
            source = REPOSITORY_ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
            if not source.is_file() or source.is_symlink() or path.read_bytes() != source.read_bytes():
                raise ImageProofError("Raspberry installed device config validator is not byte-identical to the canonical source")
            source_hash = sha256_file(source)
            identities["device_config_validator"] = {"path": "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py", "sha256": source_hash, "size": source.stat().st_size}
        if output["classification"] == "usb-gadget-composer":
            source = REPOSITORY_ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-usb-gadget"
            if not source.is_file() or source.is_symlink() or path.read_bytes() != source.read_bytes():
                raise ImageProofError("Raspberry installed USB gadget composer is not byte-identical to the canonical source")
            source_hash = sha256_file(source)
            identities["usb_gadget_composer"] = {"path": "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-usb-gadget", "sha256": source_hash, "size": source.stat().st_size}
    return identities or None


def verify_selected_initramfs_entries(path: Path, contract: dict[str, Any], root: Path, run_listing: Callable[[Path], str] = run_lsinitramfs) -> None:
    listing = run_listing(path)
    try:
        records = parse_initramfs_listing(listing)
        verify_command_layout(path, listing, contract["selected_initramfs"])
    except ValueError as error:
        raise ImageProofError(str(error)) from error
    entries = {record["name"] for record in records}
    selected = contract["selected_initramfs"]
    bindings = selected["byte_bindings"]
    required_files = [binding["archive_path"] for binding in bindings] + selected["required_regular_executables"]
    for required in required_files + [entry["path"] for entry in selected["required_symlinks"]]:
        if required not in entries:
            raise ImageProofError(f"selected initramfs is missing constructor output: {required}")
    entry_names = "\n".join(entries)
    for module in selected["required_module_names"]:
        if module not in entry_names:
            raise ImageProofError(f"selected initramfs is missing constructor module: {module}")
    try:
        validate_rootfs_bindings(root, selected)
    except (OSError, ValueError) as error:
        raise ImageProofError(str(error)) from error
    pairs = tuple((binding["archive_path"], binding["rootfs_path"]) for binding in bindings)
    try:
        extracted = extract_regular_files(path, list(required_files), lambda _: listing, tuple(required_files), selected)
        compare_rootfs_files(root, extracted, pairs)
    except (OSError, ValueError) as error:
        raise ImageProofError(str(error)) from error
    script_binding = next(binding for binding in bindings if binding["role"] == "splash-script")
    try:
        lines = extracted[script_binding["archive_path"]].decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ImageProofError("selected initramfs splash script is not UTF-8") from error
    if not any(line.strip() == INITRAMFS_SPLASH_INVOCATION for line in lines):
        raise ImageProofError("selected initramfs does not contain the exact static splash invocation")
