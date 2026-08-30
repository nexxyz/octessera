from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(ROOT))
from tools.samples.sample_library import read_manifest, stage_library, verify_media_tree


class TauriNoticeResourceTests(unittest.TestCase):
    def test_tauri_build_stages_only_manifest_driven_legal_resource(self) -> None:
        config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
        self.assertEqual(
            config["bundle"]["resources"],
            {
                "../../../release-legal/usr/share/doc/octessera": "legal",
                "../../../resources/legal/notice-bundle.json": "legal/notice-bundle.json",
                "../../../release-samples/samples": "samples",
            },
        )
        self.assertIn("legal:stage-desktop", config["build"]["beforeBuildCommand"])

    def test_native_setup_resolves_the_stable_samples_resource(self) -> None:
        samples = (ROOT / "apps/desktop/src-tauri/src/samples.rs").read_text(encoding="utf-8")
        desktop = (ROOT / "apps/desktop/src-tauri/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn('resolve("samples", BaseDirectory::Resource)', samples)
        self.assertIn("samples::initialize_samples_root(app)", desktop)
        self.assertNotIn("create samples dir", samples)

    def test_desktop_resource_stage_contains_media_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            media = root / "samples"
            stage_library(ROOT, media, None, root / "MANIFEST.tsv")
            records = read_manifest(ROOT / "samples/MANIFEST.tsv")
            verify_media_tree(media, records)
            self.assertFalse((media / "MANIFEST.tsv").exists())
            self.assertFalse((media / "SOURCE.md").exists())
            self.assertFalse((media / "upstream").exists())


if __name__ == "__main__":
    unittest.main()
