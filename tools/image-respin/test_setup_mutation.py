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


def _owner(path: Path, uid: int, gid: int) -> None:
    if os.name != "nt" and hasattr(os, "chown") and os.geteuid() == 0:
        os.chown(path, uid, gid, follow_symlinks=False)


def _parents(root: Path, contract: dict) -> None:
    directories = {item["target"] for item in contract["directories"]}
    for relative in [item["target"] for item in contract["entries"]] + [item["target"] for item in contract["symlinks"]] + [item["target"] for item in contract["preserved_paths"]]:
        current = root
        cumulative: list[str] = []
        for part in relative.split("/")[:-1]:
            cumulative.append(part)
            if "/".join(cumulative) in directories:
                continue
            current /= part
            current.mkdir(exist_ok=True)


def _setup_preimages(root: Path, contract: dict) -> None:
    for item in contract["directories"]:
        preimage = item["preimage"]
        if preimage["kind"] != "exact":
            continue
        path = root / item["target"]
        path.mkdir()
        os.chmod(path, preimage["mode"])
        _owner(path, preimage["uid"], preimage["gid"])
    for item in contract["entries"]:
        if item["preimage"]["kind"] != "exact":
            continue
        preimage = item["preimage"]
        _write(root / item["target"], _orange_preimage(item["source"]), preimage["mode"])
        _owner(root / item["target"], preimage["uid"], preimage["gid"])
    for item in contract["symlinks"]:
        preimage = item["preimage"]
        if item["classification"] == "stale-ui-root-asset" and preimage["kind"] == "exact":
            _write(root / item["target"], _orange_preimage(item["target"]), preimage["mode"])
            _owner(root / item["target"], preimage["uid"], preimage["gid"])


def _rpi_parent_sudoers_preimage(root: Path, contract: dict) -> None:
    item = next(item for item in contract["symlinks"] if item["classification"] == "parent-sudoers-removed")
    preimage = item["preimage"]
    _write(root / item["target"], b"pi ALL=(ALL) NOPASSWD: ALL\n", preimage["mode"])
    _owner(root / item["target"], preimage["uid"], preimage["gid"])


