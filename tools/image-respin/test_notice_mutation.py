from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from notice_mutation import NOTICE_TARGET, install_notices, validate_notice_record
from inventory import build_inventory
from runtime_contract import MutationError


ROOT = Path(__file__).resolve().parents[2]


def _stage(root: Path) -> Path:
    stages = list((root / "usr/share/doc").glob(".octessera-notice-stage-*"))
    if len(stages) != 1:
        raise AssertionError(f"expected one private notice stage, got {stages}")
    return stages[0] / "usr/share/doc/octessera"


class NoticeMutationTests(unittest.TestCase):
    def test_absent_target_becomes_exact_canonical_tree_and_preserves_vendor_sentinels(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "usr/share/doc/base-files").mkdir(parents=True)
            (root / "usr/share/common-licenses").mkdir(parents=True)
            (root / "usr/share/doc/base-files/copyright").write_bytes(b"vendor copyright\n")
            (root / "usr/share/common-licenses/GPL-3").write_bytes(b"vendor GPL\n")
            before = build_inventory(root)
            result = install_notices(root, before, ROOT)
            validate_notice_record(result.record, ROOT)
            self.assertEqual(result.record["preimage"], {"path": NOTICE_TARGET, "status": "absent"})
            self.assertEqual(result.record["changed_paths"], result.changed_paths)
            self.assertEqual((root / "usr/share/doc/octessera/LICENSE").read_bytes(), (ROOT / "LICENSE").read_bytes())
            self.assertEqual((root / "usr/share/doc/base-files/copyright").read_bytes(), b"vendor copyright\n")
            self.assertEqual((root / "usr/share/common-licenses/GPL-3").read_bytes(), b"vendor GPL\n")

    def test_preexisting_target_parent_and_symlink_parent_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(MutationError):
                install_notices(root, build_inventory(root), ROOT)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "usr/share/doc").mkdir(parents=True)
            (root / NOTICE_TARGET).mkdir()
            before = build_inventory(root)
            with self.assertRaises(MutationError):
                install_notices(root, before, ROOT)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "usr/share").mkdir(parents=True)
            (root / "outside").mkdir()
            (root / "usr/share/doc").symlink_to(root / "outside", target_is_directory=True)
            with self.assertRaises(MutationError):
                install_notices(root, build_inventory(root), ROOT)

    def test_staged_unknown_symlink_hardlink_and_metadata_are_rejected_and_cleaned(self) -> None:
        mutations = {
            "unknown": lambda tree: (tree / "unknown").write_bytes(b"unknown"),
            "symlink": lambda tree: ((tree / "LICENSE").unlink(), (tree / "LICENSE").symlink_to(ROOT / "LICENSE")),
            "hardlink": lambda tree: ((tree / "LICENSE").unlink(), (tree / "LICENSE").hardlink_to(tree / "NOTICE")),
            "metadata": lambda tree: os.chmod(tree / "LICENSE", 0o600),
        }
        for label, mutate in mutations.items():
            if label == "metadata" and os.name == "nt":
                continue
            with self.subTest(mutation=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / "usr/share/doc").mkdir(parents=True)
                mutate_stage = lambda name, mutate=mutate: mutate(_stage(root)) if name == "notice-staged" else None
                with self.assertRaises(MutationError):
                    install_notices(root, build_inventory(root), ROOT, mutate_stage)
                self.assertFalse((root / NOTICE_TARGET).exists())
                self.assertEqual(list((root / "usr/share/doc").glob(".octessera-notice-stage-*")), [])

    def test_partial_publish_failure_removes_target_and_private_stage(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "usr/share/doc").mkdir(parents=True)

            def fail_after_publish(name: str) -> None:
                if name == "notice-published":
                    raise RuntimeError("injected partial publish")

            with self.assertRaises(RuntimeError):
                install_notices(root, build_inventory(root), ROOT, fail_after_publish)
            self.assertFalse((root / NOTICE_TARGET).exists())
            self.assertEqual(list((root / "usr/share/doc").glob(".octessera-notice-stage-*")), [])


if __name__ == "__main__":
    unittest.main()
