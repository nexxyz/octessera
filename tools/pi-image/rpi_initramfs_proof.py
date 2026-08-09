#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path, PurePosixPath
from typing import Any

def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _safe_name(name: str) -> str:
    normalized = name.removeprefix("./")
    path = PurePosixPath(normalized)
    _require(bool(normalized) and not path.is_absolute() and ".." not in path.parts, f"unsafe initramfs path: {name}")
    return path.as_posix()


def _records(listing: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    seen: set[str] = set()
    for line in listing.splitlines():
        fields = line.split(maxsplit=8)
        if not fields or fields[0] == ".":
            continue
        _require(len(fields) == 9 and len(fields[0]) == 10, f"unparseable initramfs listing entry: {line}")
        try:
            links = int(fields[1])
            size = int(fields[4])
        except ValueError as error:
            raise ValueError(f"unparseable initramfs metadata: {line}") from error
        _require(links >= 0 and size >= 0, f"negative initramfs metadata: {line}")
        entry = fields[8]
        name, separator, target = entry.partition(" -> ")
        _require((separator != "") == (fields[0][0] == "l"), f"initramfs symlink metadata is inconsistent: {line}")
        name = _safe_name(name)
        _require(name not in seen, f"duplicate initramfs entry: {name}")
        seen.add(name)
        records.append(
            {
                "name": name,
                "mode": fields[0],
                "type": fields[0][0],
                "links": links,
                "size": size,
                "target": target if separator else None,
            }
        )
    return records


def parse_initramfs_listing(listing: str) -> list[dict[str, Any]]:
    return _records(listing)


def validate_selected_initramfs_contract(selected: dict[str, Any]) -> None:
    expected_keys = {
        "path",
        "byte_bindings",
        "required_symlinks",
        "required_regular_executables",
        "forbidden_entry_prefixes",
        "size_limits",
        "required_module_names",
    }
    _require(set(selected) == expected_keys, "selected initramfs contract fields are not exact")
    _require(isinstance(selected["path"], str), "selected initramfs path is invalid")
    _safe_name(selected["path"])

    bindings = selected["byte_bindings"]
    _require(bool(isinstance(bindings, list) and bindings), "selected initramfs byte bindings are empty")
    roles: set[str] = set()
    archive_paths: set[str] = set()
    for binding in bindings:
        _require(isinstance(binding, dict), "selected initramfs byte binding is invalid")
        rootfs_type = binding.get("rootfs_type")
        expected = {"role", "archive_path", "rootfs_path", "rootfs_type"}
        if rootfs_type == "symlink":
            expected.update({"rootfs_target", "rootfs_resolution"})
        _require(set(binding) == expected, "selected initramfs byte binding fields are not exact")
        for key in ("role", "archive_path", "rootfs_path"):
            _require(isinstance(binding[key], str) and binding[key], f"selected initramfs byte binding {key} is invalid")
        _safe_name(binding["archive_path"])
        _safe_name(binding["rootfs_path"])
        _require(binding["role"] not in roles and binding["archive_path"] not in archive_paths, "selected initramfs byte bindings contain duplicates")
        roles.add(binding["role"])
        archive_paths.add(binding["archive_path"])
        if rootfs_type == "symlink":
            _require(isinstance(binding["rootfs_target"], str) and binding["rootfs_target"], "selected initramfs rootfs symlink target is invalid")
            resolution = binding["rootfs_resolution"]
            _require(isinstance(resolution, dict) and set(resolution) == {"current_path", "current_target_pattern", "resolved_path", "resolved_type"}, "selected initramfs rootfs resolution is invalid")
            _safe_name(resolution["current_path"])
            _safe_name(resolution["resolved_path"])
            _require(isinstance(resolution["current_target_pattern"], str) and resolution["current_target_pattern"], "selected initramfs rootfs resolution pattern is invalid")
            try:
                re.compile(resolution["current_target_pattern"])
            except re.error as error:
                raise ValueError("selected initramfs rootfs resolution pattern is invalid") from error
            _require(resolution["resolved_type"] == "regular-executable", "selected initramfs rootfs resolution type is invalid")
        else:
            _require(rootfs_type == "regular-executable", "selected initramfs rootfs binding type is invalid")
    _require(roles == {"splash-script", "runtime"}, "selected initramfs byte binding roles are not exact")

    symlinks = selected["required_symlinks"]
    _require(bool(isinstance(symlinks, list) and symlinks), "selected initramfs symlink requirements are empty")
    required_paths = set(archive_paths)
    for symlink in symlinks:
        _require(isinstance(symlink, dict) and set(symlink) == {"path", "target"}, "selected initramfs symlink requirement is invalid")
        _require(isinstance(symlink["path"], str) and isinstance(symlink["target"], str), "selected initramfs symlink requirement types are invalid")
        _safe_name(symlink["path"])
        _require(symlink["target"] and not PurePosixPath(symlink["target"]).is_absolute() and ".." not in PurePosixPath(symlink["target"]).parts, "selected initramfs symlink target is unsafe")
        _require(symlink["path"] not in required_paths, "selected initramfs requirements contain duplicate paths")
        required_paths.add(symlink["path"])

    executables = selected["required_regular_executables"]
    _require(bool(isinstance(executables, list) and executables), "selected initramfs executable requirements are empty")
    for executable in executables:
        _require(isinstance(executable, str), "selected initramfs executable requirement is invalid")
        _safe_name(executable)
        _require(executable not in required_paths, "selected initramfs requirements contain duplicate paths")
        required_paths.add(executable)

    forbidden_prefixes = selected["forbidden_entry_prefixes"]
    _require(bool(isinstance(forbidden_prefixes, list) and forbidden_prefixes and all(isinstance(prefix, str) for prefix in forbidden_prefixes)), "selected initramfs forbidden prefixes are invalid")
    _require(len(forbidden_prefixes) == len(set(forbidden_prefixes)), "selected initramfs forbidden prefixes contain duplicates")
    for prefix in forbidden_prefixes:
        _require(prefix.endswith("/") and prefix == PurePosixPath(prefix[:-1]).as_posix() + "/", "selected initramfs forbidden prefix is not normalized")
        _safe_name(prefix[:-1])

    limits = selected["size_limits"]
    _require(isinstance(limits, dict) and set(limits) == {"min_regular_bytes", "max_entry_bytes", "max_total_regular_bytes", "symlink_size"}, "selected initramfs size semantics are invalid")
    for key in ("min_regular_bytes", "max_entry_bytes", "max_total_regular_bytes"):
        _require(isinstance(limits[key], int) and not isinstance(limits[key], bool) and limits[key] >= 1, f"selected initramfs size limit is invalid: {key}")
    _require(limits["min_regular_bytes"] <= limits["max_entry_bytes"] <= limits["max_total_regular_bytes"], "selected initramfs size limits are not ordered")
    _require(limits["symlink_size"] == "target-bytes", "selected initramfs symlink size semantics are invalid")

    modules = selected["required_module_names"]
    _require(bool(isinstance(modules, list) and modules and all(isinstance(module, str) and module for module in modules)), "selected initramfs module requirements are invalid")
    _require(len(modules) == len(set(modules)), "selected initramfs module requirements contain duplicates")


def _size_limits(selected: dict[str, Any]) -> dict[str, int]:
    validate_selected_initramfs_contract(selected)
    return selected["size_limits"]


def _required_regular_files(selected: dict[str, Any]) -> list[str]:
    validate_selected_initramfs_contract(selected)
    return [binding["archive_path"] for binding in selected["byte_bindings"]]


def _required_archive_paths(selected: dict[str, Any]) -> list[str]:
    validate_selected_initramfs_contract(selected)
    return _required_regular_files(selected) + [entry["path"] for entry in selected["required_symlinks"]] + selected["required_regular_executables"]


def validate_command_records(records: list[dict[str, Any]], selected: dict[str, Any]) -> None:
    limits = _size_limits(selected)
    by_name = {record["name"]: record for record in records}
    names = set(by_name)
    for module in selected["required_module_names"]:
        _require(any(module in name for name in names), f"selected initramfs is missing required module: {module}")
    for symlink in selected["required_symlinks"]:
        name = symlink["path"]
        record = by_name.get(name)
        if record is None:
            raise ValueError(f"selected initramfs is missing command symlink: {name}")
        _require(record["type"] == "l", f"selected initramfs command entry is not a symlink: {name}")
        _require(record["links"] == 1, f"selected initramfs command symlink is a hardlink: {name}")
        _require(record["target"] == symlink["target"], f"selected initramfs command symlink target is not exact: {name}")
        _require(record["size"] == len(symlink["target"].encode()), f"selected initramfs command symlink size is not exact: {name}")
    for prefix in selected["forbidden_entry_prefixes"]:
        forbidden = next((name for name in by_name if name.startswith(prefix)), None)
        _require(forbidden is None, f"selected initramfs contains forbidden entry: {forbidden}")
    for name in selected["required_regular_executables"]:
        record = by_name.get(name)
        if record is None:
            raise ValueError(f"selected initramfs is missing command executable: {name}")
        _require(record["type"] == "-", f"selected initramfs command executable is not regular: {name}")
        _require(record["links"] == 1, f"selected initramfs command executable is a hardlink: {name}")
        _require(any(character == "x" for character in record["mode"][1:]), f"selected initramfs command executable is not executable: {name}")
        _require(limits["min_regular_bytes"] <= record["size"] <= limits["max_entry_bytes"], f"selected initramfs command executable size is invalid: {name}")


def _safe_extracted_path(root: Path, name: str) -> Path:
    path = root.joinpath(*PurePosixPath(name).parts)
    current = root
    for part in PurePosixPath(name).parts[:-1]:
        current /= part
        try:
            metadata = os.lstat(current)
        except OSError as error:
            raise ValueError(f"initramfs path component is missing: {name}") from error
        _require(stat.S_ISDIR(metadata.st_mode), f"initramfs path component is not a directory: {name}")
        _require(not stat.S_ISLNK(metadata.st_mode), f"initramfs path component is a symlink: {name}")
    return path


def extract_regular_files(
    path: Path,
    required_entries: list[str],
    run_listing: Callable[[Path], str],
    executable_entries: tuple[str, ...],
    selected: dict[str, Any],
) -> dict[str, bytes]:
    limits = _size_limits(selected)
    listing = run_listing(path)
    records = _records(listing)
    by_name = {record["name"]: record for record in records}
    required = [_safe_name(entry) for entry in required_entries]
    _require(len(required) == len(set(required)), "required initramfs entries contain duplicates")
    for name in required:
        record = by_name.get(name)
        if record is None:
            raise ValueError(f"selected initramfs is missing constructor output: {name}")
        _require(record["mode"][0] == "-", f"selected initramfs entry is not regular: {name}")
        _require(record["links"] == 1, f"selected initramfs entry is a hardlink: {name}")
        _require(limits["min_regular_bytes"] <= record["size"] <= limits["max_entry_bytes"], f"selected initramfs entry size is invalid: {name}")
        if name in executable_entries:
            _require(any(character == "x" for character in record["mode"][1:]), f"selected initramfs entry is not executable: {name}")
    _validate_listing_total(records, limits)
    with extracted_initramfs(path) as destination:
        result: dict[str, bytes] = {}
        for name in required:
            extracted = _safe_extracted_path(destination, name)
            try:
                metadata = os.lstat(extracted)
            except OSError as error:
                raise ValueError(f"extracted initramfs entry is missing: {name}") from error
            _require(stat.S_ISREG(metadata.st_mode), f"extracted initramfs entry is not regular: {name}")
            _require(not stat.S_ISLNK(metadata.st_mode), f"extracted initramfs entry is a symlink: {name}")
            _require(metadata.st_nlink == 1, f"extracted initramfs entry is a hardlink: {name}")
            _require(limits["min_regular_bytes"] <= metadata.st_size <= limits["max_entry_bytes"], f"extracted initramfs entry size is invalid: {name}")
            if name in executable_entries:
                _require(metadata.st_mode & 0o111 != 0, f"extracted initramfs entry is not executable: {name}")
            try:
                data = extracted.read_bytes()
            except OSError as error:
                raise ValueError(f"cannot read extracted initramfs entry: {name}") from error
            _require(len(data) == by_name[name]["size"], f"extracted initramfs entry size changed: {name}")
            result[name] = data
        return result


@contextlib.contextmanager
def extracted_initramfs(path: Path) -> Any:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-initramfs-") as temporary:
        destination = Path(temporary)
        try:
            subprocess.run(["unmkinitramfs", str(path), str(destination)], capture_output=True, text=True, check=True)
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ValueError(f"cannot extract initramfs {path} with unmkinitramfs") from error
        yield destination


def _validate_listing_total(records: list[dict[str, Any]], limits: dict[str, int]) -> None:
    for record in records:
        if record["type"] == "-":
            _require(record["size"] <= limits["max_entry_bytes"], f"selected initramfs regular-file entry is oversized: {record['name']}")
    total = sum(record["size"] for record in records if record["type"] == "-")
    _require(total <= limits["max_total_regular_bytes"], "selected initramfs regular-file payload is oversized")


def _validate_extracted_command_layout(destination: Path, records: list[dict[str, Any]], selected: dict[str, Any]) -> None:
    limits = _size_limits(selected)
    by_name = {record["name"]: record for record in records}
    parent_paths: set[str] = set()
    required_names = [entry["path"] for entry in selected["required_symlinks"]] + selected["required_regular_executables"]
    for name in required_names:
        parts = PurePosixPath(name).parts
        parent_paths.update(PurePosixPath(*parts[:index]).as_posix() for index in range(1, len(parts)))
    for name in sorted(parent_paths):
        metadata = os.lstat(_safe_extracted_path(destination, name))
        _require(stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode), f"extracted initramfs command parent is unsafe: {name}")
    for symlink in selected["required_symlinks"]:
        name = symlink["path"]
        extracted = _safe_extracted_path(destination, name)
        metadata = os.lstat(extracted)
        _require(stat.S_ISLNK(metadata.st_mode), f"extracted initramfs command entry is not a symlink: {name}")
        _require(os.readlink(extracted) == symlink["target"], f"extracted initramfs command symlink target is not exact: {name}")
        _require(metadata.st_size == len(symlink["target"].encode()), f"extracted initramfs command symlink size is not exact: {name}")
    for name in selected["required_regular_executables"]:
        extracted = _safe_extracted_path(destination, name)
        metadata = os.lstat(extracted)
        _require(stat.S_ISREG(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode), f"extracted initramfs command entry is not regular: {name}")
        _require(metadata.st_nlink == 1, f"extracted initramfs command executable is a hardlink: {name}")
        _require(metadata.st_mode & 0o111 != 0, f"extracted initramfs command executable is not executable: {name}")
        _require(limits["min_regular_bytes"] <= metadata.st_size <= limits["max_entry_bytes"], f"extracted initramfs command executable size is invalid: {name}")
        _require(metadata.st_size == by_name[name]["size"], f"extracted initramfs command executable size changed: {name}")


