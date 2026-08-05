from __future__ import annotations

import hashlib
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from stage_notices import NoticeStageError, load_manifest, stage_notices


ROOT = Path(__file__).resolve().parents[2]


def _entry(root: Path, source: str, destination: str) -> dict[str, object]:
    path = root / source
    return {"source": source, "destination": destination, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size": path.stat().st_size}


def _manifest(root: Path, entries: list[dict[str, object]]) -> Path:
    path = root / "notice-bundle.json"
    path.write_text(json.dumps({"schema": "octessera.legal-notice-bundle/v1", "schema_version": 1, "destination_root": "/usr/share/doc/octessera", "files": entries}), encoding="utf-8")
    return path


class NoticeStagerTests(unittest.TestCase):
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
                "samples/ATTRIBUTIONS.tsv",
                "samples/upstream/LICENSE",
                "samples/upstream/README.txt",
                "licenses/README.md",
            }
            expected.update(path.relative_to(ROOT).as_posix() for base in (ROOT / "licenses/cargo", ROOT / "licenses/pnpm") for path in base.rglob("*") if path.is_file())
            self.assertEqual({item["source"] for item in manifest["files"]}, expected)
            stage_notices(ROOT, stage)
            stage_notices(ROOT, stage, check=True)
            self.assertEqual((stage / "usr/share/doc/octessera/LICENSE").read_bytes(), (ROOT / "LICENSE").read_bytes())

    def test_stale_missing_extra_mode_and_content_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.txt"
            source.write_text("canonical\n", encoding="utf-8")
            manifest = _manifest(root, [_entry(root, "source.txt", "NOTICE")])
            stage = root / "stage"
            stage_notices(root, stage, manifest)
            target = stage / "usr/share/doc/octessera/NOTICE"
            target.write_text("stale\n", encoding="utf-8")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True)
            target.write_text("canonical\n", encoding="utf-8")
            target.chmod(0o600)
            if os.name != "nt":
                with self.assertRaises(NoticeStageError):
                    stage_notices(root, stage, manifest, check=True)
            target.chmod(0o644)
            (stage / "usr/share/doc/octessera/extra.txt").write_text("extra\n", encoding="utf-8")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True)
            (stage / "usr/share/doc/octessera/extra.txt").unlink()
            source.unlink()
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, manifest, check=True)

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
                stage_notices(root, root / "stage", linked)
            stage = root / "stage"
            stage_notices(root, stage, _manifest(root, [_entry(root, "one.txt", "NOTICE")]))
            target = stage / "usr/share/doc/octessera/NOTICE"
            target.unlink()
            target.symlink_to(root / "one.txt")
            with self.assertRaises(NoticeStageError):
                stage_notices(root, stage, _manifest(root, [_entry(root, "one.txt", "NOTICE")]), check=True)


if __name__ == "__main__":
    unittest.main()
