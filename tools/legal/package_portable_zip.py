from __future__ import annotations

import argparse
import hashlib
import shutil
import tempfile
import zipfile
from pathlib import Path

from package_notice_zip import package_notice_zip
from verify_notice_archive import verify_notice_archive


def package_portable_zip(repository_root: Path, executable: Path, output: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="octessera-portable-") as temporary:
        notice_zip = Path(temporary) / "notices.zip"
        package_notice_zip(repository_root, notice_zip)
        package = Path(temporary) / "package"
        package.mkdir()
        with zipfile.ZipFile(notice_zip) as archive:
            archive.extractall(package)
        shutil.copyfile(executable, package / "octessera.exe")
        names = sorted(path.relative_to(package).as_posix() for path in package.rglob("*") if path.is_file())
        checksum_names = [name for name in names if name != "SHA256SUMS"]
        (package / "SHA256SUMS").write_text(
            "".join(f"{hashlib.sha256((package / name).read_bytes()).hexdigest()}  {name}\n" for name in checksum_names),
            encoding="utf-8",
        )
        output.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name in ["octessera.exe", *[name for name in names if name != "octessera.exe"]]:
                info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, (package / name).read_bytes())
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
