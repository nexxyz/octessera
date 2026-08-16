#!/usr/bin/env python3
"""Verify and stage the canonical Octessera sample library."""

from __future__ import annotations

import argparse
import csv
import hashlib
import shutil
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable
from urllib.parse import quote


MEDIA_SUFFIXES = frozenset({".aif", ".aiff", ".flac", ".mp3", ".ogg", ".wav"})
EXPECTED_MEDIA_COUNT = 320
INVENTORY_HEADER = (
    "path",
    "size",
    "sha256",
    "upstream_repository",
    "upstream_commit",
    "upstream_path",
    "source_url",
    "license",
    "license_url",
)
MANIFEST_HEADER = ("# path", "size", "sha256", "source", "license_source")
SUPPORT_FILES = ("ATTRIBUTIONS.tsv", "upstream/LICENSE", "upstream/README.txt")
UPSTREAM_REPOSITORY = "stargatedaw/stargate-sample-pack"
UPSTREAM_COMMIT = "dbfd6ec52d4ed53b60bdbea5fc6adf295127c027"
RAW_ROOT = f"https://raw.githubusercontent.com/{UPSTREAM_REPOSITORY}/"
LICENSE = "CC0-1.0"


class SampleLibraryError(ValueError):
    pass


@dataclass(frozen=True)
class InventoryRecord:
    path: str
    size: int
    sha256: str
    upstream_repository: str
    upstream_commit: str
    upstream_path: str
    source_url: str
    license: str
    license_url: str


SampleRecord = InventoryRecord


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise SampleLibraryError(message)


def _safe_relative(value: str, label: str) -> str:
    normalized = value.replace("\\", "/")
    parts = normalized.split("/")
    _require(
        bool(normalized)
        and not normalized.startswith("/")
        and not (len(parts[0]) == 2 and parts[0][1] == ":")
        and all(part not in {"", ".", ".."} for part in parts)
        and not any(ord(character) < 32 for character in normalized),
        f"unsafe {label}: {value!r}",
    )
    return normalized


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _regular_file(path: Path, label: str) -> None:
    _require(path.exists() and not path.is_symlink(), f"{label} is missing or symlinked: {path}")
    metadata = path.lstat()
    _require(stat.S_ISREG(metadata.st_mode), f"{label} is not a regular file: {path}")


def _tree_files(root: Path) -> list[Path]:
    _require(root.is_dir() and not root.is_symlink(), f"sample root is not a real directory: {root}")
    files: list[Path] = []

    def visit(directory: Path) -> None:
        for entry in sorted(directory.iterdir(), key=lambda path: path.name):
            relative = entry.relative_to(root).as_posix()
            _require(not entry.is_symlink(), f"sample tree contains a symlink: {relative}")
            metadata = entry.lstat()
            if stat.S_ISDIR(metadata.st_mode):
                visit(entry)
            else:
                _require(stat.S_ISREG(metadata.st_mode), f"sample tree contains a special file: {relative}")
                files.append(entry)

    visit(root)
    return files


def media_files(root: Path) -> list[Path]:
    return sorted(path for path in _tree_files(root) if path.suffix.lower() in MEDIA_SUFFIXES)


