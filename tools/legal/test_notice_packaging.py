from __future__ import annotations

import hashlib
import tempfile
import unittest
import zipfile
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).parent))

from package_notice_zip import package_notice_zip
from stage_notices import load_manifest
from verify_notice_archive import verify_notice_archive


ROOT = Path(__file__).resolve().parents[2]


class NoticePackagingTests(unittest.TestCase):
    @staticmethod
    def _rewrite_with_tampered_legal(source: Path, destination: Path) -> None:
        with zipfile.ZipFile(source) as archive, zipfile.ZipFile(destination, "w") as output:
            for info in archive.infolist():
                payload = archive.read(info.filename)
                if info.filename == "legal/LICENSE":
                    payload = b"tampered legal payload\n"
                if info.filename == "SHA256SUMS":
                    lines = []
                    for line in archive.read(info.filename).decode("utf-8").splitlines():
                        digest, name = line.split("  ", 1)
                        if name == "legal/LICENSE":
                            digest = hashlib.sha256(b"tampered legal payload\n").hexdigest()
                        lines.append(f"{digest}  {name}")
                    payload = ("\n".join(lines) + "\n").encode("utf-8")
                output.writestr(info, payload)

    def test_notice_zip_is_manifest_exact_and_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            first = Path(temporary) / "first.zip"
            second = Path(temporary) / "second.zip"
            package_notice_zip(ROOT, first)
            package_notice_zip(ROOT, second)
            verify_notice_archive(ROOT, first)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            manifest = load_manifest(ROOT / "resources/legal/notice-bundle.json")
            expected = {f"legal/{item['destination']}" for item in manifest["files"]}
            expected.add("legal/notice-bundle.json")
            with zipfile.ZipFile(first) as archive:
                names = set(archive.namelist())
                self.assertEqual(names, {*expected, "SHA256SUMS"})
                checksums = archive.read("SHA256SUMS").decode().splitlines()
                self.assertEqual({line.split("  ", 1)[1] for line in checksums}, expected)
                for line in checksums:
                    digest, name = line.split("  ", 1)
                    self.assertEqual(digest, hashlib.sha256(archive.read(name)).hexdigest())

    def test_rejects_tampered_legal_payload_with_recomputed_archive_checksum(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.zip"
            tampered = Path(temporary) / "tampered.zip"
            package_notice_zip(ROOT, source)
            self._rewrite_with_tampered_legal(source, tampered)
            with self.assertRaises(ValueError):
                verify_notice_archive(ROOT, tampered)


if __name__ == "__main__":
    unittest.main()
