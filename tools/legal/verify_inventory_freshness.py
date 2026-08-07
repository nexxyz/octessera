#!/usr/bin/env python3
"""Deterministic legal inventory freshness verification.

Checks that the checked Cargo and production pnpm license inventories match the
current lockfiles and that checksum manifests and the notice bundle are fresh.
All checks are local and deterministic; external tooling is not required.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dependency_license_check import check_checksum_manifest  # noqa: E402


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def verify_cargo_inventory(root: Path, errors: list[str]) -> None:
    inventory_path = root / "licenses/cargo/inventory.json"
    lock_path = root / "Cargo.lock"
    if not inventory_path.is_file():
        errors.append("licenses/cargo/inventory.json is missing")
        return
    inventory = read_json(inventory_path)
    if not lock_path.is_file():
        errors.append("Cargo.lock is missing")
        return
    actual = sha256(lock_path.read_bytes())
    if inventory.get("cargo_lock_sha256") != actual:
        errors.append("licenses/cargo/inventory.json is stale: cargo_lock_sha256 does not match Cargo.lock. Run corepack pnpm run licenses:cargo:generate.")
    errors.extend(
        check_checksum_manifest(root, root / "licenses/cargo/SHA256SUMS", "licenses/cargo")
    )


def verify_pnpm_inventory(root: Path, errors: list[str]) -> None:
    inventory_path = root / "licenses/pnpm/inventory.json"
    lock_path = root / "pnpm-lock.yaml"
    if not inventory_path.is_file():
        errors.append("licenses/pnpm/inventory.json is missing")
        return
    inventory = read_json(inventory_path)
    if not lock_path.is_file():
        errors.append("pnpm-lock.yaml is missing")
        return
    actual = sha256(lock_path.read_bytes())
    if inventory.get("pnpm_lock_sha256") != actual:
        errors.append("licenses/pnpm/inventory.json is stale: pnpm_lock_sha256 does not match pnpm-lock.yaml. Run corepack pnpm run licenses:pnpm:generate.")
    errors.extend(
        check_checksum_manifest(root, root / "licenses/pnpm/SHA256SUMS", "licenses/pnpm")
    )


def verify_notice_bundle(root: Path, errors: list[str]) -> None:
    bundle_path = root / "resources/legal/notice-bundle.json"
    if not bundle_path.is_file():
        errors.append("resources/legal/notice-bundle.json is missing")
        return
    bundle = read_json(bundle_path)
    for item in bundle.get("files", []):
        raw = root / item["source"]
        if not raw.is_file():
            errors.append(f"notice-bundle source is missing: {item['source']}")
            continue
        if sha256(raw.read_bytes()) != item.get("sha256"):
            errors.append(f"notice-bundle is stale: {item['source']} hash mismatch")


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify deterministic legal inventory freshness.")
    parser.add_argument("--root", default=Path(__file__).resolve().parents[2], type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    errors: list[str] = []
    verify_cargo_inventory(root, errors)
    verify_pnpm_inventory(root, errors)
    verify_notice_bundle(root, errors)

    if errors:
        print("legal inventory freshness failed:", file=sys.stderr)
        print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
        return 2
    print("legal inventory is fresh")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
