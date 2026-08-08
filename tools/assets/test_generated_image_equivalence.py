from __future__ import annotations

import struct
import shutil
import subprocess
import tempfile
import unittest
import zlib
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))
from generated_image_equivalence import images_equivalent  # noqa: E402


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
RAW = b"".join(b"\0" + bytes([x, x, x, 255]) * 128 for x in range(128))


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


def png(compression: int = 9, payload: bytes = RAW, compressed_suffix: bytes = b"") -> bytes:
    ihdr = struct.pack(">IIBBBBB", 128, 128, 8, 6, 0, 0, 0)
    compressed = zlib.compress(payload, compression) + compressed_suffix
    return PNG_SIGNATURE + chunk(b"IHDR", ihdr) + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")


def recompress(data: bytes, compression: int) -> bytes:
    position = len(PNG_SIGNATURE)
    result = bytearray(PNG_SIGNATURE)
    while position < len(data):
        length = struct.unpack_from(">I", data, position)[0]
        kind = data[position + 4 : position + 8]
        payload = data[position + 8 : position + 8 + length]
        if kind == b"IDAT":
            payload = zlib.compress(zlib.decompress(payload), compression)
        result.extend(chunk(kind, payload))
        position += 12 + length
    return bytes(result)


def ico(png_data: bytes) -> bytes:
    entry = bytes((128, 128, 0, 0)) + struct.pack("<HHII", 1, 32, len(png_data), 22)
    return struct.pack("<HHH", 0, 1, 1) + entry + png_data


class GeneratedImageEquivalenceTests(unittest.TestCase):
    def test_generator_check_and_write_preserve_alternate_encoding(self) -> None:
        root = Path(__file__).parents[2]
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory)
            (fixture / "assets").mkdir()
            (fixture / "apps/desktop/src-tauri/icons").mkdir(parents=True)
            for relative in (
                "octessera-mark.svg",
                "octessera-wordmark.svg",
                "octessera-pi-manifest.png",
                "octessera-app-large.png",
                "octessera-pi-sleeping.png",
                "octessera-pi-shutdown.png",
                "octessera-pi-booting.png",
            ):
                shutil.copy2(root / "assets" / relative, fixture / "assets" / relative)
            for relative in ("icon.png", "icon.ico"):
                shutil.copy2(root / "apps/desktop/src-tauri/icons" / relative, fixture / "apps/desktop/src-tauri/icons" / relative)
            target = fixture / "assets/octessera-pi-manifest.png"
            original = target.read_bytes()
            alternate = recompress(original, 1)
            target.write_bytes(alternate)
            icon_target = fixture / "apps/desktop/src-tauri/icons/icon.ico"
            icon_original = icon_target.read_bytes()
            icon_alternate = ico(recompress(icon_original[22:], 1))
            icon_target.write_bytes(icon_alternate)
            command = [sys.executable, str(root / "tools/assets/generate_pi_logo_pngs.py"), "--check", "--root", str(fixture)]
            self.assertEqual(subprocess.run(command, check=False).returncode, 0)
            subprocess.run([argument for argument in command if argument != "--check"], check=True)
            self.assertEqual(target.read_bytes(), alternate)
            self.assertEqual(icon_target.read_bytes(), icon_alternate)
            self.assertNotEqual(original, alternate)
            self.assertNotEqual(icon_original, icon_alternate)

    def test_alternate_compression_is_equivalent_for_png_and_ico(self) -> None:
        compact = png(9)
        alternate = png(1)
        self.assertNotEqual(compact, alternate)
        self.assertTrue(images_equivalent(compact, alternate, "png"))
        self.assertTrue(images_equivalent(ico(compact), ico(alternate), "ico"))

    def test_pixel_drift_and_malformed_data_fail(self) -> None:
        original = png()
        drifted = png(payload=RAW[:-5] + b"\0\0\0\0\0")
        self.assertFalse(images_equivalent(original, drifted, "png"))
        bad_crc = bytearray(original)
        bad_crc[-5] ^= 1
        self.assertFalse(images_equivalent(original, bytes(bad_crc), "png"))
        self.assertFalse(images_equivalent(original, original[:-1], "png"))
        self.assertFalse(images_equivalent(original, original + b"trailing", "png"))
        self.assertFalse(images_equivalent(original, png(compressed_suffix=b"trailing"), "png"))

    def test_ico_layout_and_embedded_png_are_strict(self) -> None:
        original = ico(png())
        self.assertTrue(images_equivalent(original, original, "ico"))
        self.assertFalse(images_equivalent(original, original[:-1], "ico"))
        malformed = bytearray(original)
        struct.pack_into("<I", malformed, 18, 23)
        self.assertFalse(images_equivalent(original, bytes(malformed), "ico"))


if __name__ == "__main__":
    unittest.main()
