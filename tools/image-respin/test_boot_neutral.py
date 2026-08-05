from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from boot_neutral import BootNeutralError, assert_unchanged, build_integrity, capture_state, load_policy, parent_binding
from disk_layout import DiskLayout, PartitionIdentity
from trust_manifest import load_manifest, parent_context_for_board


ROOT = Path(__file__).resolve().parents[2]
BOARD = "orange-pi-zero-2w"
RELEASE = "6.18.38-current-sunxi64"
POLICY = load_policy(ROOT)


def write(path: Path, value: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())


def make_root(root: Path) -> None:
    write(root / "boot/Image", b"kernel")
    write(root / "boot/uInitrd", b"initramfs")
    write(root / "boot/armbianEnv.txt", "verbosity=1\nfdtfile=sun50i-h618-orangepi-zero2w.dtb\n")
    write(root / f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb", b"dtb")
    write(root / f"boot/initrd.img-{RELEASE}", b"initramfs")
    write(root / f"boot/config-{RELEASE}", b"config")
    write(root / f"usr/lib/modules/{RELEASE}/modules.dep", b"modules")
    write(root / "etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash", b"initramfs-hook")
    write(root / "etc/udev/rules.d/70-octessera-orange-runtime.rules", b"udev-rule")
    write(root / "usr/lib/systemd/system-sleep/octessera-orange-oled", b"sleep-hook")
    (root / "lib").symlink_to("usr/lib")
    for relative in POLICY.contract["protected_paths"]:
        if relative == "etc/systemd/system/multi-user.target.wants/octessera.service":
            continue
        path = root / relative
        if path.exists() or path.is_symlink():
            continue
        if relative.endswith(".service"):
            write(path, "[Service]\n")
        elif relative.endswith(".svg"):
            write(path, "<svg/>\n")
        else:
            write(path, b"protected")
    link = root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service"
    if link.exists() or link.is_symlink():
        link.unlink()
    link.symlink_to("../octessera-orange-boot-splash.service")
    runtime_link = root / "etc/systemd/system/multi-user.target.wants/octessera.service"
    runtime_link.parent.mkdir(parents=True, exist_ok=True)
    runtime_link.symlink_to("../octessera.service")


def layout(image_size: int = 1024) -> DiskLayout:
    return DiskLayout(BOARD, image_size, "gpt", "disk", 0, image_size // 512, 512, (PartitionIdentity(1, "", 2048, 100, "83", None, "ext4", None, None),), "a" * 64, None)


class BootNeutralTests(unittest.TestCase):
    def test_policy_and_capture_bind_selected_boot_and_metadata_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            make_root(root)
            before = capture_state(POLICY, root, layout())
            after = capture_state(POLICY, root, layout())
            assert_unchanged(POLICY, before, after, layout())
            integrity = build_integrity(POLICY, before, after, layout())
            self.assertEqual(before["protected_inventory"]["lib"]["type"], "symlink")
            self.assertEqual(before["protected_inventory"]["lib"]["target"], "usr/lib")
            self.assertIn(f"usr/lib/modules/{RELEASE}/modules.dep", POLICY.contract["protected_paths"])
            self.assertIn("usr/lib/systemd/system-sleep/octessera-orange-oled", POLICY.contract["protected_paths"])
            self.assertEqual(integrity["pre"], integrity["post"])
            self.assertEqual(integrity["protected_paths"], POLICY.contract["protected_paths"])
            self.assertEqual(integrity["changed_paths"], [])
            self.assertEqual(integrity["selected_kernel"], "boot/Image")
            self.assertEqual(integrity["selected_initramfs"], "boot/uInitrd")
            self.assertEqual(integrity["selected_dtb"], f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb")

    def test_protected_file_selector_symlink_and_disk_mutations_abort(self) -> None:
        mutations = (
            ("protected-file", lambda root: (root / "boot/Image").write_bytes(b"tampered")),
            ("boot-addition", lambda root: write(root / "boot/unexpected.bin", b"unknown")),
            ("selector", lambda root: (root / "boot/armbianEnv.txt").write_text("verbosity=9\n", encoding="utf-8")),
            ("module", lambda root: (root / f"usr/lib/modules/{RELEASE}/modules.dep").write_bytes(b"tampered")),
            ("initramfs-hook", lambda root: (root / "etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash").write_bytes(b"tampered")),
            ("system-sleep", lambda root: (root / "usr/lib/systemd/system-sleep/octessera-orange-oled").write_bytes(b"tampered")),
            ("lib-symlink", lambda root: ((root / "lib").unlink(), (root / "lib").symlink_to("usr"))),
            ("udev", lambda root: (root / "etc/udev/rules.d/70-octessera-orange-runtime.rules").write_bytes(b"tampered")),
            ("expected-handoff", lambda root: write(root / "usr/local/sbin/octessera-orange-oled-handoff.py", b"unexpected")),
            ("symlink", lambda root: ((root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service").unlink(), (root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service").symlink_to("../octessera.service"))),
            ("mode", lambda root: None),
        )
        for label, mutate in mutations:
            with self.subTest(mutate=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                make_root(root)
                before = capture_state(POLICY, root, layout())
                mutate(root)
                if label == "expected-handoff":
                    with self.assertRaises(BootNeutralError):
                        capture_state(POLICY, root, layout())
                    continue
                after = capture_state(POLICY, root, layout())
                if label == "mode":
                    after["protected_inventory"]["etc/systemd/system/octessera.service"]["mode"] += 1
                with self.assertRaises(BootNeutralError):
                    assert_unchanged(POLICY, before, after, layout())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            make_root(root)
            before = capture_state(POLICY, root, layout())
            after = capture_state(POLICY, root, layout())
            with self.assertRaises(BootNeutralError):
                assert_unchanged(POLICY, before, after, replace(layout(), raw_prepartition_sha256="b" * 64))

    def test_parent_binding_requires_exact_manifest_and_asset_identity(self) -> None:
        manifest_path = ROOT / "resources/image-parents/v0.7.5-trust-manifest.json"
        manifest = load_manifest(manifest_path)
        context = parent_context_for_board(manifest, BOARD)
        digest = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
        binding = parent_binding(POLICY, manifest_path, digest, context)
        self.assertEqual(binding["asset"], context["asset"])
        with self.assertRaises(BootNeutralError):
            parent_binding(POLICY, manifest_path, "0" * 64, context)
        altered = json.loads(json.dumps(context))
        altered["asset"]["name"] = "wrong.img.xz"
        with self.assertRaises(BootNeutralError):
            parent_binding(POLICY, manifest_path, digest, altered)


if __name__ == "__main__":
    unittest.main()
