#!/usr/bin/env python3
"""Verify checked dependency-license outputs without changing the repository."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

from dependency_license_generate import (
    CARGO_ABOUT_VERSION,
    COPYLEFT_IDS,
    PNPM_VERSION,
    all_outputs,
    cargo_metadata,
    check_outputs,
    pnpm_licenses,
)
from cargo_dependency_license_support import PRESERVED_CARGO_PATHS, validate_policy, workspace_license_records


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def strings(value: Any) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, dict):
        return [item for child in value.values() for item in strings(child)]
    if isinstance(value, list):
        return [item for child in value for item in strings(child)]
    return []


def check_absolute_paths(value: Any) -> list[str]:
    return [
        item
        for item in strings(value)
        if re.match(r"^(?:[A-Za-z]:[\\/]|\\\\|/)", item)
    ]


def check_checksum_manifest(root: Path, manifest: Path, prefix: str) -> list[str]:
    errors = []
    lines = manifest.read_text(encoding="utf-8").splitlines()
    seen = set()
    for line in lines:
        fields = line.split("  ", 1)
        if len(fields) != 2 or not re.fullmatch(r"[0-9a-f]{64}", fields[0]):
            errors.append(f"invalid checksum line: {line}")
            continue
        digest, relative = fields
        if relative in seen or not relative.startswith(prefix + "/"):
            errors.append(f"invalid or duplicate checksum path: {relative}")
            continue
        seen.add(relative)
        path = root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(f"checksum target is missing or symlinked: {relative}")
        elif sha256(path.read_bytes()) != digest:
            errors.append(f"checksum mismatch: {relative}")
    expected = {
        path.relative_to(root).as_posix()
        for path in (root / prefix).rglob("*")
        if path.is_file() and not path.is_symlink() and path != manifest and path.relative_to(root).as_posix() not in PRESERVED_CARGO_PATHS
    }
    if seen != expected:
        errors.append(f"checksum manifest set mismatch for {prefix}")
    return errors


def check_config(root: Path) -> list[str]:
    config_path = root / "about.toml"
    template_path = root / "tools/legal/cargo_about.hbs"
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    errors = []
    accepted = set(config.get("accepted", []))
    if not COPYLEFT_IDS.issubset(accepted):
        errors.append("cargo-about detection identifiers omit a reviewed copyleft class")
    if any(str(item).strip().upper() in {"UNKNOWN", "UNLICENSED"} for item in accepted):
        errors.append("cargo-about config accepts UNKNOWN or UNLICENSED")
    template = template_path.read_text(encoding="utf-8")
    if check_absolute_paths({"config": config, "template": template}):
        errors.append("cargo-about config/template contains an absolute path")
    if "generated_at" in template or "timestamp" in template.lower():
        errors.append("cargo-about template contains a timestamp field")
    if CARGO_ABOUT_VERSION != "0.9.1":
        errors.append("internal cargo-about version pin changed")
    return errors


def check_cargo_about() -> list[str]:
    result = subprocess.run(
        ["cargo", "about", "--version"],
        check=False,
        capture_output=True,
        encoding="utf-8",
        errors="strict",
    )
    expected = f"cargo-about {CARGO_ABOUT_VERSION}"
    if result.returncode or result.stdout.strip() != expected:
        return [f"expected {expected}, found {result.stdout.strip()}"]
    return []


def package_keys(raw: dict[str, Any]) -> set[tuple[str, str]]:
    return {
        (package["name"], version)
        for group in raw.values()
        for package in group
        for version in package.get("versions", [])
    }


def check_pnpm_inventory(root: Path, inventory: dict[str, Any]) -> list[str]:
    errors = []
    raw = pnpm_licenses(root)
    actual = package_keys(raw)
    checked = {(package["name"], package["version"]) for package in inventory["packages"]}
    if actual != checked:
        errors.append(f"pnpm production package set differs: command={sorted(actual)} output={sorted(checked)}")
    if inventory.get("pnpm_version") != PNPM_VERSION:
        errors.append("pnpm inventory version pin differs")
    if inventory.get("pnpm_lock_sha256") != sha256((root / "pnpm-lock.yaml").read_bytes()):
        errors.append("pnpm-lock.yaml hash differs from inventory")
    if inventory.get("package_count") != len(checked):
        errors.append("pnpm inventory package count is incorrect")
    for package in inventory["packages"]:
        if package.get("license_class") != "permissive":
            errors.append(f"pnpm package is not permissively classified: {package['name']} {package['version']}")
        for item in package.get("files", []):
            relative = item["output_file"]
            if Path(relative).is_absolute() or ".." in Path(relative).parts:
                errors.append(f"unsafe pnpm output path: {relative}")
            if not (root / relative).is_file():
                errors.append(f"missing pnpm output file: {relative}")
    return errors


def check_cargo_inventory(root: Path, inventory: dict[str, Any]) -> list[str]:
    errors = []
    if inventory.get("cargo_about", {}).get("version") != CARGO_ABOUT_VERSION:
        errors.append("Cargo inventory cargo-about version differs")
    if inventory.get("cargo_lock_sha256") != sha256((root / "Cargo.lock").read_bytes()):
        errors.append("Cargo.lock hash differs from inventory")
    if inventory.get("package_count") != len(inventory.get("packages", [])):
        errors.append("Cargo inventory package count is incorrect")
    if "decision_required" in inventory or "missing_license_files" in inventory:
        errors.append("Cargo inventory contains stale pre-policy fields")
    cpal = [package for package in inventory["packages"] if package["name"] == "cpal" and package["version"] == "0.15.3"]
    if len(cpal) != 1:
        errors.append("vendored cpal 0.15.3 is not present exactly once")
    elif not cpal[0].get("modified") or cpal[0].get("upstream_status") != "modified-local-vendoring":
        errors.append("vendored cpal is not marked as modified local vendoring")
    if not inventory.get("cargo_about", {}).get("limitation"):
        errors.append("Cargo inventory does not record cargo-about limitations")
    for package in inventory["packages"]:
        if package.get("license_class") not in {"permissive", "file-level-copyleft", "reviewed-license-alternative", "custom-or-unknown"}:
            errors.append(f"invalid Cargo license class: {package['name']} {package['version']}")
        for item in package.get("license_files", []):
            if Path(item["source_file"]).is_absolute() or ".." in Path(item["source_file"]).parts:
                errors.append(f"absolute or parent Cargo source path: {item['source_file']}")
    expected_workspace = workspace_license_records(root, cargo_metadata(root))
    if inventory.get("workspace_license") != expected_workspace:
        errors.append("workspace license_file inheritance or LICENSE digest differs")
    try:
        validate_policy(root, inventory["packages"])
    except RuntimeError as error:
        errors.append(str(error))
    source_index = read_json(root / "licenses/cargo/SOURCE_INDEX.json")
    registry_packages = [package for package in inventory["packages"] if package.get("registry_source")]
    source_keys = {
        (package["name"], package["version"], package["registry_source"], package["checksum"])
        for package in registry_packages
    }
    index_keys = {
        (package["name"], package["version"], package["source"], package["checksum"])
        for package in source_index.get("packages", [])
    }
    if source_index.get("scope") != "cargo-lock-overinclusive" or source_keys != index_keys:
        errors.append("Cargo SOURCE_INDEX identities differ from the Cargo inventory")
    if [profile.get("name") for profile in source_index.get("release_target_profiles", [])] != [
        "desktop",
        "pi-default",
        "pi-hardware-rpi-zero-2w",
        "pi-hardware-orange-pi-zero-2w",
    ]:
        errors.append("Cargo SOURCE_INDEX release target profiles are incomplete")
    for package in source_index.get("packages", []):
        if not package["archive_url"].startswith("https://static.crates.io/crates/"):
            errors.append(f"unstable Cargo archive URL: {package['name']} {package['version']}")
        if package["source_required"] and package["source_requirement_reason"] not in {"reviewed-mpl-2.0", "manifest-license-no-file"}:
            errors.append(f"invalid source-availability review reason: {package['name']} {package['version']}")
    if inventory.get("license_classes", {}).get("custom-or-unknown") != 0:
        errors.append("Cargo inventory contains custom or unknown licenses")
    if inventory.get("reviewed_counts") != {"mpl": 10, "alternatives": 2}:
        errors.append("Cargo reviewed policy counts differ")
    if inventory.get("workspace_license_metadata_missing") != []:
        errors.append("Cargo workspace license metadata is incomplete")
    return errors


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    errors = []
    try:
        errors.extend(check_config(root))
        errors.extend(check_cargo_about())
        expected = all_outputs(root)
        errors.extend(check_outputs(root, expected) and ["generated outputs are not byte-for-byte reproducible"] or [])
        cargo_inventory = read_json(root / "licenses/cargo/inventory.json")
        pnpm_inventory = read_json(root / "licenses/pnpm/inventory.json")
        errors.extend(check_cargo_inventory(root, cargo_inventory))
        errors.extend(check_pnpm_inventory(root, pnpm_inventory))
        for relative, content in expected.items():
            if relative.endswith(".json"):
                value = json.loads(content.decode("utf-8"))
                if check_absolute_paths(value):
                    errors.append(f"absolute path in generated JSON: {relative}")
        for prefix in ("licenses/cargo", "licenses/pnpm"):
            errors.extend(check_checksum_manifest(root, root / prefix / "SHA256SUMS", prefix))
        for prefix in (root / "licenses/cargo", root / "licenses/pnpm"):
            for path in prefix.rglob("*"):
                if path.is_symlink():
                    errors.append(f"symlink in checked outputs: {path.relative_to(root).as_posix()}")
        if cargo_inventory.get("reviewed_counts", {}).get("mpl") != 10:
            errors.append("Expected ten reviewed MPL records")
    except (OSError, RuntimeError, ValueError, KeyError, tomllib.TOMLDecodeError) as error:
        errors.append(str(error))
    if errors:
        print("dependency license verification failed:", file=sys.stderr)
        print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
        return 2
    print("dependency license outputs verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
