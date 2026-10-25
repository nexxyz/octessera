#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

sys.path.insert(0, str(Path(__file__).resolve().parent))

from raspi_firmware_hook_mask import temporarily_mask_raspi_firmware_hooks  # type: ignore[import-not-found]  # noqa: E402
from rpi_kernel_image import assert_firmware_kernel  # type: ignore[import-not-found]  # noqa: E402


KERNEL_RELEASE = "6.12.93-octessera-rpi-v8-0.7.6"
FIRMWARE_KERNEL = "octessera/kernel8.img"
FIRMWARE_DEVICE_TREE = "octessera/bcm2710-rpi-zero-2-w.dtb"
FIRMWARE_INITRAMFS = f"octessera/initrd.img-{KERNEL_RELEASE}"
FIRMWARE_OVERLAY_PREFIX = "octessera/overlays/"


@dataclass(frozen=True)
class Contract:
    kernel_release: str
    package_name: str
    package_version: str
    package_architecture: str

    @property
    def package_filename(self) -> str:
        return f"{self.package_name}_{self.package_version}_{self.package_architecture}.deb"


def image_contract() -> Contract:
    return Contract(
        kernel_release=KERNEL_RELEASE,
        package_name=f"linux-image-{KERNEL_RELEASE}",
        package_version="6.12.93-octessera0.7.6-1",
        package_architecture="arm64",
    )


class ImageInstallError(ValueError):
    pass


