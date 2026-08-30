from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import stat
import sys
import zipfile
from pathlib import Path, PurePosixPath
from typing import Iterable, cast

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tools" / "legal"))
sys.path.insert(0, str(ROOT / "tools" / "device-update"))

from package_notice_zip import package_notice_zip  # type: ignore[import-not-found]
from verify_notice_archive import verify_notice_archive  # type: ignore[import-not-found]
from updater_profiles import ORANGE_PROFILE, RASPBERRY_PROFILE, updater_asset_names  # type: ignore[import-not-found]
from tools.release.board_image_release import (  # type: ignore[import-not-found]
    ReleaseArtifactError,
    verify_and_stage_board_images,
)

CHECKSUM_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")
RUNTIME_FILES = ("SHA256SUMS", "octessera-pi", "octessera-runtime.json")

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
    _ensure_empty_directory(release_assets, "release asset output")
    _ensure_empty_directory(evidence_staging, "release evidence output")
    for relative in ("windows", "ubuntu", "raspberry/image", "raspberry/device", "raspberry/kernel", "raspberry/runtime", "orange/image", "orange/device", "orange/kernel", "orange/runtime", "legal"):
        (evidence_staging / relative).mkdir(parents=True, exist_ok=True)

    prefix = f"octessera-{version}"
    windows_dir = gathered_root / "octessera-windows-release-assets"
    ubuntu_dir = gathered_root / "octessera-ubuntu-release-assets"
    rpi_device_dir = gathered_root / "octessera-raspberry-device-release-assets"
    orange_device_dir = gathered_root / "octessera-orange-device-release-assets"
    rpi_manifest = f"{prefix}-raspberry-pi-zero-2w.rpi-imager-manifest"
    rpi_device_zip, rpi_device_sums = updater_asset_names(RASPBERRY_PROFILE, version)
    orange_device_zip = f"{prefix}-orange-pi-zero-2w-standalone-manual-aarch64.zip"
    orange_updater_zip, orange_updater_sums = updater_asset_names(ORANGE_PROFILE, version)

    _require_exact_files(windows_dir, (f"{prefix}-windows-installer.exe", f"{prefix}-windows-portable.zip", "SHA256SUMS-windows.txt"))
    _require_exact_files(ubuntu_dir, (f"{prefix}-ubuntu-amd64.deb", f"{prefix}-ubuntu-x86_64.AppImage", "SHA256SUMS-ubuntu.txt"))
    _require_exact_files(rpi_device_dir, (rpi_device_zip, rpi_device_sums))
    _require_exact_files(orange_device_dir, (orange_device_zip, "SHA256SUMS-orange-pi-zero-2w-device.txt", orange_updater_zip, orange_updater_sums))

    verify_and_stage_board_images(root, gathered_root, release_assets, evidence_staging, raspberry_runtime, orange_runtime, version, source_sha)
    for source, name in (
        (windows_dir / f"{prefix}-windows-installer.exe", f"{prefix}-windows-installer.exe"),
        (windows_dir / f"{prefix}-windows-portable.zip", f"{prefix}-windows-portable.zip"),
        (ubuntu_dir / f"{prefix}-ubuntu-amd64.deb", f"{prefix}-ubuntu-amd64.deb"),
        (ubuntu_dir / f"{prefix}-ubuntu-x86_64.AppImage", f"{prefix}-ubuntu-x86_64.AppImage"),
        (rpi_device_dir / rpi_device_zip, rpi_device_zip),
        (rpi_device_dir / rpi_device_sums, rpi_device_sums),
        (orange_device_dir / orange_device_zip, orange_device_zip),
        (orange_device_dir / orange_updater_zip, orange_updater_zip),
        (orange_device_dir / orange_updater_sums, orange_updater_sums),
    ):
        _copy_file(source, release_assets / name, "release asset")

    _verify_checksum_file(windows_dir, "SHA256SUMS-windows.txt")
    _verify_checksum_file(ubuntu_dir, "SHA256SUMS-ubuntu.txt")
    _verify_checksum_file(rpi_device_dir, rpi_device_sums)
    _verify_checksum_file(orange_device_dir, "SHA256SUMS-orange-pi-zero-2w-device.txt")
    _verify_checksum_file(orange_device_dir, orange_updater_sums)
    _verify_runtime_and_devices(root, raspberry_runtime, rpi_device_dir / rpi_device_zip, version, "raspberry-pi-zero-2w")
    _verify_runtime_and_devices(root, orange_runtime, orange_device_dir / orange_device_zip, version, "orange-pi-zero-2w")
    _verify_device_zip(root, orange_runtime, orange_device_dir / orange_updater_zip, version, "orange-pi-zero-2w", updater=True)

    portable = release_assets / f"{prefix}-windows-portable.zip"
    verify_notice_archive(root, portable, "octessera.exe")
    for source, destination in (
        (windows_dir / "SHA256SUMS-windows.txt", evidence_staging / "windows/SHA256SUMS-windows.txt"),
        (ubuntu_dir / "SHA256SUMS-ubuntu.txt", evidence_staging / "ubuntu/SHA256SUMS-ubuntu.txt"),
        (rpi_device_dir / rpi_device_sums, evidence_staging / f"raspberry/device/{rpi_device_sums}"),
        (raspberry_runtime / "SHA256SUMS", evidence_staging / "raspberry/runtime/SHA256SUMS"),
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
