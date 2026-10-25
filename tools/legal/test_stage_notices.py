from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from stage_notices import NoticeStageError, stage_notices


ROOT = Path(__file__).resolve().parents[2]
LEGAL = "usr/share/doc/octessera"


def fixture(root: Path) -> Path:
    (root / "source.txt").write_bytes(b"canonical\n")
    entry = {"source": "source.txt", "destination": "licenses/NOTICE"}
    manifest = root / "notice-bundle.json"
    manifest.write_text(json.dumps({"files": [entry]}), encoding="utf-8")
    return manifest


class NoticeStagerTests(unittest.TestCase):
    def test_the_repository_manifest_stages_and_checks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            stage_notices(ROOT, Path(temporary))
            stage_notices(ROOT, Path(temporary), check=True)

    def test_check_rejects_missing_stale_and_extra_files(self) -> None:
        for change in ("missing", "stale", "extra"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                manifest = fixture(root)
                stage = root / "stage"
                stage_notices(root, stage, manifest)
                staged = stage / LEGAL / "licenses/NOTICE"
                if change == "missing":
                    staged.unlink()
                elif change == "stale":
                    staged.write_text("stale\n", encoding="utf-8")
                else:
                    (stage / LEGAL / "extra.txt").write_text("extra\n", encoding="utf-8")
                with self.assertRaises(NoticeStageError):
                    stage_notices(root, stage, manifest, check=True)

    def test_a_missing_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = fixture(root)
            (root / "source.txt").unlink()
            with self.assertRaises(NoticeStageError):
                stage_notices(root, root / "stage", manifest)


if __name__ == "__main__":
    unittest.main()
