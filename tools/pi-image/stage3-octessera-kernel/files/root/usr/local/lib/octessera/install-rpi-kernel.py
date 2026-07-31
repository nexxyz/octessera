#!/usr/bin/env python3
from __future__ import annotations

import argparse
import bz2
import gzip
import hashlib
import json
import lzma
import re
import shutil
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator

KERNEL_TOOLS = Path(__file__).resolve().parents[1] / "pi-kernel"
if not (KERNEL_TOOLS / "rpi_kernel_contract.py").is_file():
    KERNEL_TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(KERNEL_TOOLS))

from rpi_kernel_contract import (  # type: ignore[import-not-found]  # noqa: E402
    EXPECTED_FIRMWARE_DEVICE_TREE,
    EXPECTED_FIRMWARE_INITRAMFS,
    EXPECTED_FIRMWARE_KERNEL,
    EXPECTED_FIRMWARE_OVERLAY_PREFIX,
    HEX64,
    Contract,
    ContractError,
    assert_final_config,
    image_contract,
    load_contract,
    sha256_bytes,
    sha256_file,
)
from rpi_kernel_image import KernelImageError, assert_firmware_kernel  # type: ignore[import-not-found]  # noqa: E402


class ImageInstallError(ValueError):
    pass


def _run(command: list[str]) -> str:
    try:
        result = subprocess.run(command, capture_output=True, text=True, check=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        detail = ""
        if isinstance(error, subprocess.CalledProcessError):
            detail = ((error.stdout or "") + (error.stderr or "")).strip()
        raise ImageInstallError(f"command failed: {' '.join(command)}\n{detail}") from error
    return result.stdout


def _run_bytes(command: list[str], data: bytes) -> bytes:
    try:
        return subprocess.run(command, input=data, capture_output=True, check=True).stdout
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageInstallError(f"command failed: {' '.join(command)}") from error


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ImageInstallError(message)


def _control(package: Path) -> dict[str, str]:
    return {field: _run(["dpkg-deb", "-f", str(package), field]).strip() for field in ("Package", "Version", "Architecture")}


def _verify_checksum(package: Path, checksum_file: Path) -> None:
    try:
        lines = [line.split() for line in checksum_file.read_text(encoding="utf-8").splitlines() if line.strip()]
    except OSError as error:
        raise ImageInstallError(f"cannot read checksum file {checksum_file}: {error}") from error
    _require(len(lines) == 1 and len(lines[0]) == 2, "SHA256SUMS must contain exactly one package entry")
    expected, name = lines[0]
    _require(HEX64.fullmatch(expected) is not None, "SHA256SUMS contains an invalid SHA-256")
    _require(name.removeprefix("*") == package.name, "SHA256SUMS names a different package")
    _require(expected == sha256_file(package), "package SHA-256 does not match SHA256SUMS")


def _module_name(path: Path) -> str | None:
    value = path.name
    for suffix in (".xz", ".zst", ".gz", ".lz4", ".bz2"):
        if value.endswith(suffix):
            value = value[: -len(suffix)]
            break
    return value[:-3] if value.endswith(".ko") else None


def _decompress_module(path: Path) -> bytes:
    data = path.read_bytes()
    if path.name.endswith(".xz"):
        return lzma.decompress(data)
    if path.name.endswith(".gz"):
        return gzip.decompress(data)
    if path.name.endswith(".bz2"):
        return bz2.decompress(data)
    if path.name.endswith(".zst"):
        return _run_bytes(["zstd", "-d", "-c"], data)
    if path.name.endswith(".lz4"):
        return _run_bytes(["lz4", "-d", "-c"], data)
    return data


def _payload_inventory(root: Path) -> list[dict[str, str]]:
    return [{"path": path.relative_to(root).as_posix(), "sha256": sha256_file(path)} for path in sorted(root.rglob("*")) if path.is_file()]


def _file_inventory(root: Path, pattern: str) -> list[dict[str, str]]:
    return [{"path": path.relative_to(root).as_posix(), "sha256": sha256_file(path)} for path in sorted(root.glob(pattern)) if path.is_file()]


def _payload_checks(extracted: Path, contract: Contract) -> tuple[Path, Path, list[Path]]:
    for required in contract.required_payload:
        path = extracted / required.rstrip("/")
        _require(path.is_dir() if required.endswith("/") else path.is_file(), f"missing package payload: {required}")
    kernel = extracted / f"boot/vmlinuz-{contract.kernel_release}"
    try:
        assert_firmware_kernel(kernel)
    except KernelImageError as error:
        raise ImageInstallError(str(error)) from error
    dtb = extracted / f"usr/lib/linux-image-{contract.kernel_release}/broadcom/bcm2710-rpi-zero-2-w.dtb"
    _require(dtb.read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "required Raspberry DTB has an invalid header")
    overlay_root = extracted / f"usr/lib/linux-image-{contract.kernel_release}/overlays"
    overlays = sorted(path for path in overlay_root.rglob("*.dtbo") if path.is_file())
    _require(bool(overlays), "Raspberry kernel package has no DT overlays")
    module_root = extracted / f"lib/modules/{contract.kernel_release}"
    modules = [path for path in module_root.rglob("*") if path.is_file()]
    for required in contract.required_modules:
        matches = [path for path in modules if _module_name(path) in {required, required.replace("_", "-")}]
        _require(len(matches) == 1, f"expected exactly one package module for {required}")
    return kernel, dtb, overlays


@contextmanager
def _extracted(package: Path) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-image-package-") as temporary:
        extracted = Path(temporary) / "root"
        _run(["dpkg-deb", "-x", str(package), str(extracted)])
        yield extracted


def _verify_provenance(package: Path, checksum_file: Path, provenance_path: Path, contract: Contract) -> None:
    try:
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ImageInstallError(f"cannot read kernel provenance: {provenance_path}") from error
    required = {"schema", "kind", "manifest", "source", "patches", "kernel_release", "package", "config", "kernel_image", "kernel_payload", "modules", "dtb_inventory", "overlay_inventory", "build"}
    _require(isinstance(provenance, dict) and required.issubset(provenance), "kernel provenance fields are incomplete")
    _require(provenance["schema"] == 1 and provenance["kind"] == "octessera-raspberry-kernel-package", "kernel provenance identity is invalid")
    _require(provenance["kernel_release"] == contract.kernel_release, "kernel provenance release mismatch")
    _require(isinstance(provenance["source"], dict) and isinstance(provenance["manifest"], dict) and isinstance(provenance["patches"], list), "kernel provenance source blocks are invalid")
    _require(isinstance(provenance["build"], dict), "kernel provenance build block is invalid")
    _require(provenance["source"].get("commit") == contract.source_commit, "kernel provenance source mismatch")
    _require(provenance["patches"], "kernel provenance patch inventory is empty")
    fields = _control(package)
    package_record = {
        "path": package.name,
        "name": fields["Package"],
        "version": fields["Version"],
        "architecture": fields["Architecture"],
        "sha256": sha256_file(package),
    }
    _require(provenance["package"] == package_record, "kernel provenance package identity or hash mismatch")
    with _extracted(package) as extracted:
        kernel, dtb, overlays = _payload_checks(extracted, contract)
        config_path = extracted / f"boot/config-{contract.kernel_release}"
        config = assert_final_config(config_path, contract)
        image, compression = assert_firmware_kernel(kernel)
        expected_kernel_image = {
            "package_path": f"boot/vmlinuz-{contract.kernel_release}",
            "package_sha256": sha256_file(kernel),
            "firmware_sha256": sha256_bytes(image),
            "compression": compression,
        }
        _require(provenance["kernel_image"] == expected_kernel_image, "kernel provenance firmware image mismatch")
        _require(provenance["config"] == {"path": f"boot/config-{contract.kernel_release}", **config}, "kernel provenance config mismatch")
        _require(provenance["kernel_payload"] == _payload_inventory(extracted), "kernel provenance payload inventory mismatch")
        image_root = extracted / f"usr/lib/linux-image-{contract.kernel_release}"
        _require(provenance["dtb_inventory"] == _file_inventory(image_root, "**/*.dtb"), "kernel provenance DTB inventory mismatch")
        _require(provenance["overlay_inventory"] == _file_inventory(image_root / "overlays", "**/*.dtbo"), "kernel provenance overlay inventory mismatch")
        module_entries = provenance["modules"]
        _require(isinstance(module_entries, list), "kernel provenance module inventory is invalid")
        by_name: dict[str, dict[str, Any]] = {}
        for entry in module_entries:
            _require(isinstance(entry, dict) and isinstance(entry.get("name"), str), "kernel provenance module entry is invalid")
            _require(entry["name"] not in by_name, "kernel provenance contains duplicate modules")
            path = extracted / entry["path"]
            _require(path.is_file(), f"kernel provenance module is missing: {entry['path']}")
            _require(entry["sha256"] == sha256_file(path), f"kernel provenance module hash mismatch: {entry['name']}")
            _require(entry["decompressed_sha256"] == sha256_bytes(_decompress_module(path)), f"kernel provenance decompressed module hash mismatch: {entry['name']}")
            by_name[entry["name"]] = entry
        _require(set(by_name) == set(contract.required_modules), "kernel provenance required module set mismatch")


def verify_package_inputs(package: Path, checksum_file: Path, provenance_path: Path, contract: Contract) -> dict[str, str]:
    package = package.resolve()
    checksum_file = checksum_file.resolve()
    provenance_path = provenance_path.resolve()
    _require(package.is_file(), f"missing kernel package: {package}")
    _require(package.name == contract.package_filename, f"unexpected package filename: {package.name}")
    fields = _control(package)
    _require(fields["Package"] == contract.package_name, f"unexpected package name: {fields['Package']}")
    _require(fields["Version"] == contract.package_version, f"unexpected package version: {fields['Version']}")
    _require(fields["Architecture"] == contract.package_architecture, f"unexpected package architecture: {fields['Architecture']}")
    _verify_checksum(package, checksum_file)
    _verify_provenance(package, checksum_file, provenance_path, contract)
    return {**fields, "filename": package.name, "sha256": sha256_file(package)}


def _boot_dir(rootfs: Path) -> Path:
    firmware = rootfs / "boot/firmware"
    return firmware if firmware.is_dir() else rootfs / "boot"


def _stock_files(rootfs: Path) -> list[Path]:
    bases = [rootfs / "boot"]
    if (rootfs / "boot/firmware").is_dir():
        bases.append(rootfs / "boot/firmware")
    pattern = re.compile(r"^(?:vmlinuz-|kernel.*\.img$|initrd|Image$|System\.map-|config-)")
    return sorted({path for base in bases for path in base.iterdir() if path.is_file() and pattern.match(path.name)})


def _preserve_stock(rootfs: Path, boot: Path) -> list[dict[str, str]]:
    recovery = boot / "octessera/recovery-stock"
    manifest_path = recovery / "manifest.json"
    if manifest_path.is_file():
        return json.loads(manifest_path.read_text(encoding="utf-8"))
    stock = _stock_files(rootfs)
    _require(bool(stock), "stock recovery kernel files are missing")
    recovery.mkdir(parents=True, exist_ok=True)
    entries = []
    for path in stock:
        relative = path.relative_to(rootfs).as_posix()
        target = recovery / relative.replace("/", "_")
        shutil.copy2(path, target, follow_symlinks=True)
        entries.append({"path": relative, "recovery_path": target.relative_to(rootfs).as_posix(), "sha256": sha256_file(path)})
    manifest_path.write_text(json.dumps(entries, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return entries


def _copy_payload(rootfs: Path, extracted: Path, contract: Contract) -> dict[str, Any]:
    boot = _boot_dir(rootfs)
    custom = boot / "octessera"
    overlay_target = custom / "overlays"
    overlay_target.mkdir(parents=True, exist_ok=True)
    kernel, dtb, overlays = _payload_checks(extracted, contract)
    kernel_bytes, compression = assert_firmware_kernel(kernel)
    (custom / "kernel8.img").write_bytes(kernel_bytes)
    shutil.copyfile(dtb, custom / "bcm2710-rpi-zero-2-w.dtb")
    overlay_hashes = []
    source_root = extracted / f"usr/lib/linux-image-{contract.kernel_release}/overlays"
    for source in overlays:
        relative = source.relative_to(source_root)
        target = overlay_target / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        overlay_hashes.append({"path": (Path(EXPECTED_FIRMWARE_OVERLAY_PREFIX) / relative).as_posix(), "sha256": sha256_file(source)})
    return {"kernel": {"path": EXPECTED_FIRMWARE_KERNEL, "sha256": sha256_bytes(kernel_bytes), "compression": compression}, "device_tree": {"path": EXPECTED_FIRMWARE_DEVICE_TREE, "sha256": sha256_file(dtb)}, "overlays": overlay_hashes}


def _run_in_root(rootfs: Path, command: list[str]) -> None:
    _run(command if rootfs == Path("/") else ["chroot", str(rootfs), *command])


def _install_package(rootfs: Path, package: Path) -> None:
    staged = rootfs / "var/lib/octessera/rpi-kernel" / package.name
    staged.parent.mkdir(parents=True, exist_ok=True)
    if staged.resolve() != package.resolve():
        shutil.copy2(package, staged)
    _run_in_root(rootfs, ["dpkg", "-i", f"/var/lib/octessera/rpi-kernel/{package.name}"])


def _write_selectors(config: Path, contract: Contract) -> dict[str, str]:
    _require(config.is_file(), f"missing Raspberry firmware config: {config}")
    expected = {"kernel": f"kernel={EXPECTED_FIRMWARE_KERNEL}", "device_tree": f"device_tree={EXPECTED_FIRMWARE_DEVICE_TREE}", "initramfs": f"initramfs {EXPECTED_FIRMWARE_INITRAMFS} followkernel", "overlay_prefix": f"overlay_prefix={EXPECTED_FIRMWARE_OVERLAY_PREFIX}"}
    kept = []
    for line in config.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        key = stripped.split("=", 1)[0].strip() if "=" in stripped else stripped.split(None, 1)[0] if stripped else ""
        if key in {"kernel", "device_tree", "overlay_prefix", "initramfs", "auto_initramfs"}:
            continue
        kept.append(line)
    overlay_index = next((index for index, line in enumerate(kept) if line.strip().startswith("dtoverlay=")), len(kept))
    kept.insert(overlay_index, expected["overlay_prefix"])
    kept.extend(["", "# --- octessera custom kernel selectors ---", expected["kernel"], expected["device_tree"], expected["initramfs"]])
    config.write_text("\n".join(kept) + "\n", encoding="utf-8")
    return expected


def verify_selectors(config: Path, contract: Contract) -> dict[str, str]:
    lines = config.read_text(encoding="utf-8").splitlines()
    expected = {"kernel": f"kernel={EXPECTED_FIRMWARE_KERNEL}", "device_tree": f"device_tree={EXPECTED_FIRMWARE_DEVICE_TREE}", "initramfs": f"initramfs {EXPECTED_FIRMWARE_INITRAMFS} followkernel", "overlay_prefix": f"overlay_prefix={EXPECTED_FIRMWARE_OVERLAY_PREFIX}"}
    found = {key: [] for key in (*expected, "auto_initramfs")}
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("kernel="):
            found["kernel"].append(stripped)
        elif stripped.startswith("device_tree="):
            found["device_tree"].append(stripped)
        elif stripped.startswith("overlay_prefix="):
            found["overlay_prefix"].append(stripped)
        elif stripped.startswith("initramfs ") or stripped.startswith("initramfs="):
            found["initramfs"].append(stripped)
        elif stripped.startswith("auto_initramfs="):
            found["auto_initramfs"].append(stripped)
    _require(not found["auto_initramfs"], "auto_initramfs is ambiguous with explicit initramfs selection")
    for key, value in expected.items():
        _require(found[key] == [value], f"duplicate or conflicting {key} selectors")
    return expected


def _finalize(rootfs: Path, package: Path, checksum_file: Path, provenance: Path, contract: Contract) -> dict[str, Any]:
    package_facts = verify_package_inputs(package, checksum_file, provenance, contract)
    boot = _boot_dir(rootfs)
    with _extracted(package) as extracted:
        payload = _copy_payload(rootfs, extracted, contract)
    _run_in_root(rootfs, ["depmod", "-a", contract.kernel_release])
    initramfs_source = rootfs / f"boot/initrd.img-{contract.kernel_release}"
    if initramfs_source.exists():
        initramfs_source.unlink()
    _run_in_root(rootfs, ["update-initramfs", "-c", "-k", contract.kernel_release])
    _require(initramfs_source.is_file(), f"update-initramfs did not create {initramfs_source}")
    initramfs_target = boot / EXPECTED_FIRMWARE_INITRAMFS
    initramfs_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(initramfs_source, initramfs_target)
    selectors = _write_selectors(boot / "config.txt", contract)
    verify_selectors(boot / "config.txt", contract)
    evidence = {"schema": 1, "kind": "octessera-raspberry-kernel-image", "package": package_facts, "selectors": selectors, "payload": payload, "initramfs": {"path": EXPECTED_FIRMWARE_INITRAMFS, "sha256": sha256_file(initramfs_target)}, "stock_recovery": _preserve_stock(rootfs, boot)}
    (boot / "octessera/kernel-image-install.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return evidence


def install_image(rootfs: Path, package: Path, checksum_file: Path, provenance: Path, contract: Contract, *, finalize: bool) -> dict[str, Any] | None:
    package_facts = verify_package_inputs(package, checksum_file, provenance, contract)
    _preserve_stock(rootfs, _boot_dir(rootfs))
    _install_package(rootfs, package)
    with _extracted(package) as extracted:
        _copy_payload(rootfs, extracted, contract)
    return _finalize(rootfs, package, checksum_file, provenance, contract) if finalize else package_facts


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Install and select the pinned Octessera Raspberry kernel image.")
    parser.add_argument("--rootfs", type=Path, default=Path("/"))
    parser.add_argument("--package", type=Path)
    parser.add_argument("--checksums", type=Path)
    parser.add_argument("--provenance", type=Path)
    parser.add_argument("--finalize", action="store_true")
    args = parser.parse_args(argv)
    try:
        contract = load_contract(Path(__file__).resolve().parents[2])
    except ContractError:
        contract = image_contract()
    try:
        rootfs = args.rootfs.resolve()
        artifact_dir = rootfs / "var/lib/octessera/rpi-kernel"
        package = (args.package or artifact_dir / contract.package_filename).resolve()
        checksums = (args.checksums or artifact_dir / "SHA256SUMS").resolve()
        provenance = (args.provenance or artifact_dir / "provenance.json").resolve()
        if not args.finalize and args.provenance is None:
            raise ImageInstallError("kernel provenance is required before package installation")
        result = _finalize(rootfs, package, checksums, provenance, contract) if args.finalize else install_image(rootfs, package, checksums, provenance, contract, finalize=False)
        if result:
            print(json.dumps(result, indent=2, sort_keys=True))
    except (ImageInstallError, OSError, ValueError) as error:
        print(f"Raspberry kernel image installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
