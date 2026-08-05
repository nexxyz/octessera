#!/usr/bin/env python3
"""Focused tests for dependency policy and workspace decisions."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cargo_dependency_license_support import (
    load_policy,
    relative_declared_license,
    validate_policy_data,
    verify_cargo_checksum,
    workspace_license_records,
)
from dependency_license_generate import cargo_metadata


ROOT = Path(__file__).resolve().parents[2]


class DependencyLicenseTests(unittest.TestCase):
    def test_workspace_license_inheritance_and_digest(self) -> None:
        records = workspace_license_records(ROOT, cargo_metadata(ROOT))
        self.assertEqual(len(records), 7)
        self.assertEqual({record["resolved_license_file"] for record in records}, {"LICENSE"})
        self.assertEqual(len({record["license_sha256"] for record in records}), 1)

    def test_declared_license_file_missing_is_fatal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package_root = Path(directory)
            with self.assertRaises(RuntimeError):
                relative_declared_license(package_root, {"license-file": "legal/LICENSE"})

    def test_policy_has_exact_mpl_and_r_efi_decisions(self) -> None:
        inventory = json.loads((ROOT / "licenses/cargo/inventory.json").read_text(encoding="utf-8"))
        mpl = [package for package in inventory["packages"] if package.get("review_status") == "reviewed-with-source-obligation"]
        alternatives = [package for package in inventory["packages"] if package.get("review_status") == "reviewed-alternative"]
        self.assertEqual(len(mpl), 10)
        self.assertEqual(len(alternatives), 2)
        self.assertTrue(all(package["source_required"] for package in mpl))
        self.assertTrue(all(package["effective_license"] == "Apache-2.0" for package in alternatives))
        self.assertTrue(all(package["license"] == "MIT OR Apache-2.0 OR LGPL-2.1-or-later" for package in alternatives))
        validate_policy_data(ROOT, inventory["packages"], load_policy(ROOT))

    def test_policy_rejects_missing_extra_and_expression_mutations(self) -> None:
        inventory = json.loads((ROOT / "licenses/cargo/inventory.json").read_text(encoding="utf-8"))
        policy = load_policy(ROOT)
        missing = copy.deepcopy(policy)
        missing["records"].pop(next(iter(missing["records"])))
        with self.assertRaises(RuntimeError):
            validate_policy_data(ROOT, inventory["packages"], missing)
        extra = copy.deepcopy(policy)
        key = next(iter(extra["records"]))
        extra["records"][key + "|stale"] = copy.deepcopy(extra["records"][key])
        with self.assertRaises(RuntimeError):
            validate_policy_data(ROOT, inventory["packages"], extra)
        changed_packages = copy.deepcopy(inventory["packages"])
        changed = next(package for package in changed_packages if package["name"] == "colored")
        changed["license"] = "UNKNOWN"
        with self.assertRaises(RuntimeError):
            validate_policy_data(ROOT, changed_packages, policy)

    def test_cargo_checksum_sidecar_mutation_is_fatal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package_root = Path(directory)
            manifest = package_root / "Cargo.toml"
            content = b"[package]\nname = 'fixture'\n"
            manifest.write_bytes(content)
            checksum = "package-checksum"
            sidecar = {"package": checksum, "files": {"Cargo.toml": hashlib.sha256(content).hexdigest()}}
            (package_root / ".cargo-checksum.json").write_text(json.dumps(sidecar), encoding="utf-8")
            self.assertEqual(verify_cargo_checksum(package_root, "fixture", "1.0.0", checksum)["status"], "verified")
            manifest.write_bytes(b"mutated")
            with self.assertRaises(RuntimeError):
                verify_cargo_checksum(package_root, "fixture", "1.0.0", checksum)


if __name__ == "__main__":
    unittest.main()
