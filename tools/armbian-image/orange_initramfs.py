from __future__ import annotations

import gzip
import lzma
import struct
import subprocess
from pathlib import Path


class InitramfsDecodeError(ValueError):
    pass


def read_initramfs_content(path: Path) -> bytes:
    raw = path.read_bytes()
    if raw[:4] == b"\x27\x05\x19\x56":
        if len(raw) < 64:
            raise InitramfsDecodeError(f"truncated U-Boot initramfs header: {path}")
        size = struct.unpack_from(">I", raw, 12)[0]
        if size > len(raw) - 64:
            raise InitramfsDecodeError(f"truncated U-Boot initramfs payload: {path}")
        raw = raw[64 : 64 + size]
    decoded = raw
    if raw.startswith(b"\x28\xb5\x2f\xfd"):
        try:
            decoded = subprocess.run(["zstd", "-q", "-dc"], input=raw, capture_output=True, check=True).stdout
        except (FileNotFoundError, OSError, subprocess.CalledProcessError) as error:
            raise InitramfsDecodeError(f"cannot decompress zstd initramfs: {path}") from error
    elif raw.startswith(b"\x1f\x8b"):
        try:
            decoded = gzip.decompress(raw)
        except (OSError, EOFError) as error:
            raise InitramfsDecodeError(f"cannot decompress gzip initramfs: {path}") from error
    elif raw.startswith(b"\xfd7zXZ\x00"):
        try:
            decoded = lzma.decompress(raw)
        except (OSError, EOFError, lzma.LZMAError) as error:
            raise InitramfsDecodeError(f"cannot decompress xz initramfs: {path}") from error
    if not decoded.startswith((b"070701", b"070702", b"070707")):
        raise InitramfsDecodeError(f"decoded initramfs is not a supported CPIO archive: {path}")
    if b"TRAILER!!!" not in decoded:
        raise InitramfsDecodeError(f"decoded initramfs has no CPIO trailer: {path}")
    try:
        subprocess.run(["cpio", "--quiet", "-t"], input=decoded, capture_output=True, check=True)
    except (FileNotFoundError, OSError, subprocess.CalledProcessError) as error:
        raise InitramfsDecodeError(f"cannot validate CPIO initramfs: {path}") from error
    return decoded
