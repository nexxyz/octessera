#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Iterator


class ImageMountError(RuntimeError):
    pass


def _run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(command, check=True, capture_output=capture, text=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        rendered = " ".join(command)
        raise ImageMountError(f"command failed while preparing image: {rendered}") from error


def _lsblk(loop: str) -> list[str]:
    result = _run(
        [
            "lsblk",
            "--json",
            "--paths",
            "--output",
            "NAME,TYPE,FSTYPE,PARTLABEL,PARTTYPE,START,SIZE",
            loop,
        ],
        capture=True,
    )
    try:
        entries = json.loads(result.stdout)["blockdevices"]
    except (KeyError, json.JSONDecodeError) as error:
        raise ImageMountError("lsblk returned invalid partition geometry") from error
    partitions: list[str] = []
    for entry in entries:
        for child in entry.get("children", []):
            if child.get("type") == "part" and child.get("name"):
                try:
                    start = int(child.get("start", -1))
                    size = int(child.get("size", 0))
                except (TypeError, ValueError) as error:
                    raise ImageMountError("lsblk returned invalid partition geometry") from error
                if start < 0 or size <= 0:
                    raise ImageMountError("lsblk returned invalid partition geometry")
                partitions.append(str(child["name"]))
    if not partitions:
        for entry in entries:
            if entry.get("type") != "part" or not entry.get("name"):
                continue
            try:
                start = int(entry.get("start", -1))
                size = int(entry.get("size", 0))
            except (TypeError, ValueError) as error:
                raise ImageMountError("lsblk returned invalid partition geometry") from error
            if start < 0 or size <= 0:
                raise ImageMountError("lsblk returned invalid partition geometry")
            partitions.append(str(entry["name"]))
    return sorted(set(partitions))


def _mount(device: str, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    _run(["mount", "-o", "ro", device, str(destination)])


def _unmount(destination: Path) -> None:
    subprocess.run(["umount", "-l", str(destination)], check=False, capture_output=True, text=True)


def _looks_like_root(path: Path) -> bool:
    return (path / "etc/os-release").is_file() and (
        (path / "etc/octessera").is_dir() or (path / "usr").is_dir()
    )


def _looks_like_boot(path: Path) -> bool:
    return (path / "armbianEnv.txt").is_file() or (path / "extlinux/extlinux.conf").is_file() or (
        (path / "Image").exists() and (path / "dtb").exists()
    )


@contextlib.contextmanager
def mounted_image(image: Path) -> Iterator[Path]:
    image = image.resolve()
    if image.is_dir():
        yield image
        return
    if not image.is_file():
        raise ImageMountError(f"final image does not exist: {image}")

    work = Path(tempfile.mkdtemp(prefix="octessera-orange-image-proof-"))
    image_path = image
    decompressed: Path | None = None
    loop = ""
    mounted: list[Path] = []
    try:
        if image.suffixes[-2:] == [".img", ".xz"]:
            decompressed = work / "image.img"
            try:
                with decompressed.open("wb") as output:
                    subprocess.run(["xz", "-dc", str(image)], check=True, stdout=output)
            except (FileNotFoundError, subprocess.CalledProcessError, OSError) as error:
                raise ImageMountError(f"cannot decompress final image: {image}") from error
            image_path = decompressed
        elif image.suffix != ".img":
            raise ImageMountError("Orange image proof accepts only .img or .img.xz")

        try:
            loop = _run(
                ["losetup", "--find", "--show", "--read-only", "--partscan", str(image_path)],
                capture=True,
            ).stdout.strip()
        except ImageMountError:
            raise
        if not loop:
            raise ImageMountError("losetup did not return a read-only loop device")

        partitions = _lsblk(loop)
        if not partitions:
            root_mount = work / "root"
            _mount(loop, root_mount)
            mounted.append(root_mount)
            yield root_mount
            return

        candidates: list[tuple[str, Path, str]] = []
        for index, partition in enumerate(partitions):
            probe = work / f"partition-{index}"
            try:
                _mount(partition, probe)
            except ImageMountError:
                continue
            mounted.append(probe)
            kind = "root" if _looks_like_root(probe) else "boot" if _looks_like_boot(probe) else "other"
            candidates.append((partition, probe, kind))

        roots = [entry for entry in candidates if entry[2] == "root"]
        boots = [entry for entry in candidates if entry[2] == "boot"]
        if len(roots) != 1:
            raise ImageMountError(f"expected exactly one root partition by content, found {len(roots)}")
        if len(boots) > 1:
            raise ImageMountError(f"expected at most one boot partition by content, found {len(boots)}")

        root_mount = roots[0][1]
        if boots and boots[0][1] != root_mount:
            boot_mount = boots[0][1]
            boot_path = root_mount / "boot"
            boot_path.mkdir(parents=True, exist_ok=True)
            _run(["mount", "--bind", str(boot_mount), str(boot_path)])
            mounted.append(boot_path)
            _run(["mount", "-o", "remount,bind,ro", str(boot_path)])
        yield root_mount
    except ImageMountError:
        raise
    except (OSError, subprocess.CalledProcessError) as error:
        raise ImageMountError(f"cannot mount final image read-only: {image}") from error
    finally:
        for destination in reversed(mounted):
            _unmount(destination)
        if loop:
            subprocess.run(["losetup", "-d", loop], check=False, capture_output=True, text=True)
        shutil.rmtree(work, ignore_errors=True)
