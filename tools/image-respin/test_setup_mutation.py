from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from inventory import build_inventory, inventory_digest
from boot_neutral import load_policy
from setup_contract import contract_for_board, load_contract
from setup_mutation import ConstructorRequired, mutate_setup
from setup_proof import prove_setup_root


ROOT = Path(__file__).resolve().parents[2]
RPI = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
PINNED = "4eec2b7edf6619fa22c709d4a589237a5748de78"


def _write(path: Path, value: bytes | str, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())
    os.chmod(path, mode)


def _parents(root: Path, contract: dict) -> None:
    for relative in [item["target"] for item in contract["entries"]] + [item["target"] for item in contract["symlinks"]] + [item["target"] for item in contract["preserved_paths"]]:
        current = root
        for part in relative.split("/")[:-1]:
            current /= part
            current.mkdir(exist_ok=True)


def _prerequisites(root: Path, board: str) -> None:
    packages = "\n\n".join(f"Package: {name}\nStatus: install ok installed\nVersion: 1.0" for name in ("openssh-server", "network-manager", "dnsmasq", "python3-minimal"))
    _write(root / "var/lib/dpkg/status", packages + "\n")
    if board == RPI:
        passwd = "root:x:0:0:root:/root:/bin/bash\npi:x:1000:1000:Pi:/home/pi:/bin/bash\n"
        group = "root:x:0:\npi:x:1000:\n"
    else:
        passwd = "root:x:0:0:root:/root:/bin/bash\noctessera:x:1000:1000:Octessera:/home/octessera:/bin/bash\noctessera-runtime:x:995:995:Runtime:/nonexistent:/usr/sbin/nologin\n"
        group = "root:x:0:\noctessera:x:1000:\noctessera-runtime:x:995:\n"
    _write(root / "etc/passwd", passwd)
    _write(root / "etc/group", group)
    _write(root / "usr/local/bin/wifi-connect", b"wifi-connect", 0o755)
    _write(root / "usr/bin/python3", b"python3", 0o755)
    for service in ("ssh.service", "NetworkManager.service", "dnsmasq.service"):
        _write(root / "etc/systemd/system" / service, "[Unit]\n")


def _orange_preimage(path: str) -> bytes:
    return subprocess.check_output(["git", "-c", f"safe.directory={ROOT.as_posix()}", "show", f"{PINNED}:userpatches/overlay/{path}"], cwd=ROOT)


def _fixture(board: str, work: Path) -> Path:
    root = work / board
    root.mkdir()
    contract, _ = load_contract(contract_for_board(board))
    _parents(root, contract)
    _prerequisites(root, board)
    if board == ORANGE:
        for item in contract["entries"]:
            if item["preimage"]["kind"] == "exact":
                _write(root / item["target"], _orange_preimage(item["source"]), item["mode"])
        enabled = next(item for item in contract["symlinks"] if item["classification"] == "first-boot-setup-enabled")
        (root / enabled["target"]).symlink_to(enabled["link_target"])
        _write(root / "etc/ssh/sshd_config.d/10-octessera-setup.conf", b"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers octessera\n")
        policy = load_policy(ROOT)
        _write(root / "boot/Image", b"kernel")
        _write(root / "boot/uInitrd", b"initramfs")
        _write(root / "boot/armbianEnv.txt", "verbosity=1\nfdtfile=sun50i-h618-orangepi-zero2w.dtb\n")
        _write(root / f"boot/initrd.img-{policy.contract['selected_boot']['kernel_release']}", b"initramfs")
        _write(root / f"boot/config-{policy.contract['selected_boot']['kernel_release']}", b"config")
        _write(root / f"usr/lib/linux-image-{policy.contract['selected_boot']['kernel_release']}/Image", b"kernel")
        _write(root / f"usr/lib/linux-image-{policy.contract['selected_boot']['kernel_release']}/allwinner/{policy.contract['selected_boot']['dtb_name']}", b"dtb")
        _write(root / f"usr/lib/modules/{policy.contract['selected_boot']['kernel_release']}/modules.dep", b"modules")
        _write(root / "usr/lib/systemd/system-sleep/octessera-orange-oled", b"sleep-hook")
        (root / "lib").symlink_to("usr/lib")
        for relative in policy.contract["protected_paths"]:
            path = root / relative
            if path.exists() or path.is_symlink():
                continue
            _write(path, b"protected")
        link = root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service"
        if link.exists() or link.is_symlink():
            link.unlink()
        link.symlink_to("../octessera-orange-boot-splash.service")
    return root


