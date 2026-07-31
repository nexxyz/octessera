from __future__ import annotations

import gzip
import lzma
import subprocess
from pathlib import Path


class KernelImageError(ValueError):
    pass


ARM64_IMAGE_MAGIC_OFFSET = 0x38
ARM64_IMAGE_MAGIC = b"ARM\x64"


def _external(data: bytes, command: str) -> bytes:
    try:
        return subprocess.run([command, "-d", "-c"], input=data, capture_output=True, check=True).stdout
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise KernelImageError(f"cannot decompress ARM64 kernel with {command}") from error


def firmware_kernel_bytes(data: bytes, label: str) -> tuple[bytes, str]:
    if data.startswith(b"\x1f\x8b"):
        data, compression = gzip.decompress(data), "gzip"
    elif data.startswith(b"\xfd7zXZ\x00"):
        data, compression = lzma.decompress(data), "xz"
    elif data.startswith(b"\x28\xb5\x2f\xfd"):
        data, compression = _external(data, "zstd"), "zstd"
    else:
        compression = "raw"
    if len(data) <= ARM64_IMAGE_MAGIC_OFFSET + len(ARM64_IMAGE_MAGIC) or data[ARM64_IMAGE_MAGIC_OFFSET : ARM64_IMAGE_MAGIC_OFFSET + 4] != ARM64_IMAGE_MAGIC:
        raise KernelImageError(f"kernel {label} is not an ARM64 firmware-bootable Image")
    return data, compression


def assert_firmware_kernel(path: Path) -> tuple[bytes, str]:
    try:
        return firmware_kernel_bytes(path.read_bytes(), str(path))
    except OSError as error:
        raise KernelImageError(f"cannot read kernel {path}: {error}") from error
