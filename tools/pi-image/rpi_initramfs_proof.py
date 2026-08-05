#!/usr/bin/env python3
from __future__ import annotations

import os
import hashlib
import stat
import subprocess
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any

MAX_ENTRY_BYTES = 64 * 1024 * 1024
MAX_TOTAL_BYTES = 256 * 1024 * 1024


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _safe_name(name: str) -> str:
    normalized = name.removeprefix("./")
    path = PurePosixPath(normalized)
    _require(bool(normalized) and not path.is_absolute() and ".." not in path.parts, f"unsafe initramfs path: {name}")
    return path.as_posix()


def _records(listing: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    seen: set[str] = set()
    for line in listing.splitlines():
        fields = line.split(maxsplit=8)
        if not fields or fields[0] == ".":
            continue
        _require(len(fields) == 9 and len(fields[0]) == 10, f"unparseable initramfs listing entry: {line}")
        try:
            links = int(fields[1])
            size = int(fields[4])
        except ValueError as error:
            raise ValueError(f"unparseable initramfs metadata: {line}") from error
        name = _safe_name(fields[8].split(" -> ", 1)[0])
        _require(name not in seen, f"duplicate initramfs entry: {name}")
        seen.add(name)
        records.append({"name": name, "mode": fields[0], "links": links, "size": size})
    return records


def _safe_extracted_path(root: Path, name: str) -> Path:
    path = root.joinpath(*PurePosixPath(name).parts)
    current = root
    for part in PurePosixPath(name).parts[:-1]:
        current /= part
        try:
            metadata = os.lstat(current)
        except OSError as error:
            raise ValueError(f"initramfs path component is missing: {name}") from error
        _require(stat.S_ISDIR(metadata.st_mode), f"initramfs path component is not a directory: {name}")
        _require(not stat.S_ISLNK(metadata.st_mode), f"initramfs path component is a symlink: {name}")
    return path


def extract_regular_files(path: Path, required_entries: list[str], run_listing: Any) -> dict[str, bytes]:
    listing = run_listing(path)
    records = _records(listing)
    by_name = {record["name"]: record for record in records}
    required = [_safe_name(entry) for entry in required_entries]
    _require(len(required) == len(set(required)), "required initramfs entries contain duplicates")
    for name in required:
        record = by_name.get(name)
        if record is None:
            raise ValueError(f"selected initramfs is missing constructor output: {name}")
        _require(record["mode"][0] == "-", f"selected initramfs entry is not regular: {name}")
        _require(record["links"] == 1, f"selected initramfs entry is a hardlink: {name}")
        _require(0 <= record["size"] <= MAX_ENTRY_BYTES, f"selected initramfs entry is oversized: {name}")
    total = sum(record["size"] for record in records if record["mode"][0] == "-")
    _require(total <= MAX_TOTAL_BYTES, "selected initramfs regular-file payload is oversized")
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-initramfs-") as temporary:
        destination = Path(temporary)
        try:
            subprocess.run(["unmkinitramfs", str(path), str(destination)], capture_output=True, text=True, check=True)
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ValueError(f"cannot extract initramfs {path} with unmkinitramfs") from error
        result: dict[str, bytes] = {}
        for name in required:
            extracted = _safe_extracted_path(destination, name)
            try:
                metadata = os.lstat(extracted)
            except OSError as error:
                raise ValueError(f"extracted initramfs entry is missing: {name}") from error
            _require(stat.S_ISREG(metadata.st_mode), f"extracted initramfs entry is not regular: {name}")
            _require(not stat.S_ISLNK(metadata.st_mode), f"extracted initramfs entry is a symlink: {name}")
            _require(metadata.st_nlink == 1, f"extracted initramfs entry is a hardlink: {name}")
            _require(metadata.st_size <= MAX_ENTRY_BYTES, f"extracted initramfs entry is oversized: {name}")
            try:
                data = extracted.read_bytes()
            except OSError as error:
                raise ValueError(f"cannot read extracted initramfs entry: {name}") from error
            _require(len(data) == by_name[name]["size"], f"extracted initramfs entry size changed: {name}")
            result[name] = data
        return result


def _resolve_rootfs_regular_file(root: Path, relative: str) -> Path:
    root = Path(os.path.abspath(root))
    components = list(PurePosixPath(relative).parts)
    current = root
    seen: set[Path] = set()
    for _ in range(32):
        if components:
            current /= components.pop(0)
        metadata = os.lstat(current)
        if stat.S_ISLNK(metadata.st_mode):
            if current in seen:
                raise ValueError(f"rootfs file contains a symlink loop: {relative}")
            seen.add(current)
            target = os.readlink(current)
            candidate = root / target.lstrip("/") if os.path.isabs(target) else current.parent / target
            candidate = Path(os.path.abspath(candidate))
            try:
                candidate.relative_to(root)
            except ValueError as error:
                raise ValueError(f"rootfs file escapes the image root: {relative}") from error
            components = list(candidate.relative_to(root).parts) + components
            current = root
            continue
        if components:
            _require(stat.S_ISDIR(metadata.st_mode), f"rootfs file parent is not a directory: {relative}")
            continue
        _require(stat.S_ISREG(metadata.st_mode), f"rootfs file is not regular: {relative}")
        _require(metadata.st_nlink == 1, f"rootfs file is a hardlink: {relative}")
        return current
    raise ValueError(f"rootfs file has too many symlink components: {relative}")


def compare_rootfs_files(root: Path, extracted: dict[str, bytes], pairs: tuple[tuple[str, str], ...]) -> None:
    for archive_path, root_path in pairs:
        expected = _resolve_rootfs_regular_file(root, root_path).read_bytes()
        actual = extracted[archive_path]
        _require(len(actual) == len(expected), f"selected initramfs size differs from rootfs: {root_path}")
        _require(hashlib.sha256(actual).digest() == hashlib.sha256(expected).digest(), f"selected initramfs hash differs from rootfs: {root_path}")
        _require(actual == expected, f"selected initramfs bytes differ from rootfs: {root_path}")
