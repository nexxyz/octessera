from __future__ import annotations

import argparse
import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any, cast

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/image-respin"))

from boot_neutral import load_policy
from current_parent import load_record, parent_context
from disk_packaging import compression_identity
from inventory import build_inventory
from notice_mutation import install_notices
from orange_trusted_parent_proof import TrustedParentProofError, _protected_state, artifact_identity, verify_trusted, verify_trusted_roots
from provenance import TOOL_IDENTITY, build_provenance, digest_object
from test_orange_image_proof_support import VERIFY


RECORD_PATH = ROOT / "resources/image-parents/orange-pi-zero-2w-current.json"
BOARD = "orange-pi-zero-2w"
RELEASE = "6.18.46-current-sunxi64"


def _write(path: Path, value: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())


def _make_root(root: Path, contract: dict[str, Any]) -> None:
    for scope in cast(list[dict[str, Any]], contract["protected_scopes"]):
        _write(root / scope["prefix"] / ".scope-marker", b"scope")
    for relative in cast(list[str], contract["protected_paths"]):
        if relative == "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service":
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.symlink_to("../octessera-orange-oled-suspend.service")
        elif relative != "lib":
            _write(root / relative, relative.encode())
    _write(root / "boot/armbianEnv.txt", "verbosity=1\nfdtfile=sun50i-h618-orangepi-zero2w.dtb\n")
    _write(root / "boot/Image", b"kernel")
    _write(root / "boot/uInitrd", b"initramfs")
    _write(root / "usr/share/doc/base-files/copyright", b"vendor copyright\n")
    _write(root / "usr/share/common-licenses/GPL-3", b"vendor GPL\n")
    (root / "lib").symlink_to("usr/lib", target_is_directory=True)


def _layout() -> dict[str, object]:
    return {
        "board_profile": BOARD,
        "image_size": 100,
        "table_label": "gpt",
        "disk_id": None,
        "first_lba": 0,
        "last_lba": 99,
        "sector_size": 512,
        "partitions": [{"index": 1, "start": 2048, "size": 90000, "partition_type": "83", "partition_uuid": None, "filesystem_type": "ext4", "filesystem_uuid": None, "filesystem_label": None}],
        "raw_prepartition_sha256": "d" * 64,
        "raw_boot_partition_sha256": None,
    }


