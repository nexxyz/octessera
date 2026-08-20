from __future__ import annotations

from pathlib import Path
from typing import Any

try:
    from .record_hashing import regular_file_digest
    from .record_validation import RecordError, SHA_RE, require, require_keys
except ImportError:
    from record_hashing import regular_file_digest
    from record_validation import RecordError, SHA_RE, require, require_keys


def project_path(path: Path, root: Path) -> tuple[Path, str]:
    root = root.resolve()
    candidate = path if path.is_absolute() else Path.cwd() / path
    require(not candidate.is_symlink(), f"recorded path is a symlink: {path}")
    candidate = candidate.resolve(strict=True)
    try:
        relative = candidate.relative_to(root)
    except ValueError as error:
        raise RecordError(f"recorded path is outside project root: {path}") from error
    relative_text = relative.as_posix()
    require(relative_text not in {"", "."} and not relative_text.startswith("../"), f"recorded path is not project-relative: {path}")
    return candidate, relative_text


def identity(path: Path, root: Path) -> dict[str, Any]:
    candidate, relative = project_path(path, root)
    digest, size = regular_file_digest(candidate, "recorded path")
    return {"path": relative, "sha256": digest, "size": size}


def resolve(root: Path, relative: str) -> Path:
    path = Path(relative)
    require(not path.is_absolute() and ".." not in path.parts, f"record path is not project-relative: {relative}")
    return root / path


def verify_identity(value: Any, root: Path, label: str) -> dict[str, Any]:
    record = require_keys(value, {"path", "sha256", "size"}, label)
    require(isinstance(record["path"], str) and bool(record["path"]), f"{label} path is invalid")
    require(isinstance(record["sha256"], str) and SHA_RE.fullmatch(record["sha256"]) is not None, f"{label} digest is invalid")
    require(isinstance(record["size"], int) and record["size"] >= 0, f"{label} size is invalid")
    actual = identity(resolve(root, record["path"]), root)
    require(actual == record, f"{label} changed: {record['path']}")
    return record
