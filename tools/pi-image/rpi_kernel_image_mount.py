#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Iterator


class ImageProofError(ValueError):
    pass


def expected_partitions(loop: str) -> tuple[str, str]:
    try:
        result = subprocess.run(["lsblk", "--json", "--paths", "--output", "NAME,TYPE", loop], capture_output=True, text=True, check=True)
        devices = json.loads(result.stdout).get("blockdevices", [])
    except (FileNotFoundError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        raise ImageProofError("cannot inspect image partition labels and filesystems") from error

    partitions: list[dict[str, Any]] = []

    def visit(nodes: list[dict[str, Any]]) -> None:
        for node in nodes:
            if node.get("type") == "part":
                partitions.append(node)
            visit(node.get("children") or [])

    visit(devices)
    if len(partitions) != 2:
        raise ImageProofError("image must contain exactly two partitions")
    boot = [node for node in partitions if node.get("name") == f"{loop}p1"]
    root = [node for node in partitions if node.get("name") == f"{loop}p2"]
    if len(boot) != 1:
        raise ImageProofError("image must contain exactly one boot partition at p1")
    if len(root) != 1:
        raise ImageProofError("image must contain exactly one root partition at p2")
    if boot[0].get("name") == root[0].get("name"):
        raise ImageProofError("bootfs and rootfs partitions must be distinct")

    def device(node: dict[str, Any]) -> str:
        value = str(node.get("name") or node.get("path") or "")
        return value if value.startswith("/") else f"/dev/{value}"

    return device(boot[0]), device(root[0])


@contextlib.contextmanager
def mounted_image(image: Path) -> Iterator[Path]:
    if image.is_dir():
        yield image
        return
    work = Path(tempfile.mkdtemp(prefix="octessera-rpi-image-proof-"))
    loop = ""
    mounts: list[Path] = []
    try:
        try:
            loop = subprocess.run(["losetup", "--find", "--show", "--read-only", "--partscan", str(image)], capture_output=True, text=True, check=True).stdout.strip()
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ImageProofError(f"cannot attach image {image} read-only") from error
        boot_device, root_device = expected_partitions(loop)
        root_mount = work / "root"
        root_mount.mkdir()
        boot_mount = work / "boot"
        boot_mount.mkdir()
        subprocess.run(["mount", "-t", "vfat", "-o", "ro", boot_device, str(boot_mount)], check=True)
        mounts.append(boot_mount)
        subprocess.run(["mount", "-t", "ext4", "-o", "ro,noload", root_device, str(root_mount)], check=True)
        mounts.append(root_mount)
        firmware_mount = root_mount / "boot/firmware"
        firmware_mount.mkdir(parents=True, exist_ok=True)
        subprocess.run(["mount", "--bind", str(boot_mount), str(firmware_mount)], check=True)
        subprocess.run(["mount", "-o", "remount,bind,ro", str(firmware_mount)], check=True)
        mounts.append(firmware_mount)
        yield root_mount
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageProofError(f"cannot mount image read-only: {image}") from error
    finally:
        for mount in reversed(mounts):
            subprocess.run(["umount", "-l", str(mount)], capture_output=True, check=False)
        if loop:
            subprocess.run(["losetup", "-d", loop], capture_output=True, check=False)
        shutil.rmtree(work, ignore_errors=True)
