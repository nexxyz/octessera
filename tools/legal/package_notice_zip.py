from __future__ import annotations

import argparse
import hashlib
import zipfile
from pathlib import Path

from stage_notices import MANIFEST_RELATIVE, load_manifest, notice_files


def notice_payload(repository_root: Path) -> dict[str, bytes]:
    """The legal files as they appear in release archives, under `legal/`."""
    manifest_path = repository_root / MANIFEST_RELATIVE
    files = notice_files(repository_root, load_manifest(manifest_path))
    payload = {f"legal/{destination}": data for destination, data in files.items()}
    payload["legal/notice-bundle.json"] = manifest_path.read_bytes()
    return payload


def checksum_text(files: dict[str, bytes]) -> str:
    return "".join(f"{hashlib.sha256(files[name]).hexdigest()}  {name}\n" for name in sorted(files))


def write_archive(output: Path, files: dict[str, bytes], first: list[str] | None = None) -> None:
    """Writes `files` plus SHA256SUMS with fixed timestamps, so the same input gives the same bytes."""
    files = {**files, "SHA256SUMS": checksum_text(files).encode("utf-8")}
    leading = first or []
    names = [*leading, *[name for name in sorted(files) if name not in leading and name != "SHA256SUMS"], "SHA256SUMS"]
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name in names:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, files[name])


def package_notice_zip(repository_root: Path, output: Path) -> None:
    write_archive(output, notice_payload(repository_root.resolve()))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    package_notice_zip(arguments.repository_root, arguments.output)
    print(f"Notice archive created: {arguments.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
