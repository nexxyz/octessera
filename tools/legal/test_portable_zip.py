from __future__ import annotations

import tempfile
import unittest
import zipfile
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).parent))

from package_portable_zip import package_portable_zip
from test_notice_packaging import rewrite
from verify_notice_archive import verify_notice_archive
from tools.samples.sample_library import sample_media_payload_files


ROOT = Path(__file__).resolve().parents[2]


class PortableZipTests(unittest.TestCase):
    def test_portable_zip_carries_the_executable_samples_and_legal_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / "octessera.exe"
            executable.write_bytes(b"unchanged-executable")
            output = root / "portable.zip"
            package_portable_zip(ROOT, executable, output)
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(archive.namelist()[0], "octessera.exe")
                self.assertEqual(archive.read("octessera.exe"), executable.read_bytes())
                samples = sample_media_payload_files(ROOT)
                self.assertEqual({name for name in archive.namelist() if name.startswith("samples/")}, set(samples))
                self.assertIn("legal/samples/upstream/LICENSE", archive.namelist())
            changed = root / "changed.zip"
            rewrite(output, changed, "legal/LICENSE", b"changed portable legal payload\n")
            with self.assertRaises(ValueError):
                verify_notice_archive(ROOT, changed, "octessera.exe")


if __name__ == "__main__":
    unittest.main()
