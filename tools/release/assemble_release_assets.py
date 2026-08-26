from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import stat
import subprocess
import sys
import zipfile
from pathlib import Path, PurePosixPath
from typing import Iterable, Mapping, cast

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tools" / "legal"))
sys.path.insert(0, str(ROOT / "tools" / "device-update"))

from package_notice_zip import package_notice_zip  # type: ignore[import-not-found]
from verify_notice_archive import verify_notice_archive  # type: ignore[import-not-found]
from updater_profiles import ORANGE_PROFILE, RASPBERRY_PROFILE, updater_asset_names  # type: ignore[import-not-found]

CHECKSUM_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")
KERNEL_MANIFEST = Path("tools/kernel-patches/orange-midi-interface-manifest.json")
RUNTIME_FILES = ("SHA256SUMS", "octessera-pi", "octessera-runtime.json")

class ReleaseArtifactError(ValueError):
    pass

def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ReleaseArtifactError(message)


def _regular_file(path: Path, label: str) -> None:
    _require(path.exists() and not path.is_symlink(), f"{label} is missing or symlinked: {path}")
    metadata = path.lstat()
    _require(stat.S_ISREG(metadata.st_mode), f"{label} is not a regular file: {path}")

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

def _safe_name(name: str) -> PurePosixPath:
    relative = PurePosixPath(name)
    _require(
        bool(name)
        and not relative.is_absolute()
        and "\\" not in name
        and ".." not in relative.parts,
        f"unsafe checksum entry: {name}",
    )
    return relative


def _verify_checksum_file(directory: Path, checksum_name: str) -> None:
    checksum_path = directory / checksum_name
    _regular_file(checksum_path, "checksum file")
    lines = checksum_path.read_text(encoding="utf-8").splitlines()
    seen: set[str] = set()
    for line in lines:
        match = CHECKSUM_LINE.fullmatch(line)
        if match is None:
            raise ReleaseArtifactError(f"malformed checksum line in {checksum_path}: {line}")
        digest, name = match.groups()
        relative = _safe_name(name)
        _require(name not in seen, f"duplicate checksum entry: {name}")
        seen.add(name)
        target = directory.joinpath(*relative.parts)
        _regular_file(target, "checksum target")
        _require(_sha256(target) == digest, f"checksum mismatch: {target}")

def _write_checksums(directory: Path, checksum_name: str, names: Iterable[str]) -> None:
    sorted_names = sorted(names)
    (directory / checksum_name).write_text(
        "".join(f"{_sha256(directory / name)}  {name}\n" for name in sorted_names),
        encoding="utf-8",
    )
    _verify_checksum_file(directory, checksum_name)

