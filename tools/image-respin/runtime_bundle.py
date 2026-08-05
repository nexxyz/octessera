from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import struct
import sys
from pathlib import Path


BOARDS = ("raspberry-pi-zero-2w", "orange-pi-zero-2w")
VERSION_PATTERN = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
VERSION_RE = re.compile(rf"^{VERSION_PATTERN}$")
ENTRY_NAMES = ("SHA256SUMS", "octessera-pi", "octessera-runtime.json")


class RuntimeBundleError(ValueError):
    pass


def _regular_file(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise RuntimeBundleError(f"{label} does not exist: {path}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise RuntimeBundleError(f"{label} is not a regular file: {path}")


def validate_elf64_aarch64(path: Path) -> bytes:
    _regular_file(path, "runtime binary")
    try:
        payload = path.read_bytes()
    except OSError as error:
        raise RuntimeBundleError(f"cannot read runtime binary: {path}") from error
    if len(payload) < 64 or payload[:4] != b"\x7fELF":
        raise RuntimeBundleError("runtime binary is not an ELF file")
    if payload[4:7] != b"\x02\x01\x01":
        raise RuntimeBundleError("runtime binary is not a little-endian ELF64 file")
    if struct.unpack_from("<H", payload, 18)[0] != 183:
        raise RuntimeBundleError("runtime binary is not AArch64")
    header_size = struct.unpack_from("<H", payload, 52)[0]
    if header_size != 64:
        raise RuntimeBundleError("runtime binary has a malformed ELF64 header")
    program_offset = struct.unpack_from("<Q", payload, 32)[0]
    program_entry_size = struct.unpack_from("<H", payload, 54)[0]
    program_count = struct.unpack_from("<H", payload, 56)[0]
    if program_count and (
        program_entry_size < 56
        or program_offset > len(payload)
        or program_offset + program_entry_size * program_count > len(payload)
    ):
        raise RuntimeBundleError("runtime binary has truncated ELF program headers")
    return payload


def _write_exact(path: Path, payload: bytes, mode: int) -> None:
    if path.exists() or path.is_symlink():
        raise RuntimeBundleError(f"runtime bundle output already exists: {path}")
    path.write_bytes(payload)
    os.chmod(path, mode)


def _validate_bundle(output: Path, version: str, board: str, binary_hash: str) -> None:
    if output.is_symlink() or not output.is_dir():
        raise RuntimeBundleError("runtime bundle output is not a real directory")
    directory_mode = stat.S_IMODE(output.stat().st_mode)
    if directory_mode != 0o755 and not (os.name == "nt" and directory_mode in {0o755, 0o777}):
        raise RuntimeBundleError("runtime bundle directory mode is not 0755")
    entries = sorted(path.name for path in output.iterdir())
    if entries != sorted(ENTRY_NAMES):
        raise RuntimeBundleError("runtime bundle entries are not exact")
    binary = output / "octessera-pi"
    metadata = output / "octessera-runtime.json"
    sums = output / "SHA256SUMS"
    for path in (binary, metadata, sums):
        _regular_file(path, "runtime bundle entry")
    binary_mode = stat.S_IMODE(binary.stat().st_mode)
    if binary_mode != 0o755 and not (os.name == "nt" and binary_mode in {0o644, 0o666}):
        raise RuntimeBundleError("runtime binary mode is not 0755")
    metadata_mode = stat.S_IMODE(metadata.stat().st_mode)
    sums_mode = stat.S_IMODE(sums.stat().st_mode)
    if os.name != "nt" and (metadata_mode != 0o644 or sums_mode != 0o644):
        raise RuntimeBundleError("runtime metadata modes are not 0644")
    if os.name == "nt" and (metadata_mode not in {0o644, 0o666} or sums_mode not in {0o644, 0o666}):
        raise RuntimeBundleError("runtime metadata modes are not 0644")
    expected_metadata = {
        "artifact_kind": "production-runtime",
        "binary_sha256": binary_hash,
        "name": "octessera-pi",
        "profile": board,
        "runtime_ready": True,
        "version": version,
    }
    expected_metadata_bytes = (json.dumps(expected_metadata, sort_keys=True, indent=2) + "\n").encode("utf-8")
    try:
        metadata_bytes = metadata.read_bytes()
        actual_metadata = json.loads(metadata_bytes.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeBundleError("runtime metadata is malformed") from error
    if not isinstance(actual_metadata, dict) or actual_metadata != expected_metadata or metadata_bytes != expected_metadata_bytes:
        raise RuntimeBundleError("runtime metadata is not exact")
    if sums.read_text(encoding="utf-8") != f"{binary_hash}  octessera-pi\n":
        raise RuntimeBundleError("runtime checksum manifest is not exact")


def create_bundle(binary: Path, board: str, version: str, output: Path) -> Path:
    if board not in BOARDS:
        raise RuntimeBundleError(f"unsupported board profile: {board}")
    if not VERSION_RE.fullmatch(version):
        raise RuntimeBundleError("runtime version must be strict semver MAJOR.MINOR.PATCH")
    payload = validate_elf64_aarch64(binary)
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise RuntimeBundleError(f"runtime bundle output already exists: {output}")
    try:
        output.mkdir(parents=True)
        os.chmod(output, 0o755)
        binary_hash = hashlib.sha256(payload).hexdigest()
        _write_exact(output / "octessera-pi", payload, 0o755)
        metadata = {
            "artifact_kind": "production-runtime",
            "binary_sha256": binary_hash,
            "name": "octessera-pi",
            "profile": board,
            "runtime_ready": True,
            "version": version,
        }
        _write_exact(
            output / "octessera-runtime.json",
            (json.dumps(metadata, sort_keys=True, indent=2) + "\n").encode("utf-8"),
            0o644,
        )
        _write_exact(output / "SHA256SUMS", f"{binary_hash}  octessera-pi\n".encode(), 0o644)
        _validate_bundle(output, version, board, binary_hash)
        return output
    except Exception:
        shutil.rmtree(output, ignore_errors=True)
        raise


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Create an exact Octessera production runtime bundle.")
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--board", choices=BOARDS, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        create_bundle(arguments.binary, arguments.board, arguments.version, arguments.output)
    except (OSError, RuntimeBundleError) as error:
        print(f"runtime bundle rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
