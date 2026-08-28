from __future__ import annotations

import hashlib
import json
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import zipfile
from contextlib import contextmanager
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Iterator, Mapping, cast

ROOT = Path(__file__).resolve().parents[2]
RESPIN_TOOLS = ROOT / "tools" / "image-respin"
sys.path.insert(0, str(RESPIN_TOOLS))

from post_proof_record import _bundle_identity  # type: ignore[import-not-found]
from record_documents import load_json as load_record_json  # type: ignore[import-not-found]
from record_hashing import canonical_bytes  # type: ignore[import-not-found]
from record_paths import identity, verify_identity  # type: ignore[import-not-found]
from record_validation import SOURCE_RE, require, require_keys, verify_source  # type: ignore[import-not-found]
from requested_build_record import validate_record as validate_requested_record  # type: ignore[import-not-found]
from setup_contract import load_contract  # type: ignore[import-not-found]
from setup_workflow_record import (  # type: ignore[import-not-found]
    PRODUCTION_PROOF_LABELS,
    SETUP_PROOF_TOOLS,
    _document,
    _read_proof,
    _validate_proof,
    _validate_provenance,
    _validate_setup_proof_tools,
)
from record_tool_contract import verify_tool  # type: ignore[import-not-found]
from trust_manifest import load_manifest, parent_context_for_board  # type: ignore[import-not-found]


BASE_REFRESH = "base-refresh"
QUALIFIED_RESPIN = "qualified-respin"
BOARD_IMAGE_MODES = (BASE_REFRESH, QUALIFIED_RESPIN)
RPI = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
RESPIN_FEATURE_COMMANDS = {
    RPI: "cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-raspberry-pi-zero-2w",
    ORANGE: "cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w",
}
MANIFEST = Path("resources/image-parents/v0.7.5-trust-manifest.json")
KERNEL_MANIFEST = Path("tools/kernel-patches/orange-midi-interface-manifest.json")
CHECKSUM_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")
RUNTIME_FILES = ("SHA256SUMS", "octessera-pi", "octessera-runtime.json")


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


def _qualified_names(board: str, version: str) -> tuple[str, str, str, str, str, str, str]:
    _require(board in {RPI, ORANGE}, f"unsupported board profile: {board}")
    prefix = f"octessera-{version}-{board}"
    artifact = f"{prefix}-derived-setup-respin{'.zip' if board == RPI else '.img.xz'}"
    production = "raspberry-sanitized-image-proof.txt" if board == RPI else "orange-image-proof.json"
    return artifact, f"{artifact}.provenance.json", "requested-build.json", "setup-post-proof.json", "setup-layer-proof.json", production, f"SHA256SUMS-{board}.txt"


def _verify_raspberry_artifact(path: Path) -> tuple[str, int]:
    try:
        with zipfile.ZipFile(path) as archive:
            entries = archive.infolist()
            images = [entry for entry in entries if entry.filename.endswith(".img")]
            _require(len(entries) == 1 and len(images) == 1, "qualified Raspberry artifact members are not exact")
            entry = images[0]
            _require(not entry.is_dir() and not entry.filename.startswith("/") and "\\" not in entry.filename and all(part not in {"", ".", ".."} for part in PurePosixPath(entry.filename).parts), "qualified Raspberry image member is unsafe")
            _require(((entry.external_attr >> 16) & 0o170000) not in {stat.S_IFDIR, stat.S_IFLNK}, "qualified Raspberry image member is not regular")
            image_bytes = archive.read(entry)
            return hashlib.sha256(image_bytes).hexdigest(), len(image_bytes)
    except (OSError, RuntimeError, zipfile.BadZipFile) as error:
        raise ReleaseArtifactError(f"qualified Raspberry artifact is unreadable: {path}") from error


