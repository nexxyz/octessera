#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Iterator

KERNEL_TOOLS = Path(__file__).resolve().parents[1] / "pi-kernel"
REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
BOOT_LAYER_CONTRACT_PATH = REPOSITORY_ROOT / "resources/image-construction/boot-layers/raspberry-pi-zero-2w.json"
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(KERNEL_TOOLS))
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/legal"))

from rpi_kernel_contract import (  # type: ignore[import-not-found]  # noqa: E402
    EXPECTED_FIRMWARE_DEVICE_TREE,
    EXPECTED_FIRMWARE_INITRAMFS,
    EXPECTED_FIRMWARE_KERNEL,
    EXPECTED_FIRMWARE_OVERLAY_PREFIX,
    ContractError,
    load_contract,
    sha256_file,
)
from rpi_kernel_image import KernelImageError, firmware_kernel_bytes  # type: ignore[import-not-found]  # noqa: E402
from rpi_initramfs_proof import compare_rootfs_files, extract_regular_files  # type: ignore[import-not-found]  # noqa: E402


class ImageProofError(ValueError):
    pass


CURRENT_BOOT_SERVICE = "etc/systemd/system/octessera-boot-splash.service"
INITRAMFS_SPLASH_SCRIPT = "scripts/init-premount/octessera-boot-splash"
ROOTFS_SPLASH_SCRIPT = "etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
INITRAMFS_SPLASH_INVOCATION = "OCTESSERA_INITRAMFS_BOOT_SPLASH=1 setsid /usr/local/bin/octessera-pi --boot-splash-once >/dev/kmsg 2>&1 &"


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ImageProofError(message)


