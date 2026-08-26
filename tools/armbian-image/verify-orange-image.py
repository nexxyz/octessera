#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import lzma
import re
import sys
from pathlib import Path
from typing import Any, cast

from orange_boot_contract import BootContractError, constructor_proof
from orange_boot_selection import BootSelectionError
from orange_image_mount import ImageMountError, capture_image_layout, mounted_image
from orange_initramfs import InitramfsDecodeError
from orange_trusted_parent_proof import TrustedParentProofError, artifact_identity, verify_trusted


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


def _path_arg(value: Path | None, label: str) -> Path:
    require(value is not None, f"{label} is required")
    return cast(Path, value)


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
    require(args.trust_manifest is None, "--trust-manifest is forbidden for phase5-constructor")
    require(args.boot_neutral_contract is None and args.parent_image is None and args.respin_provenance is None and args.setup_proof is None and args.derivation_kind is None, "trusted-parent arguments are invalid for phase5-constructor")
    if args.mode == "production":
        _verify_resize_service(root)
    return constructor_proof(root, args, image_hash, image_name, compression, repository_root)


def _trusted(args: argparse.Namespace, derived_root: Path, derived_image: tuple[str, int], artifact: tuple[str, int], artifact_name: str, repository_root: Path, derived_layout: dict[str, object]) -> dict[str, Any]:
    require(args.manifest is None and args.construction_contract is None and args.linux_image is None and args.linux_dtb is None and args.evidence is None and args.provenance is None, "phase5 package arguments are invalid for trusted-parent mode")
    parent_image = _path_arg(args.parent_image, "--parent-image")
    require(parent_image.is_file(), "trusted parent image is missing")
    trust_manifest = _path_arg(args.trust_manifest, "--trust-manifest")
    require(trust_manifest.is_file(), "trusted parent manifest is missing")
    provenance = _path_arg(args.respin_provenance, "--respin-provenance")
    require(provenance.is_file(), "trusted respin provenance is missing")
    contract = _path_arg(args.boot_neutral_contract, "--boot-neutral-contract")
    require(contract.is_file(), "trusted boot-neutral contract is missing")
    derivation_kind = args.derivation_kind
    require(derivation_kind in {"runtime-only", "setup-portal"}, "trusted derivation kind is required")
    setup_proof = args.setup_proof
    require((derivation_kind == "setup-portal") == (setup_proof is not None), "setup proof must match trusted derivation kind")
    parent_layout = capture_image_layout(parent_image, "orange-pi-zero-2w", repository_root)
    with mounted_image(parent_image) as parent_root:
        return verify_trusted(parent_root, derived_root, parent_image, contract, trust_manifest, provenance, derivation_kind, setup_proof, repository_root, derived_image, artifact, artifact_name, parent_layout, derived_layout)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prove an exact Orange image under an explicit boot-proof mode.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--image", type=Path)
    source.add_argument("--root", type=Path)
    parser.add_argument("--image-sha256")
    parser.add_argument("--boot-proof-mode", choices=("phase5-constructor", "trusted-v0.7.5-boot-neutral"), required=True)
    parser.add_argument("--linux-image", type=Path)
    parser.add_argument("--linux-dtb", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--provenance", type=Path)
    parser.add_argument("--construction-contract", type=Path)
    parser.add_argument("--boot-neutral-contract", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--trust-manifest", type=Path)
    parser.add_argument("--parent-image", type=Path)
    parser.add_argument("--respin-provenance", type=Path)
    parser.add_argument("--derivation-kind", choices=("runtime-only", "setup-portal"))
    parser.add_argument("--setup-proof", type=Path)
    parser.add_argument("--image-provenance", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mode", choices=("diagnostic", "production"), default="production")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    repository_root = Path(__file__).resolve().parents[2]
    try:
        if args.boot_proof_mode == "phase5-constructor":
            require(args.manifest is not None and args.manifest.is_file(), f"missing Orange kernel manifest: {args.manifest}")
        else:
            require(args.construction_contract is None, "--construction-contract is forbidden for trusted-v0.7.5-boot-neutral")
        if args.root is not None:
            require(args.boot_proof_mode == "phase5-constructor", "trusted-v0.7.5-boot-neutral requires --image")
            require(args.image_sha256 is not None and re.fullmatch(r"[0-9a-fA-F]{64}", args.image_sha256) is not None, "--root requires a 64-character --image-sha256")
            root_source = cast(Path, args.root)
            require(root_source.is_dir(), "Orange proof root is missing")
            root_identity = artifact_identity(root_source)
            image_hash, image_name, compression = cast(str, args.image_sha256).lower(), root_source.name, "root-fixture"
            if args.boot_proof_mode == "trusted-v0.7.5-boot-neutral":
                require(root_identity[0] == image_hash, "--image-sha256 does not match the synthetic derived root")
            with mounted_image(root_source) as root:
                result = _phase5(args, root, image_hash, image_name, compression, repository_root)
        else:
            require(args.image is not None and args.image.is_file(), "final Orange image is missing")
            image_source = cast(Path, args.image)
            require(image_source.suffix == ".img" or image_source.suffixes[-2:] == [".img", ".xz"], "Orange image proof accepts only .img or .img.xz")
            require(args.image_sha256 is None, "--image-sha256 is only valid with --root")
            image_hash = hashlib.sha256(image_source.read_bytes()).hexdigest()
            artifact = (image_hash, image_source.stat().st_size)
            if image_source.suffix == ".xz":
                raw_digest = hashlib.sha256()
                raw_size = 0
                with lzma.open(image_source, "rb") as raw_image:
                    for chunk in iter(lambda: raw_image.read(1024 * 1024), b""):
                        raw_digest.update(chunk)
                        raw_size += len(chunk)
                derived_image = (raw_digest.hexdigest(), raw_size)
            else:
                derived_image = artifact
            compression = "xz" if image_source.suffix == ".xz" else "none"
            if args.boot_proof_mode == "trusted-v0.7.5-boot-neutral":
                derived_layout = capture_image_layout(image_source, "orange-pi-zero-2w", repository_root)
                with mounted_image(image_source) as root:
                    result = _trusted(args, root, derived_image, artifact, image_source.name, repository_root, derived_layout)
            else:
                with mounted_image(image_source) as root:
                    result = _phase5(args, root, image_hash, image_source.name, compression, repository_root)
        if args.image_provenance:
            _verify_json(args.image_provenance, result)
        if args.output:
            _write_json(args.output, result)
        print(json.dumps(result, indent=2, sort_keys=True))
        print("Orange final image proof passed")
        return 0
    except (BootContractError, BootSelectionError, ImageMountError, ImageProofError, InitramfsDecodeError, TrustedParentProofError, OSError, json.JSONDecodeError) as error:
        print(f"Orange final image proof failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
