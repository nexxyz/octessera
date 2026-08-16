from __future__ import annotations

import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "legal"))

from package_portable_zip import package_portable_zip  # type: ignore[import-not-found]
from tools.release.verify_desktop_artifact import DesktopArtifactError, verify_portable_zip, verify_resource_layout


class DesktopArtifactTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tempdir = tempfile.TemporaryDirectory(prefix="octessera-desktop-artifact-tests-")
        cls.root = Path(cls.tempdir.name)
        cls.executable = cls.root / "octessera.exe"
        cls.executable.write_bytes(b"portable executable fixture")
        cls.portable = cls.root / "portable.zip"
        package_portable_zip(ROOT, cls.executable, cls.portable)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.tempdir.cleanup()

    def copy_archive(self, name: str) -> Path:
        target = self.root / name
        shutil.copyfile(self.portable, target)
        return target

    def rewrite_archive(self, name: str, mutate) -> Path:
        target = self.copy_archive(name)
        rewritten = target.with_suffix(".rewritten.zip")
        with zipfile.ZipFile(target) as source, zipfile.ZipFile(rewritten, "w") as output:
            for info in source.infolist():
                replacement = mutate(info, source.read(info.filename))
                output.writestr(info, replacement)
        return rewritten

    def test_portable_zip_has_exact_resources_executable_and_checksums(self) -> None:
        verify_portable_zip(ROOT, self.portable, self.executable)

    def test_portable_zip_rejects_tampered_sample_hash(self) -> None:
        def mutate(info: zipfile.ZipInfo, payload: bytes) -> bytes:
            return payload + b"tampered" if info.filename.startswith("samples/") else payload

        with self.assertRaises(DesktopArtifactError):
            verify_portable_zip(ROOT, self.rewrite_archive("tampered-sample.zip", mutate), self.executable)

    def test_portable_zip_rejects_extra_entry(self) -> None:
        archive = self.copy_archive("extra-entry.zip")
        with zipfile.ZipFile(archive, "a") as output:
            info = zipfile.ZipInfo("samples/extra.wav", (1980, 1, 1, 0, 0, 0))
            info.external_attr = 0o100644 << 16
            output.writestr(info, b"extra")
        with self.assertRaises(DesktopArtifactError):
            verify_portable_zip(ROOT, archive, self.executable)

    def test_portable_zip_rejects_wrong_mode(self) -> None:
        def mutate(info: zipfile.ZipInfo, payload: bytes) -> bytes:
            if info.filename == "octessera.exe":
                info.external_attr = 0o100755 << 16
            return payload

        with self.assertRaises(DesktopArtifactError):
            verify_portable_zip(ROOT, self.rewrite_archive("wrong-mode.zip", mutate), self.executable)

    def test_extracted_resource_tree_rejects_missing_sample_and_extra_metadata(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-resource-fixture-") as temporary:
            resource_root = Path(temporary)
            with zipfile.ZipFile(self.portable) as archive:
                for info in archive.infolist():
                    target = resource_root / info.filename
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(info.filename))
            verify_resource_layout(ROOT, resource_root)
            sample = next((resource_root / "samples").rglob("*.wav"))
            sample.unlink()
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)

            with zipfile.ZipFile(self.portable) as archive:
                for info in archive.infolist():
                    target = resource_root / info.filename
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(info.filename))
            (resource_root / "samples" / "sample-manifest.tsv").write_text("duplicate metadata", encoding="utf-8")
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)

    def test_extracted_resource_tree_rejects_tampered_legal_file(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-resource-legal-fixture-") as temporary:
            resource_root = Path(temporary)
            with zipfile.ZipFile(self.portable) as archive:
                for info in archive.infolist():
                    target = resource_root / info.filename
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(info.filename))
            (resource_root / "legal" / "LICENSE").write_bytes(b"tampered legal notice")
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)

    def test_extracted_resource_tree_requires_notice_bundle_and_rejects_legal_extras(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-resource-legal-contract-") as temporary:
            resource_root = Path(temporary)
            with zipfile.ZipFile(self.portable) as archive:
                for info in archive.infolist():
                    target = resource_root / info.filename
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(info.filename))

            (resource_root / "legal" / "notice-bundle.json").unlink()
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)

            with zipfile.ZipFile(self.portable) as archive:
                for info in archive.infolist():
                    target = resource_root / info.filename
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(archive.read(info.filename))
            (resource_root / "legal" / "unexpected.txt").write_text("extra", encoding="utf-8")
            with self.assertRaises(DesktopArtifactError):
                verify_resource_layout(ROOT, resource_root)


if __name__ == "__main__":
    unittest.main()
