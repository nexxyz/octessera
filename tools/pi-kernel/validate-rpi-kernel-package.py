#!/usr/bin/env python3
from __future__ import annotations

import argparse
import bz2
import gzip
import json
import lzma
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from rpi_kernel_contract import (
    EXPECTED_INTERFACE_STRINGS,
    Contract,
    ContractError,
    assert_final_config,
    load_contract,
    sha256_bytes,
    sha256_file,
)
from rpi_kernel_image import KernelImageError, assert_firmware_kernel

class PackageValidationError(ValueError):
    pass

def _run(command: list[str], *, text: bool = True) -> str | bytes:
    try:
        result = subprocess.run(command, check=True, capture_output=True, text=text)
    except FileNotFoundError as error:
        raise PackageValidationError(f"required command is unavailable: {command[0]}") from error
    except subprocess.CalledProcessError as error:
        output = error.stdout if text else (error.stdout or b"").decode(errors="replace")
        stderr = error.stderr if text else (error.stderr or b"").decode(errors="replace")
        detail = (output + (stderr or "")).strip()
        raise PackageValidationError(f"command failed: {' '.join(command)}\n{detail}") from error
    return result.stdout

def _control(package: Path) -> dict[str, str]:
    fields = {}
    for field in ("Package", "Version", "Architecture"):
        value = str(_run(["dpkg-deb", "-f", str(package), field])).strip()
        fields[field] = value
    return fields

def _require(condition: bool, message: str) -> None:
    if not condition:
        raise PackageValidationError(message)

def _payload_path(root: Path, value: str) -> Path:
    return root / value.rstrip("/")

def _payload_inventory(root: Path) -> list[dict[str, str]]:
    entries = []
    for path in sorted(root.rglob("*")):
        if path.is_file():
            relative = path.relative_to(root).as_posix()
            entries.append({"path": relative, "sha256": sha256_file(path)})
    return entries

def _module_name(path: Path) -> str | None:
    name = path.name
    for suffix in (".xz", ".zst", ".gz", ".lz4", ".bz2"):
        if name.endswith(suffix):
            name = name[: -len(suffix)]
            break
    return name[:-3] if name.endswith(".ko") else None

def _decompress(path: Path) -> bytes:
    suffix = path.suffix
    try:
        if suffix == ".xz":
            return lzma.decompress(path.read_bytes())
        if suffix == ".gz":
            return gzip.decompress(path.read_bytes())
        if suffix == ".bz2":
            return bz2.decompress(path.read_bytes())
        if suffix in (".zst", ".lz4"):
            command = "zstd" if suffix == ".zst" else "lz4"
            output = _run([command, "-d", "-c", str(path)], text=False)
            return output if isinstance(output, bytes) else output.encode()
    except (OSError, lzma.LZMAError, gzip.BadGzipFile, EOFError) as error:
        raise PackageValidationError(f"cannot decompress kernel module {path}: {error}") from error
    return path.read_bytes()

def _assert_aarch64_elf(data: bytes, module_name: str) -> None:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-module-") as temporary:
        path = Path(temporary) / f"{module_name}.ko"
        path.write_bytes(data)
        header = _run(["readelf", "-h", "-W", str(path)])
    if not isinstance(header, str):
        header = header.decode(errors="replace")
    _require(re.search(r"^\s*Class:\s+ELF64\s*$", header, re.MULTILINE) is not None, f"{module_name} is not ELF64")
    _require(
        re.search(r"^\s*Data:\s+2's complement, little endian\s*$", header, re.MULTILINE) is not None,
        f"{module_name} is not little-endian ELF",
    )
    _require(re.search(r"^\s*Machine:\s+AArch64\s*$", header, re.MULTILINE) is not None, f"{module_name} is not AArch64 ELF")

