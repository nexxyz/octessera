#!/usr/bin/env python3
"""Stage the Octessera sample library: every WAV under samples/ plus the pack licence."""

from __future__ import annotations

import argparse
import shutil
import stat
from pathlib import Path
from typing import Iterable


MEDIA_SUFFIX = ".wav"
SUPPORT_FILES = ("upstream/LICENSE",)


class SampleLibraryError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise SampleLibraryError(message)


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


def media_paths(sample_root: Path) -> list[str]:
    paths = [
        path.relative_to(sample_root).as_posix()
        for path in _tree_files(sample_root)
        if path.suffix.lower() == MEDIA_SUFFIX
    ]
    _require(bool(paths), f"sample library has no WAV files: {sample_root}")
    return paths


def verify_media_tree(media_root: Path, source_root: Path, allowed_empty_directories: Iterable[str] = ()) -> None:
    expected = media_paths(source_root)
    actual = sorted(path.relative_to(media_root).as_posix() for path in _tree_files(media_root))
    _require(actual == expected, f"staged sample tree differs: missing={sorted(set(expected) - set(actual))!r} extra={sorted(set(actual) - set(expected))!r}")
    for relative in expected:
        staged = media_root.joinpath(*relative.split("/"))
        source = source_root.joinpath(*relative.split("/"))
        _require(staged.read_bytes() == source.read_bytes(), f"staged sample differs: {relative}")
    expected_directories = {
        "/".join(relative.split("/")[:index])
        for relative in expected
        for index in range(1, len(relative.split("/")))
    } | set(allowed_empty_directories)
    actual_directories = {
        path.relative_to(media_root).as_posix()
        for path in media_root.rglob("*")
        if path.is_dir() and not path.is_symlink()
    }
    _require(actual_directories == expected_directories, f"staged sample directories differ: missing={sorted(expected_directories - actual_directories)!r} extra={sorted(actual_directories - expected_directories)!r}")


def verify_metadata_tree(metadata_root: Path, source_root: Path) -> None:
    for relative in SUPPORT_FILES:
        target = metadata_root.joinpath(*relative.split("/"))
        source = source_root.joinpath(*relative.split("/"))
        _require(target.is_file() and target.read_bytes() == source.read_bytes(), f"staged sample licence is missing or stale: {relative}")


def validate_staging_destinations(repository_root: Path, destinations: Iterable[Path]) -> None:
    source_root = (repository_root / "samples").absolute().resolve(strict=False)
    for destination in destinations:
        resolved = Path(destination).absolute().resolve(strict=False)
        if resolved == source_root or source_root in resolved.parents or resolved in source_root.parents:
            raise SampleLibraryError(f"sample staging destination overlaps the sample source: {resolved}")


def stage_library(repository_root: Path, media_destination: Path, metadata_destination: Path | None) -> None:
    validate_staging_destinations(
        repository_root,
        [destination for destination in (media_destination, metadata_destination) if destination is not None],
    )
    source_root = repository_root / "samples"
    _require(not media_destination.is_symlink(), f"sample destination is symlinked: {media_destination}")
    if media_destination.exists():
        shutil.rmtree(media_destination)
    for relative in media_paths(source_root):
        target = media_destination.joinpath(*relative.split("/"))
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_root.joinpath(*relative.split("/")), target)
    if metadata_destination is not None:
        for relative in SUPPORT_FILES:
            target = metadata_destination.joinpath(*relative.split("/"))
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_root.joinpath(*relative.split("/")), target)


def sample_media_payload_files(repository_root: Path) -> dict[str, bytes]:
    source_root = repository_root / "samples"
    return {
        f"samples/{relative}": source_root.joinpath(*relative.split("/")).read_bytes()
        for relative in media_paths(source_root)
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--media-destination", type=Path, required=True)
    parser.add_argument("--metadata-destination", type=Path)
    arguments = parser.parse_args()
    root = arguments.repository_root.resolve()
    stage_library(root, arguments.media_destination, arguments.metadata_destination)
    print(f"staged {len(media_paths(root / 'samples'))} sample files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
