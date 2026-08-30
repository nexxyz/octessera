from __future__ import annotations

import lzma
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import disk_mount
import disk_setup_respin
from disk_packaging import file_digest
from test_disk_respin import ORANGE, _context, _orange_policy, _parent_binding, _run, _resource_sets
from test_runtime_mutation import _bundle, _fixture
from test_setup_mutation import _parents, _prerequisites, _setup_preimages
from setup_contract import contract_for_board, load_contract


TOOLS = ("sfdisk", "losetup", "mount", "umount", "mkfs.ext4", "mkfs.vfat", "e2fsck", "fsck.vfat", "blkid", "lsblk", "udevadm")


def _image(work: Path, board: str, root: Path) -> Path:
    image = work / f"parent-{board}.img"
    image.write_bytes(b"\0" * (96 * 1024 * 1024))
    layout = "label: dos\nunit: sectors\nstart=2048,type=83\n" if board == ORANGE else "label: dos\nunit: sectors\nstart=2048,size=16384,type=c\nstart=18432,type=83\n"
    _run(["sfdisk", str(image)], input_text=layout)
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
        mounted = Path(tempfile.mkdtemp(prefix="octessera-setup-disk-fixture-"))
        try:
            _run(["mount", "-o", "rw,noatime", root_device, str(mounted)])
            shutil.copytree(root, mounted, symlinks=True, dirs_exist_ok=True)
            chown = getattr(os, "chown", None)
            if chown is not None and getattr(os, "geteuid", lambda: -1)() == 0:
                for source_path in (root, *root.rglob("*")):
                    destination = mounted / source_path.relative_to(root)
                    metadata = source_path.lstat()
                    chown(destination, metadata.st_uid, metadata.st_gid, follow_symlinks=False)
            if board == ORANGE:
                (mounted / "boot").mkdir(exist_ok=True)
            _run(["sync"])
            _run(["umount", str(mounted)])
        finally:
            mounted.rmdir()
    finally:
        _run(["losetup", "-d", loop])
    return image


@unittest.skipUnless(platform.system() == "Linux" and getattr(os, "geteuid", lambda: -1)() == 0, "disk setup fixtures require Linux root")
class DiskSetupRespinTests(unittest.TestCase):
    def test_ext4_and_vfat_setup_respin_uses_the_separate_layer_and_preserves_source(self) -> None:
        missing = [tool for tool in TOOLS if shutil.which(tool) is None]
        if missing:
            self.skipTest(f"missing disk tools: {', '.join(missing)}")
        board = ORANGE
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root, bundle = _fixture(work / "runtime", board)
            contract, _ = load_contract(contract_for_board(board))
            _parents(root, contract)
            _prerequisites(root, board)
            _setup_preimages(root, contract)
            disabled = next(item for item in contract["symlinks"] if item["classification"] == "setup-service-disabled")
            (root / disabled["target"]).symlink_to(disabled["preimage"]["link_target"])
            path = root / "etc/ssh/sshd_config.d/10-octessera-setup.conf"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers octessera\n")
            os.chmod(path, 0o664)
            image = _image(work, board, root)
            source = work / f"octessera-0.8.1-{board}.img.xz"
            with lzma.open(source, "wb") as stream:
                stream.write(image.read_bytes())
            context = _context(board, source)
            output = work / "out" / f"octessera-2.0.0-{board}-derived-setup-respin.img.xz"
            proof = work / "out/setup-layer-proof.json"
            source_before = file_digest(source)
            with patch.object(disk_setup_respin, "load_policy", return_value=_orange_policy(context)), patch.object(disk_setup_respin, "verify_current_parent_asset", return_value=(source, context, context["record"]["sha256"], None)), patch.object(disk_setup_respin, "parent_binding", return_value=_parent_binding(context)):
                result = disk_setup_respin.respin_setup_image(board_profile=board, assets_directory=work, parent_record_path=Path(__file__).resolve().parents[2] / "resources/image-parents/orange-pi-zero-2w-current.json", runtime_bundle=bundle, version="2.0.0", source_identity="synthetic-source", output=output, proof_output=proof, boot_neutral_contract=Path(__file__).resolve().parents[2] / "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json")
            self.assertTrue(output.is_file())
            self.assertTrue(proof.is_file())
            self.assertEqual(result["setup_proof"]["proof"], "setup-layer-mounted")
            runtime_provenance = result["runtime_mutation"]["provenance"]
            self.assertIn("notice", runtime_provenance)
            self.assertEqual(set(runtime_provenance["notice"]["changed_paths"]), {path for path in runtime_provenance["changed_paths"] if path == "usr/share/doc/octessera" or path.startswith("usr/share/doc/octessera/")})
            self.assertNotIn("notice", result["setup_mutation"])
            self.assertEqual(file_digest(source), source_before)
            self.assertEqual(subprocess.run(["losetup", "--associated", str(image)], capture_output=True, text=True, check=True).stdout, "")


if __name__ == "__main__":
    unittest.main()
