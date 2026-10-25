#!/usr/bin/env python3
from __future__ import annotations

import gzip
import importlib.util
import lzma
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace
from typing import Any, Callable

from rpi_kernel_contract import Contract, load_contract


def _load_module(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


HERE = Path(__file__).resolve().parent
VALIDATOR = _load_module(HERE / "validate-rpi-kernel-package.py", "rpi_kernel_validator")
BUILDER = _load_module(HERE / "build-rpi-kernel.py", "rpi_kernel_builder")


def _write(path: Path, value: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())


def _elf_header() -> bytes:
    identity = b"\x7fELF\x02\x01\x01" + bytes(9)
    return struct.pack("<16sHHIQQQIHHHHHH", identity, 1, 183, 1, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0)


def _arm64_image() -> bytes:
    image = bytearray(128)
    image[0x38:0x3C] = b"ARM\x64"
    return bytes(image)


def _midi_module(contract: Contract, *, vermagic: str | None, markers: tuple[str, ...]) -> bytes:
    data = _elf_header() + b"\x00vermagic=" + (vermagic or f"{contract.kernel_release} SMP aarch64").encode() + b"\x00"
    for marker in markers:
        data += marker.encode() + b"\x00"
    return data


def _make_package(
    root: Path,
    contract: Contract,
    name: str,
    *,
    missing: str | None = None,
    vermagic: str | None = None,
    markers: tuple[str, ...] = ("interface_string", "f_midi_opts_attr_interface_string", "midi_interface_string"),
    dtb_magic: bytes = b"\xd0\x0d\xfe\xed",
    compressed_kernel: bool = False,
) -> Path:
    release = contract.kernel_release
    package_root = root / f"fixture-{name}"
    package_root.mkdir(parents=True)
    _write(
        package_root / "DEBIAN/control",
        f"Package: {contract.package_name}\nVersion: {contract.package_version}\nArchitecture: {contract.package_architecture}\n"
        "Maintainer: Octessera tests <tests@octessera.invalid>\nDescription: Raspberry kernel package fixture\n",
    )
    kernel = _arm64_image()
    _write(package_root / f"boot/vmlinuz-{release}", gzip.compress(kernel) if compressed_kernel else kernel)
    module_root = package_root / f"lib/modules/{release}/kernel/fixture"
    for module in contract.required_modules:
        if module == missing:
            continue
        if module == "usb_f_midi":
            _write(module_root / "usb_f_midi.ko.xz", lzma.compress(_midi_module(contract, vermagic=vermagic, markers=markers)))
        else:
            _write(module_root / f"{module}.ko", _elf_header())
    if missing != "dtb":
        _write(package_root / f"usr/lib/linux-image-{release}/broadcom/bcm2710-rpi-zero-2-w.dtb", dtb_magic + b"fixture-dtb")
    _write(package_root / f"usr/lib/linux-image-{release}/overlays/fixture.dtbo", b"fixture-overlay")
    package = root / contract.package_filename
    subprocess.run(["dpkg-deb", "--build", str(package_root), str(package)], check=True, capture_output=True, text=True)
    return package


def _expect_failure(label: str, operation: Callable[[], Any]) -> None:
    try:
        operation()
    except (VALIDATOR.PackageValidationError, BUILDER.BuildError):
        return
    raise AssertionError(f"fixture was accepted: {label}")


def _check_configure(root: Path, contract: Contract) -> None:
    source = root / "source"
    build = root / "build"
    _write(source / "scripts/config", "#!/bin/sh\nexit 0\n")
    log = root / "make-order.log"
    make = root / "make-fixture"
    _write(
        make,
        "#!/bin/sh\n"
        "for arg do last=\"$arg\"; done\n"
        f"printf '%s\\n' \"$last\" >> '{log}'\n"
        f"[ \"$last\" != kernelrelease ] || printf '%s\\n' '{contract.kernel_release}'\n",
    )
    os.chmod(make, 0o755)
    BUILDER._configure(contract, source, build, str(make), "aarch64-linux-gnu-")
    assert log.read_text(encoding="utf-8").splitlines() == ["bcm2711_defconfig", "olddefconfig", "syncconfig", "kernelrelease"]
    wrong = SimpleNamespace(localversion=contract.localversion, kernel_release="6.12.93-v8+")
    _expect_failure("kernelrelease mismatch", lambda: BUILDER._configure(wrong, source, build, str(make), "aarch64-linux-gnu-"))


def _check_builder_commands(contract: Contract) -> None:
    os.environ["LOCALVERSION"] = "fixture-localversion"
    os.environ["DEB_HOST_ARCH"] = "amd64"
    environment = BUILDER._package_environment("aarch64-linux-gnu-", "amd64")
    assert environment["LOCALVERSION"] == ""
    assert environment["DEB_HOST_ARCH"] == "arm64"
    assert environment["DEB_BUILD_ARCH"] == "amd64"
    os.environ.pop("LOCALVERSION")
    os.environ.pop("DEB_HOST_ARCH")
    args = SimpleNamespace(make="make", cross_compile="aarch64-linux-gnu-")
    assert BUILDER._image_package_command(args, Path("build"), contract.kernel_release) == [
        "fakeroot", "--", "make", "-C", "build", "-f", "debian/rules", "ARCH=arm64",
        "CROSS_COMPILE=aarch64-linux-gnu-", f"KERNELRELEASE={contract.kernel_release}", "binary-image",
    ]
    calls: list[list[str]] = []
    original_run = BUILDER._run
    BUILDER._run = lambda command, **kwargs: calls.append(command) or ""
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-builder-test-") as temporary:
        source = Path(temporary) / "source"
        _write(source / "scripts/package/mkdebian", "#!/bin/sh\n")
        try:
            BUILDER._generate_debian_metadata(args, contract, source, Path("build"))
        finally:
            BUILDER._run = original_run
        assert calls == [[
            "make", "-s", "-C", str(source), "O=build", "ARCH=arm64", "CROSS_COMPILE=aarch64-linux-gnu-",
            f"KERNELRELEASE={contract.kernel_release}", f"KDEB_PKGVERSION={contract.package_version}",
            f"KBUILD_RUN_COMMAND={source / 'scripts/package/mkdebian'}", "run-command",
        ]]


def main() -> int:
    contract = load_contract(HERE.parents[1])
    assert all(path.is_file() for path in contract.patch_paths)
    _check_builder_commands(contract)
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-kernel-test-") as temporary:
        work = Path(temporary)
        _check_configure(work / "configure", contract)
        good = _make_package(work, contract, "good")
        VALIDATOR.validate_package(good, contract)
        fixtures: tuple[tuple[str, dict[str, Any]], ...] = (
            ("missing module", {"missing": "snd_rawmidi"}),
            ("missing usb_f_midi", {"missing": "usb_f_midi"}),
            ("missing DTB", {"missing": "dtb"}),
            ("invalid DTB magic", {"dtb_magic": b"\x00\x00\x00\x00"}),
            ("wrong vermagic", {"vermagic": "6.12.92-wrong SMP aarch64"}),
            ("missing ConfigFS string", {"markers": ("interface_string", "midi_interface_string")}),
            ("missing instance string", {"markers": ("interface_string", "f_midi_opts_attr_interface_string")}),
        )
        for label, kwargs in fixtures:
            fixture = _make_package(work / label.replace(" ", "-"), contract, label.replace(" ", "-"), **kwargs)
            _expect_failure(label, lambda fixture=fixture: VALIDATOR.validate_package(fixture, contract))
        second = work / "second" / contract.package_filename
        second.parent.mkdir()
        shutil.copy2(good, second)
        _expect_failure("multiple linux-image packages", lambda: BUILDER.select_exact_linux_image_package([good, second], contract))
    print("Raspberry kernel constructor synthetic tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
