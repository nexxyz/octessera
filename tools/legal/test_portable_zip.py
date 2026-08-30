from __future__ import annotations

import hashlib
import tempfile
import unittest
import zipfile
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).parent))

from package_portable_zip import package_portable_zip
from verify_notice_archive import verify_notice_archive
from tools.samples.sample_library import sample_media_payload_files


ROOT = Path(__file__).resolve().parents[2]


class PortableZipTests(unittest.TestCase):
    def test_portable_zip_preserves_executable_and_legal_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / "octessera.exe"
            executable.write_bytes(b"unchanged-executable")
            output = root / "portable.zip"
            package_portable_zip(ROOT, executable, output)
            verify_notice_archive(ROOT, output, "octessera.exe")
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(archive.read("octessera.exe"), executable.read_bytes())
                self.assertIn(
                    f"{hashlib.sha256(executable.read_bytes()).hexdigest()}  octessera.exe",
                    archive.read("SHA256SUMS").decode("utf-8").splitlines(),
                )
                samples = sample_media_payload_files(ROOT)
                self.assertEqual(
                    {name for name in archive.namelist() if name.startswith("samples/")},
                    set(samples),
                )
                self.assertNotIn("samples/MANIFEST.tsv", archive.namelist())
                self.assertNotIn("samples/SOURCE.md", archive.namelist())
                self.assertIn("legal/samples/MANIFEST.tsv", archive.namelist())
                self.assertIn("legal/samples/SOURCE.md", archive.namelist())
                for name, payload in samples.items():
                    self.assertEqual(archive.read(name), payload)

    def test_rejects_tampered_legal_payload_with_recomputed_archive_checksum(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / "octessera.exe"
            executable.write_bytes(b"unchanged-executable")
            source = root / "portable.zip"
            tampered = root / "tampered.zip"
            package_portable_zip(ROOT, executable, source)
            with zipfile.ZipFile(source) as archive, zipfile.ZipFile(tampered, "w") as output:
                for info in archive.infolist():
                    payload = archive.read(info.filename)
                    if info.filename == "legal/LICENSE":
                        payload = b"tampered portable legal payload\n"
                    if info.filename == "SHA256SUMS":
                        lines = []
                        for line in archive.read(info.filename).decode("utf-8").splitlines():
                            digest, name = line.split("  ", 1)
                            if name == "legal/LICENSE":
                                digest = hashlib.sha256(b"tampered portable legal payload\n").hexdigest()
                            lines.append(f"{digest}  {name}")
                        payload = ("\n".join(lines) + "\n").encode("utf-8")
                    output.writestr(info, payload)
            with self.assertRaises(ValueError):
                verify_notice_archive(ROOT, tampered, "octessera.exe")


if __name__ == "__main__":
    unittest.main()
