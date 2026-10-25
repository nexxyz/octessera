#!/usr/bin/env python3
import argparse
import hashlib
import re
import stat
import zipfile
from pathlib import Path

PROFILES = ("raspberry-pi-zero-2w", "orange-pi-zero-2w")
RUNTIME_FILES = ("octessera-pi", "octessera-runtime.json", "SHA256SUMS")
LEGAL_FILES = ("LICENSE", "NOTICE")
VERSION_RE = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)


def device_asset_names(profile: str, version: str) -> tuple[str, str]:
    return f"octessera-{version}-{profile}-device-aarch64.zip", f"SHA256SUMS-{profile}-device.txt"


def package_bundle(runtime_bundle: Path, output_dir: Path, repository_root: Path, profile: str, tag: str, version: str) -> Path:
    if profile not in PROFILES:
        raise ValueError(f"Unsupported board profile: {profile}")
    if not VERSION_RE.fullmatch(version) or tag != f"v{version}":
        raise ValueError("Release version and tag are not an exact semver pair")
    if sorted(path.name for path in runtime_bundle.iterdir()) != sorted(RUNTIME_FILES):
        raise ValueError(f"Runtime bundle entries are not exact: {runtime_bundle}")
    sources = [runtime_bundle / name for name in RUNTIME_FILES] + [repository_root / name for name in LEGAL_FILES]
    for source in sources:
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"Device archive input is not a regular file: {source}")
    archive_name, sums_name = device_asset_names(profile, version)
    output_dir.mkdir(parents=True, exist_ok=True)
    archive = output_dir / archive_name
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as target:
        for source in sources:
            info = zipfile.ZipInfo(source.name, ZIP_TIMESTAMP)
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | (0o755 if source.name == "octessera-pi" else 0o644)) << 16
            target.writestr(info, source.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (output_dir / sums_name).write_text(f"{digest}  {archive_name}\n", encoding="ascii")
    return archive


def main() -> int:
    parser = argparse.ArgumentParser(description="Package one board's runtime bundle as its device update ZIP.")
    parser.add_argument("--runtime-bundle", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--repository-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--board-profile", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    arguments = parser.parse_args()
    try:
        package_bundle(arguments.runtime_bundle, arguments.output_dir, arguments.repository_root, arguments.board_profile, arguments.tag, arguments.version)
    except (OSError, ValueError) as error:
        raise SystemExit(str(error)) from error
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
