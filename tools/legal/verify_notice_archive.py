from __future__ import annotations

import argparse
import hashlib
import re
import sys
import zipfile
from pathlib import Path
from typing import Any

from stage_notices import load_manifest

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.samples.sample_library import sample_media_payload_files


CHECKSUM_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")


def _canonical_files(repository_root: Path, manifest: dict[str, Any]) -> dict[str, bytes]:
    files: dict[str, bytes] = {}
    for item in manifest["files"]:
        source = repository_root / item["source"]
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"canonical legal source is missing or not regular: {source}")
        payload = source.read_bytes()
        if len(payload) != item["size"] or hashlib.sha256(payload).hexdigest() != item["sha256"]:
            raise ValueError(f"canonical legal source identity is stale: {source}")
        files[f"legal/{item['destination']}"] = payload
    return files


def _archive_entries(archive: zipfile.ZipFile) -> dict[str, zipfile.ZipInfo]:
    entries: dict[str, zipfile.ZipInfo] = {}
    for info in archive.infolist():
        name = info.filename
        if not name or name.endswith("/") or name.startswith("/") or "\\" in name or ".." in Path(name).parts:
            raise ValueError(f"unsafe notice archive entry: {name}")
        if name in entries:
            raise ValueError(f"duplicate notice archive entry: {name}")
        if (info.external_attr >> 16) & 0o170000 == 0o120000:
            raise ValueError(f"notice archive entry is a symlink: {name}")
        entries[name] = info
    return entries


def verify_notice_archive(repository_root: Path, archive_path: Path, payload_name: str | None = None) -> None:
    repository_root = repository_root.resolve()
    manifest = load_manifest(repository_root / "resources/legal/notice-bundle.json")
    canonical = _canonical_files(repository_root, manifest)
    manifest_bytes = (repository_root / "resources/legal/notice-bundle.json").read_bytes()
    legal_bytes = {**canonical, "legal/notice-bundle.json": manifest_bytes}
    if payload_name is not None:
        legal_bytes.update(sample_media_payload_files(repository_root))
    expected = {*legal_bytes, "SHA256SUMS"}
    if payload_name is not None:
        if not payload_name or payload_name.startswith("/") or "\\" in payload_name or ".." in Path(payload_name).parts or payload_name in expected:
            raise ValueError(f"portable payload name is unsafe or collides: {payload_name}")
        expected.add(payload_name)
    with zipfile.ZipFile(archive_path) as archive:
        entries = _archive_entries(archive)
        if set(entries) != expected:
            raise ValueError(f"notice archive entries are not exact: {sorted(entries)}")
        for name, expected_bytes in legal_bytes.items():
            actual = archive.read(name)
            if len(actual) != len(expected_bytes) or actual != expected_bytes or hashlib.sha256(actual).hexdigest() != hashlib.sha256(expected_bytes).hexdigest():
                raise ValueError(f"notice archive legal payload is not canonical: {name}")
        checksums = archive.read("SHA256SUMS").decode("utf-8").splitlines()
        if len(checksums) != len(legal_bytes) + (1 if payload_name is not None else 0):
            raise ValueError("notice archive checksum line count is not exact")
        checksum_names: set[str] = set()
        for line in checksums:
            match = CHECKSUM_LINE.fullmatch(line)
            if match is None:
                raise ValueError("notice archive checksum line is malformed")
            digest, name = match.groups()
            if name not in expected - {"SHA256SUMS"} or name in checksum_names:
                raise ValueError(f"notice archive checksum name is not exact: {name}")
            checksum_names.add(name)
            if hashlib.sha256(archive.read(name)).hexdigest() != digest:
                raise ValueError(f"notice archive checksum is not exact: {name}")
        if checksum_names != expected - {"SHA256SUMS"}:
            raise ValueError("notice archive checksum set is not exact")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--payload-name")
    arguments = parser.parse_args()
    verify_notice_archive(arguments.repository_root, arguments.archive, arguments.payload_name)
    print("Notice archive verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
