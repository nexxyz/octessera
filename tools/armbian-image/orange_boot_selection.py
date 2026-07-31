#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path
from typing import cast


class BootSelectionError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise BootSelectionError(message)


def safe_resolve(root: Path, path: Path, label: str) -> Path:
    try:
        resolved = path.resolve(strict=True)
        resolved.relative_to(root.resolve())
    except (OSError, ValueError) as error:
        raise BootSelectionError(f"{label} escapes the final image root: {path}") from error
    return resolved


def selector_path(root: Path, boot: Path, value: str, label: str) -> Path:
    raw = value.strip()
    require(bool(raw and not raw.startswith("../") and "\x00" not in raw), f"invalid {label} selector")
    if raw.startswith("/boot/") or raw.startswith("/usr/lib/"):
        candidate = root / raw.lstrip("/")
    else:
        candidate = boot / raw.lstrip("/") if raw.startswith("/") else boot / raw
    return safe_resolve(root, candidate, label)


def select_dtb(root: Path, boot: Path, value: str, release: str) -> Path:
    raw = value.strip()
    require(bool(re.fullmatch(r"/?[A-Za-z0-9._/-]+", raw)) and ".." not in raw.split("/"), "invalid fdt selector")
    require(Path(raw).name == "sun50i-h618-orangepi-zero2w.dtb", "boot selector does not select the required Orange Zero 2W DTB")
    relative = raw.lstrip("/")
    candidates: list[Path] = []
    if relative.startswith("boot/") or relative.startswith("usr/lib/"):
        candidates.append(root / relative)
    elif relative.startswith("dtb/"):
        candidates.append(boot / relative)
    elif relative.startswith("allwinner/"):
        candidates.extend((boot / "dtb" / relative, boot / relative))
    elif relative == Path(raw).name:
        candidates.append(boot / "dtb/allwinner" / relative)
    else:
        candidates.append(boot / relative)
    candidates.extend(
        (
            boot / f"dtb-{release}/allwinner/{Path(raw).name}",
            root / f"usr/lib/linux-image-{release}/allwinner/{Path(raw).name}",
        )
    )
    resolved: list[Path] = []
    for candidate in candidates:
        if candidate.exists():
            target = safe_resolve(root, candidate, "fdt")
            if target not in resolved:
                resolved.append(target)
    require(bool(resolved), "boot selector DTB does not resolve")
    preferred = next((path for path in resolved if str(path).startswith(str(boot / "dtb-"))), resolved[0])
    require(all(path.read_bytes() == preferred.read_bytes() for path in resolved), "conflicting boot-selected Orange DTB copies")
    return preferred


def read_fdtfile(path: Path) -> str | None:
    values: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.lstrip().startswith("#"):
            require("fdtfile=" not in line, "commented fdtfile assignment is invalid")
        elif line.startswith("fdtfile="):
            values.append(line.split("=", 1)[1].strip())
        elif re.search(r"(^|[^_A-Za-z0-9])fdtfile\s*=", line):
            raise BootSelectionError("malformed fdtfile assignment")
    require(len(values) <= 1, "Armbian boot selector must contain at most one fdtfile")
    return values[0] if values else None


def parse_boot_selectors(root: Path, release: str) -> dict[str, Path]:
    boot = root / "boot"
    require(boot.is_dir(), "final image has no /boot directory")
    extlinux = boot / "extlinux/extlinux.conf"
    selectors: dict[str, list[str]] = {"linux": [], "initrd": [], "fdt": []}
    if extlinux.is_file():
        for line in extlinux.read_text(encoding="utf-8").splitlines():
            match = re.match(r"^\s*(LINUX|INITRD|FDT)\s+(\S+)\s*$", line, re.IGNORECASE)
            if match:
                selectors[match.group(1).lower()].append(match.group(2))
        require(all(len(values) == 1 for values in selectors.values()), "extlinux boot selectors must select one kernel, initramfs, and DTB")
        selected = {
            "linux": selector_path(root, boot, selectors["linux"][0], "linux"),
            "initrd": selector_path(root, boot, selectors["initrd"][0], "initrd"),
            "fdt": select_dtb(root, boot, selectors["fdt"][0], release),
        }
        if (boot / "armbianEnv.txt").is_file():
            fdtfile = read_fdtfile(boot / "armbianEnv.txt")
            if fdtfile is not None:
                require(select_dtb(root, boot, fdtfile, release).read_bytes() == selected["fdt"].read_bytes(), "Armbian and extlinux DTB selectors conflict")
        return selected
    env = boot / "armbianEnv.txt"
    require(env.is_file(), "final image has no Armbian boot selector")
    fdtfile = read_fdtfile(env)
    require(fdtfile is not None, "Armbian boot selector must contain one fdtfile")
    kernel = boot / "Image"
    initrd = boot / "uInitrd" if (boot / "uInitrd").exists() else boot / f"initrd.img-{release}"
    require(kernel.exists() and initrd.exists(), "Armbian boot selector is missing Image or uInitrd")
    return {
        "linux": safe_resolve(root, kernel, "linux"),
        "initrd": safe_resolve(root, initrd, "initrd"),
        "fdt": select_dtb(root, boot, cast(str, fdtfile), release),
    }
