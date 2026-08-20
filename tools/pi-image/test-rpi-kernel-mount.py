#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import tempfile
from pathlib import Path
from types import SimpleNamespace
from typing import Any


HERE = Path(__file__).resolve().parent


def _load(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PROOF = _load(HERE / "verify-rpi-kernel-image.py", "rpi_kernel_mount_test_proof")


def main() -> int:
    original_run = PROOF.subprocess.run

    def fake_lsblk(command: list[str], **_: Any) -> Any:
        if command[0] != "lsblk":
            return original_run(command, **_)
        assert command[command.index("--output") + 1] == "NAME,TYPE"
        return SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": [{"name": "/dev/loop0p2", "type": "part"}, {"name": "/dev/loop0p1", "type": "part"}]}]}))

    PROOF.subprocess.run = fake_lsblk
    try:
        assert PROOF._expected_partitions("/dev/loop0") == ("/dev/loop0p1", "/dev/loop0p2")
    finally:
        PROOF.subprocess.run = original_run

    for children in (
        [{"name": "/dev/loop0p3", "type": "part"}, {"name": "/dev/loop0p2", "type": "part"}],
        [{"name": "/dev/loop0p1", "type": "part"}, {"name": "/dev/loop0p2", "type": "part"}, {"name": "/dev/loop0p3", "type": "part"}],
    ):
        PROOF.subprocess.run = lambda command, **_: SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": children}]}))
        try:
            try:
                PROOF._expected_partitions("/dev/loop0")
            except PROOF.ImageProofError:
                pass
            else:
                raise AssertionError("invalid fixed partition layout was accepted")
        finally:
            PROOF.subprocess.run = original_run

    with tempfile.TemporaryDirectory(prefix="octessera-rpi-mount-test-") as temporary:
        image_path = Path(temporary) / "image.img"
        image_path.write_bytes(b"image")
        mount_commands: list[list[str]] = []

        def fake_mount_run(command: list[str], **_: Any) -> Any:
            mount_commands.append(command)
            if command[0] == "losetup" and "--show" in command:
                return SimpleNamespace(stdout="/dev/loop0\n")
            if command[0] == "lsblk":
                partitions = [{"name": "/dev/loop0p1", "type": "part"}, {"name": "/dev/loop0p2", "type": "part"}]
                return SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": partitions}]}))
            return SimpleNamespace(stdout="")

        PROOF.subprocess.run = fake_mount_run
        try:
            with PROOF._mounted_image(image_path):
                pass
        finally:
            PROOF.subprocess.run = original_run
        assert any(command[:5] == ["mount", "-t", "vfat", "-o", "ro"] for command in mount_commands)
        assert any(command[:5] == ["mount", "-t", "ext4", "-o", "ro,noload"] for command in mount_commands)
        assert any(command[:2] == ["umount", "-l"] for command in mount_commands)
        assert any(command[:2] == ["losetup", "-d"] for command in mount_commands)
    print("Raspberry kernel image mount and cleanup tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
