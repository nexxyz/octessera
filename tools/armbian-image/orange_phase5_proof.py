from __future__ import annotations

import re
from collections.abc import Callable
from pathlib import Path
from typing import Any

from orange_initramfs import read_initramfs_entries

Require = Callable[[bool, str], None]


def _matching(entries: dict[str, tuple[int, bytes]], pattern: str) -> list[str]:
    return sorted(path for path in entries if re.fullmatch(pattern, path))


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
    for module in requirements["python_extensions"]:
        matches = _matching(entries, rf"usr/lib/python3\.[0-9]+/lib-dynload/{re.escape(module)}(?:\.[^/]*)*\.so")
        require(len(matches) == 1, f"selected initramfs Python extension closure is missing: {module}")
