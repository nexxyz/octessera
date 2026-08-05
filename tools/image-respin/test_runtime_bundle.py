from __future__ import annotations

import json
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from runtime_bundle import RuntimeBundleError, create_bundle


BOARDS = ("raspberry-pi-zero-2w", "orange-pi-zero-2w")


def elf(machine: int = 183) -> bytes:
    header = bytearray(64)
    header[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<H", header, 18, machine)
    struct.pack_into("<H", header, 52, 64)
    return bytes(header) + b"synthetic production runtime"


class RuntimeBundleTests(unittest.TestCase):
    def test_both_boards_create_exact_bundle(self) -> None:
        for board in BOARDS:
            with self.subTest(board=board), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                binary = root / "octessera-pi"
                binary.write_bytes(elf())
                output = create_bundle(binary, board, "1.2.3", root / "bundle")
                self.assertEqual(
                    sorted(path.name for path in output.iterdir()),
                    ["SHA256SUMS", "octessera-pi", "octessera-runtime.json"],
                )
                self.assertEqual(output.joinpath("octessera-pi").read_bytes(), binary.read_bytes())
                if os.name != "nt":
                    self.assertEqual(output.joinpath("octessera-pi").stat().st_mode & 0o777, 0o755)
                metadata = json.loads((output / "octessera-runtime.json").read_text())
                self.assertEqual(metadata["profile"], board)
                self.assertEqual(metadata["version"], "1.2.3")
                if os.name != "nt":
                    self.assertEqual((output / "octessera-runtime.json").stat().st_mode & 0o777, 0o644)
                    self.assertEqual((output / "SHA256SUMS").stat().st_mode & 0o777, 0o644)

    def test_rejects_x86_malformed_and_non_strict_version(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, payload in (("x86", elf(62)), ("malformed", b"\x7fELF\x02")):
                binary = root / name
                binary.write_bytes(payload)
                with self.subTest(case=name), self.assertRaises(RuntimeBundleError):
                    create_bundle(binary, BOARDS[0], "1.2.3", root / f"{name}-bundle")
            binary = root / "valid"
            binary.write_bytes(elf())
            for version in ("v1.2.3", "1.2", "1.2.3-alpha", "01.2.3"):
                with self.subTest(version=version), self.assertRaises(RuntimeBundleError):
                    create_bundle(binary, BOARDS[0], version, root / f"bundle-{version}")

    def test_rejects_output_overwrite_extra_and_symlink_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "valid"
            binary.write_bytes(elf())
            output = create_bundle(binary, BOARDS[0], "1.2.3", root / "bundle")
            with self.assertRaises(RuntimeBundleError):
                create_bundle(binary, BOARDS[0], "1.2.3", output)
            (root / "external").write_bytes(b"outside")
            try:
                os.symlink(root / "external", root / "linked")
            except OSError as error:
                self.skipTest(f"symlinks unavailable: {error}")
            with self.assertRaises(RuntimeBundleError):
                create_bundle(root / "linked", BOARDS[0], "1.2.3", root / "linked-bundle")
            (output / "extra").write_bytes(b"extra")
            with self.assertRaises(RuntimeBundleError):
                from runtime_bundle import _validate_bundle

                _validate_bundle(output, "1.2.3", BOARDS[0], metadata_hash(output))


def metadata_hash(bundle: Path) -> str:
    import hashlib

    return hashlib.sha256((bundle / "octessera-pi").read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()
