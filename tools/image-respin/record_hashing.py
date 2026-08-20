from __future__ import annotations

import hashlib
import json
import stat
from pathlib import Path

try:
    from .record_validation import RecordError, require
except ImportError:
    from record_validation import RecordError, require


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n").encode("utf-8")


def digest_object(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def regular_file_digest(path: Path, label: str) -> tuple[str, int]:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise RecordError(f"{label} is unavailable: {path}") from error
    require(not stat.S_ISLNK(metadata.st_mode) and stat.S_ISREG(metadata.st_mode), f"{label} is not a regular file: {path}")
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                digest.update(chunk)
    except OSError as error:
        raise RecordError(f"cannot read {label}: {path}") from error
    return digest.hexdigest(), size
