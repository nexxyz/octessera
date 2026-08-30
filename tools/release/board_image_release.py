from __future__ import annotations

import hashlib
import json
import re
import shutil
import stat
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import Iterable, Mapping, cast

ROOT = Path(__file__).resolve().parents[2]
RESPIN_TOOLS = ROOT / "tools" / "image-respin"
sys.path.insert(0, str(RESPIN_TOOLS))

RPI = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
KERNEL_MANIFEST = Path("tools/kernel-patches/orange-midi-interface-manifest.json")
CHECKSUM_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")


class ReleaseArtifactError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ReleaseArtifactError(message)


def _regular_file(path: Path, label: str) -> None:
    _require(path.exists() and not path.is_symlink(), f"{label} is missing or symlinked: {path}")
    _require(stat.S_ISREG(path.lstat().st_mode), f"{label} is not a regular file: {path}")


def _directory_entries(path: Path, label: str) -> list[Path]:
    _require(path.is_dir() and not path.is_symlink(), f"{label} is not a real directory: {path}")
    entries = sorted(path.iterdir(), key=lambda item: item.name)
    for entry in entries:
        metadata = entry.lstat()
        _require(not entry.is_symlink(), f"{label} contains a symlink: {entry}")
        _require(stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode), f"{label} contains a special file: {entry}")
    return entries


def _require_exact_files(directory: Path, expected: Iterable[str]) -> None:
    expected_names = sorted(expected)
    entries = _directory_entries(directory, "release artifact directory")
    actual_names = sorted(entry.name for entry in entries if entry.is_file())
    _require(len(entries) == len(actual_names), f"release artifact directory contains a non-file entry: {directory}")
    _require(actual_names == expected_names, f"Unexpected files under {directory}: {actual_names}")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _file_identity(path: Path) -> tuple[str, int]:
    _regular_file(path, "recorded file")
    payload = path.read_bytes()
    return hashlib.sha256(payload).hexdigest(), len(payload)


def _safe_name(name: str) -> PurePosixPath:
    relative = PurePosixPath(name)
    _require(bool(name) and not relative.is_absolute() and "\\" not in name and ".." not in relative.parts, f"unsafe checksum entry: {name}")
    return relative


def _checksum_entries(directory: Path, checksum_name: str) -> list[str]:
    checksum_path = directory / checksum_name
    _regular_file(checksum_path, "checksum file")
    entries: list[str] = []
    for line in checksum_path.read_text(encoding="utf-8").splitlines():
        match = CHECKSUM_LINE.fullmatch(line)
        _require(match is not None, f"malformed checksum line in {checksum_path}: {line}")
        assert match is not None
        digest, name = match.groups()
        _safe_name(name)
        _require(name not in entries, f"duplicate checksum entry: {name}")
        target = directory / Path(name)
        _regular_file(target, "checksum target")
        _require(_sha256(target) == digest, f"checksum mismatch: {target}")
        entries.append(name)
    return entries


def _verify_checksum_file(directory: Path, checksum_name: str) -> None:
    _checksum_entries(directory, checksum_name)


def _write_checksum(path: Path, name: str, digest: str) -> None:
    _require(not path.exists() and not path.is_symlink(), f"release asset collision: {path}")
    path.write_text(f"{digest}  {name}\n", encoding="utf-8")


