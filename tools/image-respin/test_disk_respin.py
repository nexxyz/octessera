from __future__ import annotations

import hashlib
import copy
import json
import lzma
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from typing import Any
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import disk_mount
import disk_respin
from disk_packaging import file_digest
from boot_neutral import load_policy
from test_runtime_mutation import ORANGE, RPI, _fixture


TOOLS = ("sfdisk", "losetup", "mount", "umount", "mkfs.ext4", "mkfs.vfat", "e2fsck", "fsck.vfat", "blkid", "lsblk", "udevadm")
ROOT = Path(__file__).resolve().parents[2]


def _context(board: str, source: Path) -> dict:
    digest, size = file_digest(source)
    suffix = "img.zip" if board == RPI else "img.xz"
    return {"schema": "octessera.image-parent-trust/v1", "repository": "nexxyz/octessera", "tag": "v0.7.5", "source_commit": "4eec2b7edf6619fa22c709d4a589237a5748de78", "asset": {"name": f"octessera-0.7.5-{board}.{suffix}", "node_id": "RA_synthetic", "size": size, "sha256": digest}}


def _orange_policy(context: dict) -> Any:
    policy = load_policy(ROOT)
    contract = copy.deepcopy(policy.contract)
    contract["parent_asset"].update({key: context["asset"][key] for key in ("name", "size", "sha256")})
    return type(policy)(policy.root, policy.path, contract, policy.sha256)


def _run(command: list[str], *, input_text: str | None = None) -> str:
    return subprocess.run(command, input=input_text, text=True, capture_output=True, check=True).stdout.strip()


def _resource_sets() -> tuple[str, frozenset[str]]:
    loops = subprocess.run(["losetup", "--list", "--noheadings", "--output", "NAME,BACK-FILE"], capture_output=True, text=True, check=True).stdout
    mounts = frozenset(Path("/proc/self/mountinfo").read_text(encoding="utf-8").splitlines())
    return loops, mounts


def _make_partitioned_image(work: Path, board: str) -> Path:
    image = work / f"parent-{board}.img"
    image.write_bytes(b"\0" * (96 * 1024 * 1024))
    if board == ORANGE:
        script = "label: dos\nunit: sectors\nstart=2048,type=83\n"
    else:
        script = "label: dos\nunit: sectors\nstart=2048,size=16384,type=c\nstart=18432,type=83\n"
    _run(["sfdisk", str(image)], input_text=script)
    loop = _run(["losetup", "--find", "--show", "--partscan", str(image)])
    try:
        _run(["udevadm", "settle"])
        if board == ORANGE:
            _run(["mkfs.ext4", "-F", f"{loop}p1"])
            root_device = f"{loop}p1"
        else:
            _run(["mkfs.vfat", "-F", "32", f"{loop}p1"])
            _run(["mkfs.ext4", "-F", f"{loop}p2"])
            root_device = f"{loop}p2"
        mount = Path(tempfile.mkdtemp(prefix="octessera-disk-fixture-mount-"))
        try:
            _run(["mount", "-o", "rw,noatime", root_device, str(mount)])
            source_root, _ = _fixture(work / f"fixture-{board}", board)
            shutil.copytree(source_root, mount, symlinks=True, dirs_exist_ok=True)
            if board == ORANGE:
                (mount / "boot").mkdir(exist_ok=True)
        finally:
            subprocess.run(["sync"], check=True)
            subprocess.run(["umount", str(mount)], check=True)
            mount.rmdir()
    finally:
        subprocess.run(["losetup", "-d", loop], check=True)
    return image


