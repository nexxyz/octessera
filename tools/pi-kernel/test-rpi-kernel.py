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
from types import SimpleNamespace
from pathlib import Path
from typing import Any, Callable

from rpi_kernel_contract import Contract, ContractError, load_contract

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
BUILDER_TESTS = _load_module(HERE / "test-rpi-kernel-builder.py", "rpi_kernel_builder_tests")

def _write(path: Path, value: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())

def _elf_header(*, magic: bool = True, elf_class: int = 2, machine: int = 183) -> bytes:
    identity = bytearray(b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00")
    identity[4] = elf_class
    identity[0:4] = b"\x7fELF" if magic else b"TEXT"
    return struct.pack("<16sHHIQQQIHHHHHH", bytes(identity), 1, machine, 1, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0)

def _arm64_image(valid: bool = True) -> bytes:
    image = bytearray(128)
    if valid:
        image[0x38:0x3c] = b"ARM\x64"
    return bytes(image)

def _midi_module(
    contract: Contract,
    *,
    vermagic: str | None,
    configfs_marker: bool = True,
    default_marker: bool = True,
    generic_markers: tuple[str, ...] = ("interface_string", "interface_string", "prefix_interface_string", "interface_string_suffix"),
    duplicate_vermagic: bool = False,
    magic: bool = True,
    elf_class: int = 2,
    machine: int = 183,
) -> bytes:
    value = vermagic or f"{contract.kernel_release} SMP aarch64"
    data = _elf_header(magic=magic, elf_class=elf_class, machine=machine)
    data += b"\x00vermagic=" + value.encode() + b"\x00"
    if duplicate_vermagic:
        data += b"vermagic=" + value.encode() + b"\x00"
    if configfs_marker:
        data += b"f_midi_opts_attr_interface_string\x00"
    if default_marker:
        data += b"midi_interface_string\x00"
    for generic_marker in generic_markers:
        data += generic_marker.encode() + b"\x00"
    return data

def _make_package(
    root: Path,
    contract: Contract,
    name: str,
    *,
    package_name: str | None = None,
    version: str | None = None,
    architecture: str = "arm64",
    payload_release: str | None = None,
    missing: str | None = None,
    vermagic: str | None = None,
    configfs_marker: bool = True,
    default_marker: bool = True,
    generic_markers: tuple[str, ...] = ("interface_string", "interface_string", "prefix_interface_string", "interface_string_suffix"),
    duplicate_vermagic: bool = False,
    elf_magic: bool = True,
    elf_class: int = 2,
    elf_machine: int = 183,
    required_config: dict[str, str] | None = None,
    config_extra: str | None = None,
    kernel_valid: bool = True,
    compressed_kernel: bool = False,
) -> Path:
    release = payload_release or contract.kernel_release
    package_root = root / f"fixture-{name}"
    package_root.mkdir(parents=True)
    control = (
        f"Package: {package_name or contract.package_name}\n"
        f"Version: {version or contract.package_version}\n"
        f"Architecture: {architecture}\n"
        "Maintainer: Octessera tests <tests@octessera.invalid>\n"
        "Description: Raspberry kernel package fixture\n"
    )
    _write(package_root / "DEBIAN/control", control)
    if missing != "payload":
        kernel = _arm64_image(kernel_valid)
        _write(package_root / f"boot/vmlinuz-{release}", gzip.compress(kernel) if compressed_kernel else kernel)
        config_values = dict(contract.required_config)
        if required_config:
            config_values.update(required_config)
        config = "\n".join(f"{key}={value}" for key, value in config_values.items())
        config += f'\nCONFIG_LOCALVERSION="{contract.config_overrides["CONFIG_LOCALVERSION"]}"\n# CONFIG_LOCALVERSION_AUTO is not set\n'
        if config_extra:
            config += f"{config_extra}\n"
        _write(package_root / f"boot/config-{release}", config)
        _write(package_root / f"boot/System.map-{release}", b"system map")
    module_root = package_root / f"lib/modules/{release}/kernel"
    modules = {
        "usb_f_midi": _midi_module(
            contract,
            vermagic=vermagic,
            configfs_marker=configfs_marker,
            default_marker=default_marker,
            generic_markers=generic_markers,
            duplicate_vermagic=duplicate_vermagic,
            magic=elf_magic,
            elf_class=elf_class,
            machine=elf_machine,
        ),
        "usb_f_uac2": _elf_header(),
        "usb_f_mass_storage": _elf_header(),
        "libcomposite": _elf_header(),
        "snd_seq": _elf_header(),
        "snd_rawmidi": _elf_header(),
        "snd_usb_audio": _elf_header(),
    }
    if missing != "module":
        for module, contents in modules.items():
            suffix = ".ko.xz" if module == "usb_f_midi" else ".ko"
            data = lzma.compress(contents) if suffix.endswith(".xz") else contents
            _write(module_root / "fixture" / f"{module}{suffix}", data)
    if missing != "dtb":
        _write(package_root / f"usr/lib/linux-image-{release}/broadcom/bcm2710-rpi-zero-2-w.dtb", b"\xd0\x0d\xfe\xedfixture-dtb")
    if missing != "overlay":
        _write(package_root / f"usr/lib/linux-image-{release}/overlays/fixture.dtbo", b"fixture-overlay")
    package = root / contract.package_filename
    subprocess.run(["dpkg-deb", "--build", str(package_root), str(package)], check=True, capture_output=True, text=True)
    return package
def _expect_failure(label: str, operation: Callable[[], Any]) -> None:
    try:
        operation()
    except (ContractError, VALIDATOR.PackageValidationError, BUILDER.BuildError):
        return
    raise AssertionError(f"fixture was accepted: {label}")
def _assert_openssl_version_probe() -> None:
    calls: list[list[str]] = []
    original_run = BUILDER._run

    def capture(command: list[str], **kwargs: Any) -> str:
        calls.append(command)
        return "OpenSSL 3.0.0 fixture\n"

    BUILDER._run = capture
    try:
        fact = BUILDER._tool_version("openssl", version_argument="version", recorded_command="openssl version")
    finally:
        BUILDER._run = original_run
    assert calls == [["openssl", "version"]]
    assert fact["command"] == "openssl version"
def _configure_dry_run_fixture(root: Path, contract: Contract, *, wrong_config: bool = False) -> None:
    source = root / "source"
    build = root / "build"
    scripts = source / "scripts"
    scripts.mkdir(parents=True)
    _write(
        scripts / "config",
        "#!/bin/sh\n"
        "set -eu\n"
        "file=\n\n"
        "while [ $# -gt 0 ]; do\n"
        "  case \"$1\" in\n"
        "    --file) file=\"$2\"; shift 2 ;;\n"
        "    --set-str) key=\"$2\"; value=\"$3\"; shift 3 ;;\n"
        "    --disable) disabled=\"$2\"; shift 2 ;;\n"
        "    *) shift ;;\n"
        "  esac\n"
        "done\n"
        "sed -i -e '/^CONFIG_LOCALVERSION=/d' -e '/^CONFIG_LOCALVERSION_AUTO=/d' -e '/^# CONFIG_LOCALVERSION_AUTO is not set$/d' \"$file\"\n"
        "printf 'CONFIG_LOCALVERSION=\"%s\"\\n' \"$value\" >> \"$file\"\n"
        "printf '# CONFIG_LOCALVERSION_AUTO is not set\\n' >> \"$file\"\n",
    )
    values = dict(contract.required_config)
    if wrong_config:
        values["CONFIG_USB_DWC2"] = "m"
    config = "\n".join(f"{key}={value}" for key, value in values.items())
    config += "\nCONFIG_LOCALVERSION=\"-v8\"\nCONFIG_LOCALVERSION_AUTO=y\n"
    log = root / "make-order.log"
    final_localversion = contract.config_overrides["CONFIG_LOCALVERSION"]
    quoted_log = str(log).replace("'", "'\\''")
    make = root / "make-fixture"
    _write(
        make,
        "#!/bin/sh\n"
        "set -eu\n"
        "build=\nlast=\n"
        "for arg do\n"
        "  case \"$arg\" in O=*) build=\"${arg#O=}\" ;; esac\n"
        "  last=\"$arg\"\n"
        "done\n"
        f"printf '%s\\n' \"$last\" >> '{quoted_log}'\n"
        "case \"$last\" in\n"
        "  bcm2711_defconfig)\n"
        "    mkdir -p \"$build\"\n"
        "    cat > \"$build/.config\" <<'OCTESSERA_CONFIG'\n"
        f"{config}"
        "OCTESSERA_CONFIG\n"
        "    ;;\n"
        "  olddefconfig)\n"
        "    mkdir -p \"$build/include/config\" \"$build/include/generated\"\n"
        "    printf '%s\\n' 'CONFIG_LOCALVERSION=\"-v8\"' > \"$build/include/config/auto.conf\"\n"
        "    printf '%s\\n' '#define CONFIG_LOCALVERSION \"-v8\"' > \"$build/include/generated/autoconf.h\"\n"
        "    ;;\n"
        "  syncconfig)\n"
        "    localversion=$(sed -n 's/^CONFIG_LOCALVERSION=\"\\(.*\\)\"$/\\1/p' \"$build/.config\")\n"
        "    printf 'CONFIG_LOCALVERSION=\"%s\"\\n' \"$localversion\" > \"$build/include/config/auto.conf\"\n"
        "    printf '#define CONFIG_LOCALVERSION \"%s\"\\n' \"$localversion\" > \"$build/include/generated/autoconf.h\"\n"
        "    ;;\n"
        "  kernelrelease)\n"
        f"    if grep -qF 'CONFIG_LOCALVERSION=\"{final_localversion}\"' \"$build/include/config/auto.conf\"; then printf '%s\\n' '{contract.kernel_release}'; else printf '%s\\n' '6.12.93-v8+'; fi\n"
        "    ;;\n"
        "  *) exit 1 ;;\n"
        "esac\n",
    )
    os.chmod(make, 0o755)
    config_gate = BUILDER._configure(contract, source, build, str(make), "aarch64-linux-gnu-")
    if not wrong_config:
        assert config_gate["localversion"] == final_localversion
        assert log.read_text(encoding="utf-8").splitlines() == [
            "bcm2711_defconfig",
            "olddefconfig",
            "syncconfig",
            "kernelrelease",
        ]
        assert f'CONFIG_LOCALVERSION="{final_localversion}"' in (build / "include/config/auto.conf").read_text(encoding="utf-8")
        assert f'#define CONFIG_LOCALVERSION "{final_localversion}"' in (build / "include/generated/autoconf.h").read_text(encoding="utf-8")
def main() -> int:
    root = HERE.parents[1]
    contract = load_contract(root)
    _assert_openssl_version_probe()
    original_environment = {key: os.environ.get(key) for key in ("LOCALVERSION", "DEB_HOST_ARCH", "DEB_BUILD_ARCH")}
    os.environ["LOCALVERSION"] = "fixture-localversion"
    os.environ["DEB_HOST_ARCH"] = "arm64"
    os.environ["DEB_BUILD_ARCH"] = "arm64"
    try:
        package_environment = BUILDER._package_environment("aarch64-linux-gnu-", "amd64")
    finally:
        for key, value in original_environment.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    package_command = BUILDER._image_package_command(
        SimpleNamespace(make="make", cross_compile="aarch64-linux-gnu-"), Path("build"), contract.kernel_release
    )
    assert package_command == [
        "fakeroot",
        "--",
        "make",
        "-C",
        "build",
        "-f",
        "debian/rules",
        "ARCH=arm64",
        "CROSS_COMPILE=aarch64-linux-gnu-",
        f"KERNELRELEASE={contract.kernel_release}",
        "binary-image",
    ]
    assert all(not value.startswith("LOCALVERSION=") for value in package_command)
    assert package_environment["LOCALVERSION"] == ""
    assert package_environment["DEB_HOST_ARCH"] == "arm64"
    assert package_environment["DEB_BUILD_ARCH"] == "amd64"
    original_run = BUILDER._run
    def missing_compiler(command: list[str], **kwargs: Any) -> str:
        if command[0] == "missing-aarch64-linux-gnu-gcc":
            raise BUILDER.BuildError("required command is unavailable: missing-aarch64-linux-gnu-gcc")
        return original_run(command, **kwargs)

    BUILDER._run = missing_compiler
    try:
        _expect_failure(
            "missing native preflight compiler",
            lambda: BUILDER.preflight_build_environment(SimpleNamespace(cross_compile="missing-aarch64-linux-gnu-", make="make")),
        )
    finally:
        BUILDER._run = original_run
    package_queries: list[list[str]] = []
    def missing_native_package(command: list[str], **kwargs: Any) -> str:
        if command[0] == "dpkg" and command[1:] == ["--print-architecture"]:
            return "amd64\n"
        if command[0] == "dpkg-query" and command[1] == "-W":
            package_queries.append(command)
            if command[-1] == "debhelper":
                return "13.0\n"
            raise BUILDER.BuildError("native package is not installed")
        return f"{command[0]} fixture 1.0\n"

    BUILDER._run = missing_native_package
    try:
        _expect_failure(
            "missing native OpenSSL or ELF package",
            lambda: BUILDER.preflight_build_environment(SimpleNamespace(cross_compile="aarch64-linux-gnu-", make="make")),
        )
    finally:
        BUILDER._run = original_run
    assert package_queries[0] == ["dpkg-query", "-W", "-f=${Version}", "debhelper"]
    assert package_queries[1][-1] == "libssl-dev:amd64"
    BUILDER_TESTS.run(root, contract, BUILDER)
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-kernel-test-") as temporary:
        work = Path(temporary)
        _configure_dry_run_fixture(work / "dry-good", contract)
        _expect_failure("dry-run semantic config", lambda: _configure_dry_run_fixture(work / "dry-bad", contract, wrong_config=True))
        good = _make_package(work, contract, "good")
        inventory = VALIDATOR.validate_package(good, contract)
        assert inventory["package"]["name"] == contract.package_name
        assert inventory["kernel_release"] == contract.kernel_release
        assert inventory["modules"][0]["interface_string_markers"] == list(VALIDATOR.EXPECTED_INTERFACE_STRINGS)
        assert inventory["dtb_inventory"] and inventory["overlay_inventory"]
        generic_noise = _make_package(work / "generic-noise", contract, "generic-noise")
        VALIDATOR.validate_package(generic_noise, contract)

        fixtures: tuple[tuple[str, dict[str, Any]], ...] = (
            ("wrong name", {"package_name": "linux-image-wrong"}),
            ("wrong version", {"version": "0.0.0-test"}),
            ("wrong architecture", {"architecture": "amd64"}),
            ("wrong release", {"payload_release": "6.12.94-wrong"}),
            ("invalid firmware kernel", {"kernel_valid": False}),
            ("missing payload", {"missing": "payload"}),
            ("missing module", {"missing": "module"}),
            ("missing DTB", {"missing": "dtb"}),
            ("missing overlay", {"missing": "overlay"}),
            ("wrong semantic config", {"required_config": {"CONFIG_USB_DWC2": "m"}}),
            ("wrong vermagic", {"vermagic": "6.12.92-wrong SMP aarch64"}),
            ("missing ConfigFS ABI marker", {"configfs_marker": False}),
            ("missing instance default marker", {"default_marker": False}),
            ("invalid ELF magic", {"elf_magic": False}),
            ("invalid ELF class", {"elf_class": 1}),
            ("invalid ELF machine", {"elf_machine": 62}),
            ("duplicate vermagic", {"duplicate_vermagic": True}),
        )
        for label, kwargs in fixtures:
            fixture = _make_package(work / label.replace(" ", "-"), contract, label.replace(" ", "-"), **kwargs)
            _expect_failure(label, lambda fixture=fixture: VALIDATOR.validate_package(fixture, contract))
        second = work / "second" / contract.package_filename
        second.parent.mkdir()
        shutil.copy2(good, second)
        _expect_failure("multiple linux-image packages", lambda: BUILDER.select_exact_linux_image_package([good, second], contract))

        checksum_dir = work / "checksum"
        checksum_dir.mkdir()
        checksum_package = _make_package(checksum_dir, contract, "checksum")
        checksum_file = checksum_dir / "SHA256SUMS"
        checksum_file.write_text(f"{'0' * 64}  {checksum_package.name}\n", encoding="utf-8")
        _expect_failure("checksum mismatch", lambda: VALIDATOR.validate_package(checksum_package, contract, checksum_file))
    print("Raspberry kernel constructor synthetic tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