def read_inventory(path: Path) -> list[InventoryRecord]:
    _regular_file(path, "sample inventory")
    with path.open(encoding="utf-8", newline="") as stream:
        records = list(csv.reader(stream, delimiter="\t", strict=True))
    _require(bool(records) and tuple(records[0]) == INVENTORY_HEADER, "sample inventory header is not exact")
    result: list[InventoryRecord] = []
    for line_number, row in enumerate(records[1:], start=2):
        _require(len(row) == len(INVENTORY_HEADER) and all(row), f"malformed sample inventory row {line_number}")
        values = dict(zip(INVENTORY_HEADER, row))
        relative = _safe_relative(values["path"], "sample path")
        _require(values["size"].isdigit(), f"invalid sample size: {relative}")
        _require(len(values["sha256"]) == 64 and all(character in "0123456789abcdef" for character in values["sha256"]), f"invalid sample hash: {relative}")
        upstream_path = _safe_relative(values["upstream_path"], "upstream path")
        _require(values["upstream_repository"] == UPSTREAM_REPOSITORY, f"unexpected upstream repository: {relative}")
        _require(values["upstream_commit"] == UPSTREAM_COMMIT, f"unexpected upstream commit: {relative}")
        _require(values["license"] == LICENSE, f"unexpected sample license: {relative}")
        _require(values["source_url"] == f"{RAW_ROOT}{UPSTREAM_COMMIT}/{quote(upstream_path, safe='/')}", f"unexpected sample source URL: {relative}")
        _require(values["license_url"] == f"{RAW_ROOT}{UPSTREAM_COMMIT}/LICENSE", f"unexpected sample license URL: {relative}")
        result.append(
            InventoryRecord(
                relative,
                int(values["size"]),
                values["sha256"],
                values["upstream_repository"],
                values["upstream_commit"],
                values["upstream_path"],
                values["source_url"],
                values["license"],
                values["license_url"],
            )
        )
    return result


def verify_library(sample_root: Path, inventory_path: Path) -> list[InventoryRecord]:
    records = read_inventory(inventory_path)
    _require(len(records) == EXPECTED_MEDIA_COUNT, f"expected {EXPECTED_MEDIA_COUNT} sample records, found {len(records)}")
    paths = [record.path for record in records]
    _require(len(paths) == len(set(paths)), "sample inventory contains duplicate paths")
    _require(len([record.upstream_path for record in records]) == len({record.upstream_path for record in records}), "sample inventory contains duplicate upstream paths")
    for record in records:
        sample = sample_root.joinpath(*record.path.split("/"))
        _regular_file(sample, "sample")
        _require(sample.stat().st_size == record.size, f"sample size is stale: {record.path}")
        _require(_digest(sample) == record.sha256, f"sample hash is stale: {record.path}")

    files = _tree_files(sample_root)
    actual_media = {path.relative_to(sample_root).as_posix() for path in files if path.suffix.lower() in MEDIA_SUFFIXES}
    _require(actual_media == set(paths), f"sample media tree is not exact: missing={sorted(set(paths) - actual_media)!r} extra={sorted(actual_media - set(paths))!r}")
    actual_files = {path.relative_to(sample_root).as_posix() for path in files}
    expected_files = set(paths) | set(SUPPORT_FILES)
    _require(actual_files == expected_files, f"sample tree is not exact: missing={sorted(expected_files - actual_files)!r} extra={sorted(actual_files - expected_files)!r}")
    return records


def verify_media_tree(media_root: Path, records: Iterable[InventoryRecord], allowed_empty_directories: Iterable[str] = ()) -> None:
    expected = {record.path: record for record in records}
    files = _tree_files(media_root)
    actual = {path.relative_to(media_root).as_posix() for path in files}
    _require(actual == set(expected), f"staged sample tree is not exact: missing={sorted(set(expected) - actual)!r} extra={sorted(actual - set(expected))!r}")
    expected_directories = {
        part
        for relative in expected
        for index in range(1, len(relative.split("/")))
        for part in ["/".join(relative.split("/")[:index])]
    } | set(allowed_empty_directories)
    actual_directories = {
        path.relative_to(media_root).as_posix()
        for path in media_root.rglob("*")
        if path.is_dir() and not path.is_symlink()
    }
    _require(actual_directories == expected_directories, f"staged sample directories are not exact: missing={sorted(expected_directories - actual_directories)!r} extra={sorted(actual_directories - expected_directories)!r}")
    for relative, record in expected.items():
        path = media_root.joinpath(*relative.split("/"))
        _require(path.stat().st_size == record.size and _digest(path) == record.sha256, f"staged sample identity is stale: {relative}")


