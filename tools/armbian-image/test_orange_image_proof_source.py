from __future__ import annotations

import json
import subprocess

import orange_image_mount


def run_source_proof() -> None:
    original_run = orange_image_mount._run

    def fake_lsblk_run(command: list[str], **_: object) -> subprocess.CompletedProcess[str]:
        if "--bytes" not in command:
            raise AssertionError("lsblk partition geometry must use bytes")
        payload = {
            "blockdevices": [
                {
                    "name": "/dev/loop0",
                    "type": "loop",
                    "children": [
                        {"name": "/dev/loop0p1", "type": "part", "start": 2048, "size": 536870912}
                    ],
                }
            ]
        }
        return subprocess.CompletedProcess(command, 0, json.dumps(payload), "")

    try:
        orange_image_mount._run = fake_lsblk_run
        assert orange_image_mount._lsblk("/dev/loop0") == ["/dev/loop0p1"]
    finally:
        orange_image_mount._run = original_run
