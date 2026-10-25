from __future__ import annotations

import argparse
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from package_notice_zip import checksum_text, notice_payload
from tools.samples.sample_library import sample_media_payload_files


def verify_notice_archive(repository_root: Path, archive_path: Path, payload_name: str | None = None) -> None:
    """Checks that a release archive carries exactly the current legal files (plus, for the
    portable zip, the samples and `payload_name`) and a matching SHA256SUMS."""
    repository_root = repository_root.resolve()
    expected = notice_payload(repository_root)
    if payload_name is not None:
        expected.update(sample_media_payload_files(repository_root))
    with zipfile.ZipFile(archive_path) as archive:
        names = set(archive.namelist())
        wanted = {*expected, "SHA256SUMS"} | ({payload_name} if payload_name else set())
        if names != wanted:
            raise ValueError(f"notice archive has missing or extra entries: {sorted(names ^ wanted)}")
        for name, data in expected.items():
            if archive.read(name) != data:
                raise ValueError(f"notice archive entry is stale: {name}")
        contents = {name: archive.read(name) for name in names if name != "SHA256SUMS"}
        if archive.read("SHA256SUMS").decode("utf-8") != checksum_text(contents):
            raise ValueError("notice archive SHA256SUMS does not match its entries")


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