@unittest.skipUnless(platform.system() == "Linux" and getattr(os, "geteuid", lambda: -1)() == 0, "disk fixtures require Linux root")
class DiskRespinTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        missing = [tool for tool in TOOLS if shutil.which(tool) is None]
        if missing:
            raise AssertionError(f"missing disk tools: {', '.join(missing)}")
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "preflight.img"
            image.write_bytes(b"\0" * (2 * 1024 * 1024))
            loop = None
            try:
                loop = _run(["losetup", "--find", "--show", str(image)])
            except subprocess.CalledProcessError as exc:
                raise AssertionError(f"loop privilege preflight unavailable: {exc}") from exc
            finally:
                if loop:
                    subprocess.run(["losetup", "-d", loop], check=True)
            source_dir = Path(temporary) / "bind-source"
            mount_dir = Path(temporary) / "bind-mount"
            source_dir.mkdir()
            mount_dir.mkdir()
            mounted = False
            try:
                subprocess.run(["mount", "--bind", str(source_dir), str(mount_dir)], check=True, capture_output=True, text=True)
                mounted = True
            except subprocess.CalledProcessError as exc:
                raise AssertionError(f"mount privilege preflight unavailable: {exc}") from exc
            finally:
                if mounted:
                    subprocess.run(["umount", str(mount_dir)], check=True)

    def _run_respin(self, board: str, version: str, prior: str, work: Path) -> tuple[Path, Path, Path]:
        before_resources = _resource_sets()
        image = _make_partitioned_image(work, board)
        source = work / (f"octessera-0.7.5-{board}.img.xz" if board == ORANGE else f"octessera-0.7.5-{board}.img.zip")
        if board == ORANGE:
            with lzma.open(source, "wb") as stream:
                stream.write(image.read_bytes())
        else:
            with zipfile.ZipFile(source, "w") as archive:
                archive.writestr("parent.img", image.read_bytes())
                archive.writestr("os_list.rpi-imager-manifest", b"synthetic-imager-manifest\n")
        context = _context(board, source)
        before_source = hashlib.sha256(source.read_bytes()).hexdigest()
        _, runtime_bundle = _fixture(work / f"runtime-{board}", board, prior=prior)
        output = work / "output" / (f"octessera-{version}-{board}-derived-runtime-respin" + (".img.xz" if board == ORANGE else ".zip"))
        prepared_images: list[Path] = []
        losetup_images: list[str] = []
        real_prepare = disk_respin.prepare_parent_image
        real_mount_run = disk_mount._run

        def observe_prepare(*args, **kwargs):
            prepared = real_prepare(*args, **kwargs)
            prepared_images.append(prepared.image)
            return prepared

        def observe_mount_run(command: list[str], *, capture: bool = False):
            if command[:3] == ["losetup", "--find", "--show"]:
                losetup_images.append(command[-1])
            return real_mount_run(command, capture=capture)

        policy_patch = patch.object(disk_respin, "load_policy", return_value=_orange_policy(context)) if board == ORANGE else patch.object(disk_respin, "load_policy", side_effect=AssertionError("Raspberry must not load Orange policy"))
        with policy_patch, patch.object(disk_respin, "verify_parent_asset", return_value=(source, context, hashlib.sha256((ROOT / "resources/image-parents/v0.7.5-trust-manifest.json").read_bytes()).hexdigest(), b"synthetic-imager-manifest\n" if board == RPI else None)):
            with patch.object(disk_respin, "prepare_parent_image", side_effect=observe_prepare), patch.object(disk_mount, "_run", side_effect=observe_mount_run):
                result = disk_respin.respin_image(board_profile=board, assets_directory=work, manifest_path=ROOT / "resources/image-parents/v0.7.5-trust-manifest.json", runtime_bundle=runtime_bundle, version=version, source_identity="synthetic-source", output=output, boot_neutral_contract=ROOT / "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json" if board == ORANGE else None)
        self.assertTrue(result.output.is_file())
        self.assertTrue(result.provenance_output.is_file())
        provenance = json.loads(result.provenance_output.read_text(encoding="utf-8"))
        self.assertEqual(provenance["parent"]["context"], context)
        self.assertEqual(provenance["packaged_artifact"]["sha256"], file_digest(output)[0])
        self.assertNotIn("/dev/loop", json.dumps(provenance, sort_keys=True))
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), before_source)
        self.assertEqual(subprocess.run(["losetup", "--associated", str(image)], capture_output=True, text=True, check=True).stdout, "")
        self.assertEqual(len(prepared_images), 1)
        self.assertEqual(losetup_images, [str(prepared_images[0])])
        self.assertNotIn(str(source), losetup_images)
        self.assertFalse(prepared_images[0].exists())
        self.assertEqual(_resource_sets(), before_resources)
        return source, output, result.provenance_output

    def test_actual_orange_new_and_raspberry_same_version_respin(self) -> None:
        for board, version, prior in ((ORANGE, "2.0.0", "1.0.0"), (RPI, "2.0.0", "2.0.0")):
            with self.subTest(board=board), tempfile.TemporaryDirectory() as temporary:
                source, output, sidecar = self._run_respin(board, version, prior, Path(temporary))
                self.assertIn("derived", output.name)
                self.assertIn(board, sidecar.name)
                if board == RPI:
                    with zipfile.ZipFile(output) as archive:
                        self.assertEqual(len([name for name in archive.namelist() if name.endswith(".img")]), 1)

    def test_injected_failure_preserves_source_and_removes_derived_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            before_resources = _resource_sets()
            board = ORANGE
            image = _make_partitioned_image(work, board)
            source = work / "octessera-0.7.5-orange-pi-zero-2w.img.xz"
            with lzma.open(source, "wb") as stream:
                stream.write(image.read_bytes())
            context = _context(board, source)
            _, runtime_bundle = _fixture(work / "runtime", board)
            output = work / "output" / "octessera-2.0.0-orange-pi-zero-2w-derived-runtime-respin.img.xz"
            before = hashlib.sha256(source.read_bytes()).hexdigest()
            prepared_images: list[Path] = []
            real_prepare = disk_respin.prepare_parent_image

            def observe_prepare(*args, **kwargs):
                prepared = real_prepare(*args, **kwargs)
                prepared_images.append(prepared.image)
                return prepared

            reached: list[str] = []

            def fail(stage: str) -> None:
                reached.append(stage)
                if stage == "current-replaced":
                    raise RuntimeError("injected disk respin failure")
            with patch.object(disk_respin, "load_policy", return_value=_orange_policy(context)), patch.object(disk_respin, "verify_parent_asset", return_value=(source, context, hashlib.sha256((ROOT / "resources/image-parents/v0.7.5-trust-manifest.json").read_bytes()).hexdigest(), None)), patch.object(disk_respin, "prepare_parent_image", side_effect=observe_prepare), self.assertRaises(disk_respin.DiskRespinError):
                disk_respin.respin_image(board_profile=board, assets_directory=work, manifest_path=ROOT / "resources/image-parents/v0.7.5-trust-manifest.json", runtime_bundle=runtime_bundle, version="2.0.0", source_identity="synthetic-source", output=output, boot_neutral_contract=ROOT / "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json", mutation_hook=fail)
            self.assertIn("current-replaced", reached)
            self.assertFalse(output.exists())
            self.assertFalse(disk_respin.provenance_sidecar(output).exists())
            self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), before)
            self.assertEqual(subprocess.run(["losetup", "--associated", str(image)], capture_output=True, text=True, check=True).stdout, "")
            self.assertEqual(len(prepared_images), 1)
            self.assertFalse(prepared_images[0].exists())
            self.assertEqual(_resource_sets(), before_resources)

    def test_detach_failure_retains_actual_private_workspace_until_operator_cleanup(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            before_resources = _resource_sets()
            board = ORANGE
            image = _make_partitioned_image(work, board)
            source = work / "octessera-0.7.5-orange-pi-zero-2w.img.xz"
            with lzma.open(source, "wb") as stream:
                stream.write(image.read_bytes())
            context = _context(board, source)
            _, runtime_bundle = _fixture(work / "runtime", board)
            output = work / "output" / "octessera-2.0.0-orange-pi-zero-2w-derived-runtime-respin.img.xz"
            prepared_images: list[Path] = []
            attach_paths: list[str] = []
            real_prepare = disk_respin.prepare_parent_image
            real_mount_run = disk_mount._run
            failed = False

            def observe_prepare(*args, **kwargs):
                prepared = real_prepare(*args, **kwargs)
                prepared_images.append(prepared.image)
                return prepared

            def fail_detach(command: list[str], *, capture: bool = False):
                nonlocal failed
                if command[:3] == ["losetup", "--find", "--show"]:
                    attach_paths.append(command[-1])
                if command[:2] == ["losetup", "-d"] and not failed:
                    failed = True
                    raise disk_mount.DiskMountError("injected detach failure")
                return real_mount_run(command, capture=capture)

            with patch.object(disk_respin, "verify_parent_asset", return_value=(source, context, "c" * 64, None)), patch.object(disk_respin, "prepare_parent_image", side_effect=observe_prepare), patch.object(disk_mount, "_run", side_effect=fail_detach), self.assertRaises(disk_respin.DiskRespinError) as raised:
                disk_respin.respin_image(board_profile=board, assets_directory=work, manifest_path=work / "manifest.json", runtime_bundle=runtime_bundle, version="2.0.0", source_identity="synthetic-source", output=output)
            self.assertTrue(failed)
            self.assertEqual(len(prepared_images), 1)
            self.assertEqual(attach_paths, [str(prepared_images[0])])
            self.assertNotIn(str(source), attach_paths)
            self.assertTrue(prepared_images[0].exists())
            self.assertIn(str(prepared_images[0].parent), str(raised.exception))
            self.assertNotEqual(_resource_sets(), before_resources)

            associated = subprocess.run(["losetup", "--associated", str(prepared_images[0])], capture_output=True, text=True, check=True).stdout.strip()
            self.assertTrue(associated)
            subprocess.run(["losetup", "-d", associated.splitlines()[0].split(":", 1)[0]], check=True)
            root_mount = Path(str(raised.exception).rsplit("; root mount retained at ", 1)[1].split("; private workspace retained at ", 1)[0])
            root_mount.rmdir()
            shutil.rmtree(prepared_images[0].parent)
            self.assertEqual(_resource_sets(), before_resources)


if __name__ == "__main__":
    unittest.main()
