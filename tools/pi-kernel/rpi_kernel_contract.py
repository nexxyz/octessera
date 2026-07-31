from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


EXPECTED_PATCH_ORDER = (
    "zzzz-0001-usb-gadget-f-midi-configfs-interface-string.patch",
    "zzzz-0002-usb-gadget-f-midi-instance-local-string.patch",
)
EXPECTED_SOURCE_COMMIT = "d8ab4e908235da7727f22dd36ad5af224671677d"
EXPECTED_SOURCE_RELEASE = "6.12.93"
EXPECTED_KERNEL_RELEASE = "6.12.93-octessera-rpi-v8-0.7.5"
EXPECTED_PACKAGE_VERSION = "6.12.93-octessera0.7.5-1"
EXPECTED_PACKAGE_NAME = f"linux-image-{EXPECTED_KERNEL_RELEASE}"
EXPECTED_CONFIG_PATH = "arch/arm64/configs/bcm2711_defconfig"
EXPECTED_CONFIG_SHA256 = "f78d6805ffd78b503ec64c9107e3bc82bf734cf50b1123d349f558ce483a9406"
EXPECTED_LOCALVERSION = "-octessera-rpi-v8-0.7.5"
EXPECTED_REQUIRED_CONFIG = {
    "CONFIG_USB_DWC2": "y",
    "CONFIG_USB_CONFIGFS": "m",
    "CONFIG_USB_CONFIGFS_F_MIDI": "y",
    "CONFIG_USB_CONFIGFS_F_UAC2": "y",
    "CONFIG_USB_CONFIGFS_MASS_STORAGE": "y",
    "CONFIG_SND_SEQUENCER": "m",
    "CONFIG_SND_RAWMIDI": "m",
    "CONFIG_SND_USB_AUDIO": "m",
}
EXPECTED_INTERFACE_STRINGS = (
    "f_midi_opts_attr_interface_string",
    "midi_interface_string",
)
EXPECTED_NATIVE_PACKAGES = {
    "libssl-dev": ("/usr/include/openssl/ssl.h",),
    "libelf-dev": ("/usr/include/libelf.h",),
}
EXPECTED_PACKAGE_BUILDER = {
    "kind": "linux-generated-debian-rules",
    "generator": "scripts/package/mkdebian",
    "rules_source": "scripts/package/debian/rules",
    "rules_sha256": "66166a03eac2b439b68bc0a92e7b60f60eed24685eea10dae8af97edc9629b48",
    "target": "binary-image",
    "fakeroot": True,
    "binary_package_scope": ["linux-image"],
}
EXPECTED_PATCH_SHA256 = (
    "bd0f3cbb15b29561849b3d68ae5fc4443fb056d083bf89fa6e5272d072a10df0",
    "cf5c2efc60d5e5f8a019f26632b43d96f4cf2681101c50152a78a95b1cc8ee0b",
)
EXPECTED_PAYLOAD = (
    f"boot/vmlinuz-{EXPECTED_KERNEL_RELEASE}",
    f"boot/config-{EXPECTED_KERNEL_RELEASE}",
    f"boot/System.map-{EXPECTED_KERNEL_RELEASE}",
    f"lib/modules/{EXPECTED_KERNEL_RELEASE}/",
    f"usr/lib/linux-image-{EXPECTED_KERNEL_RELEASE}/broadcom/bcm2710-rpi-zero-2-w.dtb",
    f"usr/lib/linux-image-{EXPECTED_KERNEL_RELEASE}/overlays/",
)
EXPECTED_MODULES = (
    "usb_f_midi",
    "usb_f_uac2",
    "usb_f_mass_storage",
    "libcomposite",
    "snd_seq",
    "snd_rawmidi",
    "snd_usb_audio",
)
EXPECTED_FIRMWARE_KERNEL = "octessera/kernel8.img"
EXPECTED_FIRMWARE_DEVICE_TREE = "octessera/bcm2710-rpi-zero-2-w.dtb"
EXPECTED_FIRMWARE_INITRAMFS = f"octessera/initrd.img-{EXPECTED_KERNEL_RELEASE}"
EXPECTED_FIRMWARE_OVERLAY_PREFIX = "octessera/overlays/"
HEX64 = re.compile(r"^[0-9a-f]{64}$")
HEX40 = re.compile(r"^[0-9a-f]{40}$")


class ContractError(ValueError):
    pass


