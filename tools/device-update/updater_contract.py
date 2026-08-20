#!/usr/bin/env python3
import json
import os
import re
from pathlib import Path


VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
BINARY = "octessera-pi"
MANIFEST = "update-manifest.json"
MARKER_SCHEMA = CANDIDATE_HEALTH_PROTOCOL = 1
MANIFEST_SCHEMA = UPDATER_PROTOCOL = 2
MAX_JSON_BYTES = 2 * 1024 * 1024
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_SUMS_BYTES = 2 * 1024 * 1024
MAX_ENTRY_BYTES = 128 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED_BYTES = 128 * 1024 * 1024
MAX_ZIP_ENTRIES = 16
LOCK_TIMEOUT_SECONDS = 10


class UpdateError(Exception):
    pass


def version(value: object) -> bool:
    return isinstance(value, str) and bool(VERSION_RE.fullmatch(value))


def same_path(left: Path, right: Path) -> bool:
    normalize = lambda value: os.path.normcase(os.path.normpath(str(value).removeprefix("\\\\?\\")))
    return normalize(left) == normalize(right)


def read_json(path: Path, max_bytes: int | None = None) -> object:
    try:
        if max_bytes is not None and path.stat().st_size > max_bytes:
            raise UpdateError(f"JSON exceeds size limit: {path}")
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError) as exc:
        raise UpdateError(f"Invalid JSON: {path}") from exc


def atomic_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}-{os.urandom(4).hex()}")
    payload = (json.dumps(value, indent=2) + "\n").encode("utf-8")
    try:
        with temporary.open("wb") as handle:
            os.chmod(temporary, 0o644)
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        try:
            directory = os.open(path.parent, os.O_RDONLY)
        except OSError:
            directory = -1
        if directory >= 0:
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def atomic_symlink(path: Path, target: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}-{os.urandom(4).hex()}")
    try:
        os.symlink(target, temporary)
        try:
            os.replace(temporary, path)
        except PermissionError:
            if os.name != "nt":
                raise
            path.unlink(missing_ok=True)
            os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)
