from __future__ import annotations

import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from current_parent import parent_context
from disk_packaging import DiskPackagingError, file_digest, package_derived, prepare_parent_image, provenance_sidecar


def _context(board: str, source: Path) -> dict:
    digest, size = file_digest(source)
    return {"schema": "octessera.image-current-parent/v1", "repository": "nexxyz/octessera", "board_profile": board, "version": "0.8.1", "constructor": {"run_id": 33301343618, "source_sha": "a" * 40}, "artifact": {"id": 9730022123, "name": "octessera-orange-image-release-assets", "size": 1, "digest": "sha256:" + "a" * 64, "expires_at": "2099-01-01T00:00:00Z", "entries": []}, "image": {"name": source.name, "size": size, "sha256": digest}, "record": {"path": "resources/image-parents/orange-pi-zero-2w-current.json", "sha256": "c" * 64, "size": 1}}


class DiskPackagingTests(unittest.TestCase):
    def test_parent_context_is_derived_from_the_checked_current_parent_record(self) -> None:
        root = Path(__file__).resolve().parents[2]
        record_path = root / "resources/image-parents/orange-pi-zero-2w-current.json"
        context = parent_context(root, record_path)
        self.assertEqual(context["schema"], "octessera.image-current-parent/v1")
        self.assertEqual(context["image"]["name"], "octessera-0.8.1-orange-pi-zero-2w.img.xz")
        self.assertEqual(set(context["record"]), {"path", "sha256", "size"})

    def test_raspberry_zip_is_exact_and_repacked_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            raw = work / "parent.img"
            raw.write_bytes(b"synthetic partitioned image" * 100)
            source = work / "parent.zip"
            imager_manifest = b"synthetic-imager-manifest\n"
            with zipfile.ZipFile(source, "w") as archive:
                archive.writestr("parent.img", raw.read_bytes())
                archive.writestr("os_list.rpi-imager-manifest", imager_manifest)
            context = _context("raspberry-pi-zero-2w", source)
            prepared = prepare_parent_image(source, context, "c" * 64, "raspberry-pi-zero-2w", imager_manifest)
            try:
                output_a = package_derived(prepared.image, work / "a" / "octessera-2.0.0-raspberry-pi-zero-2w-derived-runtime-respin.zip", "raspberry-pi-zero-2w", "2.0.0")
                output_b = package_derived(prepared.image, work / "b" / "octessera-2.0.0-raspberry-pi-zero-2w-derived-runtime-respin.zip", "raspberry-pi-zero-2w", "2.0.0")
            finally:
                prepared.close()
            with zipfile.ZipFile(output_a) as archive:
                self.assertEqual(archive.namelist(), ["octessera-2.0.0-raspberry-pi-zero-2w-derived-runtime-respin.img"])
            self.assertEqual(output_a.read_bytes(), output_b.read_bytes())

    def test_unsafe_or_extra_zip_members_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "parent.zip"
            imager_manifest = b"manifest"
            with zipfile.ZipFile(source, "w") as archive:
                archive.writestr("../escape.img", b"bad")
            context = _context("raspberry-pi-zero-2w", source)
            with self.assertRaises(DiskPackagingError):
                prepare_parent_image(source, context, "c" * 64, "raspberry-pi-zero-2w", imager_manifest)
            extra = work / "extra.zip"
            with zipfile.ZipFile(extra, "w") as archive:
                archive.writestr("parent.img", b"image")
                archive.writestr("os_list.rpi-imager-manifest", imager_manifest)
                archive.writestr("extra.txt", b"extra")
            extra_context = _context("raspberry-pi-zero-2w", extra)
            with self.assertRaises(DiskPackagingError):
                prepare_parent_image(extra, extra_context, "c" * 64, "raspberry-pi-zero-2w", imager_manifest)
            symlink_zip = work / "symlink.zip"
            with zipfile.ZipFile(symlink_zip, "w") as archive:
                link = zipfile.ZipInfo("parent.img")
                link.create_system = 3
                link.external_attr = (0o120777 << 16) | 0x1
                archive.writestr(link, b"target")
                archive.writestr("os_list.rpi-imager-manifest", imager_manifest)
            symlink_context = _context("raspberry-pi-zero-2w", symlink_zip)
            with self.assertRaises(DiskPackagingError):
                prepare_parent_image(symlink_zip, symlink_context, "c" * 64, "raspberry-pi-zero-2w", imager_manifest)
            mismatch = work / "mismatch.zip"
            with zipfile.ZipFile(mismatch, "w") as archive:
                archive.writestr("parent.img", b"image")
                archive.writestr("os_list.rpi-imager-manifest", b"embedded")
            mismatch_context = _context("raspberry-pi-zero-2w", mismatch)
            with self.assertRaises(DiskPackagingError):
                prepare_parent_image(mismatch, mismatch_context, "c" * 64, "raspberry-pi-zero-2w", imager_manifest)

    def test_orange_xz_round_trip_and_output_name_are_derived(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            raw = work / "parent.img"
            raw.write_bytes(b"orange image" * 100)
            import lzma
            source = work / "parent.img.xz"
            with lzma.open(source, "wb") as stream:
                stream.write(raw.read_bytes())
            context = _context("orange-pi-zero-2w", source)
            prepared = prepare_parent_image(source, context, "c" * 64, "orange-pi-zero-2w")
            try:
                output = package_derived(prepared.image, work / "octessera-2.0.0-orange-pi-zero-2w-derived-runtime-respin.img.xz", "orange-pi-zero-2w", "2.0.0")
            finally:
                prepared.close()
            with lzma.open(output, "rb") as stream:
                self.assertEqual(stream.read(), raw.read_bytes())
            self.assertIn("derived", output.name)
            self.assertEqual(provenance_sidecar(output).name, output.name + ".provenance.json")


if __name__ == "__main__":
    unittest.main()
