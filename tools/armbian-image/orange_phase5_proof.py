from __future__ import annotations

import re
import stat
from collections.abc import Callable
from pathlib import Path
from typing import Any

from orange_initramfs import read_initramfs_entries

Require = Callable[[bool, str], None]


def _matching(entries: dict[str, tuple[int, bytes]], pattern: str) -> list[str]:
    return sorted(path for path in entries if re.fullmatch(pattern, path))


def _strict_path(path: Path, mounted_root: Path) -> bool:
    lexical = path.absolute()
    try:
        resolved = path.resolve(strict=True)
        resolved_root = mounted_root.resolve(strict=True)
    except OSError:
        return False
    return resolved == lexical and resolved != resolved_root and resolved_root in resolved.parents


def _strict_directory(path: Path, mounted_root: Path, require: Require) -> bool:
    if path.is_symlink():
        require(False, f"installed Python extension ancestor is a symlink: {path}")
    if not path.exists():
        return False
    require(path.is_dir(), f"installed Python extension ancestor is not a directory: {path}")
    require(_strict_path(path, mounted_root), f"installed Python extension ancestor escapes mounted root: {path}")
    return True


def _installed_extensions(root: Path, module: str, require: Require) -> list[Path]:
    mounted_root = root.resolve(strict=True)
    usr = root / "usr"
    if not _strict_directory(usr, mounted_root, require):
        return []
    lib = usr / "lib"
    if not _strict_directory(lib, mounted_root, require):
        return []
    installed: list[Path] = []
    pattern = rf"{re.escape(module)}(?:\.[^/]*)*\.so"
    for python_dir in sorted(lib.iterdir()):
        if not python_dir.name.startswith("python3."):
            continue
        _strict_directory(python_dir, mounted_root, require)
        dynload = python_dir / "lib-dynload"
        if not _strict_directory(dynload, mounted_root, require):
            continue
        for candidate in sorted(dynload.iterdir()):
            if re.fullmatch(pattern, candidate.name):
                require(_strict_path(candidate, mounted_root), f"installed Python extension escapes mounted root: {candidate}")
                installed.append(candidate)
    return installed


def verify_selected_initramfs(root: Path, initramfs: Path, contract: dict[str, Any], require: Require) -> None:
    entries = read_initramfs_entries(initramfs)
    requirements = contract["selected_initramfs"]
    for path in requirements["required_paths"]:
        require(path in entries, f"selected initramfs is missing Phase 5 path: {path}")
    for path in requirements["forbidden_paths"]:
        require(path not in entries, f"selected initramfs contains forbidden Phase 5 path: {path}")
    for item in requirements["installed_output_matches"]:
        source = root / item["installed_path"]
        require(source.is_file() and not source.is_symlink(), f"installed Phase 5 output is missing: {source}")
        require(entries[item["initramfs_path"]][1] == source.read_bytes(), f"selected initramfs bytes differ: {item['initramfs_path']}")
    for tool in requirements["required_tools"]:
        require(tool in entries, f"selected initramfs is missing required tool: {tool}")
    for relative in requirements["python_files"]:
        matches = _matching(entries, rf"usr/lib/python3\.[0-9]+/{re.escape(relative)}")
        require(len(matches) == 1, f"selected initramfs Python closure is missing: {relative}")
    for module in requirements["required_python_modules"]:
        pattern = rf"usr/lib/python3\.[0-9]+/lib-dynload/{re.escape(module)}(?:\.[^/]*)*\.so"
        archived = _matching(entries, pattern)
        installed = _installed_extensions(root, module, require)
        require(len(archived) <= 1 and len(installed) <= 1, f"selected initramfs Python extension candidates are ambiguous: {module}")
        installed_relative = {path.relative_to(root).as_posix() for path in installed}
        require(installed_relative == set(archived), f"selected initramfs Python extension paths differ: {module}")
        for relative in archived:
            path = root / relative
            require(_strict_path(path, root), f"installed Python extension escapes mounted root: {path}")
            require(path.is_file() and not path.is_symlink(), f"installed Python extension is not a regular file: {path}")
            require(stat.S_ISREG(entries[relative][0]), f"selected initramfs Python extension is not a regular file: {relative}")
            require(entries[relative][1] == path.read_bytes(), f"selected initramfs Python extension differs: {relative}")
