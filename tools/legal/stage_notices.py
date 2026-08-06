from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import tempfile
from pathlib import Path
from typing import Any


MANIFEST_RELATIVE = Path("resources/legal/notice-bundle.json")
SCHEMA = "octessera.legal-notice-bundle/v1"
DESTINATION_ROOT = "/usr/share/doc/octessera"
FILE_KEYS = {"source", "destination", "sha256", "size"}
TOP_KEYS = {"schema", "schema_version", "destination_root", "files"}
OWNERSHIP_POLICIES = {"root", "filesystem"}


class NoticeStageError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise NoticeStageError(message)


def _safe_relative(value: Any, label: str) -> Path:
    _require(bool(isinstance(value, str) and value.strip()), f"{label} is invalid")
    normalized = value.replace("\\", "/")
    path = Path(normalized)
    _require(not path.is_absolute() and not path.drive and re.match(r"^[A-Za-z]:/", normalized) is None, f"{label} is absolute")
    _require(path != Path(".") and ".." not in path.parts and all(part not in ("", ".") for part in path.parts), f"{label} escapes its root")
    return path


def _source_identity(path: Path, label: str) -> tuple[str, int]:
    _require(path.exists() and not path.is_symlink(), f"{label} is missing or symlinked: {path}")
    metadata = path.lstat()
    _require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1, f"{label} is not a regular single-link file: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest(), metadata.st_size


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise NoticeStageError(f"legal notice manifest is unreadable: {path}") from error
    _require(isinstance(document, dict) and set(document) == TOP_KEYS, "legal notice manifest keys are not exact")
    _require(document["schema"] == SCHEMA and document["schema_version"] == 1 and document["destination_root"] == DESTINATION_ROOT, "legal notice manifest identity is not exact")
    files = document["files"]
    _require(bool(isinstance(files, list) and files), "legal notice manifest files are empty")
    destinations: set[str] = set()
    sources: set[str] = set()
    for index, item in enumerate(files):
        _require(isinstance(item, dict) and set(item) == FILE_KEYS, f"legal notice file entry {index} keys are not exact")
        source = _safe_relative(item["source"], f"file entry {index} source")
        destination = _safe_relative(item["destination"], f"file entry {index} destination")
        source_text = source.as_posix()
        destination_text = destination.as_posix()
        _require(source_text not in sources, f"duplicate legal notice source: {source_text}")
        _require(destination_text not in destinations, f"duplicate legal notice destination: {destination_text}")
        _require(isinstance(item["sha256"], str) and len(item["sha256"]) == 64 and all(character in "0123456789abcdef" for character in item["sha256"]), f"invalid legal notice digest: {destination_text}")
        _require(isinstance(item["size"], int) and item["size"] >= 0, f"invalid legal notice size: {destination_text}")
        sources.add(source_text)
        destinations.add(destination_text)
    return document


def _root_owned(path: Path) -> bool:
    metadata = path.lstat()
    if hasattr(metadata, "st_uid"):
        return metadata.st_uid == 0 and metadata.st_gid == 0
    return True


def _require_directory(path: Path, label: str, create: bool) -> None:
    if path.exists() or path.is_symlink():
        _require(path.is_dir() and not path.is_symlink(), f"{label} is not a real directory: {path}")
    elif create:
        path.mkdir()
    else:
        raise NoticeStageError(f"{label} is missing: {path}")


def _ensure_tree(root: Path, relative: Path, create: bool) -> Path:
    current = root
    for part in relative.parts:
        current = current / part
        _require_directory(current, "legal notice directory", create)
    return current


def _verify_output(path: Path, data: bytes, label: str, ownership: str) -> None:
    _require(path.exists() and not path.is_symlink(), f"{label} is missing or symlinked: {path}")
    metadata = path.lstat()
    _require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1, f"{label} is not a regular single-link file: {path}")
    if os.name != "nt":
        _require(metadata.st_mode & 0o777 == 0o644, f"{label} mode is not 0644: {path}")
    if ownership == "root":
        _require(_root_owned(path), f"{label} is not root-owned: {path}")
    _require(path.read_bytes() == data, f"{label} content is stale: {path}")


