from __future__ import annotations

import json
from pathlib import Path
from typing import Any, cast

try:
    from .record_hashing import canonical_bytes
    from .record_validation import RecordError, require
except ImportError:
    from record_hashing import canonical_bytes
    from record_validation import RecordError, require


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RecordError(f"record is not valid JSON: {path}") from error
    require(isinstance(value, dict), f"record is not an object: {path}")
    return cast(dict[str, Any], value)


def write_new(path: Path, value: dict[str, Any]) -> None:
    require(not path.exists() and not path.is_symlink(), f"record output already exists: {path}")
    path.write_bytes(canonical_bytes(value))
