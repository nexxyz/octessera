from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
REQUIRED_ENTRY_POINTS = (
    "README.md",
    "userdocs/README.md",
    "docs/README.md",
)


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


class ReleaseDocumentationTests(unittest.TestCase):
    def test_product_documentation_entry_points_exist(self) -> None:
        for relative in REQUIRED_ENTRY_POINTS:
            with self.subTest(relative=relative):
                self.assertTrue((ROOT / relative).is_file())

    def test_root_readme_links_both_reading_guides(self) -> None:
        text = read("README.md")
        for relative in ("userdocs/README.md", "docs/README.md"):
            with self.subTest(relative=relative):
                self.assertIn(f"]({relative})", text)


if __name__ == "__main__":
    unittest.main()
