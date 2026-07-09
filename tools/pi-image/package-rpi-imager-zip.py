#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import shutil
import tempfile
import zipfile
from datetime import date
from pathlib import Path


MANIFEST_ENTRY_NAME = "os_list.rpi-imager-manifest"


def sha256_zip_member(zip_path: Path, member_name: str) -> str:
    digest = hashlib.sha256()
    with zipfile.ZipFile(zip_path, "r") as archive:
        with archive.open(member_name, "r") as member:
            for chunk in iter(lambda: member.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def find_image_member(zip_path: Path) -> zipfile.ZipInfo:
    with zipfile.ZipFile(zip_path, "r") as archive:
        image_members = [info for info in archive.infolist() if info.filename.endswith(".img")]
    if len(image_members) != 1:
        names = ", ".join(info.filename for info in image_members) or "none"
        raise SystemExit(f"Expected exactly one .img in {zip_path}; found {names}")
    return image_members[0]


def build_manifest(
    *,
    version: str,
    image_url: str,
    icon_url: str,
    release_date: str,
    extract_size: int,
    extract_sha256: str,
    image_download_size: int,
) -> dict:
    return {
        "imager": {
            "latest_version": "2.0.0",
            "url": "https://www.raspberrypi.com/software/",
            "devices": [
                {
                    "name": "Raspberry Pi Zero 2 W",
                    "description": "Raspberry Pi Zero 2 W / Pi 3-class 64-bit devices",
                    "icon": icon_url,
                    "tags": ["pi3-64bit"],
                    "default": True,
                    "matching_type": "inclusive",
                    "capabilities": ["i2c", "spi", "serial"],
                }
            ],
        },
        "os_list": [
            {
                "name": f"Octessera {version} for Raspberry Pi Zero 2 W",
                "description": "Ready-to-flash Raspberry Pi OS Lite image with Octessera hardware services preinstalled. Uses Imager systemd first-run customization for SSH, user, hostname, and Wi-Fi.",
                "url": image_url,
                "icon": icon_url,
                "website": "https://github.com/nexxyz/cellSymphony",
                "release_date": release_date,
                "extract_size": extract_size,
                "extract_sha256": extract_sha256,
                "image_download_size": image_download_size,
                "devices": ["pi3-64bit"],
                "init_format": "systemd",
                "architecture": "armv8",
                "capabilities": ["i2c", "spi", "serial", "passwordless_sudo"],
            }
        ],
    }


def write_zip_with_manifest(source_zip: Path, target_zip: Path, manifest: dict) -> None:
    manifest_bytes = manifest_to_bytes(manifest)
    with zipfile.ZipFile(source_zip, "r") as source:
        with zipfile.ZipFile(target_zip, "w", compression=zipfile.ZIP_DEFLATED) as target:
            for info in source.infolist():
                if info.filename == MANIFEST_ENTRY_NAME:
                    continue
                with source.open(info, "r") as source_member:
                    with target.open(info, "w", force_zip64=True) as target_member:
                        shutil.copyfileobj(source_member, target_member, 1024 * 1024)
            manifest_info = zipfile.ZipInfo(MANIFEST_ENTRY_NAME)
            manifest_info.external_attr = 0o644 << 16
            target.writestr(manifest_info, manifest_bytes)


def manifest_to_bytes(manifest: dict) -> bytes:
    return (json.dumps(manifest, indent=2) + "\n").encode("utf-8")


def package_manifest(args: argparse.Namespace) -> dict:
    zip_path = Path(args.zip).resolve()
    image_member = find_image_member(zip_path)
    extract_sha256 = sha256_zip_member(zip_path, image_member.filename)
    release_date = args.release_date or date.today().isoformat()

    image_url = f"https://github.com/{args.repository}/releases/download/{args.tag}/{zip_path.name}"
    icon_url = args.icon_url or f"https://raw.githubusercontent.com/{args.repository}/{args.tag}/assets/octessera-pi-manifest.png"

    current_size = os.path.getsize(zip_path)
    manifest = None
    with tempfile.TemporaryDirectory(prefix="cellsymphony-rpi-manifest-") as temp_dir:
        source_zip = zip_path
        for _ in range(10):
            manifest = build_manifest(
                version=args.version,
                image_url=image_url,
                icon_url=icon_url,
                release_date=release_date,
                extract_size=image_member.file_size,
                extract_sha256=extract_sha256,
                image_download_size=current_size,
            )
            target_zip = Path(temp_dir) / "with-manifest.zip"
            write_zip_with_manifest(source_zip, target_zip, manifest)
            next_size = os.path.getsize(target_zip)
            if next_size == current_size:
                shutil.move(target_zip, zip_path)
                break
            current_size = next_size
            source_zip = zip_path
        else:
            raise SystemExit("Could not stabilize manifest image_download_size")

    manifest_path = Path(args.manifest_out).resolve() if args.manifest_out else zip_path.with_suffix(zip_path.suffix + ".rpi-imager-manifest")
    manifest_path.write_bytes(manifest_to_bytes(manifest))
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description="Add a Raspberry Pi Imager manifest to an Octessera Pi image ZIP.")
    parser.add_argument("--zip", required=True, help="Pi image ZIP to update in place")
    parser.add_argument("--version", required=True, help="Release version without leading v")
    parser.add_argument("--tag", required=True, help="Release tag, for example v0.5.1")
    parser.add_argument("--repository", required=True, help="GitHub repository owner/name")
    parser.add_argument("--manifest-out", help="Path for a standalone copy of the generated manifest")
    parser.add_argument("--icon-url", help="Manifest icon URL. Defaults to the tagged Octessera logo asset.")
    parser.add_argument("--release-date", help="Release date in YYYY-MM-DD. Defaults to today.")
    args = parser.parse_args()
    package_manifest(args)


if __name__ == "__main__":
    main()