def _run(command: list[str]) -> str:
    try:
        result = subprocess.run(command, capture_output=True, text=True, check=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        detail = ""
        if isinstance(error, subprocess.CalledProcessError):
            detail = ((error.stdout or "") + (error.stderr or "")).strip()
        raise ImageInstallError(f"command failed: {' '.join(command)}\n{detail}") from error
    return result.stdout


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ImageInstallError(message)


@contextmanager
def _extracted(package: Path) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-image-package-") as temporary:
        extracted = Path(temporary) / "root"
        _run(["dpkg-deb", "-x", str(package), str(extracted)])
        yield extracted


def _boot_dir(rootfs: Path) -> Path:
    firmware = rootfs / "boot/firmware"
    return firmware if firmware.is_dir() else rootfs / "boot"


def _stock_files(rootfs: Path) -> list[Path]:
    bases = [rootfs / "boot"]
    if (rootfs / "boot/firmware").is_dir():
        bases.append(rootfs / "boot/firmware")
    pattern = re.compile(r"^(?:vmlinuz-|kernel.*\.img$|initrd|Image$|System\.map-|config-)")
    return sorted({path for base in bases for path in base.iterdir() if path.is_file() and pattern.match(path.name)})


def _preserve_stock(rootfs: Path, boot: Path) -> None:
    recovery = boot / "octessera/recovery-stock"
    manifest_path = recovery / "manifest.json"
    if manifest_path.is_file():
        return
    stock = _stock_files(rootfs)
    _require(bool(stock), "stock recovery kernel files are missing")
    recovery.mkdir(parents=True, exist_ok=True)
    entries = []
    for path in stock:
        relative = path.relative_to(rootfs).as_posix()
        target = recovery / relative.replace("/", "_")
        shutil.copy2(path, target, follow_symlinks=True)
        entries.append({"path": relative, "recovery_path": target.relative_to(rootfs).as_posix()})
    manifest_path.write_text(json.dumps(entries, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _copy_payload(rootfs: Path, extracted: Path, contract: Contract) -> None:
    custom = _boot_dir(rootfs) / "octessera"
    overlay_target = custom / "overlays"
    overlay_target.mkdir(parents=True, exist_ok=True)
    image_root = extracted / f"usr/lib/linux-image-{contract.kernel_release}"
    kernel_bytes, _ = assert_firmware_kernel(extracted / f"boot/vmlinuz-{contract.kernel_release}")
    (custom / "kernel8.img").write_bytes(kernel_bytes)
    shutil.copyfile(image_root / "broadcom/bcm2710-rpi-zero-2-w.dtb", custom / "bcm2710-rpi-zero-2-w.dtb")
    overlays = sorted(path for path in (image_root / "overlays").rglob("*.dtbo") if path.is_file())
    _require(bool(overlays), "Raspberry kernel package has no DT overlays")
    for source in overlays:
        target = overlay_target / source.relative_to(image_root / "overlays")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)


def _run_in_root(rootfs: Path, command: list[str]) -> None:
    _run(command if rootfs == Path("/") else ["chroot", str(rootfs), *command])


def _install_package(rootfs: Path, package: Path) -> None:
    staged = rootfs / "var/lib/octessera/rpi-kernel" / package.name
    staged.parent.mkdir(parents=True, exist_ok=True)
    if staged.resolve() != package.resolve():
        shutil.copy2(package, staged)
    with temporarily_mask_raspi_firmware_hooks(rootfs):
        _run_in_root(rootfs, ["dpkg", "-i", f"/var/lib/octessera/rpi-kernel/{package.name}"])


def _write_selectors(config: Path) -> None:
    _require(config.is_file(), f"missing Raspberry firmware config: {config}")
    kept = []
    for line in config.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        key = stripped.split("=", 1)[0].strip() if "=" in stripped else stripped.split(None, 1)[0] if stripped else ""
        if key in {"kernel", "device_tree", "overlay_prefix", "initramfs", "auto_initramfs"}:
            continue
        kept.append(line)
    overlay_index = next((index for index, line in enumerate(kept) if line.strip().startswith("dtoverlay=")), len(kept))
    kept.insert(overlay_index, f"overlay_prefix={FIRMWARE_OVERLAY_PREFIX}")
    kept.extend([
        "",
        "# --- octessera custom kernel selectors ---",
        f"kernel={FIRMWARE_KERNEL}",
        f"device_tree={FIRMWARE_DEVICE_TREE}",
        f"initramfs {FIRMWARE_INITRAMFS} followkernel",
    ])
    config.write_text("\n".join(kept) + "\n", encoding="utf-8")


def _finalize(rootfs: Path, package: Path, contract: Contract) -> None:
    boot = _boot_dir(rootfs)
    with _extracted(package) as extracted:
        _copy_payload(rootfs, extracted, contract)
    _run_in_root(rootfs, ["depmod", "-a", contract.kernel_release])
    initramfs_source = rootfs / f"boot/initrd.img-{contract.kernel_release}"
    if initramfs_source.exists():
        initramfs_source.unlink()
    with temporarily_mask_raspi_firmware_hooks(rootfs):
        _run_in_root(rootfs, ["update-initramfs", "-c", "-k", contract.kernel_release])
    _require(initramfs_source.is_file(), f"update-initramfs did not create {initramfs_source}")
    initramfs_target = boot / FIRMWARE_INITRAMFS
    initramfs_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(initramfs_source, initramfs_target)
    _write_selectors(boot / "config.txt")


def install_image(rootfs: Path, package: Path, contract: Contract) -> None:
    _require(package.is_file(), f"missing kernel package: {package}")
    _preserve_stock(rootfs, _boot_dir(rootfs))
    _install_package(rootfs, package)
    with _extracted(package) as extracted:
        _copy_payload(rootfs, extracted, contract)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Install and select the pinned Octessera Raspberry kernel image.")
    parser.add_argument("--rootfs", type=Path, default=Path("/"))
    parser.add_argument("--package", type=Path)
    parser.add_argument("--finalize", action="store_true")
    args = parser.parse_args(argv)
    contract = image_contract()
    try:
        rootfs = args.rootfs.resolve()
        package = (args.package or rootfs / "var/lib/octessera/rpi-kernel" / contract.package_filename).resolve()
        if args.finalize:
            _finalize(rootfs, package, contract)
        else:
            install_image(rootfs, package, contract)
    except (ImageInstallError, OSError, ValueError) as error:
        print(f"Raspberry kernel image installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
