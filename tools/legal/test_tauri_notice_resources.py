from __future__ import annotations

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class TauriNoticeResourceTests(unittest.TestCase):
    def test_tauri_build_stages_only_manifest_driven_legal_resource(self) -> None:
        config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
        self.assertEqual(config["bundle"]["resources"], {"../../../release-legal/usr/share/doc/octessera": "legal"})
        self.assertIn("legal:stage-desktop", config["build"]["beforeBuildCommand"])


if __name__ == "__main__":
    unittest.main()
