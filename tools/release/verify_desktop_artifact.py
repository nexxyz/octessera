from __future__ import annotations

import argparse
import hashlib
import re
import shutil
import stat
import sys
import tempfile
import zipfile
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tools" / "legal"))

from stage_notices import NoticeStageError, load_manifest  # type: ignore[import-not-found]
from tools.samples.sample_library import EXPECTED_MEDIA_COUNT, SampleLibraryError, read_manifest, verify_media_tree


CHECKSUM_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")


class DesktopArtifactError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise DesktopArtifactError(message)


def _regular_file(path: Path, label: str) -> None:
    _require(path.exists() and not path.is_symlink(), f"{label} is missing or symlinked: {path}")
    _require(stat.S_ISREG(path.lstat().st_mode), f"{label} is not a regular file: {path}")


def _safe_relative(value: str, label: str) -> PurePosixPath:
    relative = PurePosixPath(value)
    _require(bool(value) and not relative.is_absolute() and "\\" not in value and ".." not in relative.parts, f"unsafe {label}: {value}")
    return relative


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _tree_files(root: Path, label: str) -> tuple[set[str], set[str]]:
    _require(root.is_dir() and not root.is_symlink(), f"{label} is not a real directory: {root}")
    files: set[str] = set()
    directories: set[str] = set()

    def visit(directory: Path) -> None:
        for entry in sorted(directory.iterdir(), key=lambda item: item.name):
            relative = entry.relative_to(root).as_posix()
            _require(not entry.is_symlink(), f"{label} contains a symlink: {relative}")
            metadata = entry.lstat()
            if stat.S_ISDIR(metadata.st_mode):
                directories.add(relative)
                visit(entry)
            else:
                _require(stat.S_ISREG(metadata.st_mode), f"{label} contains a special file: {relative}")
                files.add(relative)

    visit(root)
    return files, directories


def _expected_directories(files: set[str]) -> set[str]:
    directories: set[str] = set()
    for relative in files:
        parts = relative.split("/")[:-1]
        directories.update("/".join(parts[:index]) for index in range(1, len(parts) + 1))
    return directories


def _canonical_legal_files(repository_root: Path) -> dict[str, tuple[bytes, str]]:
    manifest_path = repository_root / "resources/legal/notice-bundle.json"
    _regular_file(manifest_path, "canonical legal manifest")
    manifest = load_manifest(manifest_path)
    result: dict[str, tuple[bytes, str]] = {}
    for item in manifest["files"]:
        source = repository_root / item["source"]
        _regular_file(source, "canonical legal source")
        payload = source.read_bytes()
        _require(len(payload) == item["size"] and hashlib.sha256(payload).hexdigest() == item["sha256"], f"canonical legal source identity is stale: {source}")
        result[f"legal/{item['destination']}"] = (payload, item["sha256"])
    manifest_bytes = manifest_path.read_bytes()
    result["legal/notice-bundle.json"] = (manifest_bytes, hashlib.sha256(manifest_bytes).hexdigest())
    return result


def _verify_legal_tree(repository_root: Path, resource_root: Path, legal_relative: str) -> None:
    legal_root = resource_root / _safe_relative(legal_relative, "legal resource path")
    canonical = _canonical_legal_files(repository_root)
    expected = {name.removeprefix("legal/") for name in canonical}
    actual_files, actual_directories = _tree_files(legal_root, "legal resource tree")
    _require(actual_files == expected, f"legal resource tree is not exact: missing={sorted(expected - actual_files)!r} extra={sorted(actual_files - expected)!r}")
    _require(actual_directories == _expected_directories(expected), "legal resource directories are not exact")
    for relative, (payload, digest) in canonical.items():
        target = legal_root / relative.removeprefix("legal/")
        _require(target.read_bytes() == payload and target.stat().st_size == len(payload) and _sha256(target) == digest, f"legal resource content is not canonical: {relative}")


def _verify_samples(repository_root: Path, resource_root: Path, samples_relative: str) -> None:
    samples_root = resource_root / _safe_relative(samples_relative, "sample resource path")
    records = read_manifest(repository_root / "samples/MANIFEST.tsv")
    _require(len(records) == EXPECTED_MEDIA_COUNT, f"expected {EXPECTED_MEDIA_COUNT} canonical sample records, found {len(records)}")
    _require(len({record.path for record in records}) == len(records), "canonical sample inventory contains duplicate paths")
    try:
        verify_media_tree(samples_root, records)
    except SampleLibraryError as error:
        raise DesktopArtifactError(str(error)) from error
    for metadata_name in ("MANIFEST.tsv", "SOURCE.md", "upstream"):
        _require(not (samples_root / metadata_name).exists(), f"sample resource tree contains duplicate metadata: {metadata_name}")


