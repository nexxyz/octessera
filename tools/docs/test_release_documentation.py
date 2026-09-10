from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
REQUIRED_ENTRY_POINTS = (
    "docs/development-workflows.md",
    "docs/release-licensing.md",
    "release-artifacts/README.md",
    "userdocs/README.md",
    "userdocs/release-support.md",
)


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


class ReleaseDocumentationTests(unittest.TestCase):
    def test_release_and_user_doc_entry_points_exist(self) -> None:
        for relative in REQUIRED_ENTRY_POINTS:
            with self.subTest(relative=relative):
                self.assertTrue((ROOT / relative).is_file())

    def test_user_docs_readme_links_release_support(self) -> None:
        text = read("userdocs/README.md")
        self.assertRegex(text, r"\[[^\]]+\]\(release-support\.md(?:#[^)]+)?\)")

    def test_support_matrix_has_fixed_platform_rows(self) -> None:
        text = read("userdocs/release-support.md")
        table_rows = [
            [cell.strip() for cell in line.strip().strip("|").split("|")]
            for line in text.splitlines()
            if line.strip().startswith("|")
        ]
        platform_rows = {
            platform: [row for row in table_rows if platform in row]
            for platform in ("Desktop", "Raspberry Pi Zero 2 W", "Orange Pi Zero 2W")
        }
        for platform, rows in platform_rows.items():
            with self.subTest(platform=platform):
                self.assertEqual(len(rows), 1)
        rows = [rows[0] for rows in platform_rows.values()]
        self.assertEqual(len({len(row) for row in rows}), 1)
        self.assertGreaterEqual(len(rows[0]), 5)


if __name__ == "__main__":
    unittest.main()