def verify_command_layout(path: Path, listing: str, selected: dict[str, Any]) -> None:
    records = parse_initramfs_listing(listing)
    validate_command_records(records, selected)
    _validate_listing_total(records, _size_limits(selected))
    with extracted_initramfs(path) as destination:
        _validate_extracted_command_layout(destination, records, selected)


def _load_selected_contract(path: Path) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read Raspberry boot-layer contract: {path}") from error
    selected = document.get("selected_initramfs")
    _require(isinstance(selected, dict), "Raspberry selected initramfs contract is missing")
    validate_selected_initramfs_contract(selected)
    return selected


def validate_rootfs_bindings(root: Path, selected: dict[str, Any]) -> None:
    for binding in selected["byte_bindings"]:
        path = root / binding["rootfs_path"]
        metadata = os.lstat(path)
        if binding["rootfs_type"] == "symlink":
            _require(stat.S_ISLNK(metadata.st_mode), f"rootfs byte binding is not a symlink: {path}")
            _require(os.readlink(path) == binding["rootfs_target"], f"rootfs byte binding symlink target is not exact: {path}")
            resolution = binding["rootfs_resolution"]
            current = root / resolution["current_path"]
            current_metadata = os.lstat(current)
            _require(stat.S_ISLNK(current_metadata.st_mode), f"rootfs runtime release link is not a symlink: {current}")
            current_target = os.readlink(current)
            _require(re.fullmatch(resolution["current_target_pattern"], current_target) is not None, f"rootfs runtime release link target is unsafe: {current}")
            resolved = _resolve_rootfs_regular_file(root, resolution["resolved_path"])
            resolved_metadata = os.stat(resolved)
            _require(resolved_metadata.st_mode & 0o111 != 0, f"rootfs resolved runtime is not executable: {resolved}")
        else:
            _require(stat.S_ISREG(metadata.st_mode) and metadata.st_mode & 0o111 != 0, f"rootfs byte binding is not an executable regular file: {path}")


