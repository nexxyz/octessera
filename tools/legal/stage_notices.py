from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


MANIFEST_RELATIVE = Path("resources/legal/notice-bundle.json")
LEGAL_RELATIVE = Path("usr/share/doc/octessera")


class NoticeStageError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise NoticeStageError(message)


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise NoticeStageError(f"legal notice manifest is unreadable: {path}") from error
    files = document.get("files") if isinstance(document, dict) else None
    _require(bool(isinstance(files, list) and files), "legal notice manifest has no files")
    destinations = [item["destination"] for item in files]
    _require(len(set(destinations)) == len(destinations), "legal notice manifest repeats a destination")
    for destination in destinations:
        _require(".." not in Path(destination).parts and not Path(destination).is_absolute(), f"legal notice destination escapes the tree: {destination}")
    return document


def notice_files(repository_root: Path, manifest: dict[str, Any]) -> dict[str, bytes]:
    """The manifest's files by destination."""
    files: dict[str, bytes] = {}
    for item in manifest["files"]:
        source = repository_root / item["source"]
        _require(source.is_file(), f"legal notice source is missing: {source}")
        files[item["destination"]] = source.read_bytes()
    return files


def stage_notices(repository_root: Path, destination_root: Path, manifest_path: Path | None = None, check: bool = False) -> None:
    repository_root = repository_root.resolve()
    manifest = load_manifest(manifest_path or repository_root / MANIFEST_RELATIVE)
    expected = notice_files(repository_root, manifest)
    legal_root = destination_root / LEGAL_RELATIVE
    if not check:
        for destination, data in expected.items():
            target = legal_root / destination
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            target.chmod(0o644)
    _require(legal_root.is_dir(), f"staged legal tree is missing: {legal_root}")
    staged = {path.relative_to(legal_root).as_posix() for path in legal_root.rglob("*") if path.is_file()}
    _require(staged == set(expected), f"staged legal tree has missing or extra files: {sorted(staged ^ set(expected))}")
    for destination, data in expected.items():
        _require((legal_root / destination).read_bytes() == data, f"staged legal file is stale: {legal_root / destination}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage the Octessera legal notice bundle.")
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--destination-root", type=Path, required=True)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--check", action="store_true", help="only verify an already staged tree")
    arguments = parser.parse_args()
    try:
        stage_notices(arguments.repository_root, arguments.destination_root, arguments.manifest, arguments.check)
    except (OSError, NoticeStageError) as error:
        print(f"Legal notice staging failed: {error}", file=sys.stderr)
        return 1
    print("Legal notice staging passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
