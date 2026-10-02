#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, cast

from orange_boot_contract import BootContractError, constructor_proof
from orange_boot_selection import BootSelectionError
from orange_image_mount import ImageMountError, mounted_image
from orange_initramfs import InitramfsDecodeError


class ImageProofError(ValueError):
    pass


_RESIZE_SERVICE_PATH = Path("usr/lib/systemd/system/armbian-resize-filesystem.service")
_RESIZE_ENABLE_PATH = Path("etc/systemd/system/basic.target.wants/armbian-resize-filesystem.service")
_RESIZE_DIRECTIVES = {
    "Unit": {
        "After": "sysinit.target local-fs.target",
        "Before": "basic.target",
        "DefaultDependencies": "no",
    },
    "Service": {
        "Type": "oneshot",
        "TimeoutStartSec": "6min",
    },
    "Install": {"WantedBy": "basic.target"},
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ImageProofError(message)


def _verify_json(path: Path, expected: dict[str, Any]) -> None:
    try:
        actual = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ImageProofError(f"cannot read structured Orange proof: {path}") from error
    require(actual == expected, "Orange structured proof changed")


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _verify_resize_service(root: Path) -> None:
    service = root / _RESIZE_SERVICE_PATH
    require(service.is_file() and not service.is_symlink(), "Orange resize service is missing or symlinked")
    try:
        lines = service.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as error:
        raise ImageProofError("Orange resize service is unreadable") from error
    section: str | None = None
    seen: set[tuple[str, str]] = set()
    for raw_line in lines:
        line = raw_line.strip()
        if not line or line.startswith(("#", ";")):
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1].strip()
            continue
        key, separator, value = line.partition("=")
        if separator and section in _RESIZE_DIRECTIVES and key.strip() in _RESIZE_DIRECTIVES[section]:
            directive = (section, key.strip())
            require(directive not in seen, f"Orange resize service directive is duplicated: {section}.{key.strip()}")
            seen.add(directive)
            require(value.strip() == _RESIZE_DIRECTIVES[section][key.strip()], f"Orange resize service directive is wrong: {section}.{key.strip()}")
    for section_name, directives in _RESIZE_DIRECTIVES.items():
        for key in directives:
            require((section_name, key) in seen, f"Orange resize service directive is missing: {section_name}.{key}")
    enabled = root / _RESIZE_ENABLE_PATH
    require(
        enabled.is_symlink()
        and enabled.readlink().as_posix()
        in {"../../../usr/lib/systemd/system/armbian-resize-filesystem.service", "/usr/lib/systemd/system/armbian-resize-filesystem.service"},
        "Orange resize service is not enabled for basic.target",
    )


def _phase5(args: argparse.Namespace, root: Path, image_hash: str, image_name: str, compression: str, repository_root: Path) -> dict[str, Any]:
    required = {
        "--linux-image": args.linux_image,
        "--linux-dtb": args.linux_dtb,
        "--evidence": args.evidence,
        "--provenance": args.provenance,
    }
    for label, value in required.items():
        require(value is not None, f"{label} is required for phase5-constructor")
    require(args.construction_contract is not None, "--construction-contract is required for phase5-constructor")
    require(args.manifest is not None, "--manifest is required for phase5-constructor")
    if args.mode == "production":
        _verify_resize_service(root)
    return constructor_proof(root, args, image_hash, image_name, compression, repository_root)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prove an exact Orange image under an explicit boot-proof mode.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--image", type=Path)
    source.add_argument("--root", type=Path)
    parser.add_argument("--image-sha256")
    parser.add_argument("--boot-proof-mode", choices=("phase5-constructor",), required=True)
    parser.add_argument("--linux-image", type=Path)
    parser.add_argument("--linux-dtb", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--provenance", type=Path)
    parser.add_argument("--construction-contract", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--image-provenance", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mode", choices=("diagnostic", "production"), default="production")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    repository_root = Path(__file__).resolve().parents[2]
    try:
        require(args.manifest is not None and args.manifest.is_file(), f"missing Orange kernel manifest: {args.manifest}")
        if args.root is not None:
            require(args.image_sha256 is not None and re.fullmatch(r"[0-9a-fA-F]{64}", args.image_sha256) is not None, "--root requires a 64-character --image-sha256")
            root_source = cast(Path, args.root)
            require(root_source.is_dir(), "Orange proof root is missing")
            image_hash, image_name, compression = cast(str, args.image_sha256).lower(), root_source.name, "root-fixture"
            with mounted_image(root_source) as root:
                result = _phase5(args, root, image_hash, image_name, compression, repository_root)
        else:
            require(args.image is not None and args.image.is_file(), "final Orange image is missing")
            image_source = cast(Path, args.image)
            require(image_source.suffix == ".img" or image_source.suffixes[-2:] == [".img", ".xz"], "Orange image proof accepts only .img or .img.xz")
            require(args.image_sha256 is None, "--image-sha256 is only valid with --root")
            image_hash = hashlib.sha256(image_source.read_bytes()).hexdigest()
            compression = "xz" if image_source.suffix == ".xz" else "none"
            with mounted_image(image_source) as root:
                result = _phase5(args, root, image_hash, image_source.name, compression, repository_root)
        if args.image_provenance:
            _verify_json(args.image_provenance, result)
        if args.output:
            _write_json(args.output, result)
        print(json.dumps(result, indent=2, sort_keys=True))
        print("Orange final image proof passed")
        return 0
    except (BootContractError, BootSelectionError, ImageMountError, ImageProofError, InitramfsDecodeError, OSError, json.JSONDecodeError) as error:
        print(f"Orange final image proof failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