@dataclass(frozen=True)
class Contract:
    root: Path
    manifest_path: Path
    manifest: dict[str, Any]
    patch_root: Path
    patch_paths: tuple[Path, ...]
    source_repository: str

    @property
    def source_commit(self) -> str:
        return self.manifest["kernels"]["raspberry"]["commit"]

    @property
    def source_release(self) -> str:
        return self.manifest["kernels"]["raspberry"]["release"]

    @property
    def kernel_release(self) -> str:
        return self.manifest["kernels"]["raspberry"]["kernel_release"]

    @property
    def package_name(self) -> str:
        return self.manifest["kernels"]["raspberry"]["package"]["name"]

    @property
    def package_version(self) -> str:
        return self.manifest["kernels"]["raspberry"]["package"]["version"]

    @property
    def package_architecture(self) -> str:
        return self.manifest["kernels"]["raspberry"]["package"]["architecture"]

    @property
    def config_path(self) -> str:
        return self.manifest["kernels"]["raspberry"]["config_base"]["path"]

    @property
    def config_sha256(self) -> str:
        return self.manifest["kernels"]["raspberry"]["config_base"]["sha256"]

    @property
    def config_overrides(self) -> dict[str, str]:
        return self.manifest["kernels"]["raspberry"]["config_overrides"]

    @property
    def package_builder(self) -> dict[str, Any]:
        return self.manifest["kernels"]["raspberry"]["package"]["builder"]

    @property
    def required_config(self) -> dict[str, str]:
        return self.manifest["kernels"]["raspberry"]["required_config"]

    @property
    def required_payload(self) -> tuple[str, ...]:
        return tuple(self.manifest["kernels"]["raspberry"]["required_payload"])

    @property
    def required_modules(self) -> tuple[str, ...]:
        return tuple(self.manifest["kernels"]["raspberry"]["required_modules"])

    @property
    def package_filename(self) -> str:
        return f"{self.package_name}_{self.package_version}_{self.package_architecture}.deb"


def image_contract() -> Contract:
    raspberry = {
        "repository": "https://github.com/raspberrypi/linux.git",
        "commit": EXPECTED_SOURCE_COMMIT,
        "release": EXPECTED_SOURCE_RELEASE,
        "kernel_release": EXPECTED_KERNEL_RELEASE,
        "package": {
            "name": EXPECTED_PACKAGE_NAME,
            "version": EXPECTED_PACKAGE_VERSION,
            "architecture": "arm64",
            "builder": dict(EXPECTED_PACKAGE_BUILDER),
        },
        "config_base": {"path": EXPECTED_CONFIG_PATH, "sha256": EXPECTED_CONFIG_SHA256},
        "config_overrides": {
            "CONFIG_LOCALVERSION": EXPECTED_LOCALVERSION,
            "CONFIG_LOCALVERSION_AUTO": "n",
        },
        "required_config": dict(EXPECTED_REQUIRED_CONFIG),
        "required_payload": list(EXPECTED_PAYLOAD),
        "required_modules": list(EXPECTED_MODULES),
    }
    return Contract(
        root=Path("/"),
        manifest_path=Path("/etc/octessera/rpi-kernel-contract.json"),
        manifest={"kernels": {"raspberry": raspberry}},
        patch_root=Path("/"),
        patch_paths=(),
        source_repository=raspberry["repository"],
    )


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def _require_string(value: Any, message: str) -> str:
    _require(isinstance(value, str) and value != "", message)
    return value


