#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

KERNEL_TOOLS = Path(__file__).resolve().parents[1] / "pi-kernel"
REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
BOOT_LAYER_CONTRACT_PATH = REPOSITORY_ROOT / "resources/image-construction/boot-layers/raspberry-pi-zero-2w.json"
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(KERNEL_TOOLS))
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/legal"))

from rpi_kernel_contract import EXPECTED_FIRMWARE_INITRAMFS, ContractError, load_contract  # type: ignore[import-not-found]  # noqa: E402
import rpi_kernel_image_mount as _image_mount  # noqa: E402
from rpi_kernel_image_mount import ImageProofError, expected_partitions, mounted_image  # noqa: E402
from rpi_kernel_boot_proof import (  # noqa: E402
    boot_dir,
    classify_boot_layer,
    hash_matches,
    load_boot_layer_contract,
    load_installer,
    load_validator,
    resolve_regular_file,
    run_lsinitramfs,
    verify_initramfs,
    verify_managed_boot_outputs,
    verify_selected_initramfs,
    verify_selected_initramfs_entries,
    verify_selectors,
)
from rpi_kernel_payload_proof import verify_payload  # noqa: E402
from rpi_kernel_stock_recovery import verify_stock_recovery  # noqa: E402

subprocess = _image_mount.subprocess


def _expected_partitions(loop: str) -> tuple[str, str]:
    return expected_partitions(loop)


def _mounted_image(image: Path):
    return mounted_image(image)


def _load_validator() -> Any:
    return load_validator()


def _load_installer() -> Any:
    return load_installer()


def _boot_dir(root: Path) -> Path:
    return boot_dir(root)


def _hash_matches(path: Path, expected: str, label: str) -> None:
    hash_matches(path, expected, label)


def _verify_selectors(config: Path) -> None:
    verify_selectors(config)


def _run_lsinitramfs(path: Path) -> str:
    return run_lsinitramfs(path)


def _resolve_regular_file(root: Path, path: Path, label: str) -> Path:
    return resolve_regular_file(root, path, label)


def _verify_initramfs(path: Path) -> None:
    verify_initramfs(path, _run_lsinitramfs)


def _verify_selected_initramfs(boot: Path, path: Path) -> None:
    verify_selected_initramfs(boot, path, _run_lsinitramfs)


def _classify_boot_layer(root: Path, boot_layer_contract_path: Path = BOOT_LAYER_CONTRACT_PATH) -> tuple[str, dict[str, Any] | None]:
    return classify_boot_layer(root, boot_layer_contract_path)


def _load_boot_layer_contract(path: Path = BOOT_LAYER_CONTRACT_PATH) -> dict[str, Any]:
    return load_boot_layer_contract(path)


def _verify_managed_boot_outputs(root: Path, contract: dict[str, Any]) -> dict[str, Any] | None:
    return verify_managed_boot_outputs(root, contract)


def _verify_selected_initramfs_entries(path: Path, contract: dict[str, Any], root: Path) -> None:
    verify_selected_initramfs_entries(path, contract, root, _run_lsinitramfs)


def _verify_stock_recovery(root: Path, boot: Path) -> list[dict[str, str]]:
    return verify_stock_recovery(root, boot, _run_lsinitramfs)


def _verify_payload(root: Path, boot: Path, package: Path, package_inventory: dict[str, Any]) -> dict[str, Any]:
    return verify_payload(root, boot, package, package_inventory)


def prove_root(root: Path, package: Path, checksum: Path, provenance: Path, contract: Any, boot_layer_contract_path: Path = BOOT_LAYER_CONTRACT_PATH) -> dict[str, Any]:
    root = root.resolve()
    if not root.is_dir():
        raise ImageProofError(f"mounted image root does not exist: {root}")
    boot = _boot_dir(root)
    boot_layer_classification, boot_layer = _classify_boot_layer(root, boot_layer_contract_path)
    managed_outputs = _verify_managed_boot_outputs(root, boot_layer) if boot_layer is not None else None
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
    boot_layer_result: dict[str, Any] = {"classification": boot_layer_classification}
    if boot_layer is not None:
        if managed_outputs is None or "device_config_validator" not in managed_outputs:
            raise ImageProofError("Raspberry device config validator identity is missing")
        boot_layer_result["schema"] = str(boot_layer["schema"])
        boot_layer_result.update(managed_outputs)
    return {"package": package_inventory["package"], "payload": payload, "stock_recovery": stock, "boot_layer": boot_layer_result}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Prove the selected Raspberry custom kernel in a mounted root or image.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--root", type=Path, help="mounted image root")
    source.add_argument("--image", type=Path, help="offline .img file")
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--checksum", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--boot-layer-contract", type=Path, default=BOOT_LAYER_CONTRACT_PATH)
    args = parser.parse_args(argv)
    try:
        contract = load_contract(REPOSITORY_ROOT, args.manifest)
        with _mounted_image(args.root or args.image) as root:
            result = prove_root(root, args.package, args.checksum, args.provenance, contract, args.boot_layer_contract)
        print(json.dumps(result, indent=2, sort_keys=True))
    except (ContractError, ImageProofError, OSError) as error:
        print(f"Raspberry kernel image proof failed: {error}", file=sys.stderr)
        return 1
    print("Raspberry custom kernel image proof passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
