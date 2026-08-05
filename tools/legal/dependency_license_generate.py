#!/usr/bin/env python3
"""Generate the checked Cargo and production pnpm license inventory."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

from cargo_dependency_license_support import (
    PRESERVED_CARGO_PATHS,
    REGISTRY_SOURCE,
    license_source_files,
    manifest_data,
    policy_key,
    reference_paths_for,
    validate_policy,
    verify_cargo_checksum,
    workspace_license_records,
)
from cargo_dependency_license_render import render_cargo_outputs


CARGO_ABOUT_VERSION = "0.9.1"
PNPM_VERSION = "9.12.0"
COREPACK = "corepack.cmd" if os.name == "nt" else "corepack"
KNOWN_LICENSE_IDS = {
    "0BSD",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MIT-0",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
    "Apache-2.0 WITH LLVM-exception",
    "LGPL-2.1-or-later",
    "MPL-2.0",
}
COPYLEFT_IDS = {"LGPL-2.1-or-later", "MPL-2.0"}
PNPM_NOTICE_PREFIXES = (
    "LICENSE",
    "LICENCE",
    "NOTICE",
    "AUTHORS",
    "THIRD-PARTY",
    "THIRD_PARTY",
    "COPYING",
    "PATENTS",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def run_json(command: list[str], cwd: Path) -> Any:
    result = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        encoding="utf-8",
        errors="strict",
    )
    if result.returncode:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"{result.stderr.strip()}"
        )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"command did not produce JSON: {' '.join(command)}") from error


def cargo_metadata(root: Path) -> dict[str, Any]:
    return run_json(
        ["cargo", "metadata", "--locked", "--offline", "--all-features", "--format-version", "1"],
        root,
    )


def pnpm_licenses(root: Path) -> dict[str, Any]:
    version = subprocess.run(
        [COREPACK, "pnpm", "--version"],
        cwd=root,
        check=False,
        capture_output=True,
        encoding="utf-8",
        errors="strict",
    )
    if version.returncode or version.stdout.strip() != PNPM_VERSION:
        raise RuntimeError(f"expected pnpm {PNPM_VERSION}, found {version.stdout.strip()}")
    return run_json([COREPACK, "pnpm", "licenses", "list", "--prod", "--json"], root)


def classify_expression(expression: str | None) -> tuple[str, list[str]]:
    if not expression:
        return "custom-or-unknown", []
    normalized = expression.replace("/", " OR ")
    identifiers = []
    for license_id in sorted(KNOWN_LICENSE_IDS, key=len, reverse=True):
        if license_id in normalized:
            identifiers.append(license_id)
            normalized = normalized.replace(license_id, " ")
    normalized = re.sub(r"\b(?:AND|OR|WITH)\b", " ", normalized)
    normalized = re.sub(r"[().+\-0-9_\s]+", "", normalized)
    if normalized:
        return "custom-or-unknown", sorted(set(identifiers))
    if set(identifiers) & COPYLEFT_IDS:
        return "copyleft", sorted(set(identifiers))
    return "permissive", sorted(set(identifiers))


def is_cargo_license_file(name: str) -> bool:
    upper = name.upper()
    return upper in {"LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE"} or upper.startswith(
        (
            "LICENSE-",
            "LICENSE.",
            "LICENCE-",
            "LICENCE.",
            "COPYING-",
            "COPYING.",
            "UNLICENSE-",
            "NOTICE-",
            "NOTICE.",
            "PATENTS",
        )
    )


def lock_entries(root: Path) -> dict[tuple[str, str, str | None], dict[str, Any]]:
    lock = tomllib.loads(read_text(root / "Cargo.lock"))
    return {
        (entry["name"], entry["version"], entry.get("source")): entry
        for entry in lock.get("package", [])
    }


def stable_lock_source(source: str | None, package_root: Path, root: Path) -> str:
    if source == REGISTRY_SOURCE:
        return "crates.io"
    relative = package_root.relative_to(root).as_posix()
    return f"path:{relative}"


def cargo_inventory(root: Path) -> tuple[dict[str, bytes], list[dict[str, Any]]]:
    metadata = cargo_metadata(root)
    entries = lock_entries(root)
    workspace_members = set(metadata["workspace_members"])
    workspace_license = workspace_license_records(root, metadata)
    packages = [
        package
        for package in metadata["packages"]
        if package["source"] == REGISTRY_SOURCE
        or package["manifest_path"].replace("\\", "/")
        == (root / "third_party/cpal-0.15.3/Cargo.toml").as_posix()
    ]
    packages.sort(key=lambda package: (package["name"], package["version"]))
    metadata_keys = {(package["name"], package["version"], package["source"]) for package in packages}
    lock_keys = {
        key
        for key in entries
        if key[2] == REGISTRY_SOURCE or key == ("cpal", "0.15.3", None)
    }
    if metadata_keys != lock_keys:
        raise RuntimeError("Cargo.lock package identities differ from cargo metadata")
    documents: dict[str, dict[str, Any]] = {}
    inventory_packages = []
    manifest_license_no_file = []
    source_index = []

    for package in packages:
        package_root = Path(package["manifest_path"]).parent
        key = (package["name"], package["version"], package["source"])
        if key not in entries:
            raise RuntimeError(f"Cargo.lock is missing {package['name']} {package['version']}")
        lock_entry = entries[key]
        is_vendored_cpal = package_root.resolve() == (root / "third_party/cpal-0.15.3").resolve()
        license_expression = package.get("license")
        manifest = manifest_data(package_root)
        if package.get("license_file") is not None:
            manifest["license-file"] = package["license_file"]
        if is_vendored_cpal:
            license_expression = manifest.get("license")
        license_class, identifiers = classify_expression(license_expression)
        declared_license_file, source_files, license_file_status = license_source_files(
            package_root,
            manifest,
            is_cargo_license_file,
        )
        if license_file_status == "manifest-license-no-file":
            manifest_license_no_file.append(f"{package['name']} {package['version']}")
        license_files = []
        for source_file in source_files:
            content = source_file.read_bytes()
            document_id = sha256(content)
            document = documents.setdefault(
                document_id,
                {"content": content.decode("utf-8"), "sources": []},
            )
            source_name = (
                f"third_party/cpal-0.15.3/{source_file.name}"
                if is_vendored_cpal
                else source_file.name
            )
            document["sources"].append(f"{package['name']} {package['version']}: {source_name}")
            license_files.append(
                {
                    "source_file": source_name,
                    "document_sha256": document_id,
                    "bytes": len(content),
                }
            )
        cargo_checksum = None
        if package["source"] == REGISTRY_SOURCE:
            cargo_checksum = verify_cargo_checksum(package_root, package["name"], package["version"], lock_entry["checksum"])
        record = {
            "name": package["name"],
            "version": package["version"],
            "license": license_expression,
            "license_class": license_class,
            "license_identifiers": identifiers,
            "source": stable_lock_source(package["source"], package_root, root),
            "registry_source": package["source"],
            "checksum": lock_entry.get("checksum"),
            "license_files": license_files,
            "manifest_sha256": sha256((package_root / "Cargo.toml").read_bytes()),
            "manifest_authors": package.get("authors"),
            "manifest_repository": package.get("repository"),
            "manifest_license_file": declared_license_file,
            "license_file_status": license_file_status,
            "source_required": license_file_status == "manifest-license-no-file",
        }
        if cargo_checksum is not None:
            record["cargo_checksum"] = cargo_checksum
        if is_vendored_cpal:
            provenance = package_root / "PROVENANCE.md"
            if not provenance.is_file() or provenance.is_symlink():
                raise RuntimeError("vendored cpal PROVENANCE.md is missing or a symlink")
            record["modified"] = True
            record["upstream_status"] = "modified-local-vendoring"
            record["provenance_source"] = "third_party/cpal-0.15.3/PROVENANCE.md"
            record["provenance_sha256"] = sha256(provenance.read_bytes())
        inventory_packages.append(record)

        if package["source"] == REGISTRY_SOURCE:
            source_index.append(
                {
                    "name": package["name"],
                    "version": package["version"],
                    "source": package["source"],
                    "checksum": lock_entry["checksum"],
                    "archive_url": f"https://static.crates.io/crates/{package['name']}/{package['name']}-{package['version']}.crate",
                    "archive_file": f"{package['name']}-{package['version']}.crate",
                    "manifest_sha256": record["manifest_sha256"],
                    "cargo_checksum_sha256": cargo_checksum["sha256"] if cargo_checksum else None,
                    "cargo_checksum_status": cargo_checksum["status"] if cargo_checksum else "not-applicable",
                    "manifest_authors": package.get("authors"),
                    "manifest_repository": package.get("repository"),
                    "manifest_license": license_expression,
                    "reference_texts": reference_paths_for(record),
                    "source_requirement_reason": "manifest-license-no-file" if record["source_required"] else "license-source-review",
                    "source_required": record["source_required"],
                }
            )

    reviewed_policy = validate_policy(root, inventory_packages)
    for package in inventory_packages:
        decision = reviewed_policy.get(policy_key(package)) if package["registry_source"] else None
        if decision:
            package["original_license_class"] = package["license_class"]
            package["license_class"] = decision["license_class"]
            package["review"] = decision
            package["review_status"] = decision["review_status"]
            package["effective_license"] = decision["effective_license"]
            package["source_required"] = package["source_required"] or decision["source_required"]
    by_package = {(package["name"], package["version"]): package for package in inventory_packages}
    for item in source_index:
        package = by_package[(item["name"], item["version"])]
        item["source_required"] = package["source_required"]
        item["source_requirement_reason"] = (
            "reviewed-mpl-2.0" if package.get("review_status") == "reviewed-with-source-obligation" else
            "manifest-license-no-file" if package["license_file_status"] == "manifest-license-no-file" else
            "license-source-review"
        )
    source_index.sort(key=lambda item: (item["name"], item["version"]))

    config = root / "about.toml"
    template = root / "tools/legal/cargo_about.hbs"
    inventory = {
        "generated": True,
        "generator": "tools/legal/dependency_license_generate.py",
        "cargo_about": {
            "version": CARGO_ABOUT_VERSION,
            "config": "about.toml",
            "config_sha256": sha256(config.read_bytes()),
            "template": "tools/legal/cargo_about.hbs",
            "template_sha256": sha256(template.read_bytes()),
            "machine_output_checked": False,
            "limitation": "cargo-about 0.9.1 is advisory here: custom first-party license-file metadata and policy review are not represented by its accepted identifiers, so the checked inventory is rendered from Cargo.lock, manifests, and the exact reviewed policy instead.",
        },
        "cargo_lock_sha256": sha256((root / "Cargo.lock").read_bytes()),
        "package_count": len(inventory_packages),
        "license_classes": {
            "permissive": sum(p["license_class"] == "permissive" for p in inventory_packages),
            "file-level-copyleft": sum(p["license_class"] == "file-level-copyleft" for p in inventory_packages),
            "reviewed-license-alternative": sum(p["license_class"] == "reviewed-license-alternative" for p in inventory_packages),
            "custom-or-unknown": sum(p["license_class"] == "custom-or-unknown" for p in inventory_packages),
        },
        "packages": inventory_packages,
        "manifest_license_no_file": manifest_license_no_file,
        "reviewed_counts": {
            "mpl": sum(package.get("review_status") == "reviewed-with-source-obligation" for package in inventory_packages),
            "alternatives": sum(package.get("review_status") == "reviewed-alternative" for package in inventory_packages),
        },
        "source_availability_review_count": sum(package["source_required"] for package in inventory_packages),
        "workspace_license": workspace_license,
        "workspace_license_metadata_missing": [],
        "reviewed_policy_sha256": sha256((root / "licenses/cargo/reviewed-dependency-policy.json").read_bytes()),
    }
    output = render_cargo_outputs(root, inventory, inventory_packages, documents, source_index)
    return output, inventory_packages


def all_outputs(root: Path) -> dict[str, bytes]:
    from pnpm_dependency_license_generate import pnpm_inventory

    cargo, _ = cargo_inventory(root)
    cargo.update(pnpm_inventory(root))
    return cargo


def write_outputs(root: Path, output: dict[str, bytes]) -> None:
    generated_paths = set(output)
    for directory in (root / "licenses/cargo", root / "licenses/pnpm"):
        if directory.exists():
            for path in directory.rglob("*"):
                relative = path.relative_to(root).as_posix()
                if path.is_file() and relative in generated_paths:
                    path.unlink()
            for path in sorted(directory.rglob("*"), reverse=True):
                if path.is_dir() and not any(path.iterdir()):
                    path.rmdir()
    for relative, content in output.items():
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)


def check_outputs(root: Path, expected: dict[str, bytes]) -> int:
    actual_paths = {
        path.relative_to(root).as_posix()
        for prefix in (root / "licenses/cargo", root / "licenses/pnpm")
        if prefix.exists()
        for path in prefix.rglob("*")
        if path.is_file()
    }
    expected_paths = set(expected)
    errors = []
    for relative in sorted(expected_paths | actual_paths):
        if relative not in expected_paths:
            if relative not in PRESERVED_CARGO_PATHS:
                errors.append(f"unexpected generated file: {relative}")
            continue
        actual = (root / relative).read_bytes() if relative in actual_paths else None
        if relative not in actual_paths:
            errors.append(f"generated file is missing: {relative}")
        elif actual != expected[relative]:
            errors.append(f"generated file differs: {relative}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="compare outputs without modifying files")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    try:
        output = all_outputs(root)
        if args.check:
            return check_outputs(root, output)
        write_outputs(root, output)
        return 0
    except (OSError, RuntimeError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"dependency license generation failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