def verify_metadata_tree(metadata_root: Path, source_root: Path) -> None:
    expected = set(SUPPORT_FILES) | {"sample-manifest.tsv"}
    files = _tree_files(metadata_root)
    media_directory = metadata_root / "files"
    if media_directory.exists() or media_directory.is_symlink():
        _require(media_directory.is_dir() and not media_directory.is_symlink(), "staged sample media directory is missing")
    actual = {
        path.relative_to(metadata_root).as_posix()
        for path in files
        if not media_directory.exists() or not path.relative_to(metadata_root).as_posix().startswith("files/")
    }
    _require(actual == expected, f"staged sample metadata is not exact: missing={sorted(expected - actual)!r} extra={sorted(actual - expected)!r}")
    for relative in SUPPORT_FILES:
        source = source_root.joinpath(*relative.split("/"))
        target = metadata_root.joinpath(*relative.split("/"))
        _require(target.read_bytes() == source.read_bytes(), f"staged sample metadata is stale: {relative}")


def _copy_files(source_root: Path, destination_root: Path, relative_paths: Iterable[str]) -> None:
    _require(not destination_root.exists() or not destination_root.is_symlink(), f"sample destination is symlinked: {destination_root}")
    if destination_root.exists():
        shutil.rmtree(destination_root)
    destination_root.mkdir(parents=True)
    for relative in relative_paths:
        source = source_root.joinpath(*relative.split("/"))
        target = destination_root.joinpath(*relative.split("/"))
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)


def validate_staging_destinations(repository_root: Path, destinations: Iterable[Path]) -> None:
    source_root = (repository_root / "samples").absolute().resolve(strict=False)
    resolved_destinations = [Path(destination).absolute().resolve(strict=False) for destination in destinations]
    for destination in resolved_destinations:
        if destination == source_root or source_root in destination.parents or destination in source_root.parents:
            raise SampleLibraryError(f"sample staging destination overlaps canonical source: {destination}")


def write_manifest(records: Iterable[InventoryRecord], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(MANIFEST_HEADER)
        for record in sorted(records, key=lambda item: item.path):
            writer.writerow((record.path, record.size, record.sha256, record.source_url, record.license_url))


def verify_manifest(path: Path, records: Iterable[InventoryRecord]) -> None:
    _regular_file(path, "sample manifest")
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.reader(stream, delimiter="\t", strict=True))
    expected = [["# path", "size", "sha256", "source", "license_source"]]
    expected.extend(
        [record.path, str(record.size), record.sha256, record.source_url, record.license_url]
        for record in sorted(records, key=lambda item: item.path)
    )
    _require(rows == expected, "staged sample manifest is not exact")


def stage_library(repository_root: Path, media_destination: Path, metadata_destination: Path | None, manifest_destination: Path) -> None:
    validate_staging_destinations(
        repository_root,
        [destination for destination in (media_destination, metadata_destination, manifest_destination.parent) if destination is not None],
    )
    source_root = repository_root / "samples"
    records = verify_library(source_root, source_root / "ATTRIBUTIONS.tsv")
    _copy_files(source_root, media_destination, (record.path for record in records))
    if metadata_destination is not None:
        metadata_destination.mkdir(parents=True, exist_ok=True)
        for relative in SUPPORT_FILES:
            source = source_root.joinpath(*relative.split("/"))
            target = metadata_destination.joinpath(*relative.split("/"))
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
    write_manifest(records, manifest_destination)
    verify_manifest(manifest_destination, records)


def sample_media_payload_files(repository_root: Path) -> dict[str, bytes]:
    source_root = repository_root / "samples"
    records = verify_library(source_root, source_root / "ATTRIBUTIONS.tsv")
    return {
        f"samples/{record.path}": source_root.joinpath(*record.path.split("/")).read_bytes()
        for record in records
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--media-destination", type=Path)
    parser.add_argument("--metadata-destination", type=Path)
    parser.add_argument("--manifest-destination", type=Path)
    parser.add_argument("--verify", action="store_true")
    arguments = parser.parse_args()
    root = arguments.repository_root.resolve()
    source = root / "samples"
    if arguments.verify:
        verify_library(source, source / "ATTRIBUTIONS.tsv")
    else:
        if arguments.media_destination is None or arguments.manifest_destination is None:
            parser.error("--media-destination and --manifest-destination are required unless --verify is used")
        stage_library(root, arguments.media_destination, arguments.metadata_destination, arguments.manifest_destination)
    print(f"verified {EXPECTED_MEDIA_COUNT} sample media files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
