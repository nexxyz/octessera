from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from disk_layout import DiskLayoutError, assert_no_drift, capture_layout


class DiskLayoutTests(unittest.TestCase):
    def _runner(self, board: str, loop_device: str = "/dev/loop0"):
        if board == "orange-pi-zero-2w":
            partitions = [{"node": f"{loop_device}p1", "start": 8, "size": 64, "type": "83", "uuid": "1111"}]
        else:
            partitions = [{"node": f"{loop_device}p1", "start": 8, "size": 16, "type": "c", "uuid": "1111"}, {"node": f"{loop_device}p2", "start": 24, "size": 64, "type": "83", "uuid": "2222"}]
        def run(command: list[str]) -> subprocess.CompletedProcess[str]:
            if command[0] == "sfdisk":
                return subprocess.CompletedProcess(command, 0, json.dumps({"partitiontable": {"label": "dos", "id": "disk-id", "firstlba": 1, "lastlba": 1024, "sectorsize": 512, "partitions": partitions}}), "")
            index = int(command[-1].rsplit("p", 1)[-1])
            filesystem = "ext4" if board == "orange-pi-zero-2w" or index == 2 else "vfat"
            return subprocess.CompletedProcess(command, 0, f"TYPE={filesystem}\nUUID={index:04d}\nLABEL=fixture\n", "")
        return run

    def test_captures_geometry_filesystem_identity_and_raw_regions_for_both_boards(self) -> None:
        for board in ("orange-pi-zero-2w", "raspberry-pi-zero-2w"):
            with self.subTest(board=board), tempfile.TemporaryDirectory() as temporary:
                image = Path(temporary) / "image.img"
                image.write_bytes(bytes(range(256)) * 256)
                layout = capture_layout(image, board, "/dev/loop0", self._runner(board))
                self.assertEqual(len(layout.partitions), 1 if board.startswith("orange") else 2)
                self.assertEqual(layout.partitions[-1].filesystem_type, "ext4")
                self.assertRegex(layout.raw_prepartition_sha256, r"^[0-9a-f]{64}$")
                if board.startswith("raspberry"):
                    self.assertRegex(layout.raw_boot_partition_sha256 or "", r"^[0-9a-f]{64}$")

    def test_wrong_filesystem_and_geometry_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "image.img"
            image.write_bytes(b"x" * 65536)
            def wrong_fs(command: list[str]) -> subprocess.CompletedProcess[str]:
                result = self._runner("orange-pi-zero-2w")(command)
                if command[0] == "blkid":
                    result.stdout = "TYPE=vfat\n"
                return result
            with self.assertRaises(DiskLayoutError):
                capture_layout(image, "orange-pi-zero-2w", "/dev/loop0", wrong_fs)
            before = capture_layout(image, "orange-pi-zero-2w", "/dev/loop0", self._runner("orange-pi-zero-2w"))
            changed = replace(before, partitions=(replace(before.partitions[0], size=before.partitions[0].size + 1),))
            with self.assertRaises(DiskLayoutError):
                assert_no_drift(before, changed)

    def test_loop_device_names_are_not_part_of_semantic_layout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "image.img"
            image.write_bytes(b"x" * 65536)
            first = capture_layout(image, "raspberry-pi-zero-2w", "/dev/loop0", self._runner("raspberry-pi-zero-2w", "/dev/loop0"))
            second = capture_layout(image, "raspberry-pi-zero-2w", "/dev/loop7", self._runner("raspberry-pi-zero-2w", "/dev/loop7"))
            self.assertEqual(first.as_dict(), second.as_dict())
            self.assertNotEqual(first.partitions[0].node, second.partitions[0].node)


if __name__ == "__main__":
    unittest.main()
