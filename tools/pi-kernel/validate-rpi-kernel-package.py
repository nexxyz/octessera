#!/usr/bin/env python3
from __future__ import annotations

import argparse
import gzip
import lzma
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from rpi_kernel_contract import Contract, ContractError, load_contract

INTERFACE_STRINGS = (b"f_midi_opts_attr_interface_string", b"midi_interface_string")


class PackageValidationError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise PackageValidationError(message)


def _module_name(path: Path) -> str | None:
    name = path.name
    for suffix in (".xz", ".zst", ".gz"):
        name = name.removesuffix(suffix)
    return name[:-3] if name.endswith(".ko") else None


def _decompress(path: Path) -> bytes:
    if path.suffix == ".xz":
        return lzma.decompress(path.read_bytes())
    if path.suffix == ".gz":
        return gzip.decompress(path.read_bytes())
    if path.suffix == ".zst":
        return subprocess.run(["zstd", "-q", "-d", "-c", str(path)], check=True, capture_output=True).stdout
    return path.read_bytes()


def _check_midi_module(path: Path, kernel_release: str) -> None:
    data = _decompress(path)
    vermagic = re.findall(rb"(?<![^\x00])vermagic=([^\x00]+)", data)
    _require(len(vermagic) == 1, "usb_f_midi must contain exactly one vermagic")
    _require(vermagic[0].split(b" ", 1)[0] == kernel_release.encode(), f"wrong usb_f_midi vermagic: {vermagic[0]!r}")
    entries = set(re.split(rb"[\x00\n]", data))
    missing = [marker.decode() for marker in INTERFACE_STRINGS if marker not in entries]
    _require(not missing, f"usb_f_midi is missing interface strings: {', '.join(missing)}")


def validate_package(package: Path, contract: Contract) -> None:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-package-") as temporary:
        root = Path(temporary)
        try:
            subprocess.run(["dpkg-deb", "-x", str(package), str(root)], check=True, capture_output=True)
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise PackageValidationError(f"cannot extract {package}") from error
        modules = [path for path in (root / f"lib/modules/{contract.kernel_release}").rglob("*") if path.is_file()]
        for required in contract.required_modules:
            matches = [path for path in modules if _module_name(path) in {required, required.replace("_", "-")}]
            _require(len(matches) == 1, f"expected exactly one module for {required}, found {len(matches)}")
            if required == "usb_f_midi":
                _check_midi_module(matches[0], contract.kernel_release)
        dtb = root / f"usr/lib/linux-image-{contract.kernel_release}/broadcom/bcm2710-rpi-zero-2-w.dtb"
        _require(dtb.is_file() and dtb.read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "Raspberry Zero 2 W DTB is missing or invalid")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Check the Octessera Raspberry linux-image package.")
    parser.add_argument("package", type=Path)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args(argv)
    try:
        validate_package(args.package, load_contract(Path(__file__).resolve().parents[2], args.manifest))
    except (ContractError, PackageValidationError) as error:
        print(f"Raspberry kernel package check failed: {error}", file=sys.stderr)
        return 1
    print(f"Raspberry kernel package check passed: {args.package}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
