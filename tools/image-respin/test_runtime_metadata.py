from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from runtime_contract import BUILD_METADATA_KEY_ORDER
from runtime_mutation import MutationError, mutate_runtime
from test_runtime_mutation import ORANGE, RPI, _fixture, _parent_context


class RuntimeMetadataTests(unittest.TestCase):
    def test_orange_build_metadata_preserves_non_runtime_lines_and_rejects_bad_structure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, bundle = _fixture(Path(temporary), ORANGE)
            metadata_path = root / "etc/octessera/build-metadata.env"
            mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))
            output_lines = metadata_path.read_bytes().splitlines(keepends=True)
            self.assertEqual(tuple(line.decode().split("=", 1)[0] for line in output_lines), BUILD_METADATA_KEY_ORDER)
            self.assertEqual(output_lines[0], b"OCTESSERA_IMAGE_KIND=armbian\n")
            self.assertEqual(output_lines[1], b"OCTESSERA_IMAGE_MODE=production\n")
        for mutation in ("crlf", "duplicate", "missing", "extra-runtime", "inconsistent", "prior-version"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root, bundle = _fixture(Path(temporary), ORANGE)
                path = root / "etc/octessera/build-metadata.env"
                raw = path.read_bytes()
                if mutation == "crlf":
                    raw = raw.replace(b"\n", b"\r\n")
                elif mutation == "duplicate":
                    raw += b"OCTESSERA_IMAGE_MODE=production\n"
                elif mutation == "missing":
                    raw = b"\n".join(line for line in raw.splitlines() if not line.startswith(b"OCTESSERA_RUNTIME_VERSION=")) + b"\n"
                elif mutation == "extra-runtime":
                    raw += b"OCTESSERA_RUNTIME_UNKNOWN=x\n"
                elif mutation == "prior-version":
                    raw = raw.replace(b"OCTESSERA_RUNTIME_VERSION=1.0.0\n", b"OCTESSERA_RUNTIME_VERSION=1.0\n")
                else:
                    raw = b"".join(b"OCTESSERA_RUNTIME_BINARY_SHA256=" + b"0" * 64 + b"\n" if line.startswith(b"OCTESSERA_RUNTIME_BINARY_SHA256=") else line for line in raw.splitlines(keepends=True))
                path.write_bytes(raw)
                with self.assertRaises(MutationError):
                    mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))

    def test_raspberry_does_not_own_or_transform_orange_build_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, bundle = _fixture(Path(temporary), RPI)
            metadata_path = root / "etc/octessera/build-metadata.env"
            metadata_path.parent.mkdir(parents=True)
            metadata_path.write_bytes(b"OCTESSERA_IMAGE_MODE=production\n")
            def mutate_metadata(name: str) -> None:
                if name == "staged":
                    metadata_path.write_bytes(b"OCTESSERA_IMAGE_MODE=diagnostic\n")
            with self.assertRaises(MutationError):
                mutate_runtime(root, bundle, RPI, "2.0.0", "source-1", _parent_context(RPI), mutation_hook=mutate_metadata)


if __name__ == "__main__":
    unittest.main()