def _copy_file(source: Path, destination: Path, label: str) -> None:
    _regular_file(source, label)
    _require(not destination.exists() and not destination.is_symlink(), f"release asset collision: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)

def _load_json(path: Path, label: str) -> dict[str, object]:
    _regular_file(path, label)
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseArtifactError(f"{label} is not valid JSON: {path}") from error
    _require(isinstance(document, dict), f"{label} is not a JSON object: {path}")
    return document

def _manifest_mapping(value: object, label: str) -> dict[str, object]:
    _require(isinstance(value, dict), f"{label} is missing or malformed")
    return cast(dict[str, object], value)

def _manifest_string(value: object, label: str) -> str:
    _require(isinstance(value, str) and bool(value), f"{label} is missing or malformed")
    return cast(str, value)

def _package_filename(value: object, label: str) -> str:
    filename = _manifest_string(value, f"{label} package declaration")
    _require(filename.endswith(".deb") and PurePosixPath(filename).name == filename and "\\" not in filename, f"{label} package declaration is malformed or unsafe: {filename}")
    return filename


def _package_filenames(manifest: Mapping[str, object]) -> tuple[str, str, str]:
    kernels = _manifest_mapping(manifest.get("kernels"), "kernel manifest kernels declaration")
    raspberry = _manifest_mapping(kernels.get("raspberry"), "Raspberry kernel declaration")
    raspberry_package = _manifest_mapping(raspberry.get("package"), "Raspberry package declaration")
    raspberry_parts = [_manifest_string(raspberry_package.get(field), f"Raspberry package {field} declaration") for field in ("name", "version", "architecture")]
    raspberry_package_name = _package_filename(f"{raspberry_parts[0]}_{raspberry_parts[1]}_{raspberry_parts[2]}.deb", "Raspberry")

    orange = _manifest_mapping(kernels.get("orange"), "Orange kernel declaration")
    orange_packages = orange.get("packages")
    _require(isinstance(orange_packages, list), "Orange package declaration is missing or malformed")
    _require(len(cast(list[object], orange_packages)) == 2, "Orange package declaration must contain exactly two packages")
    orange_package_names = tuple(_package_filename(package, "Orange") for package in cast(list[object], orange_packages))
    _require(len(set(orange_package_names)) == len(orange_package_names), "Orange package declaration contains duplicate packages")
    return raspberry_package_name, orange_package_names[0], orange_package_names[1]


def _run(root: Path, command: list[str], label: str) -> None:
    try:
        completed = subprocess.run(command, cwd=root, check=False, text=True)
    except OSError as error:
        raise ReleaseArtifactError(f"{label} could not start: {error}") from error
    if completed.returncode != 0:
        raise ReleaseArtifactError(f"{label} failed with exit code {completed.returncode}")


def _verify_runtime_bundle(bundle: Path, version: str, profile: str) -> None:
    _require_exact_files(bundle, RUNTIME_FILES)
    _verify_checksum_file(bundle, "SHA256SUMS")
    metadata = _load_json(bundle / "octessera-runtime.json", "runtime metadata")
    expected = {
        "artifact_kind": "production-runtime",
        "binary_sha256": _sha256(bundle / "octessera-pi"),
        "name": "octessera-pi",
        "profile": profile,
        "runtime_ready": True,
        "version": version,
    }
    _require(metadata == expected, f"{profile} runtime metadata is not exact")


def _zip_entries(archive: zipfile.ZipFile, label: str) -> list[zipfile.ZipInfo]:
    entries = archive.infolist()
    names: set[str] = set()
    for info in entries:
        name = info.filename
        relative = _safe_name(name)
        _require(not name.endswith("/"), f"{label} contains a directory entry: {name}")
        _require(name not in names, f"{label} contains a duplicate entry: {name}")
        names.add(name)
        file_type = (info.external_attr >> 16) & 0o170000
        _require(file_type == stat.S_IFREG, f"{label} contains a non-regular entry: {name}")
        _require(relative.as_posix() == name, f"{label} contains an unsafe entry: {name}")
    return entries


def _verify_device_zip(root: Path, bundle: Path, zip_path: Path, version: str, profile: str, updater: bool = False) -> None:
    expected_names = ["octessera-pi", "octessera-device-release.json", "LICENSE", "NOTICE"]
    if profile == "orange-pi-zero-2w" and not updater:
        expected_names = ["octessera-pi", "octessera-runtime.json", "SHA256SUMS", *expected_names[1:]]
    _regular_file(zip_path, "device archive")
    try:
        with zipfile.ZipFile(zip_path) as archive:
            entries = _zip_entries(archive, f"{profile} device archive")
            _require([entry.filename for entry in entries] == expected_names, f"{profile} device ZIP inventory is not exact")
            for entry in entries:
                mode = (entry.external_attr >> 16) & 0o777
                expected_mode = 0o755 if entry.filename == "octessera-pi" else 0o644
                _require(mode == expected_mode, f"{profile} device ZIP mode is not exact: {entry.filename}")
            _require(archive.read("LICENSE") == (root / "LICENSE").read_bytes(), f"{profile} device ZIP LICENSE differs")
            _require(archive.read("NOTICE") == (root / "NOTICE").read_bytes(), f"{profile} device ZIP NOTICE differs")
            _require(archive.read("octessera-pi") == (bundle / "octessera-pi").read_bytes(), f"{profile} device ZIP binary differs")
            manifest = _load_json_bytes(archive.read("octessera-device-release.json"), "device release metadata")
            if profile == "raspberry-pi-zero-2w" or updater:
                _require(
                    manifest.get("updater_protocol") == 2
                    and manifest.get("board_profile") == profile
                    and manifest.get("version") == version,
                    f"{profile} device metadata is not updater-compatible",
                )
                if profile == "orange-pi-zero-2w":
                    _require(
                        manifest.get("updater_supported") is True
                        and manifest.get("distribution") == "runtime-updater",
                        "Orange updater metadata does not declare the runtime-updater contract",
                    )
            else:
                _require(
                    archive.read("octessera-runtime.json") == (bundle / "octessera-runtime.json").read_bytes()
                    and archive.read("SHA256SUMS") == (bundle / "SHA256SUMS").read_bytes(),
                    "Orange standalone ZIP does not reuse runtime metadata/checksum",
                )
                _require(
                    manifest.get("updater_supported") is False
                    and "updater_protocol" not in manifest
                    and manifest.get("candidate_health_protocol") == 1
                    and manifest.get("distribution") == "standalone-manual",
                    "Orange standalone device metadata claims unsupported updater behavior",
                )
    except zipfile.BadZipFile as error:
        raise ReleaseArtifactError(f"device archive is not a valid ZIP: {zip_path}") from error


def _load_json_bytes(payload: bytes, label: str) -> dict[str, object]:
    try:
        document = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseArtifactError(f"{label} is not valid JSON") from error
    _require(isinstance(document, dict), f"{label} is not a JSON object")
    return document


def _verify_raspberry_kernel(root: Path, kernel_dir: Path, package_name: str) -> None:
    _require_exact_files(kernel_dir, (package_name, "SHA256SUMS", "inventory.json", "provenance.json"))
    _verify_checksum_file(kernel_dir, "SHA256SUMS")
    _run(
        root,
        [
            sys.executable,
            "tools/pi-kernel/validate-rpi-kernel-package.py",
            str(kernel_dir / package_name),
            "--manifest",
            KERNEL_MANIFEST.as_posix(),
            "--checksum-file",
            str(kernel_dir / "SHA256SUMS"),
            "--provenance-in",
            str(kernel_dir / "provenance.json"),
        ],
        "Raspberry kernel package validation",
    )
    inventory = _load_json(kernel_dir / "inventory.json", "Raspberry kernel inventory")
    provenance = _load_json(kernel_dir / "provenance.json", "Raspberry kernel provenance")
    _require({key: value for key, value in provenance.items() if key != "build"} == inventory, "Raspberry inventory and provenance chain differ")
    package = inventory.get("package")
    _require(isinstance(package, dict) and package.get("path") == package_name, "Raspberry kernel inventory package path changed")


def _verify_orange_provenance(root: Path, image_dir: Path, source_sha: str, image_package: str, dtb_package: str, manifest: Mapping[str, object]) -> None:
    evidence_path = image_dir / "octessera-orange-kernel-evidence.env"
    provenance_path = image_dir / "octessera-orange-kernel-provenance.txt"
    values = dict(line.split("=", 1) for line in evidence_path.read_text(encoding="utf-8").splitlines())
    facts = dict(line.split("=", 1) for line in provenance_path.read_text(encoding="utf-8").splitlines() if "=" in line)
    frameworks = _manifest_mapping(manifest.get("build_frameworks"), "kernel manifest build frameworks declaration")
    armbian = _manifest_mapping(frameworks.get("armbian"), "Armbian framework declaration")
    orange = _manifest_mapping(_manifest_mapping(manifest.get("kernels"), "kernel manifest kernels declaration").get("orange"), "Orange kernel declaration")
    source_lock = _manifest_mapping(manifest.get("source_lock"), "kernel manifest source lock declaration")
    expected_suffix = _manifest_string(armbian.get("native_artifact_suffix"), "Armbian native artifact suffix")
    expected_native_packages = tuple(f"{package.removesuffix('.deb')}__{expected_suffix}.deb" for package in (image_package, dtb_package))
    for key, filename in (("image_package_sha256", image_package), ("dtb_package_sha256", dtb_package)):
        _require(values.get(key) == _sha256(image_dir / filename), f"Orange provenance hash mismatch: {filename}")
    _require(facts.get("image_package") == image_package and facts.get("dtb_package") == dtb_package, "Orange provenance package chain is incomplete")
    expected_facts = {"evidence_sha256": _sha256(evidence_path), "armbian_build_ref": armbian.get("commit"), "armbian_build_tag": armbian.get("tag"), "armbian_build_repository": armbian.get("repository"), "github_source_sha": source_sha, "kernel_source_repository": orange.get("repository"), "kernel_source_branch": orange.get("branch"), "kernel_source_commit": orange.get("commit"), "kernel_release": armbian.get("kernel_release"), "package_revision": armbian.get("package_revision"), "revision_argument": armbian.get("revision_argument"), "source_lock_path": source_lock.get("path"), "source_lock_sha256": source_lock.get("sha256"), "source_lock_source": orange.get("repository"), "source_lock_branch": orange.get("branch"), "source_lock_commit": orange.get("commit"), "source_lock_effective_path": "config/sources/git_sources.json", "source_lock_effective_sha256": "e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30", "image_package_native": expected_native_packages[0], "dtb_package_native": expected_native_packages[1]}
    _require(all(facts.get(key) == expected for key, expected in expected_facts.items()), "Orange provenance evidence or build pin is not bound")
    for key, expected_native in (("image_package_native_basename", expected_native_packages[0]), ("dtb_package_native_basename", expected_native_packages[1])):
        native_name = values.get(key, "")
        _require(native_name == expected_native, "Orange native package name is not manifest-approved")
    _require(values.get("artifact_suffix") == expected_suffix and facts.get("artifact_suffix") == expected_suffix, "Orange artifact suffix is not the manifest-approved suffix")


def _verify_orange_image(root: Path, image_dir: Path, version: str, image_package: str, dtb_package: str) -> None:
    image = image_dir / f"octessera-{version}-orange-pi-zero-2w.img.xz"
    _run(
        root,
        [
            "sudo",
            "bash",
            "tools/armbian-image/verify-orange-image.sh",
            "--image",
            str(image),
            "--linux-image",
            str(image_dir / image_package),
            "--linux-dtb",
            str(image_dir / dtb_package),
            "--evidence",
            str(image_dir / "octessera-orange-kernel-evidence.env"),
            "--provenance",
            str(image_dir / "octessera-orange-kernel-provenance.txt"),
            "--manifest",
            KERNEL_MANIFEST.as_posix(),
            "--boot-proof-mode",
            "phase5-constructor",
            "--construction-contract",
            "resources/image-construction/boot-layers/orange-pi-zero-2w.json",
            "--image-provenance",
            str(image_dir / "octessera-orange-image-proof.json"),
            "--mode",
            "production",
        ],
        "Orange image validation",
    )


def _verify_runtime_and_devices(root: Path, runtime_root: Path, device_zip: Path, version: str, profile: str) -> None:
    _verify_runtime_bundle(runtime_root, version, profile)
    _verify_device_zip(root, runtime_root, device_zip, version, profile)


def _make_evidence_zip(staging: Path, output: Path) -> None:
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(staging.rglob("*")):
            if path.is_file():
                relative = path.relative_to(staging).as_posix()
                info = zipfile.ZipInfo(relative, (1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, path.read_bytes())
    with zipfile.ZipFile(output) as archive:
        _require(archive.testzip() is None, f"evidence ZIP is corrupt: {output}")


def _ensure_empty_directory(path: Path, label: str) -> None:
    if path.exists() or path.is_symlink():
        _require(path.is_dir() and not path.is_symlink(), f"{label} is not a real directory: {path}")
        _require(not any(path.iterdir()), f"{label} must be empty: {path}")
    else:
        path.mkdir(parents=True)


def assemble_release_assets(
    root: Path,
    gathered_root: Path,
    raspberry_runtime: Path,
    orange_runtime: Path,
    release_assets: Path,
    evidence_staging: Path,
    version: str,
    source_sha: str,
) -> None:
    root = root.resolve()
    gathered_root = gathered_root.resolve()
    release_assets = release_assets.resolve()
    evidence_staging = evidence_staging.resolve()
    rpi_kernel_package, orange_kernel_image, orange_kernel_dtb = _package_filenames(
        _load_json(root / KERNEL_MANIFEST, "kernel package manifest")
    )
    _ensure_empty_directory(release_assets, "release asset output")
    _ensure_empty_directory(evidence_staging, "release evidence output")
    for relative in ("windows", "ubuntu", "raspberry/image", "raspberry/device", "raspberry/kernel", "raspberry/runtime", "orange/image", "orange/device", "orange/kernel", "orange/runtime", "legal"):
        (evidence_staging / relative).mkdir(parents=True, exist_ok=True)

    prefix = f"octessera-{version}"
    windows_dir = gathered_root / "octessera-windows-release-assets"
    ubuntu_dir = gathered_root / "octessera-ubuntu-release-assets"
    rpi_image_dir = gathered_root / "octessera-raspberry-image-release-assets"
    rpi_device_dir = gathered_root / "octessera-raspberry-device-release-assets"
    rpi_kernel_dir = gathered_root / "octessera-raspberry-kernel-release-assets"
    orange_image_dir = gathered_root / "octessera-orange-image-release-assets"
    orange_device_dir = gathered_root / "octessera-orange-device-release-assets"
    rpi_manifest = f"{prefix}-raspberry-pi-zero-2w.rpi-imager-manifest"
    rpi_device_zip, rpi_device_sums = updater_asset_names(RASPBERRY_PROFILE, version)
    orange_device_zip = f"{prefix}-orange-pi-zero-2w-standalone-manual-aarch64.zip"
    orange_updater_zip, orange_updater_sums = updater_asset_names(ORANGE_PROFILE, version)

    _require_exact_files(windows_dir, (f"{prefix}-windows-installer.exe", f"{prefix}-windows-portable.zip", "SHA256SUMS-windows.txt"))
    _require_exact_files(ubuntu_dir, (f"{prefix}-ubuntu-amd64.deb", f"{prefix}-ubuntu-x86_64.AppImage", "SHA256SUMS-ubuntu.txt"))
    _require_exact_files(rpi_kernel_dir, (rpi_kernel_package, "SHA256SUMS", "inventory.json", "provenance.json"))
    _require_exact_files(rpi_image_dir, (f"{prefix}-raspberry-pi-zero-2w.img.zip", rpi_manifest, "SHA256SUMS-pi.txt"))
    _require_exact_files(rpi_device_dir, (rpi_device_zip, rpi_device_sums))
    _require_exact_files(orange_image_dir, (f"{prefix}-orange-pi-zero-2w.img.xz", f"{prefix}-orange-pi-zero-2w.img.xz.sha256", orange_kernel_image, orange_kernel_dtb, "octessera-orange-kernel-evidence.env", "octessera-orange-kernel-provenance.txt", "octessera-orange-image-proof.json", "SHA256SUMS-orange-pi-zero-2w.txt"))
    _require_exact_files(orange_device_dir, (orange_device_zip, "SHA256SUMS-orange-pi-zero-2w-device.txt", orange_updater_zip, orange_updater_sums))

    for source, name in (
        (windows_dir / f"{prefix}-windows-installer.exe", f"{prefix}-windows-installer.exe"),
        (windows_dir / f"{prefix}-windows-portable.zip", f"{prefix}-windows-portable.zip"),
        (ubuntu_dir / f"{prefix}-ubuntu-amd64.deb", f"{prefix}-ubuntu-amd64.deb"),
        (ubuntu_dir / f"{prefix}-ubuntu-x86_64.AppImage", f"{prefix}-ubuntu-x86_64.AppImage"),
        (rpi_image_dir / f"{prefix}-raspberry-pi-zero-2w.img.zip", f"{prefix}-raspberry-pi-zero-2w.img.zip"),
        (rpi_image_dir / rpi_manifest, rpi_manifest),
        (rpi_device_dir / rpi_device_zip, rpi_device_zip),
        (rpi_device_dir / rpi_device_sums, rpi_device_sums),
        (orange_image_dir / f"{prefix}-orange-pi-zero-2w.img.xz", f"{prefix}-orange-pi-zero-2w.img.xz"),
        (orange_device_dir / orange_device_zip, orange_device_zip),
        (orange_device_dir / orange_updater_zip, orange_updater_zip),
        (orange_device_dir / orange_updater_sums, orange_updater_sums),
    ):
        _copy_file(source, release_assets / name, "release asset")

    _verify_checksum_file(windows_dir, "SHA256SUMS-windows.txt")
    _verify_checksum_file(ubuntu_dir, "SHA256SUMS-ubuntu.txt")
    _verify_checksum_file(rpi_image_dir, "SHA256SUMS-pi.txt")
    _verify_checksum_file(rpi_device_dir, rpi_device_sums)
    _verify_raspberry_kernel(root, rpi_kernel_dir, rpi_kernel_package)
    _verify_checksum_file(orange_image_dir, f"{prefix}-orange-pi-zero-2w.img.xz.sha256")
    _verify_checksum_file(orange_image_dir, "SHA256SUMS-orange-pi-zero-2w.txt")
    _verify_checksum_file(orange_device_dir, "SHA256SUMS-orange-pi-zero-2w-device.txt")
    _verify_checksum_file(orange_device_dir, orange_updater_sums)
    _verify_orange_provenance(root, orange_image_dir, source_sha, orange_kernel_image, orange_kernel_dtb, _load_json(root / KERNEL_MANIFEST, "kernel package manifest"))
    _verify_orange_image(root, orange_image_dir, version, orange_kernel_image, orange_kernel_dtb)
    _verify_runtime_and_devices(root, raspberry_runtime, rpi_device_dir / rpi_device_zip, version, "raspberry-pi-zero-2w")
    _verify_runtime_and_devices(root, orange_runtime, orange_device_dir / orange_device_zip, version, "orange-pi-zero-2w")
    _verify_device_zip(root, orange_runtime, orange_device_dir / orange_updater_zip, version, "orange-pi-zero-2w", updater=True)

    portable = release_assets / f"{prefix}-windows-portable.zip"
    verify_notice_archive(root, portable, "octessera.exe")
    for source, destination in (
        (windows_dir / "SHA256SUMS-windows.txt", evidence_staging / "windows/SHA256SUMS-windows.txt"),
        (ubuntu_dir / "SHA256SUMS-ubuntu.txt", evidence_staging / "ubuntu/SHA256SUMS-ubuntu.txt"),
        (rpi_image_dir / "SHA256SUMS-pi.txt", evidence_staging / "raspberry/image/SHA256SUMS-pi.txt"),
        (rpi_image_dir / rpi_manifest, evidence_staging / f"raspberry/image/{rpi_manifest}"),
        (rpi_device_dir / rpi_device_sums, evidence_staging / f"raspberry/device/{rpi_device_sums}"),
        (rpi_kernel_dir / rpi_kernel_package, evidence_staging / f"raspberry/kernel/{rpi_kernel_package}"),
        (rpi_kernel_dir / "SHA256SUMS", evidence_staging / "raspberry/kernel/SHA256SUMS"),
        (rpi_kernel_dir / "inventory.json", evidence_staging / "raspberry/kernel/inventory.json"),
        (rpi_kernel_dir / "provenance.json", evidence_staging / "raspberry/kernel/provenance.json"),
        (raspberry_runtime / "SHA256SUMS", evidence_staging / "raspberry/runtime/SHA256SUMS"),
        (orange_image_dir / f"{prefix}-orange-pi-zero-2w.img.xz.sha256", evidence_staging / f"orange/image/{prefix}-orange-pi-zero-2w.img.xz.sha256"),
        (orange_image_dir / "SHA256SUMS-orange-pi-zero-2w.txt", evidence_staging / "orange/image/SHA256SUMS-orange-pi-zero-2w.txt"),
        (orange_image_dir / orange_kernel_image, evidence_staging / f"orange/kernel/{orange_kernel_image}"),
        (orange_image_dir / orange_kernel_dtb, evidence_staging / f"orange/kernel/{orange_kernel_dtb}"),
        (orange_image_dir / "octessera-orange-kernel-evidence.env", evidence_staging / "orange/kernel/octessera-orange-kernel-evidence.env"),
        (orange_image_dir / "octessera-orange-kernel-provenance.txt", evidence_staging / "orange/kernel/octessera-orange-kernel-provenance.txt"),
        (orange_image_dir / "octessera-orange-image-proof.json", evidence_staging / "orange/image/octessera-orange-image-proof.json"),
        (orange_device_dir / "SHA256SUMS-orange-pi-zero-2w-device.txt", evidence_staging / "orange/device/SHA256SUMS-orange-pi-zero-2w-device.txt"),
        (orange_device_dir / orange_updater_sums, evidence_staging / f"orange/device/{orange_updater_sums}"),
        (orange_runtime / "SHA256SUMS", evidence_staging / "orange/runtime/SHA256SUMS"),
    ):
        _copy_file(source, destination, "release evidence")

    notices = evidence_staging / "legal" / f"{prefix}-notices.zip"
    package_notice_zip(root, notices)
    verify_notice_archive(root, notices)
    evidence_zip = release_assets / f"{prefix}-release-evidence.zip"
    _make_evidence_zip(evidence_staging, evidence_zip)

    expected_root_assets = [
        f"{prefix}-windows-installer.exe",
        f"{prefix}-windows-portable.zip",
        f"{prefix}-ubuntu-amd64.deb",
        f"{prefix}-ubuntu-x86_64.AppImage",
        f"{prefix}-raspberry-pi-zero-2w.img.zip",
        rpi_manifest,
        rpi_device_zip,
        rpi_device_sums,
        f"{prefix}-orange-pi-zero-2w.img.xz",
        orange_device_zip,
        orange_updater_zip,
        orange_updater_sums,
        f"{prefix}-release-evidence.zip",
    ]
    _write_checksums(release_assets, "SHA256SUMS.txt", expected_root_assets)
    expected_root_assets.append("SHA256SUMS.txt")
    _require_exact_files(release_assets, expected_root_assets)
    _require(len(expected_root_assets) == 14, "release root asset contract is not exactly fourteen files")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Assemble and verify the exact Octessera release asset set.")
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--gathered-root", type=Path, required=True)
    parser.add_argument("--raspberry-runtime", type=Path, required=True)
    parser.add_argument("--orange-runtime", type=Path, required=True)
    parser.add_argument("--release-assets", type=Path, required=True)
    parser.add_argument("--evidence-staging", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    args = parser.parse_args(argv)
    try:
        assemble_release_assets(
            args.repository_root,
            args.gathered_root,
            args.raspberry_runtime,
            args.orange_runtime,
            args.release_assets,
            args.evidence_staging,
            args.version,
            args.source_sha,
        )
    except (ReleaseArtifactError, ValueError, KeyError, OSError, zipfile.BadZipFile) as error:
        print(f"release asset verification failed: {error}", file=sys.stderr)
        return 1
    print(f"Verified exact release asset set under {args.release_assets}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
