from __future__ import annotations

import importlib
import sys
from pathlib import Path, PurePosixPath
from typing import Any


class OrangeBootInventoryError(ValueError):
    pass


def _inventory_module(repository_root: Path) -> Any:
    directory = str(repository_root / "tools/image-respin")
    inserted = directory not in sys.path
    if inserted:
        sys.path.insert(0, directory)
    try:
        return importlib.import_module("inventory")
    finally:
        if inserted:
            sys.path.remove(directory)


def _relative(path: str, label: str) -> str:
    value = PurePosixPath(path)
    if path != value.as_posix() or not path or path.startswith("/") or "\\" in path or ".." in value.parts:
        raise OrangeBootInventoryError(f"{label} is not a safe relative POSIX path")
    return path


def _ancestors(relative: str) -> set[str]:
    parts = PurePosixPath(relative).parts
    return {"/".join(parts[:index]) for index in range(1, len(parts) + 1)}


def protected_inventory(root: Path, contract: dict[str, Any]) -> dict[str, dict[str, Any]]:
    repository_root = Path(__file__).resolve().parents[2]
    module = _inventory_module(repository_root)
    full = module.build_inventory(root)
    scopes = contract["protected_scopes"]
    paths = contract["protected_paths"]
    selected: set[str] = {"."}
    for scope in scopes:
        prefix = _relative(scope["prefix"], f"protected scope {scope['name']}")
        if prefix not in full or full[prefix]["type"] != "directory":
            raise OrangeBootInventoryError(f"protected scope is missing: {prefix}")
        selected.update(relative for relative in full if relative == prefix or relative.startswith(f"{prefix}/"))
    for relative in paths:
        _relative(relative, "protected path")
        if relative not in full:
            raise OrangeBootInventoryError(f"protected path is missing: {relative}")
        selected.update(_ancestors(relative))
    inventory = {relative: full[relative] for relative in sorted(selected)}
    return inventory


def expected_absent_paths(root: Path, contract: dict[str, Any]) -> list[str]:
    module = _inventory_module(Path(__file__).resolve().parents[2])
    full = module.build_inventory(root)
    absent = [_relative(path, "expected-absent path") for path in contract["expected_absent_paths"]]
    present = [path for path in absent if path in full]
    if present:
        raise OrangeBootInventoryError(f"expected-absent protected path is present: {present[0]}")
    return absent


def capture(root: Path, contract: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], list[str]]:
    return protected_inventory(root, contract), expected_absent_paths(root, contract)


__all__ = ["OrangeBootInventoryError", "capture", "expected_absent_paths", "protected_inventory"]