def verify_resource_layout(repository_root: Path, resource_root: Path, samples_relative: str = "samples", legal_relative: str = "legal") -> None:
    repository_root = repository_root.resolve()
    _require(resource_root.is_dir() and not resource_root.is_symlink(), f"Tauri resource root is not a real directory: {resource_root}")
    resource_root = resource_root.resolve()
    _verify_samples(repository_root, resource_root, samples_relative)
    _verify_legal_tree(repository_root, resource_root, legal_relative)


def _verify_archive_checksums(root: Path, checksum_name: str, expected_names: set[str]) -> None:
    checksum_path = root / checksum_name
    _regular_file(checksum_path, "portable checksum file")
    lines = checksum_path.read_text(encoding="utf-8").splitlines()
    seen: set[str] = set()
    for line in lines:
        match = CHECKSUM_LINE.fullmatch(line)
        if match is None:
            raise DesktopArtifactError(f"portable checksum line is malformed: {line}")
        digest, name = match.groups()
        _safe_relative(name, "portable checksum entry")
        _require(name in expected_names and name not in seen, f"portable checksum entry is not exact: {name}")
        seen.add(name)
        _require(_sha256(root / name) == digest, f"portable checksum mismatch: {name}")
    _require(seen == expected_names, "portable checksum set is not exact")


def _zip_entries(archive: zipfile.ZipFile) -> list[zipfile.ZipInfo]:
    entries = archive.infolist()
    names: set[str] = set()
    for info in entries:
        name = info.filename
        _safe_relative(name, "portable archive entry")
        _require(not name.endswith("/"), f"portable archive contains a directory entry: {name}")
        _require(name not in names, f"portable archive contains a duplicate entry: {name}")
        names.add(name)
        _require((info.external_attr >> 16) & 0o170000 == stat.S_IFREG, f"portable archive entry is not regular: {name}")
        _require((info.external_attr >> 16) & 0o777 == 0o644, f"portable archive entry mode is not 0644: {name}")
    return entries


def verify_portable_zip(repository_root: Path, archive_path: Path, executable: Path, executable_name: str = "octessera.exe") -> None:
    _regular_file(archive_path, "portable ZIP")
    _regular_file(executable, "portable executable")
    with tempfile.TemporaryDirectory(prefix="octessera-portable-verify-") as temporary:
        extracted = Path(temporary)
        try:
            with zipfile.ZipFile(archive_path) as archive:
                entries = _zip_entries(archive)
                names = {entry.filename for entry in entries}
                expected_prefixes = {f"samples/{record.path}" for record in read_manifest(repository_root / "samples/MANIFEST.tsv")}
                expected_legal = set(_canonical_legal_files(repository_root))
                expected = expected_prefixes | expected_legal | {executable_name, "SHA256SUMS"}
                _require(names == expected, f"portable archive entries are not exact: {sorted(names)}")
                for entry in entries:
                    target = extracted / entry.filename
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(entry.filename))
        except zipfile.BadZipFile as error:
            raise DesktopArtifactError(f"portable ZIP is not valid: {archive_path}") from error
        _require((extracted / executable_name).read_bytes() == executable.read_bytes(), "portable executable bytes differ from the built executable")
        checksum_names = expected - {"SHA256SUMS"}
        _verify_archive_checksums(extracted, "SHA256SUMS", checksum_names)
        verify_resource_layout(repository_root, extracted)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Verify Octessera desktop sample and legal resources.")
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--resource-root", type=Path)
    parser.add_argument("--samples-relative", default="samples")
    parser.add_argument("--legal-relative", default="legal")
    parser.add_argument("--portable-zip", type=Path)
    parser.add_argument("--executable", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.portable_zip is not None:
            _require(args.executable is not None, "--executable is required with --portable-zip")
            verify_portable_zip(args.repository_root, args.portable_zip, args.executable)
        else:
            _require(args.resource_root is not None, "--resource-root is required without --portable-zip")
            verify_resource_layout(args.repository_root, args.resource_root, args.samples_relative, args.legal_relative)
    except (DesktopArtifactError, NoticeStageError, SampleLibraryError, OSError, zipfile.BadZipFile) as error:
        print(f"desktop artifact verification failed: {error}", file=sys.stderr)
        return 1
    print("Desktop artifact resources verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
