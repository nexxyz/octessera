#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from rpi_kernel_contract import EXPECTED_FIRMWARE_DEVICE_TREE, EXPECTED_FIRMWARE_KERNEL, EXPECTED_FIRMWARE_OVERLAY_PREFIX  # type: ignore[import-not-found]
from rpi_kernel_image import KernelImageError, firmware_kernel_bytes  # type: ignore[import-not-found]
from rpi_kernel_image_mount import ImageProofError
from rpi_kernel_boot_proof import hash_matches


def verify_payload(root: Path, boot: Path, package: Path, package_inventory: dict[str, Any]) -> dict[str, Any]:
    module_root = root / f"lib/modules/{package_inventory['kernel_release']}"
    if not (module_root / "modules.dep").is_file():
        raise ImageProofError("depmod did not create modules.dep for the selected kernel")
    selected_kernel = boot / EXPECTED_FIRMWARE_KERNEL
    selected_dtb = boot / EXPECTED_FIRMWARE_DEVICE_TREE
    kernel_image = package_inventory["kernel_image"]
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-proof-") as temporary:
        extracted = Path(temporary) / "root"
        try:
            subprocess.run(["dpkg-deb", "-x", str(package), str(extracted)], check=True, capture_output=True, text=True)
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ImageProofError("cannot extract the exact package for image proof") from error
        package_kernel = extracted / kernel_image["package_path"]
        try:
            expected_kernel, compression = firmware_kernel_bytes(package_kernel.read_bytes(), str(package_kernel))
        except (OSError, KernelImageError) as error:
            raise ImageProofError(f"cannot read package firmware kernel: {package_kernel}") from error
        if compression != kernel_image["compression"]:
            raise ImageProofError("package kernel compression provenance changed")
        if hashlib.sha256(expected_kernel).hexdigest() != kernel_image["firmware_sha256"]:
            raise ImageProofError("package firmware kernel provenance changed")
        if not selected_kernel.is_file() or selected_kernel.read_bytes() != expected_kernel:
            raise ImageProofError("selected firmware kernel differs from exact package Image")
        package_dtb = extracted / f"usr/lib/linux-image-{package_inventory['kernel_release']}/broadcom/bcm2710-rpi-zero-2-w.dtb"
        if not selected_dtb.is_file() or not package_dtb.is_file() or selected_dtb.read_bytes() != package_dtb.read_bytes():
            raise ImageProofError("selected DTB differs from exact package DTB")
        for entry in package_inventory["overlay_inventory"]:
            hash_matches(boot / EXPECTED_FIRMWARE_OVERLAY_PREFIX / Path(entry["path"]), entry["sha256"], "selected packaged overlay")
        for entry in package_inventory["kernel_payload"]:
            if entry["path"].startswith("lib/modules/"):
                hash_matches(root / entry["path"], entry["sha256"], "installed package module payload")
    overlay = boot / EXPECTED_FIRMWARE_OVERLAY_PREFIX / "i2s-dac-no20.dtbo"
    if not overlay.is_file():
        raise ImageProofError("i2s-dac overlay was not resolved under the custom prefix")
    return {"kernel": str(selected_kernel), "device_tree": str(selected_dtb)}
