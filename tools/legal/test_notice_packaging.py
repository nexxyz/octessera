from __future__ import annotations

import tempfile
import unittest
import zipfile
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).parent))

from package_notice_zip import notice_payload, package_notice_zip
from verify_notice_archive import verify_notice_archive


ROOT = Path(__file__).resolve().parents[2]


def rewrite(source: Path, destination: Path, name: str, payload: bytes) -> None:
    with zipfile.ZipFile(source) as archive, zipfile.ZipFile(destination, "w") as output:
        for info in archive.infolist():
            output.writestr(info, payload if info.filename == name else archive.read(info.filename))


class NoticePackagingTests(unittest.TestCase):
    def test_notice_zip_holds_exactly_the_legal_files_and_is_reproducible(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            first = Path(temporary) / "first.zip"
            second = Path(temporary) / "second.zip"
            package_notice_zip(ROOT, first)
            package_notice_zip(ROOT, second)
            verify_notice_archive(ROOT, first)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with zipfile.ZipFile(first) as archive:
                self.assertEqual(set(archive.namelist()), {*notice_payload(ROOT), "SHA256SUMS"})

    def test_verification_rejects_a_changed_legal_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "source.zip"
            changed = Path(temporary) / "changed.zip"
            package_notice_zip(ROOT, source)
            rewrite(source, changed, "legal/LICENSE", b"changed legal payload\n")
            with self.assertRaises(ValueError):
                verify_notice_archive(ROOT, changed)


if __name__ == "__main__":
    unittest.main()
