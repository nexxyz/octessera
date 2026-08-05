from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
from pathlib import Path
from pathlib import PurePosixPath
from typing import Any


class InventoryError(ValueError):
    pass


InventoryEntry = dict[str, Any]
Inventory = dict[str, InventoryEntry]

_UNSUPPORTED_XATTR_ERRORS = {
    getattr(os, "ENOTSUP", 95),
    getattr(os, "EOPNOTSUPP", 95),
    getattr(os, "ENODATA", 61),
}


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise InventoryError(f"cannot hash inventory file: {path}") from exc
    return digest.hexdigest()


def _read_xattrs(path: Path) -> tuple[dict[str, str], str | None]:
    listxattr = getattr(os, "listxattr", None)
    getxattr = getattr(os, "getxattr", None)
    if listxattr is None or getxattr is None:
        return {}, None
    try:
        names = listxattr(path, follow_symlinks=False)
    except (OSError, TypeError) as exc:
        if isinstance(exc, OSError) and exc.errno in _UNSUPPORTED_XATTR_ERRORS:
            return {}, None
        if isinstance(exc, TypeError):
            return {}, None
        raise InventoryError(f"cannot list extended attributes: {path}") from exc
    values: dict[str, str] = {}
    for name in sorted(names):
        try:
            value = getxattr(path, name, follow_symlinks=False)
        except (OSError, TypeError) as exc:
            if isinstance(exc, OSError) and exc.errno in _UNSUPPORTED_XATTR_ERRORS:
                continue
            if isinstance(exc, TypeError):
                return {}, None
            raise InventoryError(f"cannot read extended attribute {name}: {path}") from exc
        values[str(name)] = bytes(value).hex()
    return values, values.get("security.capability")


def _entry(path: Path, relative: str) -> InventoryEntry:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise InventoryError(f"cannot inspect inventory path: {path}") from exc
    mode = metadata.st_mode
    if stat.S_ISDIR(mode):
        entry_type = "directory"
    elif stat.S_ISREG(mode):
        entry_type = "file"
    elif stat.S_ISLNK(mode):
        entry_type = "symlink"
    elif stat.S_ISFIFO(mode):
        entry_type = "fifo"
    elif stat.S_ISCHR(mode):
        entry_type = "character-device"
    elif stat.S_ISBLK(mode):
        entry_type = "block-device"
    elif stat.S_ISSOCK(mode):
        entry_type = "socket"
    else:
        entry_type = "other"
    xattrs, capability = _read_xattrs(path)
    return {
        "path": relative,
        "type": entry_type,
        "uid": int(getattr(metadata, "st_uid", 0)),
        "gid": int(getattr(metadata, "st_gid", 0)),
        "mode": stat.S_IMODE(mode),
        "symlink": stat.S_ISLNK(mode),
        "target": os.readlink(path) if stat.S_ISLNK(mode) else None,
        "sha256": _sha256_file(path) if stat.S_ISREG(mode) else None,
        "xattrs": xattrs,
        "capability": capability,
    }


def _walk(path: Path, relative: str, result: Inventory) -> None:
    result[relative] = _entry(path, relative)
    if result[relative]["type"] != "directory":
        return
    try:
        children = sorted(os.scandir(path), key=lambda item: item.name)
    except OSError as exc:
        raise InventoryError(f"cannot enumerate inventory directory: {path}") from exc
    for child in children:
        child_relative = child.name if relative == "." else f"{relative}/{child.name}"
        _walk(Path(child.path), child_relative, result)


def build_inventory(root: Path) -> Inventory:
    root = Path(root)
    try:
        metadata = root.lstat()
    except OSError as exc:
        raise InventoryError(f"inventory root is unavailable: {root}") from exc
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise InventoryError(f"inventory root is not a real directory: {root}")
    result: Inventory = {}
    _walk(root, ".", result)
    return result


def inventory_digest(inventory: Inventory) -> str:
    payload = json.dumps(inventory, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def changed_paths(before: Inventory, after: Inventory) -> list[str]:
    paths = set(before) | set(after)
    return sorted(path for path in paths if before.get(path) != after.get(path))


def _relative_target(relative: str, target: str) -> str:
    if "\\" in target or (len(target) > 1 and target[1] == ":"):
        raise InventoryError(f"symlink target is not a POSIX root path: {target}")
    candidate = PurePosixPath(target.lstrip("/")) if target.startswith("/") else PurePosixPath(relative).parent / PurePosixPath(target)
    parts = candidate.parts
    normalized: list[str] = []
    for part in parts:
        if part in {"", "."}:
            continue
        if part == "..":
            if not normalized:
                raise InventoryError(f"symlink escapes the root: {relative} -> {target}")
            normalized.pop()
        else:
            normalized.append(part)
    return "/".join(normalized)


def virtual_symlink_target(root: Path, path: Path, target: str) -> Path:
    root = Path(root).resolve(strict=False)
    relative = path.relative_to(root).as_posix()
    return root / _relative_target(relative, target)


def ensure_inventory_symlinks_contained(root: Path, inventory: Inventory) -> None:
    root = Path(root).resolve(strict=False)
    for relative, entry in inventory.items():
        if entry["type"] != "symlink":
            continue
        candidate = _relative_target(relative, str(entry["target"]))
        seen: set[str] = set()
        while True:
            components = candidate.split("/")
            link = next(("/".join(components[:index]) for index in range(1, len(components) + 1) if "/".join(components[:index]) in inventory and inventory["/".join(components[:index])]["type"] == "symlink"), None)
            if link is None:
                if candidate not in inventory:
                    raise InventoryError(f"symlink target is missing from the root: {relative} -> {entry['target']}")
                break
            if link in seen:
                raise InventoryError(f"symlink loop in root: {relative}")
            seen.add(link)
            suffix = "/".join(components[len(link.split("/")):])
            target = _relative_target(link, str(inventory[link]["target"]))
            candidate = _relative_target(".", f"{target}/{suffix}" if suffix else target)


def write_inventory(path: Path, inventory: Inventory) -> None:
    payload = json.dumps(inventory, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    Path(path).write_text(payload, encoding="utf-8")


def _remove_readonly(function: Any, path: str, _: Any) -> None:
    if os.name == "nt":
        os.chmod(path, 0o777 if os.path.isdir(path) else 0o666)
    function(path)


def remove_path(path: Path) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
        shutil.rmtree(path, onerror=_remove_readonly)
    else:
        if os.name == "nt":
            os.chmod(path, 0o666)
        path.unlink()

