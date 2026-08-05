from __future__ import annotations

import hashlib
import lzma
import os
import shutil
import stat
import sys
import tempfile
import zipfile
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    from .provenance import write_provenance
    from .trust_manifest import ManifestError, load_manifest, parent_context_for_board, validate_downloaded_directory
except ImportError:
    from provenance import write_provenance
    from trust_manifest import ManifestError, load_manifest, parent_context_for_board, validate_downloaded_directory


class DiskPackagingError(ValueError):
    pass


@dataclass
class PreparedImage:
    work: Path
    image: Path
    source: Path
    parent_context: dict[str, Any]
    manifest_digest: str

    def verify_source_unchanged(self) -> None:
        expected = self.parent_context["asset"]
        digest, size = file_digest(self.source)
        if digest != expected["sha256"] or size != expected["size"]:
            raise DiskPackagingError("trusted source asset changed during respin")

    def close(self) -> None:
        shutil.rmtree(self.work, ignore_errors=False)


def file_digest(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    try:
        with Path(path).open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                digest.update(chunk)
    except OSError as exc:
        raise DiskPackagingError(f"cannot read asset: {path}") from exc
    return digest.hexdigest(), size


def verify_parent_asset(assets_directory: Path, manifest_path: Path, board_profile: str) -> tuple[Path, dict[str, Any], str, bytes | None]:
    try:
        manifest_bytes = Path(manifest_path).read_bytes()
        manifest = load_manifest(Path(manifest_path))
        validate_downloaded_directory(Path(assets_directory), manifest, (board_profile,))
        context = parent_context_for_board(manifest, board_profile)
    except (OSError, ManifestError) as exc:
        raise DiskPackagingError(f"trusted board asset set rejected: {exc}") from exc
    source = Path(assets_directory) / context["asset"]["name"]
    if source.is_symlink() or not source.is_file():
        raise DiskPackagingError("trusted parent asset is not a regular file")
    imager_manifest: bytes | None = None
    if board_profile == "raspberry-pi-zero-2w":
        parent = next(parent for parent in manifest["image_parents"] if parent["board"] == board_profile)
        companion = Path(assets_directory) / parent["proof_companion_assets"][0]
        if companion.is_symlink() or not companion.is_file():
            raise DiskPackagingError("Raspberry standalone Imager manifest is not a regular file")
        embedded_manifest = companion.read_bytes()
        _validate_raspberry_archive(source, embedded_manifest)
        imager_manifest = embedded_manifest
    return source, context, hashlib.sha256(manifest_bytes).hexdigest(), imager_manifest


def _safe_zip_member(info: zipfile.ZipInfo) -> None:
    name = info.filename
    if not name or name.startswith("/") or "\\" in name or any(part in {"", ".", ".."} for part in Path(name).parts):
        raise DiskPackagingError("Raspberry parent ZIP contains an unsafe member path")
    mode = (info.external_attr >> 16) & 0o170000
    if info.is_dir() or mode == stat.S_IFLNK:
        raise DiskPackagingError("Raspberry parent ZIP contains a non-regular member")


def _validate_raspberry_archive(source: Path, imager_manifest: bytes) -> str:
    try:
        with zipfile.ZipFile(source, "r") as archive:
            entries = archive.infolist()
            if len(entries) != 2:
                raise DiskPackagingError("Raspberry parent ZIP must contain exactly one .img and one Imager manifest")
            image_entries = [entry for entry in entries if entry.filename.endswith(".img")]
            manifest_entries = [entry for entry in entries if entry.filename == "os_list.rpi-imager-manifest"]
            if len(image_entries) != 1 or len(manifest_entries) != 1:
                raise DiskPackagingError("Raspberry parent ZIP members are not exact")
            for entry in entries:
                _safe_zip_member(entry)
            if archive.read(manifest_entries[0]) != imager_manifest:
                raise DiskPackagingError("embedded Raspberry Imager manifest differs from its trusted companion")
            return image_entries[0].filename
    except (OSError, zipfile.BadZipFile) as exc:
        raise DiskPackagingError("Raspberry parent ZIP is unreadable") from exc


def prepare_parent_image(source: Path, parent_context: dict[str, Any], manifest_digest: str, board_profile: str, imager_manifest: bytes | None = None) -> PreparedImage:
    work = Path(tempfile.mkdtemp(prefix="octessera-image-respin-"))
    source = Path(source)
    try:
        image = work / f"parent-{board_profile}.img"
        if board_profile == "orange-pi-zero-2w":
            if source.suffixes[-2:] != [".img", ".xz"]:
                raise DiskPackagingError("Orange parent must be an .img.xz asset")
            with lzma.open(source, "rb") as source_stream, image.open("wb") as destination:
                shutil.copyfileobj(source_stream, destination, 1024 * 1024)
        elif board_profile == "raspberry-pi-zero-2w":
            if source.suffix != ".zip":
                raise DiskPackagingError("Raspberry parent must be an .img.zip asset")
            if imager_manifest is None:
                raise DiskPackagingError("Raspberry parent requires the exact standalone Imager manifest")
            with zipfile.ZipFile(source, "r") as archive:
                image_name = _validate_raspberry_archive(source, imager_manifest)
                info = archive.getinfo(image_name)
                with archive.open(info, "r") as source_stream, image.open("wb") as destination:
                    shutil.copyfileobj(source_stream, destination, 1024 * 1024)
        else:
            raise DiskPackagingError(f"unsupported board profile: {board_profile}")
        if image.is_symlink() or not image.is_file():
            raise DiskPackagingError("decompressed parent is not a regular image")
        return PreparedImage(work, image, source, parent_context, manifest_digest)
    except Exception:
        shutil.rmtree(work, ignore_errors=True)
        raise


def _output_stem(version: str, board_profile: str, artifact_kind: str = "runtime") -> str:
    if not version or "/" in version or "\\" in version:
        raise DiskPackagingError("derived output version is unsafe")
    if board_profile not in {"raspberry-pi-zero-2w", "orange-pi-zero-2w"}:
        raise DiskPackagingError("unsupported board profile")
    if artifact_kind not in {"runtime", "setup"}:
        raise DiskPackagingError("unsupported derived artifact kind")
    return f"octessera-{version}-{board_profile}-derived-{artifact_kind}-respin"


def _prepare_output(output: Path, expected_suffix: str, stem: str) -> Path:
    output = Path(output)
    if output.name != stem + expected_suffix or output.is_symlink() or output.exists():
        raise DiskPackagingError(f"output must be a new board-qualified derived artifact: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    return output


def package_derived(image: Path, output: Path, board_profile: str, version: str, artifact_kind: str = "runtime") -> Path:
    stem = _output_stem(version, board_profile, artifact_kind)
    suffix = ".img.xz" if board_profile == "orange-pi-zero-2w" else ".zip"
    output = _prepare_output(output, suffix, stem)
    temporary = output.with_name(f".{output.name}.tmp-{os.getpid()}")
    try:
        if board_profile == "orange-pi-zero-2w":
            with image.open("rb") as source, lzma.open(temporary, "wb", format=lzma.FORMAT_XZ, preset=9) as destination:
                shutil.copyfileobj(source, destination, 1024 * 1024)
        else:
            member = stem + ".img"
            info = zipfile.ZipInfo(member, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive, image.open("rb") as source, archive.open(info, "w", force_zip64=True) as destination:
                shutil.copyfileobj(source, destination, 1024 * 1024)
        os.replace(temporary, output)
        return output
    except Exception:
        temporary.unlink(missing_ok=True)
        output.unlink(missing_ok=True)
        raise


def provenance_sidecar(output: Path) -> Path:
    return Path(output).with_suffix(Path(output).suffix + ".provenance.json")


def compression_identity(board_profile: str) -> str:
    if board_profile == "orange-pi-zero-2w":
        return f"python-{sys.version_info.major}.{sys.version_info.minor}-lzma-xz-preset-9"
    return f"python-{sys.version_info.major}.{sys.version_info.minor}-zip-deflated-level-9-zlib-{zlib.ZLIB_VERSION}"


def write_derived_sidecar(output: Path, provenance: dict[str, Any]) -> Path:
    sidecar = provenance_sidecar(output)
    if sidecar.exists() or sidecar.is_symlink():
        raise DiskPackagingError(f"derived provenance output already exists: {sidecar}")
    try:
        write_provenance(sidecar, provenance)
    except OSError as exc:
        raise DiskPackagingError(f"cannot write derived provenance: {sidecar}") from exc
    return sidecar
