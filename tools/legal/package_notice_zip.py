from __future__ import annotations

import argparse
import hashlib
import shutil
import tempfile
import zipfile
from pathlib import Path

from stage_notices import load_manifest, stage_notices


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_notice_zip(repository_root: Path, output: Path) -> None:
    repository_root = repository_root.resolve()
    manifest_path = repository_root / "resources/legal/notice-bundle.json"
    manifest = load_manifest(manifest_path)
    with tempfile.TemporaryDirectory(prefix="octessera-notice-zip-") as temporary:
        staged = Path(temporary) / "staged"
        stage_notices(repository_root, staged, ownership="filesystem")
        legal_root = staged / "usr/share/doc/octessera"
        package_root = Path(temporary) / "package"
        (package_root / "legal").mkdir(parents=True)
        for item in manifest["files"]:
            source = legal_root / item["destination"]
            target = package_root / "legal" / item["destination"]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        shutil.copyfile(manifest_path, package_root / "legal/notice-bundle.json")
        checksum_paths = sorted(path.relative_to(package_root).as_posix() for path in (package_root / "legal").rglob("*") if path.is_file())
        checksum_text = "".join(f"{_digest(package_root / relative)}  {relative}\n" for relative in checksum_paths)
        (package_root / "SHA256SUMS").write_text(checksum_text, encoding="utf-8")
        output.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for relative in [*checksum_paths, "SHA256SUMS"]:
                path = package_root / relative
                info = zipfile.ZipInfo(relative, (1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, path.read_bytes())


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
