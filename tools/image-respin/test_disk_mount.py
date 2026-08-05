from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

from disk_layout import DiskLayout, PartitionIdentity
from disk_mount import DiskMountError, MountedRuntime


class DiskMountTests(unittest.TestCase):
    def test_root_mount_uses_exact_runtime_safety_options(self) -> None:
        layout = DiskLayout("orange-pi-zero-2w", 1024, "dos", "disk-id", 1, 100, 512, (PartitionIdentity(1, "/dev/loop0p1", 8, 64, "83", "part", "ext4", "fs", None),), "a" * 64, None)
        commands: list[list[str]] = []
        def fake_run(command: list[str], *, capture: bool = False):
            commands.append(command)
            if command[0] == "losetup":
                return subprocess.CompletedProcess(command, 0, "/dev/loop0\n", "")
            return subprocess.CompletedProcess(command, 0, "", "")
        with patch("disk_mount._run", side_effect=fake_run), patch("disk_mount.capture_layout", return_value=layout):
            session = MountedRuntime(Path("image.img"), "orange-pi-zero-2w")
            session.open()
            session.close()
        self.assertIn(["mount", "-o", "rw,noatime,nodev,nosuid,noexec", "/dev/loop0p1", str(session.root_mount)], commands)

    def test_unmount_failure_retries_normally_and_never_fscks_or_detaches(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mount = Path(temporary) / "mount"
            mount.mkdir()
            session = MountedRuntime(Path(temporary) / "image.img", "orange-pi-zero-2w", "/dev/loop-test", mount, mounted=True, attached=True)
            commands: list[list[str]] = []
            def fail_unmount(command: list[str], *, capture: bool = False):
                commands.append(command)
                if command[0] == "sync":
                    return None
                if command[0] == "umount":
                    raise DiskMountError("injected unmount failure")
                raise AssertionError(f"unexpected command after unmount failure: {command}")
            with patch("disk_mount._run", side_effect=fail_unmount), self.assertRaises(DiskMountError):
                session.close()
            self.assertEqual([command[0] for command in commands], ["sync", "umount", "umount", "umount"])
            self.assertFalse(any(command[0] in {"e2fsck", "fsck.vfat", "losetup"} for command in commands))
            self.assertFalse(session._closed)
            self.assertTrue(session.retain_workspace)
            self.assertEqual(session.backing_path, Path(temporary))
            self.assertTrue(mount.exists())
            def retry_run(command: list[str], *, capture: bool = False):
                if command[0] == "sync":
                    return None
                if command[0] == "umount":
                    return None
                if command[0] == "losetup":
                    return None
                raise AssertionError(f"unexpected retry command: {command}")
            with patch("disk_mount._run", side_effect=retry_run):
                session.close()
            self.assertTrue(session._closed)

    def test_detach_failure_retains_backing_workspace_and_is_retryable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "image.img"
            image.write_bytes(b"image")
            mount = Path(temporary) / "mount"
            mount.mkdir()
            layout = DiskLayout("orange-pi-zero-2w", 1024, "dos", "disk-id", 1, 100, 512, (PartitionIdentity(1, "/dev/loop-testp1", 8, 64, "83", "part", "ext4", "fs", None),), "a" * 64, None)
            session = MountedRuntime(image, "orange-pi-zero-2w", "/dev/loop-test", mount, pre_layout=layout, mounted=True, attached=True)
            commands: list[list[str]] = []
            detached = False

            def fail_detach(command: list[str], *, capture: bool = False):
                nonlocal detached
                commands.append(command)
                if command[0] == "losetup" and command[1] == "-d" and not detached:
                    detached = True
                    raise DiskMountError("injected detach failure")
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch("disk_mount._run", side_effect=fail_detach), patch("disk_mount.capture_layout", return_value=layout), self.assertRaises(DiskMountError) as raised:
                session.close()
            self.assertTrue(session.retain_workspace)
            self.assertTrue(raised.exception.retain_workspace)
            self.assertEqual(raised.exception.backing_path, Path(temporary))
            self.assertFalse(session.mounted)
            self.assertTrue(session.attached)
            self.assertFalse(session._closed)
            self.assertTrue(image.exists())
            self.assertTrue(mount.exists())

            with patch("disk_mount._run", side_effect=fail_detach), patch("disk_mount.capture_layout", return_value=layout):
                session.close()
            self.assertFalse(session.retain_workspace)
            self.assertFalse(session.attached)
            self.assertTrue(session._closed)
            self.assertFalse(mount.exists())


if __name__ == "__main__":
    unittest.main()
