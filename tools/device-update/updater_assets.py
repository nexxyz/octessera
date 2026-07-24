#!/usr/bin/env python3
import hashlib
import os
import re
import shutil
import stat
import tempfile
import zipfile
from pathlib import Path, PurePosixPath

from updater_protocol import BINARY, MANIFEST, MAX_ARCHIVE_BYTES, MAX_ENTRY_BYTES, MAX_SUMS_BYTES, MAX_TOTAL_UNCOMPRESSED_BYTES, MAX_ZIP_ENTRIES


def extract_zip(updater, archive: Path, destination: Path, expected_version: str) -> dict:
    error = updater.error
    allowed = {BINARY, "octessera-device-release.json", "LICENSE", "NOTICE"}
    try:
        with zipfile.ZipFile(archive) as source:
            infos = source.infolist()
            if len(infos) > MAX_ZIP_ENTRIES:
                raise error("ZIP contains too many entries")
            names = [info.filename for info in infos]
            if len(names) != len(set(names)):
                raise error("ZIP contains duplicate entries")
            for info in infos:
                name = info.filename
                path = PurePosixPath(name)
                mode = (info.external_attr >> 16) & 0o170000
                if "\\" in name or name.startswith("/") or len(path.parts) != 1 or any(part in ("", ".", "..") for part in path.parts):
                    raise error(f"Unsafe ZIP path: {name}")
                if name.endswith("/") or name not in allowed or info.flag_bits & 1:
                    raise error(f"Unsafe ZIP entry: {name}")
                if mode and mode != stat.S_IFREG:
                    raise error(f"Unsafe ZIP entry type: {name}")
                if info.file_size > MAX_ENTRY_BYTES:
                    raise error(f"ZIP entry exceeds size limit: {name}")
            if sum(info.file_size for info in infos) > MAX_TOTAL_UNCOMPRESSED_BYTES:
                raise error("ZIP uncompressed content exceeds size limit")
            if BINARY not in names or "octessera-device-release.json" not in names:
                raise error("ZIP must include binary and manifest")
            binary_info = source.getinfo(BINARY)
            if not ((binary_info.external_attr >> 16) & 0o111):
                raise error("ZIP binary is not executable")
            destination.mkdir()
            for info in infos:
                output = destination / (MANIFEST if info.filename == "octessera-device-release.json" else info.filename)
                written = 0
                with source.open(info, "r") as input_stream, output.open("wb") as output_stream:
                    while True:
                        chunk = input_stream.read(1024 * 1024)
                        if not chunk:
                            break
                        written += len(chunk)
                        if written > MAX_ENTRY_BYTES:
                            raise error(f"ZIP entry exceeds stream limit: {info.filename}")
                        output_stream.write(chunk)
            os.chmod(destination / BINARY, 0o755)
    except zipfile.BadZipFile as exc:
        raise error("Downloaded asset is not a valid ZIP") from exc
    manifest = updater.validate_manifest(destination / MANIFEST, expected_version)
    if not os.access(destination / BINARY, os.X_OK):
        raise error("Extracted binary is not executable")
    return manifest


def download_candidate(updater, tag: str) -> tuple[Path, dict]:
    payload = updater.release_json(tag)
    actual_tag = payload["tag_name"]
    release_version = actual_tag[1:]
    archive_name, sums_name = updater.asset_names(release_version)
    archive_url = updater.asset_url(payload, archive_name, actual_tag)
    sums_url = updater.asset_url(payload, sums_name, actual_tag)
    updater.releases.mkdir(parents=True, exist_ok=True)
    final = updater.releases / release_version
    if final.exists():
        manifest = updater.validate_release(final)
        updater.immutable(final)
        return final, manifest
    work = Path(tempfile.mkdtemp(prefix=f".tmp-{release_version}-", dir=updater.releases))
    try:
        archive = work / archive_name
        sums = work / sums_name
        updater.curl(archive_url, archive, MAX_ARCHIVE_BYTES)
        updater.curl(sums_url, sums, MAX_SUMS_BYTES)
        if sums.stat().st_size > MAX_SUMS_BYTES:
            raise updater.error("Checksum file exceeds size limit")
        matches = []
        for line in sums.read_text(encoding="utf-8").splitlines():
            fields = line.split()
            if len(fields) == 2 and re.fullmatch(r"[0-9a-fA-F]{64}", fields[0]) and fields[1].lstrip("*") == archive_name:
                matches.append(fields[0].lower())
        digest = hashlib.sha256()
        with archive.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                digest.update(chunk)
        if len(matches) != 1 or digest.hexdigest() != matches[0]:
            raise updater.error("Checksum is missing, duplicated, or does not match")
        extracted = work / "release"
        manifest = extract_zip(updater, archive, extracted, release_version)
        updater.atomic_json(extracted / "update-asset.json", {"repo": updater.repo, "tag": actual_tag, "name": archive_name, "sha256": matches[0], "downloaded_at": updater.now_iso()})
        if final.exists():
            raise updater.error("Release appeared while it was being staged")
        os.replace(extracted, final)
        updater.immutable(final)
        return final, manifest
    finally:
        shutil.rmtree(work, ignore_errors=True)