def _copy_file(source: Path, destination: Path, label: str) -> None:
    _regular_file(source, label)
    _require(not destination.exists() and not destination.is_symlink(), f"release asset collision: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)


def _load_json(path: Path, label: str) -> dict[str, object]:
    _regular_file(path, label)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseArtifactError(f"{label} is not valid JSON: {path}") from error
    _require(isinstance(value, dict), f"{label} is not a JSON object: {path}")
    return cast(dict[str, object], value)


def _package_filename(value: object, label: str) -> str:
    _require(isinstance(value, str), f"{label} package declaration is malformed or unsafe: {value}")
    filename = cast(str, value)
    _require(filename.endswith(".deb") and PurePosixPath(filename).name == filename and "\\" not in filename, f"{label} package declaration is malformed or unsafe: {filename}")
    return filename


def _manifest_mapping(value: object, label: str) -> dict[str, object]:
    _require(isinstance(value, dict), f"{label} is missing or malformed")
    return cast(dict[str, object], value)


def _manifest_string(value: object, label: str) -> str:
    _require(isinstance(value, str) and bool(value), f"{label} is missing or malformed")
    return cast(str, value)


def _package_filenames(manifest: Mapping[str, object]) -> tuple[str, str, str]:
    kernels = _manifest_mapping(manifest.get("kernels"), "kernel manifest kernels declaration")
    raspberry = _manifest_mapping(kernels.get("raspberry"), "Raspberry kernel declaration")
    raspberry_package = _manifest_mapping(raspberry.get("package"), "Raspberry package declaration")
    raspberry_parts = [_manifest_string(raspberry_package.get(field), f"Raspberry package {field} declaration") for field in ("name", "version", "architecture")]
    rpi_package = _package_filename(f"{raspberry_parts[0]}_{raspberry_parts[1]}_{raspberry_parts[2]}.deb", "Raspberry")
    orange = _manifest_mapping(kernels.get("orange"), "Orange kernel declaration")
    packages = orange.get("packages")
    _require(isinstance(packages, list) and len(packages) == 2, "Orange package declaration must contain exactly two packages")
    orange_packages = tuple(_package_filename(package, "Orange") for package in cast(list[object], packages))
    _require(len(set(orange_packages)) == 2, "Orange package declaration contains duplicate packages")
    return rpi_package, orange_packages[0], orange_packages[1]


def _run(root: Path, command: list[str], label: str) -> None:
    try:
        completed = subprocess.run(command, cwd=root, check=False, text=True)
    except OSError as error:
        raise ReleaseArtifactError(f"{label} could not start: {error}") from error
    _require(completed.returncode == 0, f"{label} failed with exit code {completed.returncode}")


def _verify_raspberry_kernel(root: Path, kernel_dir: Path, package_name: str) -> None:
    _require_exact_files(kernel_dir, (package_name, "SHA256SUMS", "inventory.json", "provenance.json"))
    _verify_checksum_file(kernel_dir, "SHA256SUMS")
    _run(root, [sys.executable, "tools/pi-kernel/validate-rpi-kernel-package.py", str(kernel_dir / package_name), "--manifest", KERNEL_MANIFEST.as_posix(), "--checksum-file", str(kernel_dir / "SHA256SUMS"), "--provenance-in", str(kernel_dir / "provenance.json")], "Raspberry kernel package validation")
    inventory = _load_json(kernel_dir / "inventory.json", "Raspberry kernel inventory")
    provenance = _load_json(kernel_dir / "provenance.json", "Raspberry kernel provenance")
    _require({key: value for key, value in provenance.items() if key != "build"} == inventory, "Raspberry inventory and provenance chain differ")
    package = inventory.get("package")
    _require(isinstance(package, dict) and package.get("path") == package_name, "Raspberry kernel inventory package path changed")


def _verify_orange_provenance(root: Path, image_dir: Path, source_sha: str, image_package: str, dtb_package: str, manifest: Mapping[str, object]) -> None:
    evidence_path = image_dir / "octessera-orange-kernel-evidence.env"
    provenance_path = image_dir / "octessera-orange-kernel-provenance.txt"
    try:
        values = dict(line.split("=", 1) for line in evidence_path.read_text(encoding="utf-8").splitlines())
        facts = dict(line.split("=", 1) for line in provenance_path.read_text(encoding="utf-8").splitlines() if "=" in line)
    except (OSError, UnicodeDecodeError, ValueError) as error:
        raise ReleaseArtifactError("Orange provenance evidence is malformed") from error
    frameworks = _manifest_mapping(manifest.get("build_frameworks"), "kernel manifest build frameworks declaration")
    armbian = _manifest_mapping(frameworks.get("armbian"), "Armbian framework declaration")
    kernels = _manifest_mapping(manifest.get("kernels"), "kernel manifest kernels declaration")
    orange = _manifest_mapping(kernels.get("orange"), "Orange kernel declaration")
    source_lock = _manifest_mapping(manifest.get("source_lock"), "kernel manifest source lock declaration")
    expected_suffix = armbian["native_artifact_suffix"]
    _require(bool(isinstance(expected_suffix, str) and expected_suffix), "Armbian native artifact suffix is malformed")
    expected_native = tuple(f"{package.removesuffix('.deb')}__{expected_suffix}.deb" for package in (image_package, dtb_package))
    for key, filename in (("image_package_sha256", image_package), ("dtb_package_sha256", dtb_package)):
        _require(values.get(key) == _sha256(image_dir / filename), f"Orange provenance hash mismatch: {filename}")
    _require(facts.get("image_package") == image_package and facts.get("dtb_package") == dtb_package, "Orange provenance package chain is incomplete")
    expected_facts = {"evidence_sha256": _sha256(evidence_path), "armbian_build_ref": armbian["commit"], "armbian_build_tag": armbian["tag"], "armbian_build_repository": armbian["repository"], "github_source_sha": source_sha, "kernel_source_repository": orange["repository"], "kernel_source_branch": orange["branch"], "kernel_source_commit": orange["commit"], "kernel_release": armbian["kernel_release"], "package_revision": armbian["package_revision"], "revision_argument": armbian["revision_argument"], "source_lock_path": source_lock["path"], "source_lock_sha256": source_lock["sha256"], "source_lock_source": orange["repository"], "source_lock_branch": orange["branch"], "source_lock_commit": orange["commit"], "source_lock_effective_path": "config/sources/git_sources.json", "source_lock_effective_sha256": "e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30", "image_package_native": expected_native[0], "dtb_package_native": expected_native[1]}
    _require(all(facts.get(key) == expected for key, expected in expected_facts.items()), "Orange provenance evidence or build pin is not bound")
    for key, expected_native_name in (("image_package_native_basename", expected_native[0]), ("dtb_package_native_basename", expected_native[1])):
        _require(values.get(key) == expected_native_name, "Orange native package name is not manifest-approved")
    _require(values.get("artifact_suffix") == expected_suffix and facts.get("artifact_suffix") == expected_suffix, "Orange artifact suffix is not the manifest-approved suffix")


def _verify_orange_image(root: Path, image_dir: Path, version: str, image_package: str, dtb_package: str) -> None:
    image = image_dir / f"octessera-{version}-orange-pi-zero-2w.img.xz"
    _run(root, ["sudo", "bash", "tools/armbian-image/verify-orange-image.sh", "--image", str(image), "--linux-image", str(image_dir / image_package), "--linux-dtb", str(image_dir / dtb_package), "--evidence", str(image_dir / "octessera-orange-kernel-evidence.env"), "--provenance", str(image_dir / "octessera-orange-kernel-provenance.txt"), "--manifest", KERNEL_MANIFEST.as_posix(), "--boot-proof-mode", "phase5-constructor", "--construction-contract", "resources/image-construction/boot-layers/orange-pi-zero-2w.json", "--image-provenance", str(image_dir / "octessera-orange-image-proof.json"), "--mode", "production"], "Orange image validation")


def _base_refresh_images(root: Path, gathered_root: Path, release_assets: Path, evidence_staging: Path, version: str, source_sha: str) -> None:
    manifest = _load_json(root / KERNEL_MANIFEST, "kernel package manifest")
    rpi_kernel_package, orange_kernel_image, orange_kernel_dtb = _package_filenames(manifest)
    prefix = f"octessera-{version}"
    rpi_image_dir = gathered_root / "octessera-raspberry-image-release-assets"
    rpi_kernel_dir = gathered_root / "octessera-raspberry-kernel-release-assets"
    orange_image_dir = gathered_root / "octessera-orange-image-release-assets"
    rpi_manifest = f"{prefix}-raspberry-pi-zero-2w.rpi-imager-manifest"
    orange_image = f"{prefix}-orange-pi-zero-2w.img.xz"
    _require_exact_files(rpi_kernel_dir, (rpi_kernel_package, "SHA256SUMS", "inventory.json", "provenance.json"))
    _require_exact_files(rpi_image_dir, (f"{prefix}-raspberry-pi-zero-2w.img.zip", rpi_manifest, "SHA256SUMS-pi.txt"))
    _require_exact_files(orange_image_dir, (orange_image, f"{orange_image}.sha256", orange_kernel_image, orange_kernel_dtb, "octessera-orange-kernel-evidence.env", "octessera-orange-kernel-provenance.txt", "octessera-orange-image-proof.json", "SHA256SUMS-orange-pi-zero-2w.txt"))
    _verify_checksum_file(rpi_image_dir, "SHA256SUMS-pi.txt")
    _verify_raspberry_kernel(root, rpi_kernel_dir, rpi_kernel_package)
    _verify_checksum_file(orange_image_dir, f"{orange_image}.sha256")
    _verify_checksum_file(orange_image_dir, "SHA256SUMS-orange-pi-zero-2w.txt")
    _verify_orange_provenance(root, orange_image_dir, source_sha, orange_kernel_image, orange_kernel_dtb, manifest)
    _verify_orange_image(root, orange_image_dir, version, orange_kernel_image, orange_kernel_dtb)
    _copy_file(rpi_image_dir / f"{prefix}-raspberry-pi-zero-2w.img.zip", release_assets / f"{prefix}-raspberry-pi-zero-2w.img.zip", "release asset")
    _copy_file(rpi_image_dir / rpi_manifest, release_assets / rpi_manifest, "release asset")
    _copy_file(orange_image_dir / orange_image, release_assets / orange_image, "release asset")
    for source, destination in (
        (rpi_image_dir / "SHA256SUMS-pi.txt", evidence_staging / "raspberry/image/SHA256SUMS-pi.txt"),
        (rpi_image_dir / rpi_manifest, evidence_staging / f"raspberry/image/{rpi_manifest}"),
        (rpi_kernel_dir / rpi_kernel_package, evidence_staging / f"raspberry/kernel/{rpi_kernel_package}"),
        (rpi_kernel_dir / "SHA256SUMS", evidence_staging / "raspberry/kernel/SHA256SUMS"),
        (rpi_kernel_dir / "inventory.json", evidence_staging / "raspberry/kernel/inventory.json"),
        (rpi_kernel_dir / "provenance.json", evidence_staging / "raspberry/kernel/provenance.json"),
        (orange_image_dir / f"{orange_image}.sha256", evidence_staging / f"orange/image/{orange_image}.sha256"),
        (orange_image_dir / "SHA256SUMS-orange-pi-zero-2w.txt", evidence_staging / "orange/image/SHA256SUMS-orange-pi-zero-2w.txt"),
        (orange_image_dir / orange_kernel_image, evidence_staging / f"orange/kernel/{orange_kernel_image}"),
        (orange_image_dir / orange_kernel_dtb, evidence_staging / f"orange/kernel/{orange_kernel_dtb}"),
        (orange_image_dir / "octessera-orange-kernel-evidence.env", evidence_staging / "orange/kernel/octessera-orange-kernel-evidence.env"),
        (orange_image_dir / "octessera-orange-kernel-provenance.txt", evidence_staging / "orange/kernel/octessera-orange-kernel-provenance.txt"),
        (orange_image_dir / "octessera-orange-image-proof.json", evidence_staging / "orange/image/octessera-orange-image-proof.json"),
    ):
        _copy_file(source, destination, "release evidence")


def verify_and_stage_board_images(root: Path, gathered_root: Path, release_assets: Path, evidence_staging: Path, raspberry_runtime: Path, orange_runtime: Path, version: str, source_sha: str) -> None:
    try:
        _require(re.fullmatch(r"[0-9a-f]{40}", source_sha) is not None, "source SHA is invalid")
        _base_refresh_images(root, gathered_root, release_assets, evidence_staging, version, source_sha)
    except ReleaseArtifactError:
        raise
    except (KeyError, OSError, TypeError, ValueError) as error:
        raise ReleaseArtifactError(str(error)) from error


__all__ = ["ReleaseArtifactError", "verify_and_stage_board_images"]
