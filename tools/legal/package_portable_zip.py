from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from package_notice_zip import notice_payload, write_archive
from verify_notice_archive import verify_notice_archive
from tools.samples.sample_library import sample_media_payload_files


def package_portable_zip(repository_root: Path, executable: Path, output: Path) -> None:
    repository_root = repository_root.resolve()
    files = {
        **notice_payload(repository_root),
        **sample_media_payload_files(repository_root),
        "octessera.exe": executable.read_bytes(),
    }
    write_archive(output, files, first=["octessera.exe"])
    verify_notice_archive(repository_root, output, "octessera.exe")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    package_portable_zip(arguments.repository_root, arguments.executable, arguments.output)
    print(f"Portable legal ZIP created: {arguments.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