def _verify_generated_raspberry_manifest(image: Path, manifest: Path, version: str, expected_image: tuple[str, int]) -> None:
    _regular_file(manifest, "generated Raspberry Imager manifest")
    try:
        with zipfile.ZipFile(image) as archive:
            entries = archive.infolist()
            image_entries = [entry for entry in entries if entry.filename.endswith(".img")]
            manifest_entries = [entry for entry in entries if entry.filename == "os_list.rpi-imager-manifest"]
            _require(len(entries) == 2 and len(image_entries) == 1 and len(manifest_entries) == 1, "generated Raspberry Imager ZIP inventory is not exact")
            manifest_bytes = archive.read(manifest_entries[0])
            _require(manifest_bytes == manifest.read_bytes(), "generated Raspberry Imager manifest differs from its embedded copy")
            image_bytes = archive.read(image_entries[0])
    except (OSError, RuntimeError, zipfile.BadZipFile) as error:
        raise ReleaseArtifactError(f"generated Raspberry Imager ZIP is unreadable: {image}") from error
    document = _load_json(manifest, "generated Raspberry Imager manifest")
    _require(document.get("board_profile") == RPI, "generated Raspberry Imager manifest board is not exact")
    os_list = document.get("os_list")
    _require(isinstance(os_list, list) and len(cast(list[object], os_list)) == 1 and isinstance(cast(list[object], os_list)[0], dict), "generated Raspberry Imager manifest OS list is not exact")
    item = cast(dict[str, object], cast(list[object], os_list)[0])
    image_identity = (hashlib.sha256(image_bytes).hexdigest(), len(image_bytes))
    _require(image_identity == expected_image and item.get("board_profile") == RPI and item.get("extract_size") == image_identity[1] and item.get("extract_sha256") == image_identity[0] and item.get("image_download_size") == image.stat().st_size and item.get("url") == f"https://github.com/nexxyz/octessera/releases/download/v{version}/{image.name}", "generated Raspberry Imager manifest image identity is not exact")


def _verify_qualified_checksum(directory: Path, checksum_name: str, expected: Iterable[str]) -> None:
    expected_names = tuple(expected)
    actual = _checksum_entries(directory, checksum_name)
    _require(set(actual) == set(expected_names) and len(actual) == len(expected_names), f"qualified board checksum inventory is not exact: {checksum_name}")


def _record_identity(value: Any, label: str, expected_path: str, actual: Path) -> dict[str, Any]:
    checked = require_keys(value, {"path", "sha256", "size"}, label)
    digest, size = _file_identity(actual)
    _require(checked == {"path": expected_path, "sha256": digest, "size": size}, f"{label} is not bound to the qualified handoff")
    return checked


def _compare_bundle_record(record: Any, actual: dict[str, Any]) -> None:
    bundle = require_keys(record, {"path", "entries", "sha256", "inventory_sha256"}, "runtime bundle")
    _require(bundle["path"] == "runtime-bundle" and bundle["inventory_sha256"] == actual["inventory_sha256"], "runtime bundle identity changed")
    entries = bundle["entries"]
    _require(isinstance(entries, list) and len(entries) == len(RUNTIME_FILES), "runtime bundle entries are invalid")
    actual_entries = {Path(item["path"]).name: item for item in actual["entries"]}
    _require(set(actual_entries) == set(RUNTIME_FILES), "runtime bundle entry set changed")
    expected = {f"runtime-bundle/{name}": actual_entries[name] for name in RUNTIME_FILES}
    _require({item.get("path") for item in entries if isinstance(item, dict)} == set(expected), "runtime bundle entry set changed")
    _require([item.get("path") for item in entries if isinstance(item, dict)] == sorted(expected), "runtime bundle entry order changed")
    normalized_entries = [{"path": path, "sha256": expected[path]["sha256"], "size": expected[path]["size"]} for path in sorted(expected)]
    normalized_sha = hashlib.sha256(canonical_bytes(normalized_entries)).hexdigest()
    _require(bundle["sha256"] == normalized_sha, "runtime bundle identity changed")
    for item in entries:
        checked = require_keys(item, {"path", "sha256", "size"}, "runtime bundle entry")
        _require(isinstance(checked["path"], str) and checked["path"] in expected, "runtime bundle entry path changed")
        expected_item = expected[checked["path"]]
        _require(checked["sha256"] == expected_item["sha256"] and checked["size"] == expected_item["size"], "runtime bundle entry changed")


