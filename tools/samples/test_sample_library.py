from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from sample_library import (
    EXPECTED_MEDIA_COUNT,
    SampleLibraryError,
    read_manifest,
    stage_library,
    verify_library,
    verify_manifest,
    verify_media_tree,
    verify_metadata_tree,
)


ROOT = Path(__file__).resolve().parents[2]


class SampleLibraryTests(unittest.TestCase):
    def test_repository_inventory_is_complete_and_hashed(self) -> None:
        records = verify_library(ROOT / "samples", ROOT / "samples/MANIFEST.tsv")
        self.assertEqual(len(records), EXPECTED_MEDIA_COUNT)

    def test_staged_media_metadata_manifest_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            media = stage / "samples/files"
            metadata = stage / "samples"
            manifest = metadata / "MANIFEST.tsv"
            stage_library(ROOT, media, metadata, manifest)
            records = read_manifest(ROOT / "samples/MANIFEST.tsv")
            verify_media_tree(media, records)
            verify_metadata_tree(metadata, ROOT / "samples")
            verify_manifest(manifest, records)

    def test_staging_rejects_symlinked_source_tree_entries(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "samples"
            (source / "upstream").mkdir(parents=True)
            (source / "MANIFEST.tsv").write_bytes(b"manifest")
            (source / "SOURCE.md").write_bytes(b"source")
            (source / "upstream/LICENSE").write_bytes(b"license")
            try:
                (source / "Drum").mkdir()
                (source / "Drum/link.wav").symlink_to(ROOT / "samples/Drum/snare/173087__yellowtree__wood-snare-sample-3.wav")
            except OSError:
                self.skipTest("symlink creation unavailable")
            with self.assertRaises(SampleLibraryError):
                verify_library(source, source / "MANIFEST.tsv")

    def test_staging_rejects_source_overlap_before_mutation(self) -> None:
        source_sample = ROOT / "samples/Drum/snare/173087__yellowtree__wood-snare-sample-3.wav"
        destination = ROOT / "samples/.rejected-staging"
        with self.assertRaises(SampleLibraryError):
            stage_library(ROOT, destination, None, destination / "MANIFEST.tsv")
        self.assertTrue(source_sample.is_file())
        self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