def _reject_unknown_outputs(root: Path, expected: set[str]) -> None:
    if not root.exists():
        return
    allowed_directories: set[str] = set()
    for relative in expected:
        parts = relative.split("/")[:-1]
        for index in range(1, len(parts) + 1):
            allowed_directories.add("/".join(parts[:index]))
    for path in root.rglob("*"):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            raise NoticeStageError(f"legal notice tree contains a symlink: {path}")
        if path.is_dir() and relative not in allowed_directories:
            raise NoticeStageError(f"legal notice tree contains an unknown directory: {path}")
        if path.is_file() and relative not in expected:
            raise NoticeStageError(f"legal notice tree contains an unknown output: {path}")


def stage_notices(repository_root: Path, destination_root: Path, manifest_path: Path | None = None, check: bool = False, ownership: str = "root") -> None:
    _require(ownership in OWNERSHIP_POLICIES, f"invalid ownership policy: {ownership}")
    repository_root = repository_root.resolve()
    destination_root = destination_root.resolve()
    if not destination_root.exists() and not check:
        destination_root.mkdir(parents=True)
    _require(destination_root.is_dir() and not destination_root.is_symlink(), f"destination root is not a real directory: {destination_root}")
    manifest = load_manifest(manifest_path or repository_root / MANIFEST_RELATIVE)
    entries = manifest["files"]
    source_data: list[tuple[Path, Path, bytes]] = []
    for item in entries:
        source = repository_root / _safe_relative(item["source"], "source")
        digest, size = _source_identity(source, "legal notice source")
        _require(digest == item["sha256"] and size == item["size"], f"legal notice source identity changed: {source}")
        source_data.append((source, _safe_relative(item["destination"], "destination"), source.read_bytes()))
    legal_root = destination_root / "usr/share/doc/octessera"
    expected = {destination.as_posix() for _, destination, _ in source_data}
    if check:
        _require(legal_root.is_dir() and not legal_root.is_symlink(), f"staged legal tree is missing: {legal_root}")
    else:
        _ensure_tree(destination_root, Path("usr/share/doc/octessera"), True)
    _reject_unknown_outputs(legal_root, expected)
    for source, destination, data in source_data:
        target = legal_root / destination
        parent = target.parent
        if check:
            _require(parent.is_dir() and not parent.is_symlink(), f"staged legal parent is missing: {parent}")
        else:
            _ensure_tree(legal_root, destination.parent, True)
        if target.exists() or target.is_symlink():
            _verify_output(target, data, "staged legal file", ownership)
        elif check:
            raise NoticeStageError(f"staged legal file is missing: {target}")
        else:
            descriptor, temporary_name = tempfile.mkstemp(prefix=f".{target.name}.", dir=parent)
            temporary = Path(temporary_name)
            try:
                with os.fdopen(descriptor, "wb") as stream:
                    stream.write(data)
                    stream.flush()
                    os.fsync(stream.fileno())
                os.chmod(temporary, 0o644)
                if ownership == "root" and hasattr(os, "chown"):
                    os.chown(temporary, 0, 0)  # type: ignore[attr-defined]
                os.replace(temporary, target)
            finally:
                if temporary.exists() or temporary.is_symlink():
                    temporary.unlink()
            _verify_output(target, data, "staged legal file", ownership)


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage the canonical Octessera legal notice bundle.")
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--destination-root", type=Path, required=True)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--ownership", choices=sorted(OWNERSHIP_POLICIES), default="root")
    arguments = parser.parse_args()
    try:
        stage_notices(arguments.repository_root, arguments.destination_root, arguments.manifest, arguments.check, arguments.ownership)
    except (OSError, NoticeStageError) as error:
        print(f"Legal notice staging failed: {error}")
        return 1
    print("Legal notice staging passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
