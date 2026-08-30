from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools" / "release" / "check_version_consistency.py"


class VersionConsistencyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        self._write_fixture()

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def _write_json(self, path: str, value: object) -> None:
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(value), encoding="utf-8")

    def _write_fixture(self) -> None:
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/core"]\n', encoding="utf-8"
        )
        (self.root / "crates" / "core").mkdir(parents=True, exist_ok=True)
        (self.root / "crates" / "core" / "Cargo.toml").write_text(
            '[package]\nname = "core"\nversion = "1.2.3"\n', encoding="utf-8"
        )
        (self.root / "pnpm-workspace.yaml").write_text(
            'packages:\n  - "apps/*"\n  - "packages/*"\n', encoding="utf-8"
        )
        for path in ("package.json", "apps/desktop/package.json", "packages/device-contracts/package.json"):
            self._write_json(path, {"name": path, "version": "1.2.3"})
        self._write_json("apps/desktop/src-tauri/tauri.conf.json", {"version": "1.2.3"})

    def run_checker(self, tag: str | None = None) -> subprocess.CompletedProcess[str]:
        command = [sys.executable, str(CHECKER), "--root", str(self.root)]
        if tag is not None:
            command.extend(("--tag", tag))
        return subprocess.run(command, capture_output=True, text=True, check=False)

    def test_all_current_versions_match(self) -> None:
        result = self.run_checker("v1.2.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "1.2.3")

    def test_one_mismatch_reports_paths_and_values(self) -> None:
        path = self.root / "apps" / "desktop" / "package.json"
        document = json.loads(path.read_text(encoding="utf-8"))
        document["version"] = "1.2.4"
        path.write_text(json.dumps(document), encoding="utf-8")

        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Version mismatch", result.stderr)
        self.assertIn("apps/desktop/package.json='1.2.4'", result.stderr)
        self.assertIn("crates/core/Cargo.toml='1.2.3'", result.stderr)

    def test_malformed_and_missing_fields_are_clear(self) -> None:
        cases = (
            ("apps/desktop/package.json", "not json", "Malformed JSON apps/desktop/package.json"),
            (
                "packages/device-contracts/package.json",
                {"name": "device-contracts"},
                "Missing version field: packages/device-contracts/package.json",
            ),
        )
        for path, content, expected in cases:
            with self.subTest(path=path):
                target = self.root / path
                target.write_text(content if isinstance(content, str) else json.dumps(content), encoding="utf-8")
                result = self.run_checker()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(expected, result.stderr)
                self._write_fixture()

    def test_bad_tag_is_rejected(self) -> None:
        for tag, expected in (("1.2.3", "Malformed release tag"), ("v1.2.4", "does not match application version")):
            with self.subTest(tag=tag):
                result = self.run_checker(tag)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(expected, result.stderr)

    def test_current_parent_fixture_is_not_application_metadata(self) -> None:
        self._write_json("resources/image-parents/orange-pi-zero-2w-current.json", {"version": "0.8.1"})
        result = self.run_checker("v1.2.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "1.2.3")


if __name__ == "__main__":
    unittest.main()
