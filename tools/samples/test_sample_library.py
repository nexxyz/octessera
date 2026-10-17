from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from sample_library import (
    SampleLibraryError,
    media_paths,
    stage_library,
    verify_media_tree,
    verify_metadata_tree,
)


ROOT = Path(__file__).resolve().parents[2]


class SampleLibraryTests(unittest.TestCase):
    def test_staged_media_and_licence_match_the_sample_folder(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            media = stage / "samples/files"
            metadata = stage / "samples"
            stage_library(ROOT, media, metadata)
            verify_media_tree(media, ROOT / "samples")
            verify_metadata_tree(metadata, ROOT / "samples")

    def test_staged_tree_with_a_changed_sample_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            media = Path(temporary) / "files"
            stage_library(ROOT, media, None)
            first = media / media_paths(ROOT / "samples")[0]
            first.write_bytes(b"changed")
            with self.assertRaises(SampleLibraryError):
                verify_media_tree(media, ROOT / "samples")

    def test_symlinked_source_entries_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "samples"
            (source / "Drum").mkdir(parents=True)
            try:
                (source / "Drum/link.wav").symlink_to(ROOT / "samples/Drum/snare/173087__yellowtree__wood-snare-sample-3.wav")
            except OSError:
                self.skipTest("symlink creation unavailable")
            with self.assertRaises(SampleLibraryError):
                media_paths(source)

    def test_staging_rejects_source_overlap_before_mutation(self) -> None:
        source_sample = ROOT / "samples/Drum/snare/173087__yellowtree__wood-snare-sample-3.wav"
        destination = ROOT / "samples/.rejected-staging"
        with self.assertRaises(SampleLibraryError):
            stage_library(ROOT, destination, None)
        self.assertTrue(source_sample.is_file())
        self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