def _verify_companions(value: Any, manifest: dict[str, Any], board: str) -> None:
    parent = next(item for item in manifest["image_parents"] if item["board"] == board)
    expected_names = (parent["asset"], *parent["proof_companion_assets"])
    assets = {item["name"]: item for item in manifest["assets"]}
    _require(isinstance(value, list) and len(value) == len(expected_names), "qualified companion set is not exact")
    seen: set[str] = set()
    for item in value:
        checked = require_keys(item, {"path", "sha256", "size"}, "qualified companion")
        _require(isinstance(checked["path"], str), "qualified companion path is malformed")
        name = Path(checked["path"]).name
        _require(checked["path"] == f"parent-assets/{name}" and name in expected_names and name not in seen, "qualified companion path set is not exact")
        anchor = assets[name]
        _require(checked["sha256"] == anchor["sha256"] and checked["size"] == anchor["size"], f"qualified companion differs from trust manifest: {name}")
        seen.add(name)
    _require(seen == set(expected_names), "qualified companion set is not exact")
    _require([item["path"] for item in value] == sorted(item["path"] for item in value), "qualified companion order is not exact")


def _verify_respin_proof_shape(path: Path, board: str) -> dict[str, Any] | None:
    proof = _read_proof(path, board)
    if board == RPI:
        try:
            text = path.read_text(encoding="utf-8").lower()
        except (OSError, UnicodeDecodeError) as error:
            raise ReleaseArtifactError(f"qualified Raspberry proof is unreadable: {path}") from error
        _require(not any(marker in text for marker in ("full-constructor", "phase5-constructor", "constructor-required")), "qualified Raspberry proof makes a constructor claim")
    return proof


@contextmanager
def _validation_files(root: Path, runtime_bundle: Path, handoff: Path, names: tuple[str, str, str, str, str, str, str]) -> Iterator[dict[str, Path]]:
    temporary = Path(tempfile.mkdtemp(prefix=".octessera-qualified-respin-", dir=root))
    try:
        staged_bundle = temporary / "runtime-bundle"
        staged_bundle.mkdir()
        for name in RUNTIME_FILES:
            _copy_file(runtime_bundle / name, staged_bundle / name, "runtime bundle")
        staged: dict[str, Path] = {"runtime_bundle": staged_bundle}
        for name in names:
            source = handoff / name
            destination = temporary / name
            _copy_file(source, destination, "qualified handoff")
            staged[name] = destination
        yield staged
    finally:
        shutil.rmtree(temporary, ignore_errors=False)