def _strings_entries(data: bytes, module_name: str) -> list[str]:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-module-strings-") as temporary:
        path = Path(temporary) / f"{module_name}.ko"
        path.write_bytes(data)
        output = _run(["strings", "-a", str(path)])
    if not isinstance(output, str):
        output = output.decode(errors="replace")
    return output.splitlines()

def _module_inventory(root: Path, contract: Contract) -> list[dict[str, Any]]:
    module_root = _payload_path(root, f"lib/modules/{contract.kernel_release}/")
    all_modules = [path for path in module_root.rglob("*") if path.is_file()]
    inventory = []
    for required in contract.required_modules:
        acceptable = {required, required.replace("_", "-")}
        matches = [path for path in all_modules if _module_name(path) in acceptable]
        _require(len(matches) == 1, f"expected exactly one module for {required}, found {len(matches)}")
        path = matches[0]
        decompressed = _decompress(path)
        _assert_aarch64_elf(decompressed, required)
        entry: dict[str, Any] = {
            "name": required,
            "path": path.relative_to(root).as_posix(),
            "sha256": sha256_file(path),
            "decompressed_sha256": sha256_bytes(decompressed),
        }
        if required == "usb_f_midi":
            try:
                vermagic_values = [
                    match.decode("ascii")
                    for match in re.findall(rb"(?<![^\x00])vermagic=([^\x00]+)", decompressed)
                ]
            except UnicodeDecodeError as error:
                raise PackageValidationError("usb_f_midi vermagic is not ASCII") from error
            _require(len(vermagic_values) == 1, "usb_f_midi must contain exactly one vermagic marker")
            vermagic = vermagic_values[0]
            _require(vermagic.split(" ", 1)[0] == contract.kernel_release, f"wrong usb_f_midi vermagic: {vermagic}")
            strings_entries = _strings_entries(decompressed, required)
            missing_strings = [marker for marker in EXPECTED_INTERFACE_STRINGS if marker not in strings_entries]
            _require(not missing_strings, f"usb_f_midi is missing required strings entries: {', '.join(missing_strings)}")
            entry["vermagic"] = vermagic
            entry["interface_string_markers"] = list(EXPECTED_INTERFACE_STRINGS)
        inventory.append(entry)
    return inventory


def _file_inventory(root: Path, pattern: str) -> list[dict[str, str]]:
    entries = []
    for path in sorted(root.glob(pattern)):
        if path.is_file():
            entries.append({"path": path.relative_to(root).as_posix(), "sha256": sha256_file(path)})
    return entries


def _dtb_and_overlay_inventory(root: Path, contract: Contract) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    image_root = _payload_path(root, f"usr/lib/linux-image-{contract.kernel_release}")
    dtbs = _file_inventory(image_root, "**/*.dtb")
    overlays = _file_inventory(image_root / "overlays", "**/*.dtbo")
    required_dtb = _payload_path(
        root,
        f"usr/lib/linux-image-{contract.kernel_release}/broadcom/bcm2710-rpi-zero-2-w.dtb",
    )
    _require(required_dtb in [image_root / entry["path"] for entry in dtbs], "required Raspberry Zero 2 W DTB is missing")
    _require(required_dtb.read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "required Raspberry DTB has an invalid header")
    _require(len(overlays) > 0, "Raspberry kernel package has no DT overlays")
    return dtbs, overlays


def _kernel_image_inventory(root: Path, contract: Contract) -> dict[str, str]:
    path = _payload_path(root, f"boot/vmlinuz-{contract.kernel_release}")
    try:
        image, compression = assert_firmware_kernel(path)
    except KernelImageError as error:
        raise PackageValidationError(str(error)) from error
    return {
        "package_path": path.relative_to(root).as_posix(),
        "package_sha256": sha256_file(path),
        "firmware_sha256": sha256_bytes(image),
        "compression": compression,
    }


