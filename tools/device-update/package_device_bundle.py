#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import re
import stat
import tempfile
import zipfile
from pathlib import Path


BINARY = "octessera-pi"
RUNTIME_METADATA = "octessera-runtime.json"
RUNTIME_SUMS = "SHA256SUMS"
MANIFEST = "octessera-device-release.json"
RASPBERRY_PROFILE = "raspberry-pi-zero-2w"
ORANGE_PROFILE = "orange-pi-zero-2w"
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
TAG_RE = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+$")
ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)


def fail(message: str) -> None:
    raise ValueError(message)


def regular_file(path: Path, label: str) -> bytes:
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        fail(f"{label} is not a regular file: {path}")
    return path.read_bytes()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def validate_runtime(
    runtime_bundle: Path, profile: str, version: str
) -> tuple[bytes, bytes, bytes]:
    if not runtime_bundle.is_dir() or runtime_bundle.is_symlink():
        fail(f"Runtime bundle is not a directory: {runtime_bundle}")
    expected_entries = {BINARY, RUNTIME_METADATA, RUNTIME_SUMS}
    actual_entries = {path.name for path in runtime_bundle.iterdir()}
    if actual_entries != expected_entries:
        fail(f"Runtime bundle entries are not exact: {sorted(actual_entries)}")
    binary = regular_file(runtime_bundle / BINARY, "Runtime binary")
    metadata_bytes = regular_file(runtime_bundle / RUNTIME_METADATA, "Runtime metadata")
    sums_bytes = regular_file(runtime_bundle / RUNTIME_SUMS, "Runtime checksums")
    try:
        metadata = json.loads(metadata_bytes.decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as exc:
        raise ValueError("Runtime metadata is not valid JSON") from exc
    expected_keys = {
        "artifact_kind",
        "binary_sha256",
        "name",
        "profile",
        "runtime_ready",
        "version",
    }
    if not isinstance(metadata, dict) or set(metadata) != expected_keys:
        fail("Runtime metadata keys are not exact")
    digest = sha256_bytes(binary)
    if metadata != {
        "artifact_kind": "production-runtime",
        "binary_sha256": digest,
        "name": BINARY,
        "profile": profile,
        "runtime_ready": True,
        "version": version,
    }:
        fail("Runtime metadata does not match the requested release or binary")
    if sums_bytes != f"{digest}  {BINARY}\n".encode("ascii"):
        fail("Runtime checksums are not exact")
    return binary, metadata_bytes, sums_bytes


def release_manifest(profile: str, tag: str, version: str) -> bytes:
    if profile == RASPBERRY_PROFILE:
        payload = {
            "schema_version": 2,
            "updater_protocol": 2,
            "candidate_health_protocol": 1,
            "tag": tag,
            "version": version,
            "board_profile": profile,
            "arch": "aarch64-unknown-linux-gnu",
            "binary": BINARY,
            "platforms": [profile, "linux-aarch64-device"],
        }
    else:
        payload = {
            "schema_version": 2,
            "updater_supported": False,
            "candidate_health_protocol": 1,
            "distribution": "standalone-manual",
            "tag": tag,
            "version": version,
            "board_profile": profile,
            "arch": "aarch64-unknown-linux-gnu",
            "binary": BINARY,
            "platforms": [profile, "linux-aarch64-device"],
        }
    return (json.dumps(payload, indent=2) + "\n").encode("utf-8")


def zip_entry(name: str, mode: int) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, ZIP_TIMESTAMP)
    info.create_system = 3
    info.external_attr = (stat.S_IFREG | mode) << 16
    info.internal_attr = 0
    info.flag_bits = 0
    return info


def atomic_write(path: Path, value: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp", delete=False
    ) as handle:
        temporary = Path(handle.name)
        os.chmod(temporary, 0o644)
        handle.write(value)
        handle.flush()
        os.fsync(handle.fileno())
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def write_archive(path: Path, entries: list[tuple[str, bytes, int]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp", delete=False
    ) as handle:
        temporary = Path(handle.name)
    try:
        with zipfile.ZipFile(
            temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
        ) as archive:
            for name, value, mode in entries:
                archive.writestr(
                    zip_entry(name, mode),
                    value,
                    compress_type=zipfile.ZIP_DEFLATED,
                    compresslevel=9,
                )
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def validate_archive(
    path: Path, expected_entries: list[tuple[str, bytes, int]]
) -> None:
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        if [info.filename for info in infos] != [
            entry[0] for entry in expected_entries
        ]:
            fail("Device archive entries are not exact")
        for info, (_, expected_value, expected_mode) in zip(infos, expected_entries):
            mode = (info.external_attr >> 16) & 0o777
            file_type = (info.external_attr >> 16) & 0o170000
            if (
                file_type != stat.S_IFREG
                or mode != expected_mode
                or info.date_time != ZIP_TIMESTAMP
            ):
                fail(f"Device archive metadata is not deterministic: {info.filename}")
            if archive.read(info) != expected_value:
                fail(
                    f"Device archive content does not match its source: {info.filename}"
                )


def package_bundle(
    runtime_bundle: Path,
    output_dir: Path,
    repository_root: Path,
    profile: str,
    tag: str,
    version: str,
) -> tuple[Path, Path]:
    if profile not in (RASPBERRY_PROFILE, ORANGE_PROFILE):
        fail(f"Unsupported board profile: {profile}")
    if (
        not VERSION_RE.fullmatch(version)
        or not TAG_RE.fullmatch(tag)
        or tag[1:] != version
    ):
        fail("Release version and tag are not an exact semver pair")
    binary, runtime_metadata, runtime_sums = validate_runtime(
        runtime_bundle, profile, version
    )
    license_bytes = regular_file(repository_root / "LICENSE", "LICENSE")
    notice_bytes = regular_file(repository_root / "NOTICE", "NOTICE")
    manifest = release_manifest(profile, tag, version)
    if profile == RASPBERRY_PROFILE:
        archive_name = f"octessera-{version}-{profile}-device-aarch64.zip"
        entries = [
            (BINARY, binary, 0o755),
            (MANIFEST, manifest, 0o644),
            ("LICENSE", license_bytes, 0o644),
            ("NOTICE", notice_bytes, 0o644),
        ]
    else:
        archive_name = f"octessera-{version}-{profile}-standalone-manual-aarch64.zip"
        entries = [
            (BINARY, binary, 0o755),
            (RUNTIME_METADATA, runtime_metadata, 0o644),
            (RUNTIME_SUMS, runtime_sums, 0o644),
            (MANIFEST, manifest, 0o644),
            ("LICENSE", license_bytes, 0o644),
            ("NOTICE", notice_bytes, 0o644),
        ]
    archive_path = output_dir / archive_name
    write_archive(archive_path, entries)
    validate_archive(archive_path, entries)
    checksum_path = output_dir / f"SHA256SUMS-{profile}-device.txt"
    atomic_write(
        checksum_path,
        f"{sha256_bytes(archive_path.read_bytes())}  {archive_name}\n".encode("ascii"),
    )
    return archive_path, checksum_path


def parse_args() -> argparse.Namespace:
    repository_root = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-bundle", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--repository-root", type=Path, default=repository_root)
    parser.add_argument("--board-profile", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    return parser.parse_args()


def main() -> int:
    arguments = parse_args()
    try:
        package_bundle(
            arguments.runtime_bundle,
            arguments.output_dir,
            arguments.repository_root,
            arguments.board_profile,
            arguments.tag,
            arguments.version,
        )
    except (OSError, ValueError, zipfile.BadZipFile) as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
