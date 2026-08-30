from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from stage_notices import NoticeStageError, check_finalized_notices, load_manifest, stage_notices


ROOT = Path(__file__).resolve().parents[2]


def _entry(root: Path, source: str, destination: str) -> dict[str, object]:
    path = root / source
    return {"source": source, "destination": destination, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size": path.stat().st_size}


def _manifest(root: Path, entries: list[dict[str, object]], name: str = "notice-bundle.json") -> Path:
    path = root / name
    path.write_text(json.dumps({"schema": "octessera.legal-notice-bundle/v1", "schema_version": 1, "destination_root": "/usr/share/doc/octessera", "files": entries}), encoding="utf-8")
    return path


class NoticeStagerTests(unittest.TestCase):
    def test_filesystem_policy_succeeds_and_checks_as_unprivileged_user(self) -> None:
        if os.name == "nt" or (hasattr(os, "geteuid") and os.geteuid() == 0):
            self.skipTest("requires an ordinary POSIX user")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "source.txt").write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            stage = root / "stage"
            stage_notices(root, stage, manifest, ownership="filesystem")
            stage_notices(root, stage, manifest, check=True, ownership="filesystem")

    @unittest.skipUnless(os.name != "nt", "root ownership is POSIX-only")
    def test_root_policy_rejects_user_owned_output(self) -> None:
        if hasattr(os, "geteuid") and os.geteuid() == 0:
            self.skipTest("requires an ordinary POSIX user")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "source.txt").write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            stage = root / "stage"
            stage_notices(root, stage, manifest, ownership="filesystem")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True)
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True, ownership="root")
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest)

    def test_invalid_ownership_policy_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "source.txt").write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            with self.assertRaises(NoticeStageError):
                stage_notices(root, root / "stage", manifest, ownership="invalid")

    def test_repository_manifest_hashes_match_raw_working_sources_and_lf_attributes(self) -> None:
        manifest = load_manifest(ROOT / "resources/legal/notice-bundle.json")
        for item in manifest["files"]:
            raw = (ROOT / item["source"]).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), item["sha256"], item["source"])
            self.assertEqual(len(raw), item["size"], item["source"])
        canonical_lf_paths = (
            "samples/SOURCE.md",
            "samples/MANIFEST.tsv",
            "samples/upstream/LICENSE",
            "licenses/cargo/THIRD_PARTY_LICENSES.txt",
            "licenses/cargo/SHA256SUMS",
            "resources/legal/notice-bundle.json",
        )
        attributes = (ROOT / ".gitattributes").read_text(encoding="utf-8")
        for path in canonical_lf_paths:
            self.assertIn(f"/{path} text eol=lf\n", attributes)
        result = subprocess.run(["git", "check-attr", "eol", "--", *canonical_lf_paths], cwd=ROOT, check=True, capture_output=True, text=True)
        effective = {line.split(": ", 2)[0]: line.split(": ", 2)[2] for line in result.stdout.splitlines()}
        self.assertEqual(effective, {path: "lf" for path in canonical_lf_paths})

    def test_repository_manifest_stages_and_checks_exact_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            manifest = load_manifest(ROOT / "resources/legal/notice-bundle.json")
            expected = {
                "LICENSE",
                "NOTICE",
                "THIRD_PARTY_NOTICES.md",
                "docs/release-licensing.md",
                "hardware/ATTRIBUTIONS.md",
                "samples/SOURCE.md",
                "samples/MANIFEST.tsv",
                "samples/upstream/LICENSE",
                "licenses/README.md",
            }
            expected.update(path.relative_to(ROOT).as_posix() for base in (ROOT / "licenses/cargo", ROOT / "licenses/pnpm") for path in base.rglob("*") if path.is_file())
            self.assertEqual({item["source"] for item in manifest["files"]}, expected)
            stage_notices(ROOT, stage, ownership="filesystem")
            stage_notices(ROOT, stage, check=True, ownership="filesystem")
            self.assertEqual((stage / "usr/share/doc/octessera/LICENSE").read_bytes(), (ROOT / "LICENSE").read_bytes())

    def test_finalized_mode_accepts_expected_hardlinks_but_rejects_ordinary_check_and_external_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "one.txt").write_text("canonical\n", encoding="utf-8")
            (root / "two.txt").write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "one.txt", "NOTICE-one"), _entry(root, "two.txt", "NOTICE-two")])
            stage = root / "stage"
            stage_notices(root, stage, manifest, ownership="filesystem")
            first = stage / "usr/share/doc/octessera/NOTICE-one"
            second = stage / "usr/share/doc/octessera/NOTICE-two"
            second.unlink()
            os.link(first, second)
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True, ownership="filesystem")
            check_finalized_notices(root, stage, manifest, ownership="filesystem")
            external = stage / "usr/share/doc/external-legal-alias"
            os.link(first, external)
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest, ownership="filesystem")

    def test_finalized_mode_rejects_differing_destination_content_and_hardlinked_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "one.txt").write_text("first-data\n", encoding="utf-8")
            (root / "two.txt").write_text("other-data\n", encoding="utf-8")
            entries = [_entry(root, "one.txt", "NOTICE-one"), _entry(root, "two.txt", "NOTICE-two")]
            manifest = _manifest(root, entries)
            stage = root / "stage"
            stage_notices(root, stage, manifest, ownership="filesystem")
            first = stage / "usr/share/doc/octessera/NOTICE-one"
            second = stage / "usr/share/doc/octessera/NOTICE-two"
            second.unlink()
            os.link(first, second)
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest, ownership="filesystem")
            os.link(root / "one.txt", root / "source-alias.txt")
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest, ownership="filesystem")

    def test_finalized_mode_never_creates_or_stages(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "source.txt").write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            destination = root / "not-created"
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, destination, manifest, ownership="filesystem")
            self.assertFalse(destination.exists())
            with self.assertRaises(NoticeStageError):
                stage_notices(root, destination, manifest, check=True, check_finalized=True, ownership="filesystem")
            self.assertFalse(destination.exists())

    def test_cli_failures_use_stderr_and_operations_are_exclusive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "source.txt").write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            command = [
                sys.executable,
                str(ROOT / "tools/legal/stage_notices.py"),
                "--repository-root",
                str(root),
                "--destination-root",
                str(root / "not-created"),
                "--manifest",
                str(manifest),
                "--ownership",
                "filesystem",
            ]
            failed = subprocess.run([*command, "--check-finalized"], capture_output=True, text=True)
            self.assertEqual(failed.returncode, 1)
            self.assertEqual(failed.stdout, "")
            self.assertIn("Legal notice staging failed", failed.stderr)
            conflict = subprocess.run([*command, "--check", "--check-finalized"], capture_output=True, text=True)
            self.assertEqual(conflict.returncode, 2)
            self.assertIn("not allowed with argument", conflict.stderr)

    def test_stale_missing_extra_mode_and_content_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.txt"
            source.write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            stage = root / "stage"
            stage_notices(root, stage, manifest, ownership="filesystem")
            target = stage / "usr/share/doc/octessera/NOTICE"
            target.write_text("stale\n", encoding="utf-8")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True, ownership="filesystem")
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest, ownership="filesystem")
            target.write_text("canonical\n", encoding="utf-8")
            target.chmod(0o600)
            if os.name != "nt":
                with self.assertRaises(NoticeStageError):
                    stage_notices(root, stage, manifest, check=True, ownership="filesystem")
                with self.assertRaises(NoticeStageError):
                    check_finalized_notices(root, stage, manifest, ownership="filesystem")
            target.chmod(0o644)
            (stage / "usr/share/doc/octessera/extra.txt").write_text("extra\n", encoding="utf-8")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True, ownership="filesystem")
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest, ownership="filesystem")
            (stage / "usr/share/doc/octessera/extra.txt").unlink()
            source.unlink()
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True, ownership="filesystem")
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, manifest, ownership="filesystem")

    def test_symlink_collision_and_escape_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "one.txt").write_text("one", encoding="utf-8")
            (root / "two.txt").write_text("two", encoding="utf-8")
            collision = _manifest(root, [_entry(root, "one.txt", "NOTICE"), _entry(root, "two.txt", "NOTICE")])
            with self.assertRaises(NoticeStageError):
                load_manifest(collision)
            escape = _manifest(root, [{**_entry(root, "one.txt", "NOTICE"), "destination": "../NOTICE"}])
            with self.assertRaises(NoticeStageError):
                load_manifest(escape)
            source_link = root / "source-link.txt"
            try:
                source_link.symlink_to(root / "one.txt")
            except OSError:
                self.skipTest("symlink creation is unavailable")
            linked = _manifest(root, [{**_entry(root, "one.txt", "NOTICE"), "source": "source-link.txt"}])
            with self.assertRaises(NoticeStageError):
                stage_notices(root, root / "stage", linked, ownership="filesystem")
            stage = root / "stage"
            stage_notices(root, stage, _manifest(root, [_entry(root, "one.txt", "NOTICE")]), ownership="filesystem")
            target = stage / "usr/share/doc/octessera/NOTICE"
            target.unlink()
            target.symlink_to(root / "one.txt")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, _manifest(root, [_entry(root, "one.txt", "NOTICE")]), check=True, ownership="filesystem")
            with self.assertRaises(NoticeStageError):
                check_finalized_notices(root, stage, _manifest(root, [_entry(root, "one.txt", "NOTICE")]), ownership="filesystem")


if __name__ == "__main__":
    unittest.main()