def _fixture(work: Path) -> dict[str, Any]:
    policy = load_policy(ROOT)
    record, record_digest = load_record(ROOT, RECORD_PATH)
    context = parent_context(ROOT, RECORD_PATH)
    parent = work / "parent"
    derived = work / "derived"
    _make_root(parent, policy.contract)
    _make_root(derived, policy.contract)
    notice = install_notices(derived, build_inventory(derived), ROOT)
    _write(derived / "opt/octessera/runtime.txt", "runtime-only\n")
    parent_state = _protected_state(parent, policy.contract)
    derived_state = _protected_state(derived, policy.contract)
    layout = _layout()
    parent_identity = {"board_profile": BOARD, "prior_version": context["version"], "prior_release_entries": {"octessera-pi": "a" * 64, "octessera-runtime.json": "b" * 64, "SHA256SUMS": "c" * 64}, "prior_release_digest": "d" * 64, "prior_state_preimage_sha256": None, "prior_build_metadata_preimage_sha256": "e" * 64, "current_target": "/opt/octessera/releases/0.8.1", "parent_context": context, "parent_context_sha256": digest_object(context)}
    runtime = build_provenance(board_profile=BOARD, version="2.0.0", source_identity="fixture", parent_identity=parent_identity, payload_digest="f" * 64, mutation_contract_digest=hashlib.sha256((ROOT / "resources/image-mutations/orange-pi-zero-2w.json").read_bytes()).hexdigest(), pre_inventory_digest="1" * 64, post_inventory_digest="2" * 64, changed_paths=sorted(["opt/octessera/runtime.txt", *notice.changed_paths]), notice=notice.record)
    binding = {"path": policy.contract["parent_record"], "sha256": record_digest, "size": RECORD_PATH.stat().st_size}
    parent_document = {"record": binding, "context": context, "image": record["image"], "digest": digest_object({"context": context, "record": binding, "image": record["image"]})}
    boot_inventory = {"digest": parent_state["digest"], "count": parent_state["count"]}
    selected_boot = {"selected_kernel": "boot/Image", "selected_initramfs": "boot/uInitrd", "selected_dtb": f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb"}
    boot_integrity = {"pre": boot_inventory, "post": boot_inventory, **selected_boot, "selectors": {"format": "armbianEnv.txt", "kernel": "Image", "initramfs": "uInitrd", "dtb": "sun50i-h618-orangepi-zero2w.dtb"}, "protected_scopes": policy.contract["protected_scopes"], "protected_paths": policy.contract["protected_paths"], "expected_absent_paths": policy.contract["expected_absent_paths"], "changed_paths": []}
    identity = artifact_identity(derived)
    runtime_finalizer = runtime["finalizer"]
    provenance = {"proof_schema": policy.contract["respin_provenance"]["runtime_schema"], "schema_version": 2, "proof_mode": policy.proof_mode, "derivation_kind": "runtime-only", "board_profile": BOARD, "version": "2.0.0", "source_identity": "fixture", "boot_mutation": False, "phase5_claim": False, "policy": policy.policy, "parent": parent_document, "runtime_mutation": {"digest": digest_object(runtime), "provenance": runtime}, "boot_integrity": boot_integrity, "disk_invariants": {"pre": layout, "post": layout, "digest": digest_object({"pre": layout, "post": layout})}, "derived_image": {"sha256": identity[0], "size": identity[1]}, "packaged_artifact": {"sha256": identity[0], "size": identity[1], "path": "derived.img.xz"}, "finalizer": {"tool_identity": TOOL_IDENTITY, "compression_identity": compression_identity(BOARD), "runtime_tool_code_schema": runtime_finalizer["tool_code_schema"], "runtime_tool_code_version": runtime_finalizer["tool_code_version"], "runtime_tool_code_digest": runtime_finalizer["tool_code_digest"], "runtime_tool_code_files": runtime_finalizer["tool_code_files"]}}
    provenance_path = work / "provenance.json"
    provenance_path.write_text(json.dumps(provenance, sort_keys=True) + "\n", encoding="utf-8")
    return {"policy": policy, "record": record, "context": context, "record_digest": record_digest, "parent": parent, "derived": derived, "layout": layout, "state": parent_state, "identity": identity, "provenance": provenance_path}


def _verify(fixture: dict[str, Any], provenance: Path | None = None, derived: Path | None = None) -> dict[str, Any]:
    policy = fixture["policy"]
    record = fixture["record"]
    context = fixture["context"]
    parent = fixture["parent"]
    derived_root = fixture["derived"] if derived is None else derived
    return verify_trusted_roots(parent, derived_root, record, context, fixture["record_digest"], policy.contract, fixture["provenance"] if provenance is None else provenance, "runtime-only", None, ROOT, fixture["identity"], fixture["identity"], "derived.img.xz", fixture["layout"], fixture["layout"], RECORD_PATH, policy.path)


class ValidatedParentProofTests(unittest.TestCase):
    def test_runtime_only_validated_parent_proof_succeeds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = _fixture(Path(temporary))
            proof = _verify(fixture)
            self.assertEqual(proof["proof_mode"], "validated-parent")
            self.assertEqual(proof["runtime"], {"derivation_kind": "runtime-only", "setup_proof": False, "boot_mutation": False})
            self.assertEqual(proof["parent"]["image"], fixture["record"]["image"])

    def test_parent_image_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = _fixture(Path(temporary))
            wrong_image = Path(temporary) / fixture["record"]["image"]["name"]
            wrong_image.write_bytes(b"wrong parent image")
            with self.assertRaises(TrustedParentProofError):
                verify_trusted(fixture["parent"], fixture["derived"], wrong_image, fixture["policy"].path, RECORD_PATH, fixture["provenance"], "runtime-only", None, ROOT, fixture["identity"], fixture["identity"], "derived.img.xz", fixture["layout"], fixture["layout"])

    def test_protected_parent_derived_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = _fixture(Path(temporary))
            tampered = Path(temporary) / "tampered"
            _make_root(tampered, fixture["policy"].contract)
            _write(tampered / "boot/Image", b"tampered")
            with self.assertRaises(TrustedParentProofError):
                _verify(fixture, derived=tampered)

    def test_oled_replacements_are_protected_and_obsolete_hooks_are_absent(self) -> None:
        current_paths = (
            "etc/systemd/system/octessera-orange-oled-suspend.service",
            "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service",
            "usr/local/sbin/octessera-orange-oled-suspend",
            "usr/local/sbin/octessera-orange-oled-handoff.py",
        )
        for relative in current_paths:
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as temporary:
                fixture = _fixture(Path(temporary))
                path = fixture["derived"] / relative
                if relative.endswith("sleep.target.requires/octessera-orange-oled-suspend.service"):
                    path.unlink()
                    path.symlink_to("../octessera.service")
                else:
                    _write(path, b"tampered")
                with self.assertRaises(TrustedParentProofError):
                    _verify(fixture)
        for relative in (
            "lib/systemd/system-sleep/octessera-orange-oled",
            "usr/lib/systemd/system-sleep/octessera-orange-oled",
        ):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as temporary:
                fixture = _fixture(Path(temporary))
                if relative.startswith("lib/"):
                    (fixture["derived"] / "lib").unlink()
                _write(fixture["derived"] / relative, b"obsolete")
                with self.assertRaises(TrustedParentProofError):
                    _verify(fixture)

    def test_provenance_tampering_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = _fixture(Path(temporary))
            tampered = copy.deepcopy(json.loads(fixture["provenance"].read_text(encoding="utf-8")))
            tampered["parent"]["image"]["sha256"] = "0" * 64
            path = Path(temporary) / "tampered-provenance.json"
            path.write_text(json.dumps(tampered) + "\n", encoding="utf-8")
            with self.assertRaises(TrustedParentProofError):
                _verify(fixture, provenance=path)

    def test_constructor_only_arguments_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = _fixture(Path(temporary))
            base = {"manifest": None, "construction_contract": None, "linux_image": None, "linux_dtb": None, "evidence": None, "provenance": None, "parent_image": Path(temporary) / "parent.img.xz", "parent_record": RECORD_PATH, "respin_provenance": fixture["provenance"], "boot_neutral_contract": fixture["policy"].path, "derivation_kind": "runtime-only", "setup_proof": None}
            for name in ("manifest", "construction_contract", "linux_image", "linux_dtb", "evidence", "provenance"):
                with self.subTest(name=name):
                    args = argparse.Namespace(**base)
                    setattr(args, name, Path(temporary) / f"{name}.input")
                    with self.assertRaises(VERIFY.ImageProofError):
                        VERIFY._trusted(args, fixture["derived"], fixture["identity"], fixture["identity"], "derived.img.xz", ROOT, fixture["layout"])


if __name__ == "__main__":
    unittest.main()