def load_contract(root: Path, manifest_path: Path | None = None) -> Contract:
    root = root.resolve()
    path = (manifest_path or root / "tools/kernel-patches/orange-midi-interface-manifest.json").resolve()
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot load manifest {path}: {error}") from error

    _require(manifest.get("schema") == 1, "manifest schema must be 1")
    _require(manifest.get("patch_root") == "userpatches/kernel/archive/sunxi-6.18", "unexpected patch root")
    _require(tuple(manifest.get("patch_order", ())) == EXPECTED_PATCH_ORDER, "unexpected manifest patch order")
    patches = manifest.get("patches", {})
    accepted = patches.get("accepted_upstream", {})
    follow_up = patches.get("octessera_follow_up", {})
    _require(accepted.get("sha256") == EXPECTED_PATCH_SHA256[0], "unexpected accepted patch hash in manifest")
    _require(follow_up.get("sha256") == EXPECTED_PATCH_SHA256[1], "unexpected follow-up patch hash in manifest")
    _require(HEX40.fullmatch(accepted.get("commit", "")) is not None, "accepted patch commit is not a full SHA")

    raspberry = manifest.get("kernels", {}).get("raspberry", {})
    _require(raspberry.get("repository") == "https://github.com/raspberrypi/linux.git", "unexpected Raspberry repository")
    _require(raspberry.get("commit") == EXPECTED_SOURCE_COMMIT, "unexpected Raspberry source commit")
    _require(raspberry.get("release") == EXPECTED_SOURCE_RELEASE, "unexpected Raspberry source release")
    _require(raspberry.get("kernel_release") == EXPECTED_KERNEL_RELEASE, "unexpected Raspberry kernel release")
    package = raspberry.get("package", {})
    _require(package.get("builder") == EXPECTED_PACKAGE_BUILDER, "unexpected Raspberry package builder")
    _require(package.get("name") == EXPECTED_PACKAGE_NAME, "unexpected Raspberry package name")
    _require(package.get("version") == EXPECTED_PACKAGE_VERSION, "unexpected Raspberry package version")
    _require(package.get("architecture") == "arm64", "unexpected Raspberry package architecture")
    _require(raspberry.get("config_overrides") == {
        "CONFIG_LOCALVERSION": EXPECTED_LOCALVERSION,
        "CONFIG_LOCALVERSION_AUTO": "n",
    }, "unexpected Raspberry config overrides")
    _require(raspberry.get("required_config") == EXPECTED_REQUIRED_CONFIG, "unexpected Raspberry required config")
    _require(tuple(raspberry.get("required_payload", ())) == EXPECTED_PAYLOAD, "unexpected required Raspberry payload")
    _require(tuple(raspberry.get("required_modules", ())) == EXPECTED_MODULES, "unexpected required Raspberry modules")
    config_base = raspberry.get("config_base", {})
    _require(config_base.get("path") == EXPECTED_CONFIG_PATH, "unexpected Raspberry config base path")
    _require(config_base.get("sha256") == EXPECTED_CONFIG_SHA256, "unexpected Raspberry config base hash")

    patch_root = (root / manifest["patch_root"]).resolve()
    _require(patch_root.is_dir(), f"missing patch root: {patch_root}")
    actual_patch_paths = tuple(sorted(path.relative_to(patch_root).as_posix() for path in patch_root.rglob("*.patch")))
    _require(actual_patch_paths == tuple(sorted(EXPECTED_PATCH_ORDER)), "patch root differs from manifest patch order")
    patch_paths = tuple(patch_root / name for name in EXPECTED_PATCH_ORDER)
    for patch_path, expected_hash in zip(patch_paths, EXPECTED_PATCH_SHA256):
        _require(patch_path.is_file(), f"missing patch: {patch_path}")
        _require(sha256_file(patch_path) == expected_hash, f"patch hash mismatch: {patch_path}")

    return Contract(
        root=root,
        manifest_path=path,
        manifest=manifest,
        patch_root=patch_root,
        patch_paths=patch_paths,
        source_repository=raspberry["repository"],
    )


def config_override_lines(contract: Contract) -> tuple[str, str]:
    localversion = contract.config_overrides["CONFIG_LOCALVERSION"]
    return (f'CONFIG_LOCALVERSION="{localversion}"', "# CONFIG_LOCALVERSION_AUTO is not set")


def _config_values(lines: list[str]) -> dict[str, list[str]]:
    values: dict[str, list[str]] = {}
    for line in lines:
        match = re.fullmatch(r"(CONFIG_[A-Za-z0-9_]+)=(.*)", line)
        if match:
            values.setdefault(match.group(1), []).append(match.group(2))
            continue
        match = re.fullmatch(r"# (CONFIG_[A-Za-z0-9_]+) is not set", line)
        if match:
            values.setdefault(match.group(1), []).append("n")
    return values


def _require_config_value(values: dict[str, list[str]], key: str, expected: str) -> None:
    actual = values.get(key, [])
    _require(len(actual) == 1 and actual[0] == expected, f"final config requires exactly {key}={expected}")


def assert_final_config(path: Path, contract: Contract) -> dict[str, Any]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise ContractError(f"cannot read final kernel config {path}: {error}") from error
    values = _config_values(lines)
    _require_config_value(values, "CONFIG_LOCALVERSION", f'"{contract.config_overrides["CONFIG_LOCALVERSION"]}"')
    _require_config_value(values, "CONFIG_LOCALVERSION_AUTO", "n")
    for key, expected in contract.required_config.items():
        _require_config_value(values, key, expected)
    return {
        "localversion": contract.config_overrides["CONFIG_LOCALVERSION"],
        "localversion_auto": "n",
        "required_config": dict(contract.required_config),
        "sha256": sha256_file(path),
    }