def _prerequisites(root: Path, board: str) -> None:
    packages = "\n\n".join(f"Package: {name}\nStatus: install ok installed\nVersion: 1.0" for name in ("openssh-server", "network-manager", "dnsmasq", "python3-minimal", "iw", "iproute2", "coreutils", "util-linux"))
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
    for command in ("usr/sbin/iw", "usr/bin/nmcli", "usr/sbin/ip", "usr/bin/timeout", "usr/bin/ss", "usr/bin/setsid"):
        _write(root / command, command.encode(), 0o755)
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
    if board == RPI:
        _rpi_parent_sudoers_preimage(root, contract)
    _write(root / "usr/share/doc/base-files/copyright", b"vendor copyright\n")
    _write(root / "usr/share/common-licenses/GPL-3", b"vendor GPL\n")
    if board == ORANGE:
        _setup_preimages(root, contract)
        disabled = next(item for item in contract["symlinks"] if item["classification"] == "setup-service-disabled")
        (root / disabled["target"]).symlink_to(disabled["preimage"]["link_target"])
        _write(root / "etc/ssh/sshd_config.d/10-octessera-setup.conf", b"PermitRootLogin no\nPasswordAuthentication no\nAllowUsers octessera\n", 0o664)
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
                directory = root / "usr/local/share/octessera-setup-ui"
                self.assertTrue(directory.is_dir())
                self.assertEqual((directory.stat().st_mode & 0o777, directory.stat().st_uid, directory.stat().st_gid), (0o755, 0, 0))
                self.assertIn("usr/local/share/octessera-setup-ui", result.changed_paths)
                self.assertIn("usr/local/share/octessera-setup-ui", proof["verified_paths"])
                self.assertEqual(result.provenance["setup_layer"]["contract_digest"], result.contract_digest)
                self.assertEqual(len(result.provenance["setup_layer"]["source_inputs"]), len(contract["source_inputs"]))
                self.assertRegex(result.provenance["finalizer"]["tool_code_digest"], r"^[0-9a-f]{64}$")
                self.assertIn("etc/octessera/setup-profile", result.changed_paths)
                if board == RPI:
                    self.assertIn("etc/sudoers.d/010_pi-nopasswd", result.changed_paths)
                    self.assertFalse((root / "etc/sudoers.d/010_pi-nopasswd").exists())
                    self.assertIn("etc/sudoers.d/010_pi-nopasswd", proof["verified_paths"])
                else:
                    self.assertNotIn("etc/sudoers.d/010_pi-nopasswd", result.changed_paths)
                enabled = root / "etc/systemd/system/multi-user.target.wants/octessera-setup-request.path"
                self.assertEqual(enabled.readlink().as_posix(), "../octessera-setup-request.path")
                service = root / "etc/systemd/system/multi-user.target.wants/octessera-setup.service"
                self.assertFalse(service.exists() or service.is_symlink())
                if board == ORANGE:
                    for item in contract["entries"]:
                        if item["target"].startswith("usr/local/share/octessera-setup-ui/"):
                            metadata = (root / item["target"]).stat()
                            self.assertEqual((metadata.st_mode & 0o777, metadata.st_uid, metadata.st_gid), (0o644, 0, 0))

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

    def test_setup_ui_directory_preimages_and_closed_tree_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            directory = root / "usr/local/share/octessera-setup-ui"
            directory.mkdir()
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, RPI, "a" * 40)
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(ORANGE, Path(temporary))
            directory = root / "usr/local/share/octessera-setup-ui"
            _owner(directory, 0, 0)
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, ORANGE, "a" * 40)
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(ORANGE, Path(temporary))
            _write(root / "usr/local/share/octessera-setup-ui/undeclared", b"unexpected")
            _owner(root / "usr/local/share/octessera-setup-ui/undeclared", 1001, 1001)
            before = inventory_digest(build_inventory(root))
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, ORANGE, "a" * 40)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            mutate_setup(root, RPI, "a" * 40)
            _write(root / "usr/local/share/octessera-setup-ui/undeclared", b"unexpected")
            with self.assertRaises(ValueError):
                prove_setup_root(root, RPI)
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            mutate_setup(root, RPI, "a" * 40)
            os.chmod(root / "usr/local/share/octessera-setup-ui/js", 0o700)
            with self.assertRaises(ValueError):
                prove_setup_root(root, RPI)

    def test_old_root_ui_assets_are_removed_during_mutation(self) -> None:
        stale = (
            "usr/local/share/octessera-setup-ui/app.js",
            "usr/local/share/octessera-setup-ui/styles.css",
            "usr/local/share/octessera-setup-ui/octessera-mark.svg",
            "usr/local/share/octessera-setup-ui/octessera-wordmark.svg",
        )
        for board in (RPI, ORANGE):
            with self.subTest(board=board), tempfile.TemporaryDirectory() as temporary:
                root = _fixture(board, Path(temporary))
                if board == RPI:
                    for path in stale:
                        _write(root / path, b"stale", 0o644)
                mutate_setup(root, board, "a" * 40)
                for path in stale:
                    self.assertFalse((root / path).exists())

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
            self.assertFalse((root / "usr/local/share/octessera-setup-ui").exists())
        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(ORANGE, Path(temporary))
            before = inventory_digest(build_inventory(root))

            def interrupted_orange(stage: str) -> None:
                if stage.startswith("disabled:"):
                    raise RuntimeError("interrupted")

            with self.assertRaises(Exception):
                mutate_setup(root, ORANGE, "a" * 40, mutation_hook=interrupted_orange)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
            self.assertTrue((root / "etc/systemd/system/multi-user.target.wants/octessera-setup.service").is_symlink())
            directory = root / "usr/local/share/octessera-setup-ui"
            self.assertEqual((directory.stat().st_uid, directory.stat().st_gid), (1001, 1001))
            for item in load_contract(contract_for_board(ORANGE))[0]["entries"]:
                if item["target"].startswith("usr/local/share/octessera-setup-ui/"):
                    metadata = (root / item["target"]).stat()
                    self.assertEqual((metadata.st_mode & 0o777, metadata.st_uid, metadata.st_gid), (0o644, 1001, 1001))

        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            sudoers = root / "etc/sudoers.d/010_pi-nopasswd"
            sudoers.write_bytes(b"pi ALL=(ALL) NOPASSWD: /bin/true\n")
            before = inventory_digest(build_inventory(root))
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, RPI, "a" * 40)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
            self.assertTrue(sudoers.is_file())

        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            sudoers = root / "etc/sudoers.d/010_pi-nopasswd"
            sudoers.unlink()
            sudoers.symlink_to("/etc/sudoers.d/other")
            before = inventory_digest(build_inventory(root))
            with self.assertRaises(ConstructorRequired):
                mutate_setup(root, RPI, "a" * 40)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
            self.assertTrue(sudoers.is_symlink())

        with tempfile.TemporaryDirectory() as temporary:
            root = _fixture(RPI, Path(temporary))
            before = inventory_digest(build_inventory(root))

            def interrupted_sudoers(stage: str) -> None:
                if stage == "disabled:etc/sudoers.d/010_pi-nopasswd":
                    raise RuntimeError("interrupted")

            with self.assertRaises(Exception):
                mutate_setup(root, RPI, "a" * 40, mutation_hook=interrupted_sudoers)
            self.assertEqual(before, inventory_digest(build_inventory(root)))
            self.assertEqual((root / "etc/sudoers.d/010_pi-nopasswd").read_bytes(), b"pi ALL=(ALL) NOPASSWD: ALL\n")

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
