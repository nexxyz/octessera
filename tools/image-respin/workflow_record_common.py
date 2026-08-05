from __future__ import annotations

import hashlib
import json
import re
import stat
from pathlib import Path
from typing import Any, cast


class RecordError(ValueError):
    pass


SHA_RE = re.compile(r"^[0-9a-f]{64}$")
SOURCE_RE = re.compile(r"^[0-9a-f]{40}$")
VERSION_RE = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
DOCKER_ID_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
DOCKER_DIGEST_RE = re.compile(r"^[^\s/@]+(?:/[^\s/@]+)*@sha256:[0-9a-f]{64}$")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RecordError(message)


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n").encode("utf-8")


def digest_object(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def require_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    require(isinstance(value, dict) and set(value) == keys, f"{label} keys are not exact")
    return cast(dict[str, Any], value)


def _regular_file(path: Path, label: str) -> tuple[str, int]:
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
    digest, size = _regular_file(candidate, "recorded path")
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


def verify_docker_id(value: str, label: str) -> None:
    require(DOCKER_ID_RE.fullmatch(value) is not None, f"{label} is not a Docker image ID")


def verify_docker_digests(values: Any, label: str, *, required: bool) -> None:
    require(isinstance(values, list), f"{label} digests are not an array")
    require((bool(values) or not required) and all(isinstance(value, str) and DOCKER_DIGEST_RE.fullmatch(value) is not None for value in values), f"{label} digests are invalid")


def verify_source(source_sha: str, version: str, board: str, boards: set[str]) -> None:
    require(SOURCE_RE.fullmatch(source_sha) is not None, "source SHA is invalid")
    require(VERSION_RE.fullmatch(version) is not None, "version is not strict semver")
    require(board in boards, f"unsupported board: {board}")


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


def tool_identity(module_path: Path, root: Path, relative_path: str, name: str) -> dict[str, Any]:
    record = identity(module_path, root)
    return {"name": name, "version": 1, "path": relative_path, "sha256": record["sha256"], "size": record["size"]}


def verify_tool(record: Any, module_path: Path, root: Path, relative_path: str, name: str) -> None:
    tool = require_keys(record, {"name", "version", "path", "sha256", "size"}, "tool")
    require(tool["name"] == name and tool["version"] == 1 and tool["path"] == relative_path, "tool identity is not exact")
    require(tool["sha256"] == identity(module_path, root)["sha256"] and tool["size"] == identity(module_path, root)["size"], "record tool changed")
