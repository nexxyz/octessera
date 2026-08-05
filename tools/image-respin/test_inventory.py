from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from inventory import InventoryError, build_inventory, ensure_inventory_symlinks_contained, inventory_digest


class InventoryTests(unittest.TestCase):
    def test_inventory_records_types_metadata_hash_and_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "folder").mkdir()
            (root / "folder/file").write_bytes(b"inventory")
            (root / "link").symlink_to("folder/file")
            inventory = build_inventory(root)
            self.assertEqual(inventory["folder"]["type"], "directory")
            self.assertEqual(inventory["folder/file"]["sha256"], "b11a85b296a90afc460430434a504e7acb04b064575b7277724688af0e59d189")
            self.assertEqual(inventory["link"]["target"], "folder/file")
            self.assertEqual(inventory["link"]["sha256"], None)

    def test_digest_is_stable_and_xattrs_are_recorded_when_supported(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "file"
            path.write_bytes(b"xattrs")
            setter = getattr(os, "setxattr", None)
            if setter is None:
                self.skipTest("xattrs are unavailable")
            try:
                setter(path, "user.octessera-test", b"one")
            except (AttributeError, OSError):
                self.skipTest("filesystem does not support test xattrs")
            first = build_inventory(root)
            second = build_inventory(root)
            self.assertEqual(inventory_digest(first), inventory_digest(second))
            self.assertEqual(first["file"]["xattrs"]["user.octessera-test"], b"one".hex())

    def test_escape_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            root.mkdir()
            outside = Path(temporary) / "outside"
            outside.write_text("outside", encoding="utf-8")
            (root / "escape").symlink_to(outside)
            inventory = build_inventory(root)
            with self.assertRaises(InventoryError):
                ensure_inventory_symlinks_contained(root, inventory)


if __name__ == "__main__":
    unittest.main()
