#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Iterator

KERNEL_TOOLS = Path(__file__).resolve().parents[1] / "pi-kernel"
sys.path.insert(0, str(KERNEL_TOOLS))

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


class ImageProofError(ValueError):
    pass


BOOT_PARTITION_LABEL = "bootfs"
ROOT_PARTITION_LABEL = "rootfs"


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


def _verify_initramfs(path: Path, release: str, modules: tuple[str, ...]) -> None:
    _require(path.is_file() and path.stat().st_size > 0, f"missing or empty selected initramfs: {path}")
    listing = _run_lsinitramfs(path)
    _require(release in listing, "selected initramfs does not contain the exact kernel release")
    for module in modules:
        _require(module in listing or module.replace("_", "-") in listing, f"selected initramfs omits module {module}")


def _verify_stock_recovery(root: Path, boot: Path) -> list[dict[str, str]]:
    manifest = boot / "octessera/recovery-stock/manifest.json"
    _require(manifest.is_file(), "stock recovery manifest is missing")
    try:
        entries = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ImageProofError(f"invalid stock recovery manifest: {manifest}") from error
    _require(isinstance(entries, list) and bool(entries), "stock recovery manifest is empty")
    for entry in entries:
        _require(isinstance(entry, dict) and set(entry) == {"path", "recovery_path", "sha256"}, "stock recovery manifest entry changed")
        _hash_matches(root / entry["recovery_path"], entry["sha256"], "stock recovery file")
        _hash_matches(root / entry["path"], entry["sha256"], "retained stock file")
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
    validator = _load_validator()
    try:
        package_inventory = validator.validate_package(package, contract, checksum, provenance)
    except (validator.PackageValidationError, ContractError) as error:
        raise ImageProofError(f"exact package/provenance validation failed: {error}") from error
    boot = _boot_dir(root)
    _verify_selectors(boot / "config.txt")
    payload = _verify_payload(root, boot, package, package_inventory)
    _verify_initramfs(boot / EXPECTED_FIRMWARE_INITRAMFS, contract.kernel_release, contract.required_modules)
    stock = _verify_stock_recovery(root, boot)
    return {"package": package_inventory["package"], "payload": payload, "stock_recovery": stock}


def _expected_partitions(loop: str) -> tuple[str, str]:
    try:
        result = subprocess.run(
            ["lsblk", "--json", "--paths", "--output", "NAME,TYPE,FSTYPE,LABEL,PARTLABEL", loop],
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
    boot = [node for node in partitions if node.get("label") == BOOT_PARTITION_LABEL and str(node.get("fstype", "")).lower() in {"vfat", "fat16", "fat32"}]
    root = [node for node in partitions if node.get("label") == ROOT_PARTITION_LABEL and str(node.get("fstype", "")).lower() == "ext4"]
    _require(len(boot) == 1, "image must contain exactly one bootfs vfat partition")
    _require(len(root) == 1, "image must contain exactly one rootfs ext4 partition")
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
        subprocess.run(["mount", "-o", "ro", boot_device, str(boot_mount)], check=True)
        mounts.append(boot_mount)
        subprocess.run(["mount", "-o", "ro,noload", root_device, str(root_mount)], check=True)
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