def _load_validator() -> Any:
    path = KERNEL_TOOLS / "validate-rpi-kernel-package.py"
    spec = importlib.util.spec_from_file_location("octessera_rpi_kernel_validator", path)
    if spec is None or spec.loader is None:
        raise ImageProofError(f"cannot load package validator: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_installer() -> Any:
    path = Path(__file__).with_name("install-rpi-kernel.py")
    spec = importlib.util.spec_from_file_location("octessera_rpi_kernel_installer", path)
    if spec is None or spec.loader is None:
        raise ImageProofError(f"cannot load image installer: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _boot_dir(root: Path) -> Path:
    firmware = root / "boot/firmware"
    return firmware if firmware.is_dir() else root / "boot"


def _hash_matches(path: Path, expected: str, label: str) -> None:
    _require(path.is_file(), f"missing {label}: {path}")
    _require(sha256_file(path) == expected, f"{label} hash does not match package provenance: {path}")


def _verify_selectors(config: Path) -> None:
    installer = _load_installer()
    try:
        installer.verify_selectors(config, installer.image_contract())
    except (installer.ImageInstallError, OSError) as error:
        raise ImageProofError(f"duplicate or conflicting firmware selectors: {error}") from error


def _run_lsinitramfs(path: Path) -> str:
    try:
        result = subprocess.run(["lsinitramfs", "-l", str(path)], capture_output=True, text=True, check=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageProofError(f"cannot inspect initramfs {path} with lsinitramfs") from error
    return result.stdout


def _resolve_regular_file(root: Path, path: Path, label: str) -> Path:
    try:
        root_resolved = root.resolve(strict=True)
        selected = path.resolve(strict=True)
        selected.relative_to(root_resolved)
    except (OSError, ValueError) as error:
        raise ImageProofError(f"{label} escapes the image root: {path}") from error
    _require(path.is_file() and not path.is_symlink(), f"{label} is not a regular file: {path}")
    return selected


def _verify_initramfs(path: Path) -> None:
    _require(path.is_file() and not path.is_symlink(), f"initramfs is not a regular file: {path}")
    _require(path.stat().st_size > 0, f"initramfs is empty: {path}")
    listing = _run_lsinitramfs(path)
    _require(bool(listing.strip()), f"lsinitramfs returned an empty listing: {path}")


def _verify_selected_initramfs(boot: Path, path: Path) -> None:
    selected = _resolve_regular_file(boot, path, "selected initramfs")
    _verify_initramfs(selected)


def _initramfs_entries(listing: str) -> set[str]:
    entries = set()
    for line in listing.splitlines():
        fields = line.split()
        if fields:
            entries.add(fields[-1].removeprefix("./"))
    return entries


def _classify_boot_layer(root: Path) -> tuple[str, dict[str, Any] | None]:
    service = root / CURRENT_BOOT_SERVICE
    runtime = root / "etc/systemd/system/octessera.service"
    _require(service.is_file() and not service.is_symlink(), f"Raspberry boot service is missing: {service}")
    _require(runtime.is_file() and not runtime.is_symlink(), f"Raspberry runtime service is missing: {runtime}")
    try:
        lines = set(service.read_text(encoding="utf-8").splitlines())
        runtime_lines = set(runtime.read_text(encoding="utf-8").splitlines())
    except (OSError, UnicodeDecodeError) as error:
        raise ImageProofError(f"Raspberry boot service output is unreadable: {service}") from error
    if "Type=simple" in lines:
        _require(
            "ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop" in lines
            and "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1" in lines,
            "Raspberry constructor boot service is incomplete",
        )
        for required in (
            "After=systemd-modules-load.service systemd-udevd.service systemd-udev-trigger.service",
            "Before=sysinit.target octessera.service",
            "DevicePolicy=closed",
            "DeviceAllow=/dev/spidev0.0 rw",
            "DeviceAllow=/dev/gpiomem rw",
        ):
            _require(required in lines, f"Raspberry constructor boot service is missing {required}")
        _require(sum(line.startswith("Type=") for line in lines) == 1, "Raspberry constructor boot service has an extra Type directive")
        _require(sum(line.startswith("ExecStart=") for line in lines) == 1, "Raspberry constructor boot service has an extra CLI")
        _require(
            sum(line.startswith("Environment=OCTESSERA_OLED_BOOT_HANDOFF=") for line in lines) == 1,
            "Raspberry constructor boot service has an extra OLED handoff environment",
        )
        for required in ("Wants=octessera-boot-splash.service", "After=octessera-boot-splash.service", "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1"):
            _require(required in runtime_lines, f"Raspberry constructor runtime service is missing {required}")
        _require(
            sum(line.startswith("Environment=OCTESSERA_OLED_BOOT_HANDOFF=") for line in runtime_lines) == 1,
            "Raspberry constructor runtime service has an extra OLED handoff environment",
        )
        return "constructor-required", _load_boot_layer_contract()
    if "Type=oneshot" in lines:
        for required in (
            "After=systemd-modules-load.service systemd-udevd.service",
            "Before=sysinit.target octessera.service",
            "ExecStart=-/usr/local/bin/octessera-pi --boot-splash-once",
            "TimeoutStartSec=2",
            "WantedBy=sysinit.target",
        ):
            _require(required in lines, f"trusted-parent-v0.7.5 boot service is missing {required}")
        _require(
            "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1" not in lines
            and "ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop" not in lines,
            "trusted-parent-v0.7.5 parent unexpectedly contains the v1 boot layer",
        )
        _require(sum(line.startswith("Type=") for line in lines) == 1, "trusted-parent-v0.7.5 boot service has an extra Type directive")
        _require(sum(line.startswith("ExecStart=") for line in lines) == 1, "trusted-parent-v0.7.5 boot service has an extra CLI")
        _require(
            "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1" not in runtime_lines
            and "Wants=octessera-boot-splash.service" not in runtime_lines
            and "After=octessera-boot-splash.service" not in runtime_lines,
            "trusted-parent-v0.7.5 runtime service unexpectedly contains the v1 handoff",
        )
        return "trusted-parent-v0.7.5", None
    raise ImageProofError("Raspberry boot service is neither current constructor output nor the v0.7.5 trusted parent")


def _load_boot_layer_contract() -> dict[str, Any]:
    try:
        contract = json.loads(BOOT_LAYER_CONTRACT_PATH.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ImageProofError("Raspberry v1 boot-layer contract is unreadable") from error
    _require(isinstance(contract, dict), "Raspberry v1 boot-layer contract is not an object")
    _require(contract.get("schema") == "octessera.image-construction/boot-layer/v1", "Raspberry boot-layer schema is not v1")
    _require(contract.get("schema_version") == 1, "Raspberry boot-layer schema version is not 1")
    _require(contract.get("contract_kind") == "boot-layer", "Raspberry boot-layer contract kind is invalid")
    _require(contract.get("board_profile") == "raspberry-pi-zero-2w", "Raspberry boot-layer board is invalid")
    _require(contract.get("classification") == "constructor-required", "Raspberry boot-layer classification is invalid")
    _require(contract.get("trusted_parent_finalization") == "forbidden", "Raspberry trusted-parent boot finalization is not forbidden")
    _require(contract.get("mutation_authority") == "none", "Raspberry boot-layer is executable mutation authority")
    _require(contract.get("selected_initramfs_regeneration") == "required", "selected initramfs regeneration is not required")
    source_inputs = contract.get("source_inputs")
    _require(bool(isinstance(source_inputs, list) and source_inputs), "Raspberry boot-layer source inputs are empty")
    for source in source_inputs:
        _require(isinstance(source, dict), "Raspberry boot-layer source input changed")
        _require(set(source) == {"path", "sha256", "size"}, "Raspberry boot-layer source input changed")
        source_path = REPOSITORY_ROOT / source["path"]
        _hash_matches(source_path, source["sha256"], "Raspberry boot-layer source input")
        _require(source_path.stat().st_size == source["size"], "Raspberry boot-layer source input size changed")
    live_parity = contract.get("live_parity_inputs")
    _require(
        live_parity == [
            {"path": "tools/pi/deploy-pi.sh", "sha256": "5ba7792299b16e74f42362a346b116bcc1f10f320cb0faae4dd4e5e3be291b80", "size": 15497},
            {"path": "tools/pi/provision/provision.sh", "sha256": "f7619799f5ad2ab8f8b82243bff344199b47c1252bc568fef8b10ad8bb095e06", "size": 10993},
        ],
        "Raspberry live parity inputs changed",
    )
    for source in live_parity:
        source_path = REPOSITORY_ROOT / source["path"]
        _hash_matches(source_path, source["sha256"], "Raspberry live parity input")
        _require(source_path.stat().st_size == source["size"], "Raspberry live parity input size changed")
    _require(
        contract.get("notice_bundle") == {
            "manifest": "resources/legal/notice-bundle.json",
            "stager": "tools/legal/stage_notices.py",
            "installed_root": "usr/share/doc/octessera",
            "installed_outputs": "manifest-files",
            "proof": "tools/pi-image/verify-boot-layout.sh",
            "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"],
            "firmware_license_path": None,
            "firmware_license_gate": "full-constructor",
        },
        "Raspberry legal notice contract changed",
    )
    selected = contract.get("selected_initramfs")
    _require(isinstance(selected, dict), "Raspberry selected initramfs contract is missing")
    _require(selected.get("path") == EXPECTED_FIRMWARE_INITRAMFS, "Raspberry selected initramfs path changed")
    required_entries = selected.get("required_entries")
    required_modules = selected.get("required_module_names")
    _require(bool(isinstance(required_entries, list) and required_entries), "Raspberry initramfs entry contract is empty")
    _require(bool(isinstance(required_modules, list) and required_modules == ["spi-bcm2835", "spidev"]), "Raspberry initramfs module contract changed")
    _require(
        contract.get("uart_invariants") == {
            "required_config": ["dtoverlay=disable-bt", "enable_uart=0"],
            "forbidden_config": ["enable_uart=1"],
            "forbidden_cmdline_prefixes": ["console=serial0", "console=ttyAMA0", "console=ttyS0"],
            "masks": ["serial-getty@serial0.service", "serial-getty@ttyAMA0.service", "serial-getty@ttyS0.service"],
            "disabled_services": ["bluetooth.service", "hciuart.service"],
        },
        "Raspberry UART invariants changed",
    )
    return contract


def _verify_managed_boot_outputs(root: Path, contract: dict[str, Any]) -> None:
    outputs = contract.get("managed_outputs")
    if not isinstance(outputs, list) or not outputs:
        raise ImageProofError("Raspberry managed boot outputs are empty")
    for output in outputs:
        _require(isinstance(output, dict), "Raspberry managed boot output is invalid")
        path = root / output["path"]
        if output["type"] == "symlink":
            _require(path.is_symlink() and path.readlink().as_posix() == output["target"], f"managed boot symlink is not exact: {path}")
            continue
        _require(path.is_file() and not path.is_symlink(), f"managed boot output is missing: {path}")
        metadata = path.stat()
        _require(metadata.st_uid == output["uid"] and metadata.st_gid == output["gid"], f"managed boot ownership changed: {path}")
        _require(metadata.st_mode & 0o7777 == output["mode"], f"managed boot mode changed: {path}")


def _verify_selected_initramfs_entries(path: Path, contract: dict[str, Any], root: Path) -> None:
    listing = _run_lsinitramfs(path)
    entries = _initramfs_entries(listing)
    selected = contract["selected_initramfs"]
    for required in selected["required_entries"]:
        _require(required in entries, f"selected initramfs is missing constructor output: {required}")
    entry_names = "\n".join(entries)
    for module in selected["required_module_names"]:
        _require(module in entry_names, f"selected initramfs is missing constructor module: {module}")
    required_files = (INITRAMFS_SPLASH_SCRIPT, "usr/local/bin/octessera-pi")
    for required in required_files:
        _require(required in selected["required_entries"], f"selected initramfs contract is missing {required}")
    pairs = (
        (INITRAMFS_SPLASH_SCRIPT, ROOTFS_SPLASH_SCRIPT),
        ("usr/local/bin/octessera-pi", "usr/local/bin/octessera-pi"),
    )
    try:
        extracted = extract_regular_files(path, [pair[0] for pair in pairs], lambda _: listing)
    except ValueError as error:
        raise ImageProofError(str(error)) from error
    try:
        compare_rootfs_files(root, extracted, pairs)
    except (OSError, ValueError) as error:
        raise ImageProofError(str(error)) from error
    script = extracted[INITRAMFS_SPLASH_SCRIPT]
    try:
        lines = script.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ImageProofError("selected initramfs splash script is not UTF-8") from error
    _require(
        any(line.strip() == INITRAMFS_SPLASH_INVOCATION for line in lines),
        "selected initramfs does not contain the exact one-cycle splash invocation",
    )


def _verify_stock_recovery(root: Path, boot: Path) -> list[dict[str, str]]:
    recovery_root = boot / "octessera/recovery-stock"
    _require(recovery_root.is_dir() and not recovery_root.is_symlink(), "stock recovery directory is missing or unsafe")
    recovery_root = recovery_root.resolve(strict=True)
    manifest = _resolve_regular_file(root, recovery_root / "manifest.json", "stock recovery manifest")
    try:
        entries = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ImageProofError(f"invalid stock recovery manifest: {manifest}") from error
    _require(isinstance(entries, list) and bool(entries), "stock recovery manifest is empty")
    for entry in entries:
        _require(isinstance(entry, dict) and set(entry) == {"path", "recovery_path", "sha256"}, "stock recovery manifest entry changed")
        _require(all(isinstance(entry[key], str) for key in ("path", "recovery_path", "sha256")), "stock recovery manifest value types changed")
        _require(re.fullmatch(r"[0-9a-f]{64}", entry["sha256"]) is not None, "stock recovery manifest hash changed")
        retained = _resolve_regular_file(root, root / entry["path"], "retained stock file")
        recovery = _resolve_regular_file(root, root / entry["recovery_path"], "stock recovery file")
        try:
            recovery.relative_to(recovery_root)
        except ValueError as error:
            raise ImageProofError(f"stock recovery file is outside the recovery directory: {recovery}") from error
        _hash_matches(recovery, entry["sha256"], "stock recovery file")
        is_initramfs = Path(entry["path"]).parent == Path("boot") and Path(entry["path"]).name.startswith("initrd.img-")
        if is_initramfs:
            _verify_initramfs(retained)
            _verify_initramfs(recovery)
        else:
            _hash_matches(retained, entry["sha256"], "retained stock file")
    return entries


def _verify_payload(root: Path, boot: Path, package: Path, package_inventory: dict[str, Any]) -> dict[str, Any]:
    module_root = root / f"lib/modules/{package_inventory['kernel_release']}"
    _require((module_root / "modules.dep").is_file(), "depmod did not create modules.dep for the selected kernel")
    selected_kernel = boot / EXPECTED_FIRMWARE_KERNEL
    selected_dtb = boot / EXPECTED_FIRMWARE_DEVICE_TREE
    kernel_image = package_inventory["kernel_image"]
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-proof-") as temporary:
        extracted = Path(temporary) / "root"
        try:
            subprocess.run(["dpkg-deb", "-x", str(package), str(extracted)], check=True, capture_output=True, text=True)
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ImageProofError("cannot extract the exact package for image proof") from error
        package_kernel = extracted / kernel_image["package_path"]
        try:
            expected_kernel, compression = firmware_kernel_bytes(package_kernel.read_bytes(), str(package_kernel))
        except (OSError, KernelImageError) as error:
            raise ImageProofError(f"cannot read package firmware kernel: {package_kernel}") from error
        _require(compression == kernel_image["compression"], "package kernel compression provenance changed")
        _require(hashlib.sha256(expected_kernel).hexdigest() == kernel_image["firmware_sha256"], "package firmware kernel provenance changed")
        _require(selected_kernel.is_file(), "selected firmware kernel is missing")
        _require(selected_kernel.read_bytes() == expected_kernel, "selected firmware kernel differs from exact package Image")
        package_dtb = extracted / f"usr/lib/linux-image-{package_inventory['kernel_release']}/broadcom/bcm2710-rpi-zero-2-w.dtb"
        _require(selected_dtb.is_file() and package_dtb.is_file(), "selected or packaged DTB is missing")
        _require(selected_dtb.read_bytes() == package_dtb.read_bytes(), "selected DTB differs from exact package DTB")
        for entry in package_inventory["overlay_inventory"]:
            relative = Path(entry["path"])
            selected = boot / EXPECTED_FIRMWARE_OVERLAY_PREFIX / relative
            _hash_matches(selected, entry["sha256"], "selected packaged overlay")
        for entry in package_inventory["kernel_payload"]:
            if entry["path"].startswith("lib/modules/"):
                _hash_matches(root / entry["path"], entry["sha256"], "installed package module payload")
    _require((boot / EXPECTED_FIRMWARE_OVERLAY_PREFIX / "i2s-dac-no20.dtbo").is_file(), "i2s-dac overlay was not resolved under the custom prefix")
    return {"kernel": str(selected_kernel), "device_tree": str(selected_dtb)}


def prove_root(root: Path, package: Path, checksum: Path, provenance: Path, contract: Any) -> dict[str, Any]:
    root = root.resolve()
    _require(root.is_dir(), f"mounted image root does not exist: {root}")
    boot = _boot_dir(root)
    boot_layer_classification, boot_layer = _classify_boot_layer(root)
    if boot_layer is not None:
        _verify_managed_boot_outputs(root, boot_layer)
    validator = _load_validator()
    try:
        package_inventory = validator.validate_package(package, contract, checksum, provenance)
    except (validator.PackageValidationError, ContractError) as error:
        raise ImageProofError(f"exact package/provenance validation failed: {error}") from error
    _verify_selectors(boot / "config.txt")
    payload = _verify_payload(root, boot, package, package_inventory)
    _verify_selected_initramfs(boot, boot / EXPECTED_FIRMWARE_INITRAMFS)
    if boot_layer is not None:
        _verify_selected_initramfs_entries(boot / EXPECTED_FIRMWARE_INITRAMFS, boot_layer, root)
    stock = _verify_stock_recovery(root, boot)
    boot_layer_result: dict[str, str] = {"classification": boot_layer_classification}
    if boot_layer is not None:
        boot_layer_result["schema"] = str(boot_layer["schema"])
    return {
        "package": package_inventory["package"],
        "payload": payload,
        "stock_recovery": stock,
        "boot_layer": boot_layer_result,
    }


def _expected_partitions(loop: str) -> tuple[str, str]:
    try:
        result = subprocess.run(
            ["lsblk", "--json", "--paths", "--output", "NAME,TYPE", loop],
            capture_output=True,
            text=True,
            check=True,
        )
        devices = json.loads(result.stdout).get("blockdevices", [])
    except (FileNotFoundError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        raise ImageProofError("cannot inspect image partition labels and filesystems") from error

    partitions: list[dict[str, Any]] = []

    def visit(nodes: list[dict[str, Any]]) -> None:
        for node in nodes:
            if node.get("type") == "part":
                partitions.append(node)
            visit(node.get("children") or [])

    visit(devices)
    _require(len(partitions) == 2, "image must contain exactly two partitions")
    boot = [node for node in partitions if node.get("name") == f"{loop}p1"]
    root = [node for node in partitions if node.get("name") == f"{loop}p2"]
    _require(len(boot) == 1, "image must contain exactly one boot partition at p1")
    _require(len(root) == 1, "image must contain exactly one root partition at p2")
    _require(boot[0].get("name") != root[0].get("name"), "bootfs and rootfs partitions must be distinct")
    def device(node: dict[str, Any]) -> str:
        value = str(node.get("name") or node.get("path") or "")
        return value if value.startswith("/") else f"/dev/{value}"

    return device(boot[0]), device(root[0])


@contextlib.contextmanager
def _mounted_image(image: Path) -> Iterator[Path]:
    if image.is_dir():
        yield image
        return
    work = Path(tempfile.mkdtemp(prefix="octessera-rpi-image-proof-"))
    loop = ""
    mounts: list[Path] = []
    try:
        try:
            loop = subprocess.run(["losetup", "--find", "--show", "--read-only", "--partscan", str(image)], capture_output=True, text=True, check=True).stdout.strip()
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ImageProofError(f"cannot attach image {image} read-only") from error
        boot_device, root_device = _expected_partitions(loop)
        root_mount = work / "root"
        root_mount.mkdir()
        boot_mount = work / "boot"
        boot_mount.mkdir()
        subprocess.run(["mount", "-t", "vfat", "-o", "ro", boot_device, str(boot_mount)], check=True)
        mounts.append(boot_mount)
        subprocess.run(["mount", "-t", "ext4", "-o", "ro,noload", root_device, str(root_mount)], check=True)
        mounts.append(root_mount)
        firmware_mount = root_mount / "boot/firmware"
        firmware_mount.mkdir(parents=True, exist_ok=True)
        subprocess.run(["mount", "--bind", str(boot_mount), str(firmware_mount)], check=True)
        subprocess.run(["mount", "-o", "remount,bind,ro", str(firmware_mount)], check=True)
        mounts.append(firmware_mount)
        yield root_mount
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageProofError(f"cannot mount image read-only: {image}") from error
    finally:
        for mount in reversed(mounts):
            subprocess.run(["umount", "-l", str(mount)], capture_output=True, check=False)
        if loop:
            subprocess.run(["losetup", "-d", loop], capture_output=True, check=False)
        shutil.rmtree(work, ignore_errors=True)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Prove the selected Raspberry custom kernel in a mounted root or image.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--root", type=Path, help="mounted image root")
    source.add_argument("--image", type=Path, help="offline .img file")
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--checksum", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args(argv)
    repository = Path(__file__).resolve().parents[2]
    try:
        contract = load_contract(repository, args.manifest)
        with _mounted_image(args.root or args.image) as root:
            result = prove_root(root, args.package, args.checksum, args.provenance, contract)
        print(json.dumps(result, indent=2, sort_keys=True))
    except (ContractError, ImageProofError, OSError) as error:
        print(f"Raspberry kernel image proof failed: {error}", file=sys.stderr)
        return 1
    print("Raspberry custom kernel image proof passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
