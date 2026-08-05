"""Production pnpm license discovery and copying."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from dependency_license_generate import (
    PNPM_NOTICE_PREFIXES,
    PNPM_VERSION,
    classify_expression,
    json_bytes,
    pnpm_licenses,
    read_text,
    sha256,
)
from cargo_dependency_license_render import checksum_manifest


def package_component(name: str) -> str:
    if not name or name.startswith("/") or "\\" in name or ".." in name:
        raise RuntimeError(f"unsafe pnpm package name: {name!r}")
    return name.replace("/", "__")


def pnpm_files(package_root: Path) -> list[Path]:
    if package_root.is_symlink():
        raise RuntimeError(f"pnpm package root is a symlink: {package_root}")
    files = []
    for path in package_root.iterdir():
        if path.is_symlink():
            raise RuntimeError(f"symlink in pnpm package: {path}")
        if path.is_file() and path.name.upper().startswith(PNPM_NOTICE_PREFIXES):
            files.append(path)
    return sorted(files, key=lambda path: path.name.casefold())


def lock_has_package(lock_text: str, name: str, version: str) -> bool:
    packages_section = lock_text.split("\npackages:\n", 1)[-1]
    return any(f"{name}@{version}" in line for line in packages_section.splitlines())


def source_path(raw: dict[str, Any], name: str, version: str) -> Path:
    for package_group in raw.values():
        for package in package_group:
            if package.get("name") != name:
                continue
            for found_version, raw_path in zip(package.get("versions", []), package.get("paths", [])):
                if found_version == version:
                    return Path(os.path.normpath(raw_path))
    raise RuntimeError(f"pnpm source path disappeared: {name} {version}")


def pnpm_inventory(root: Path) -> dict[str, bytes]:
    raw = pnpm_licenses(root)
    lock_text = read_text(root / "pnpm-lock.yaml")
    records = []
    for license_name, package_group in raw.items():
        if str(license_name).strip().upper() in {"UNKNOWN", "UNLICENSED", ""}:
            raise RuntimeError(f"pnpm reported unknown license: {license_name}")
        for package in package_group:
            versions = package.get("versions", [])
            paths = package.get("paths", [])
            if len(versions) != len(paths):
                raise RuntimeError(f"pnpm versions/paths mismatch for {package.get('name')}")
            for version, raw_path in zip(versions, paths):
                package_path = Path(os.path.normpath(raw_path))
                if not package_path.is_absolute():
                    raise RuntimeError(f"pnpm returned a non-absolute package path: {raw_path}")
                package_json = package_path / "package.json"
                if not package_json.is_file() or package_json.is_symlink():
                    raise RuntimeError(f"missing or symlinked pnpm package.json: {raw_path}")
                metadata = json.loads(read_text(package_json))
                expression = metadata.get("license")
                if metadata.get("name") != package["name"] or metadata.get("version") != version:
                    raise RuntimeError(f"pnpm package metadata mismatch: {raw_path}")
                license_class, identifiers = classify_expression(expression)
                if not expression or str(expression).strip().upper() in {"UNKNOWN", "UNLICENSED"}:
                    raise RuntimeError(f"missing pnpm license metadata: {package['name']} {version}")
                if license_class != "permissive":
                    raise RuntimeError(f"pnpm license needs a decision: {package['name']} {version} {expression}")
                if package.get("license") != expression:
                    raise RuntimeError(f"pnpm license metadata mismatch: {package['name']} {version}")
                if not lock_has_package(lock_text, package["name"], version):
                    raise RuntimeError(f"pnpm-lock.yaml is missing {package['name']} {version}")
                files = pnpm_files(package_path)
                if not files:
                    raise RuntimeError(f"missing pnpm license/notice files: {package['name']} {version}")
                copied = []
                for source_file in files:
                    relative = f"licenses/pnpm/{package_component(package['name'])}/{version}/{source_file.name}"
                    if Path(relative).is_absolute() or ".." in Path(relative).parts:
                        raise RuntimeError(f"unsafe generated pnpm path: {relative}")
                    content = source_file.read_bytes()
                    copied.append({"source_file": source_file.name, "output_file": relative, "sha256": sha256(content), "bytes": len(content)})
                records.append({"name": package["name"], "version": version, "license": expression, "license_class": license_class, "license_identifiers": identifiers, "files": copied})
    records.sort(key=lambda record: (record["name"], record["version"]))
    keys = [(record["name"], record["version"]) for record in records]
    if len(keys) != len(set(keys)):
        raise RuntimeError("pnpm production license output contains duplicate package versions")
    inventory = {"generated": True, "generator": "tools/legal/dependency_license_generate.py", "pnpm_version": PNPM_VERSION, "command": "corepack pnpm licenses list --prod --json", "pnpm_lock_sha256": sha256((root / "pnpm-lock.yaml").read_bytes()), "package_count": len(records), "packages": records}
    output = {"licenses/pnpm/inventory.json": json_bytes(inventory)}
    for record in records:
        root_path = source_path(raw, record["name"], record["version"])
        for item in record["files"]:
            output[item["output_file"]] = (root_path / item["source_file"]).read_bytes()
    output["licenses/pnpm/SHA256SUMS"] = checksum_manifest(output, "licenses/pnpm")
    return output