def verify_command_layout_file(path: Path, contract_path: Path, root: Path | None = None) -> None:
    selected = _load_selected_contract(contract_path)
    try:
        listing = subprocess.run(["lsinitramfs", "-l", str(path)], capture_output=True, text=True, check=True).stdout
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ValueError(f"cannot inspect initramfs {path} with lsinitramfs") from error
    verify_command_layout(path, listing, selected)
    if root is not None:
        validate_rootfs_bindings(root, selected)
        bindings = selected["byte_bindings"]
        required = _required_regular_files(selected) + selected["required_regular_executables"]
        executable_entries = tuple(required)
        extracted = extract_regular_files(path, required, lambda _: listing, executable_entries, selected)
        compare_rootfs_files(root, extracted, tuple((binding["archive_path"], binding["rootfs_path"]) for binding in bindings))


def _resolve_rootfs_regular_file(root: Path, relative: str) -> Path:
    root = Path(os.path.abspath(root))
    components = list(PurePosixPath(relative).parts)
    current = root
    seen: set[Path] = set()
    for _ in range(32):
        if components:
            current /= components.pop(0)
        metadata = os.lstat(current)
        if stat.S_ISLNK(metadata.st_mode):
            if current in seen:
                raise ValueError(f"rootfs file contains a symlink loop: {relative}")
            seen.add(current)
            target = os.readlink(current)
            candidate = root / target.lstrip("/") if os.path.isabs(target) else current.parent / target
            candidate = Path(os.path.abspath(candidate))
            try:
                candidate.relative_to(root)
            except ValueError as error:
                raise ValueError(f"rootfs file escapes the image root: {relative}") from error
            components = list(candidate.relative_to(root).parts) + components
            current = root
            continue
        if components:
            _require(stat.S_ISDIR(metadata.st_mode), f"rootfs file parent is not a directory: {relative}")
            continue
        _require(stat.S_ISREG(metadata.st_mode), f"rootfs file is not regular: {relative}")
        _require(metadata.st_nlink == 1, f"rootfs file is a hardlink: {relative}")
        return current
    raise ValueError(f"rootfs file has too many symlink components: {relative}")


def compare_rootfs_files(root: Path, extracted: dict[str, bytes], pairs: tuple[tuple[str, str], ...]) -> None:
    for archive_path, root_path in pairs:
        expected = _resolve_rootfs_regular_file(root, root_path).read_bytes()
        actual = extracted[archive_path]
        _require(len(actual) == len(expected), f"selected initramfs size differs from rootfs: {root_path}")
        _require(hashlib.sha256(actual).digest() == hashlib.sha256(expected).digest(), f"selected initramfs hash differs from rootfs: {root_path}")
        _require(actual == expected, f"selected initramfs bytes differ from rootfs: {root_path}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Validate the Raspberry initramfs command closure.")
    parser.add_argument("--validate-command-layout", type=Path, required=True)
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--root", type=Path)
    args = parser.parse_args(argv)
    try:
        verify_command_layout_file(args.validate_command_layout, args.contract, args.root)
    except (OSError, ValueError) as error:
        print(f"Raspberry initramfs command closure proof failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