@unittest.skipUnless(getattr(os, "geteuid", lambda: -1)() == 0, "setup mutation fixtures require root")
class SetupMutationTests(unittest.TestCase):
    def test_both_boards_install_exact_layer_and_prove_the_mounted_result(self) -> None:
        for board in (RPI, ORANGE):
            with self.subTest(board=board), tempfile.TemporaryDirectory() as temporary:
                root = _fixture(board, Path(temporary))
                marker = root / "var/lib/octessera/setup-complete"
                _write(marker, b"stale\n")
                before = inventory_digest(build_inventory(root))
                result = mutate_setup(root, board, "a" * 40)
                self.assertEqual(result.pre_inventory_digest, before)
                self.assertFalse(marker.exists())
                proof = prove_setup_root(root, board)
                self.assertEqual(proof["contract_sha256"], result.contract_digest)
                contract, _ = load_contract(contract_for_board(board))
                self.assertEqual(result.provenance["setup_layer"]["contract_digest"], result.contract_digest)
                self.assertEqual(len(result.provenance["setup_layer"]["source_inputs"]), len(contract["source_inputs"]))
                self.assertRegex(result.provenance["finalizer"]["tool_code_digest"], r"^[0-9a-f]{64}$")
                self.assertIn("etc/octessera/setup-profile", result.changed_paths)
                enabled = root / "etc/systemd/system/multi-user.target.wants/octessera-setup-request.path"
                self.assertEqual(enabled.readlink().as_posix(), "../octessera-setup-request.path")
                service = root / "etc/systemd/system/multi-user.target.wants/octessera-setup.service"
                self.assertEqual(service.is_symlink(), board == ORANGE)

    def test_orange_requires_exact_pinned_preimages_and_raspberry_requires_absence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(ORANGE, Path(temporary))
            _write(root / "usr/local/sbin/octessera-setup-sidecar", b"not-the-pinned-preimage", 0o755)
            before = inventory_digest(build_inventory(root))
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, ORANGE, "a" * 40)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            _write(root / "usr/local/sbin/octessera-setup-sidecar", b"unexpected", 0o755)
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, RPI, "a" * 40)

    def test_symlink_escape_owner_mode_and_unauthorized_diff_fail_closed_with_rollback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root = _fixture(RPI, work)
            outside = work / "outside"
            outside.mkdir()
            shutil.rmtree(root / "usr/local/sbin")
            (root / "usr/local/sbin").symlink_to(outside)
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, RPI, "a" * 40)
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            before = inventory_digest(build_inventory(root))
            def unauthorized(stage: str) -> None:
                if stage == "validated":
                    _write(root / "etc/octessera/unauthorized", b"no")
            with self.assertRaises(Exception):
                mutate_setup(root, RPI, "a" * 40, mutation_hook=unauthorized)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(ORANGE, Path(temporary))
            os.chmod(root / "usr/local/sbin/octessera-setup-sidecar", 0o444)
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, ORANGE, "a" * 40)
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(ORANGE, Path(temporary))
            setter = getattr(os, "setxattr", None)
            if setter is not None:
                try:
                    setter(root / "usr/local/sbin/octessera-setup-sidecar", "user.setup-test", b"unexpected")
                except OSError:
                    pass
                else:
                    with self.assertRaises(ConstructorRequired):
                        mutate_setup(root, ORANGE, "a" * 40)
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            before = inventory_digest(build_inventory(root))
            def interrupted(stage: str) -> None:
                if stage.startswith("installed:"):
                    raise RuntimeError("interrupted")
            with self.assertRaises(Exception):
                mutate_setup(root, RPI, "a" * 40, mutation_hook=interrupted)
            self.assertEqual(before, inventory_digest(build_inventory(root)))

    def test_prerequisite_account_and_package_removal_is_constructor_required(self) -> None:
        for board in (RPI, ORANGE):
            with self.subTest(board=board), tempfile.TemporaryDirectory() as temporary:
                root = _fixture(board, Path(temporary))
                status = root / "var/lib/dpkg/status"
                status.write_text(status.read_text().replace("Package: dnsmasq", "Package: missing"), encoding="utf-8")
                with self.assertRaises(ConstructorRequired):
                    mutate_setup(root, board, "a" * 40)


if __name__ == "__main__":
    unittest.main()