def _verify_checksum(package: Path, checksum_file: Path) -> None:
    expected = []
    try:
        lines = checksum_file.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise PackageValidationError(f"cannot read checksum file {checksum_file}: {error}") from error
    for line in lines:
        value = line.strip()
        if not value:
            continue
        parts = value.split(maxsplit=1)
        _require(len(parts) == 2 and len(parts[0]) == 64, f"invalid checksum line: {line}")
        expected.append((parts[0], parts[1].lstrip(" *")))
    _require(len(expected) == 1 and expected[0][1] == package.name, "checksum file must contain this package exactly once")
    _require(expected[0][0] == sha256_file(package), "package SHA-256 does not match checksum file")


def validate_package(
    package: Path,
    contract: Contract,
    checksum_file: Path | None = None,
) -> dict[str, Any]:
    package = package.resolve()
    _require(package.is_file(), f"missing Raspberry linux-image package: {package}")
    _require(package.name == contract.package_filename, f"unexpected package filename: {package.name}")
    fields = _control(package)
    _require(fields["Package"] == contract.package_name, f"unexpected package name: {fields['Package']}")
    _require(fields["Version"] == contract.package_version, f"unexpected package version: {fields['Version']}")
    _require(fields["Architecture"] == contract.package_architecture, f"unexpected package architecture: {fields['Architecture']}")
    if checksum_file:
        _verify_checksum(package, checksum_file.resolve())

    with tempfile.TemporaryDirectory(prefix="octessera-rpi-package-") as temporary:
        extracted = Path(temporary) / "root"
        _run(["dpkg-deb", "-x", str(package), str(extracted)])
        for required in contract.required_payload:
            path = _payload_path(extracted, required)
            if required.endswith("/"):
                _require(path.is_dir(), f"missing required payload directory: {required}")
            else:
                _require(path.is_file(), f"missing required payload file: {required}")
        config_path = _payload_path(extracted, f"boot/config-{contract.kernel_release}")
        config = assert_final_config(config_path, contract)
        kernel_image = _kernel_image_inventory(extracted, contract)
        modules = _module_inventory(extracted, contract)
        dtbs, overlays = _dtb_and_overlay_inventory(extracted, contract)
        payload = _payload_inventory(extracted)

    inventory: dict[str, Any] = {
        "schema": 1,
        "kind": "octessera-raspberry-kernel-package",
        "manifest": {
            "path": contract.manifest_path.relative_to(contract.root).as_posix()
            if contract.manifest_path.is_relative_to(contract.root)
            else str(contract.manifest_path),
            "sha256": sha256_file(contract.manifest_path),
        },
        "source": {
            "repository": contract.source_repository,
            "commit": contract.source_commit,
            "release": contract.source_release,
            "config_base_path": contract.config_path,
            "config_base_sha256": contract.config_sha256,
        },
        "patches": [
            {"path": path.relative_to(contract.root).as_posix(), "sha256": sha256_file(path)}
            for path in contract.patch_paths
        ],
        "kernel_release": contract.kernel_release,
        "package": {
            "path": package.name,
            "name": fields["Package"],
            "version": fields["Version"],
            "architecture": fields["Architecture"],
            "sha256": sha256_file(package),
        },
        "config": {
            "path": f"boot/config-{contract.kernel_release}",
            **config,
        },
        "kernel_image": kernel_image,
        "kernel_payload": payload,
        "modules": modules,
        "dtb_inventory": dtbs,
        "overlay_inventory": overlays,
    }
    return inventory


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Validate the pinned Octessera Raspberry linux-image package.")
    parser.add_argument("package", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--checksum-file", type=Path)
    parser.add_argument("--inventory-out", type=Path)
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parents[2]
    try:
        contract = load_contract(root, args.manifest)
        inventory = validate_package(args.package, contract, args.checksum_file)
        if args.inventory_out:
            _write_json(args.inventory_out, inventory)
    except (ContractError, PackageValidationError) as error:
        print(f"Raspberry kernel package validation failed: {error}", file=sys.stderr)
        return 1
    print(f"Raspberry linux-image package validation passed: {args.package}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