def _verify_requested_and_setup_records(root: Path, handoff: Path, runtime_bundle: Path, version: str, source_sha: str, board: str, names: tuple[str, str, str, str, str, str, str]) -> None:
    artifact, provenance, requested_name, setup_record, setup_proof, production, _ = names
    requested_path = handoff / requested_name
    requested = load_record_json(requested_path)
    try:
        validate_requested_record(requested, root)
    except TypeError as error:
        raise ReleaseArtifactError("qualified requested build is malformed") from error
    source = require_keys(requested["source"], {"sha", "version", "board", "feature_command"}, "qualified requested source")
    _require(isinstance(source["sha"], str) and isinstance(source["version"], str) and isinstance(source["board"], str), "qualified requested source is malformed")
    verify_source(source["sha"], source["version"], source["board"], {RPI, ORANGE})
    _require(source["sha"] == source_sha and source["version"] == version and source["board"] == board, "qualified requested source differs from release")
    feature_command = source["feature_command"]
    _require(isinstance(feature_command, str), "qualified feature command is malformed")
    _require(feature_command == RESPIN_FEATURE_COMMANDS[board], "qualified requested feature command is not exact")
    setup = require_keys(requested.get("setup"), {"mode", "contract", "inputs", "tool_files"}, "qualified requested setup")
    _require(setup["mode"] == "setup-portal", "qualified handoff is not a setup respin")
    _record_identity(requested["trust_manifest"], "qualified requested trust manifest", MANIFEST.as_posix(), root / MANIFEST)
    setup_record_value = load_record_json(handoff / setup_record)
    expected_top = {"schema", "schema_version", "record_kind", "result", "source", "requested_build", "parent", "runtime_bundle", "derived_artifact", "setup_provenance", "setup_proof", "production_proofs", "proof_tools", "companions", "workflow", "tool"}
    top = require_keys(setup_record_value, expected_top, "qualified setup post-proof")
    _require(top["schema"] == "octessera.image-respin-setup-post-proof/v1" and top["schema_version"] == 1 and top["record_kind"] == "setup-post-proof" and top["result"] == {"status": "success", "setup_proof_succeeded": True}, "qualified setup post-proof identity is not exact")
    verify_tool(top["tool"], RESPIN_TOOLS / "setup_workflow_record.py", root, "tools/image-respin/setup_workflow_record.py", "octessera-image-respin-setup-post-proof")
    _require(top["source"] == source, "qualified setup post-proof source differs from requested build")
    _record_identity(top["requested_build"], "qualified requested build", f"respin-output/{requested_name}", requested_path)
    parent = require_keys(top["parent"], {"context", "trust_manifest"}, "qualified setup parent")
    manifest_identity = _record_identity(parent["trust_manifest"], "qualified setup trust manifest", MANIFEST.as_posix(), root / MANIFEST)
    checked_manifest = load_manifest(root / MANIFEST)
    expected_parent = parent_context_for_board(checked_manifest, board)
    _require(parent["context"] == expected_parent, "qualified setup parent context changed")
    contract_identity = verify_identity(setup["contract"], root, "qualified setup contract")
    contract, _ = load_contract(root / contract_identity["path"])
    _verify_companions(top["companions"], checked_manifest, board)
    _record_identity(top["workflow"], "qualified workflow", ".github/workflows/respin-board-image.yml", root / ".github/workflows/respin-board-image.yml")
    _require(isinstance(top["proof_tools"], list), "qualified proof tools are invalid")
    _validate_setup_proof_tools(top["proof_tools"], root, board)
    with _validation_files(root, runtime_bundle, handoff, names) as staged:
        bundle = _bundle_identity(staged["runtime_bundle"], root)
        _compare_bundle_record(top["runtime_bundle"], bundle)
        artifact_identity = _record_identity(top["derived_artifact"], f"qualified {board} derived artifact", f"respin-output/{artifact}", handoff / artifact)
        _record_identity(top["setup_provenance"], "qualified setup provenance", f"respin-output/{provenance}", handoff / provenance)
        proof_identity = _record_identity(top["setup_proof"], "qualified setup proof", f"respin-output/{setup_proof}", handoff / setup_proof)
        proof = _document(staged[setup_proof], "qualified setup proof")
        _validate_proof(proof, board, contract_identity, contract)
        proofs = require_keys(top["production_proofs"], set(PRODUCTION_PROOF_LABELS[board]), "qualified production proofs")
        production_identity = _record_identity(proofs[PRODUCTION_PROOF_LABELS[board][0]], f"qualified {board} production proof", f"respin-output/{production}", handoff / production)
        structured = _verify_respin_proof_shape(staged[production], board)
        _validate_provenance(staged[provenance], root, requested, contract, contract_identity, proof, expected_parent, manifest_identity["sha256"], bundle, identity(staged[artifact], root), structured)
        _require(artifact_identity["sha256"] == identity(staged[artifact], root)["sha256"] and production_identity["sha256"] == identity(staged[production], root)["sha256"], "qualified proof identity changed")


