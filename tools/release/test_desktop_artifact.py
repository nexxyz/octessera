from __future__ import annotations

import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "legal"))

from package_portable_zip import package_portable_zip  # type: ignore[import-not-found]
from tools.release.verify_desktop_artifact import DesktopArtifactError, verify_resource_layout


class DesktopArtifactTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tempdir = tempfile.TemporaryDirectory(prefix="octessera-desktop-artifact-tests-")
        root = Path(cls.tempdir.name)
        executable = root / "octessera.exe"
        executable.write_bytes(b"portable executable fixture")
        cls.portable = root / "portable.zip"
        package_portable_zip(ROOT, executable, cls.portable)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.tempdir.cleanup()

    def resources(self, temporary: str) -> Path:
        """An installed resource tree, as the portable zip lays it out."""
        resource_root = Path(temporary)
        with zipfile.ZipFile(self.portable) as archive:
            archive.extractall(resource_root)
        verify_resource_layout(ROOT, resource_root)
        return resource_root

    def test_rejects_a_missing_sample(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            resource_root = self.resources(temporary)
            next((resource_root / "samples").rglob("*.wav")).unlink()
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)

    def test_rejects_an_extra_sample_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            resource_root = self.resources(temporary)
            (resource_root / "samples" / "notes.txt").write_text("unexpected file", encoding="utf-8")
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)

    def test_rejects_a_changed_missing_or_extra_legal_file(self) -> None:
        for change in ("changed", "missing", "extra"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                legal = self.resources(temporary) / "legal"
                if change == "changed":
                    (legal / "LICENSE").write_bytes(b"changed legal notice")
                elif change == "missing":
                    (legal / "notice-bundle.json").unlink()
                else:
                    (legal / "unexpected.txt").write_text("extra", encoding="utf-8")
                with self.assertRaises(DesktopArtifactError):
                    verify_resource_layout(ROOT, legal.parent)


if __name__ == "__main__":
    unittest.main()
