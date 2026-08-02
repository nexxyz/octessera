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
    if raw[:4] == b"\x27\x05\x19\x56" and len(raw) >= 64:
        size = struct.unpack_from(">I", raw, 12)[0]
        raw = raw[64 : 64 + size]
    if raw.startswith(b"\x28\xb5\x2f\xfd"):
        try:
            return subprocess.run(["zstd", "-q", "-dc"], input=raw, capture_output=True, check=True).stdout
        except (FileNotFoundError, OSError, subprocess.CalledProcessError) as error:
            raise InitramfsDecodeError(f"cannot decompress zstd initramfs: {path}") from error
    for decoder in (gzip.decompress, lzma.decompress):
        try:
            return decoder(raw)
        except (OSError, EOFError, lzma.LZMAError):
            continue
    return raw