def _qualified_respin_images(root: Path, gathered_root: Path, release_assets: Path, evidence_staging: Path, runtime_bundle: Path, version: str, source_sha: str, board: str) -> None:
    names = _qualified_names(board, version)
    artifact, provenance, requested, setup_record, setup_proof, production, checksum = names
    handoff = gathered_root / f"octessera-{board}-image-release-assets"
    _require_exact_files(handoff, names)
    _verify_qualified_checksum(handoff, checksum, (artifact, provenance, requested, setup_record, setup_proof, production))
    raspberry_image_identity: tuple[str, int] | None = None
    if board == RPI:
        raspberry_image_identity = _verify_raspberry_artifact(handoff / artifact)
    _verify_requested_and_setup_records(root, handoff, runtime_bundle, version, source_sha, board, names)
    prefix = f"octessera-{version}"
    image_checksum: Path | None = None
    if board == RPI:
        image = release_assets / f"{prefix}-raspberry-pi-zero-2w.img.zip"
        _copy_file(handoff / artifact, image, "qualified Raspberry image")
        manifest = release_assets / f"{prefix}-raspberry-pi-zero-2w.rpi-imager-manifest"
        _run(root, [sys.executable, "tools/pi-image/package-rpi-imager-zip.py", "--zip", str(image), "--version", version, "--tag", f"v{version}", "--repository", "nexxyz/octessera", "--manifest-out", str(manifest), "--board-profile", RPI], "Raspberry Imager manifest generation")
        assert raspberry_image_identity is not None
        _verify_generated_raspberry_manifest(image, manifest, version, raspberry_image_identity)
        evidence = "raspberry/image"
        evidence_files = ((handoff / checksum, evidence + f"/{checksum}"), (manifest, evidence + f"/{manifest.name}"), (root / MANIFEST, evidence + "/v0.7.5-trust-manifest.json"), (handoff / provenance, evidence + f"/{provenance}"), (handoff / requested, evidence + f"/{requested}"), (handoff / setup_record, evidence + f"/{setup_record}"), (handoff / setup_proof, evidence + f"/{setup_proof}"), (handoff / production, evidence + f"/{production}"))
    else:
        image = release_assets / f"{prefix}-orange-pi-zero-2w.img.xz"
        _copy_file(handoff / artifact, image, "qualified Orange image")
        image_checksum = release_assets / f"{image.name}.sha256"
        _write_checksum(image_checksum, image.name, _sha256(image))
        _verify_checksum_file(release_assets, image_checksum.name)
        evidence = "orange/image"
        evidence_files = ((image_checksum, evidence + f"/{image_checksum.name}"), (root / MANIFEST, evidence + "/v0.7.5-trust-manifest.json"), (handoff / provenance, evidence + f"/{provenance}"), (handoff / requested, evidence + f"/{requested}"), (handoff / setup_record, evidence + f"/{setup_record}"), (handoff / setup_proof, evidence + f"/{setup_proof}"), (handoff / production, evidence + f"/{production}"), (handoff / checksum, evidence + f"/{checksum}"))
    for source, relative in evidence_files:
        _copy_file(source, evidence_staging / relative, "qualified release evidence")
    if board == ORANGE:
        assert image_checksum is not None
        image_checksum.unlink()


def verify_and_stage_board_images(root: Path, gathered_root: Path, release_assets: Path, evidence_staging: Path, raspberry_runtime: Path, orange_runtime: Path, version: str, source_sha: str, board_image_mode: str = BASE_REFRESH) -> None:
    try:
        _require(board_image_mode in BOARD_IMAGE_MODES, f"unknown board image mode: {board_image_mode}")
        _require(SOURCE_RE.fullmatch(source_sha) is not None, "source SHA is invalid")
        if board_image_mode == BASE_REFRESH:
            _base_refresh_images(root, gathered_root, release_assets, evidence_staging, version, source_sha)
        else:
            _qualified_respin_images(root, gathered_root, release_assets, evidence_staging, raspberry_runtime, version, source_sha, RPI)
            _qualified_respin_images(root, gathered_root, release_assets, evidence_staging, orange_runtime, version, source_sha, ORANGE)
    except ReleaseArtifactError:
        raise
    except (KeyError, OSError, TypeError, ValueError) as error:
        raise ReleaseArtifactError(str(error)) from error


__all__ = ["BASE_REFRESH", "BOARD_IMAGE_MODES", "QUALIFIED_RESPIN", "RESPIN_FEATURE_COMMANDS", "ReleaseArtifactError", "verify_and_stage_board_images"]
