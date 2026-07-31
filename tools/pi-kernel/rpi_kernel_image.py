from __future__ import annotations

import gzip
import lzma
import subprocess
from pathlib import Path


class KernelImageError(ValueError):
    pass


ARM64_IMAGE_MAGIC_OFFSET = 0x38
ARM64_IMAGE_MAGIC = b"ARM\x64"


def _decompress_external(data: bytes, command: str) -> bytes:
    try:
        result = subprocess.run(
            [command, "-d", "-c"],
            input=data,
            capture_output=True,
            check=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise KernelImageError(f"cannot decompress ARM64 kernel with {command}") from error
    return result.stdout


def firmware_kernel_bytes(data: bytes, label: str) -> tuple[bytes, str]:
    if data.startswith(b"\x1f\x8b"):
        try:
            data = gzip.decompress(data)
        except (OSError, EOFError) as error:
            raise KernelImageError(f"cannot decompress kernel {label}") from error
        compression = "gzip"
    elif data.startswith(b"\xfd7zXZ\x00"):
        try:
            data = lzma.decompress(data)
        except (lzma.LZMAError, EOFError) as error:
            raise KernelImageError(f"cannot decompress kernel {label}") from error
        compression = "xz"
    elif data.startswith(b"\x28\xb5\x2f\xfd"):
        data = _decompress_external(data, "zstd")
        compression = "zstd"
    else:
        compression = "raw"
    if len(data) <= ARM64_IMAGE_MAGIC_OFFSET + len(ARM64_IMAGE_MAGIC):
        raise KernelImageError(f"kernel {label} is too short for an ARM64 Image header")
    actual = data[ARM64_IMAGE_MAGIC_OFFSET : ARM64_IMAGE_MAGIC_OFFSET + len(ARM64_IMAGE_MAGIC)]
    if actual != ARM64_IMAGE_MAGIC:
        raise KernelImageError(f"kernel {label} is not an ARM64 firmware-bootable Image")
    return data, compression


def assert_firmware_kernel(path: Path) -> tuple[bytes, str]:
    try:
        data = path.read_bytes()
    except OSError as error:
        raise KernelImageError(f"cannot read kernel {path}: {error}") from error
    return firmware_kernel_bytes(data, str(path))
