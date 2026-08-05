from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from typing import Any

import sys

sys.path.insert(0, str(Path(__file__).parent))

from post_proof_record import (
    PROOF_TEMPLATE,
    RPI_TOOLS,
    build_record as build_post_record,
    validate_record as validate_post_record,
)
from requested_build_record import (
    PROOF_PACKAGES,
    SETUP_TOOL_FILES,
    build_record as build_requested_record,
    validate_record as validate_requested_record,
)
from setup_contract import contract_for_board, load_contract
from workflow_record_common import RecordError as WorkflowRecordError, identity
from disk_layout import DiskLayout
from disk_packaging import compression_identity
from disk_provenance import build_derived_provenance, provenance_bytes
from inventory import build_inventory, inventory_digest
from boot_neutral import load_policy
from provenance import TOOL_IDENTITY, build_provenance, digest_object, tool_code_model
import post_proof_record


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "resources/image-parents/v0.7.5-trust-manifest.json"
INPUTS = [
    Path("Cargo.lock"),
    Path("Cargo.toml"),
    Path("package.json"),
    Path("apps/pi-zero/Cargo.toml"),
    Path("Cross.toml"),
    Path("Dockerfile.pi-zero"),
    Path(".github/workflows/respin-board-image.yml"),
    Path("tools/image-respin/runtime_bundle.py"),
    Path("tools/image-respin/boot_neutral.py"),
]


def requested(board: str) -> dict:
    return build_requested_record(
        root=ROOT,
        source_sha="a" * 40,
        version="0.7.6",
        board=board,
        feature_command=f"cross build --release --features hardware-{board}",
        input_files=INPUTS,
        trust_manifest=MANIFEST,
        rustc_vv="rustc 1.90.0\nhost: x86_64-unknown-linux-gnu\n",
        cargo_version="cargo 1.90.0",
        cross_version="cross 0.2.5",
        container_rustc_vv="rustc 1.90.0 container",
        container_cargo_version="cargo 1.90.0 container",
        cross_image_id="sha256:" + "a" * 64,
        cross_repo_digests=[],
        base_image_id="sha256:" + "c" * 64,
        base_repo_digests=["rust@sha256:" + "b" * 64],
        proof_packages={name: "1.0.0" for name in PROOF_PACKAGES},
    )


def setup_inputs(board: str) -> tuple[Path, list[Path]]:
    contract_path = contract_for_board(board)
    contract, _ = load_contract(contract_path)
    inputs = [ROOT / item["path"] for item in contract["source_inputs"]]
    inputs.extend([contract_path, *(ROOT / path for path in SETUP_TOOL_FILES)])
    return contract_path, inputs


def requested_setup(board: str, input_files: list[Path] | None = None) -> dict:
    contract_path, expected_inputs = setup_inputs(board)
    return build_requested_record(
        root=ROOT,
        source_sha="a" * 40,
        version="0.7.6",
        board=board,
        feature_command=f"cross build --release --features hardware-{board}",
        input_files=INPUTS,
        trust_manifest=MANIFEST,
        rustc_vv="rustc 1.90.0\nhost: x86_64-unknown-linux-gnu\n",
        cargo_version="cargo 1.90.0",
        cross_version="cross 0.2.5",
        container_rustc_vv="rustc 1.90.0 container",
        container_cargo_version="cargo 1.90.0 container",
        cross_image_id="sha256:" + "a" * 64,
        cross_repo_digests=[],
        base_image_id="sha256:" + "c" * 64,
        base_repo_digests=["rust@sha256:" + "b" * 64],
        proof_packages={name: "1.0.0" for name in PROOF_PACKAGES},
        setup_layer="setup-portal",
        setup_contract=contract_path,
        setup_input_files=expected_inputs if input_files is None else input_files,
    )


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def synthetic_manifest(board: str, companions: list[Path]) -> dict:
    assets = []
    parent = next(item for item in json.loads(MANIFEST.read_text())["image_parents"] if item["board"] == board)
    for path in companions:
        payload = path.read_bytes()
        assets.append({"name": path.name, "node_id": "fixture", "sha256": __import__("hashlib").sha256(payload).hexdigest(), "size": len(payload)})
    return {"image_parents": [{"board": board, "asset": parent["asset"], "proof_companion_assets": parent["proof_companion_assets"]}], "assets": assets}


def synthetic_context(board: str, checked: dict) -> dict:
    return {"schema": "octessera.image-parent-trust/v1", "repository": "nexxyz/octessera", "tag": "v0.7.5", "source_commit": "4eec2b7edf6619fa22c709d4a589237a5748de78", "asset": checked["assets"][0]}


def write_orange_proof(path: Path, provenance: Path, artifact: Path, context: dict, kind: str = "runtime-only") -> None:
    document = {"schema": "octessera.image-proof/v2", "schema_version": 2, "proof_mode": "trusted-v0.7.5-boot-neutral", "phase5_claim": False, "boot_state": "v0.7.5-preserved", "artifact": {"name": artifact.name, "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(), "size": artifact.stat().st_size}, "board_profile": "orange-pi-zero-2w", "runtime": {"derivation_kind": kind, "setup_proof": kind == "setup-portal", "boot_mutation": False}, "parent": {"trust_manifest": "resources/image-parents/v0.7.5-trust-manifest.json", "name": context["asset"]["name"], "sha256": context["asset"]["sha256"], "size": context["asset"]["size"]}, "selected_boot": {"selected_kernel": "boot/Image", "selected_initramfs": "boot/uInitrd", "selected_dtb": "usr/lib/linux-image-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb"}, "contract": {"path": "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json", "sha256": load_policy(ROOT).sha256}, "respin_provenance_sha256": hashlib.sha256(provenance.read_bytes()).hexdigest()}
    write_json(path, document)


def synthetic_policy(context: dict) -> Any:
    policy = load_policy(ROOT)
    contract = copy.deepcopy(policy.contract)
    contract["parent_asset"].update({key: context["asset"][key] for key in ("name", "size", "sha256")})
    return type(policy)(policy.root, policy.path, contract, policy.sha256)


def write_respin_provenance(path: Path, board: str, version: str, context: dict, bundle: Path, artifact: Path) -> None:
    payload_digest = inventory_digest(build_inventory(bundle))
    contract_digest = hashlib.sha256((ROOT / "resources/image-mutations" / f"{board}.json").read_bytes()).hexdigest()
    parent_entries = {"octessera-pi": "0" * 64, "update-manifest.json": "1" * 64} if board == "raspberry-pi-zero-2w" else {"octessera-pi": "0" * 64, "octessera-runtime.json": "1" * 64, "SHA256SUMS": "2" * 64}
    parent_identity = {"board_profile": board, "prior_version": "0.7.5", "prior_release_entries": parent_entries, "prior_release_digest": "3" * 64, "prior_state_preimage_sha256": "4" * 64 if board == "raspberry-pi-zero-2w" else None, "prior_build_metadata_preimage_sha256": "5" * 64 if board == "orange-pi-zero-2w" else None, "current_target": "releases/0.7.5", "parent_context": context, "parent_context_sha256": digest_object(context)}
    runtime = build_provenance(board_profile=board, version=version, source_identity="a" * 40, parent_identity=parent_identity, payload_digest=payload_digest, mutation_contract_digest=contract_digest, pre_inventory_digest="d" * 64, post_inventory_digest="e" * 64, changed_paths=[])
    image = artifact.with_name("prepared.img")
    image.write_bytes(b"prepared")
    from disk_layout import PartitionIdentity
    partitions = (PartitionIdentity(1, "", 1, 1, "83", None, "ext4", None, None),) if board == "orange-pi-zero-2w" else ()
    layout = DiskLayout(board, 1, "dos", "disk", 1, 2, 512, partitions, "0" * 64, None)
    if board == "orange-pi-zero-2w":
        policy = load_policy(ROOT)
        boot_inventory = {"digest": "a" * 64, "count": 1}
        boot_integrity = {"pre": boot_inventory, "post": boot_inventory, "selected_kernel": "boot/Image", "selected_initramfs": "boot/uInitrd", "selected_dtb": "usr/lib/linux-image-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb", "selectors": {"format": "armbianEnv.txt", "kernel": "Image", "initramfs": "uInitrd", "dtb": "sun50i-h618-orangepi-zero2w.dtb"}, "protected_scopes": policy.contract["protected_scopes"], "protected_paths": policy.contract["protected_paths"], "expected_absent_paths": policy.contract["expected_absent_paths"], "changed_paths": []}
        document = build_derived_provenance(board_profile=board, version=version, source_identity="a" * 40, parent_context=context, trust_manifest_digest=hashlib.sha256(MANIFEST.read_bytes()).hexdigest(), runtime_provenance=runtime, pre_layout=layout, post_layout=layout, image=image, packaged=artifact, compression_identity=compression_identity(board), tool_identity=TOOL_IDENTITY, boot_integrity=boot_integrity, boot_policy={"proof_mode": policy.proof_mode, "policy": policy.policy}, parent_binding={"context": context, "asset": context["asset"], "trust_manifest_sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest(), "digest": digest_object({"context": context, "trust_manifest_sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest()})}, derivation_kind="runtime-only")
    else:
        document = build_derived_provenance(board_profile=board, version=version, source_identity="a" * 40, parent_context=context, trust_manifest_digest=hashlib.sha256(MANIFEST.read_bytes()).hexdigest(), runtime_provenance=runtime, pre_layout=layout, post_layout=layout, image=image, packaged=artifact, compression_identity=compression_identity(board), tool_identity=TOOL_IDENTITY)
    path.write_bytes(provenance_bytes(document))


class WorkflowRecordTests(unittest.TestCase):
    def test_setup_requested_records_cover_both_boards_and_exact_inputs(self) -> None:
        for board in ("orange-pi-zero-2w", "raspberry-pi-zero-2w"):
            with self.subTest(board=board):
                contract_path, input_files = setup_inputs(board)
                record = requested_setup(board)
                validate_requested_record(record, ROOT)
                self.assertEqual(record["setup"]["contract"]["path"], contract_path.relative_to(ROOT).as_posix())
                missing_contract = [path for path in input_files if path != contract_path]
                with self.assertRaisesRegex(WorkflowRecordError, "missing=.*resources/image-mutations"):
                    requested_setup(board, missing_contract)
                with self.assertRaisesRegex(WorkflowRecordError, "duplicates=.*resources/image-mutations"):
                    requested_setup(board, [*input_files, contract_path])
                missing_record = copy.deepcopy(record)
                missing_record["setup"]["inputs"].pop()
                with self.assertRaisesRegex(WorkflowRecordError, "requested setup inputs mismatch: missing="):
                    validate_requested_record(missing_record, ROOT)
                duplicate_record = copy.deepcopy(record)
                duplicate_record["setup"]["inputs"].append(copy.deepcopy(duplicate_record["setup"]["inputs"][0]))
                with self.assertRaisesRegex(WorkflowRecordError, r"requested setup inputs mismatch: missing=\[\]; extra=\[\]; duplicates="):
                    validate_requested_record(duplicate_record, ROOT)

    def test_requested_record_is_deterministic_and_validates_required_identities(self) -> None:
        first = requested("orange-pi-zero-2w")
        second = requested("orange-pi-zero-2w")
        self.assertEqual(first, second)
        validate_requested_record(first, ROOT)
        self.assertTrue(all(not Path(item["path"]).is_absolute() for item in first["inputs"]))
        malformed = copy.deepcopy(first)
        malformed["toolchain"]["host_orchestration"]["cross_version"] = ""
        with self.assertRaises(WorkflowRecordError):
            validate_requested_record(malformed, ROOT)
        malformed = requested("orange-pi-zero-2w")
        malformed["toolchain"].pop("base_image")
        with self.assertRaises(WorkflowRecordError):
            validate_requested_record(malformed, ROOT)

    def test_post_record_binds_companions_outputs_tools_and_success(self) -> None:
        for board in ("orange-pi-zero-2w", "raspberry-pi-zero-2w"):
            with self.subTest(board=board), tempfile.TemporaryDirectory(dir=ROOT) as temporary:
                work = Path(temporary)
                requested_path = work / "requested-build.json"
                write_json(requested_path, requested(board))
                parent = next(item for item in json.loads(MANIFEST.read_text())["image_parents"] if item["board"] == board)
                companions = []
                for name in (parent["asset"], *parent["proof_companion_assets"]):
                    path = work / name
                    path.write_bytes(name.encode("utf-8"))
                    companions.append(path)
                bundle = work / "runtime-bundle"
                bundle.mkdir()
                for name in ("SHA256SUMS", "octessera-pi", "octessera-runtime.json"):
                    (bundle / name).write_bytes(name.encode("utf-8"))
                artifact = work / "derived.img.xz"
                provenance = work / "derived.img.xz.provenance.json"
                artifact.write_bytes(b"derived")
                provenance.write_bytes(b"provenance")
                if board == "orange-pi-zero-2w":
                    proof_outputs = {"orange-image": work / "orange-proof.txt"}
                else:
                    proof_outputs = {
                        "raspberry-sanitized": work / "sanitized-proof.txt",
                        "raspberry-kernel": work / "kernel-proof.txt",
                    }
                for path in proof_outputs.values():
                    path.write_text("proof passed\n", encoding="utf-8")
                checked = synthetic_manifest(board, companions)
                context = synthetic_context(board, checked)
                write_respin_provenance(provenance, board, "0.7.6", context, bundle, artifact)
                if board == "orange-pi-zero-2w":
                    write_orange_proof(proof_outputs["orange-image"], provenance, artifact, context)
                with patch.object(post_proof_record, "load_manifest", return_value=checked), patch.object(post_proof_record, "parent_context_for_board", return_value=context), patch.object(post_proof_record, "load_policy", return_value=synthetic_policy(context)):
                    record = build_post_record(
                        root=ROOT,
                        requested_build=requested_path,
                        manifest=MANIFEST,
                        board=board,
                        runtime_bundle=bundle,
                        artifact=artifact,
                        respin_provenance=provenance,
                        proof_outputs=proof_outputs,
                        template_ids={key: PROOF_TEMPLATE[key][0] for key in proof_outputs},
                        companions=companions,
                        workflow=ROOT / ".github/workflows/respin-board-image.yml",
                    )
                    validate_post_record(record, ROOT)
                    if board == "raspberry-pi-zero-2w":
                        self.assertEqual({item["path"] for item in record["proof_tools"]}, set(RPI_TOOLS))
                        self.assertIn("tools/pi-image/rpi_initramfs_proof.py", {item["path"] for item in record["proof_tools"]})
                        self.assertIn("resources/image-construction/boot-layers/raspberry-pi-zero-2w.json", {item["path"] for item in record["proof_tools"]})
                        missing_output = copy.deepcopy(record)
                        missing_output["proofs"].pop()
                        with self.assertRaises(WorkflowRecordError):
                            validate_post_record(missing_output, ROOT)
                        tampered_output = copy.deepcopy(record)
                        tampered_output["proofs"][0]["output"]["sha256"] = "0" * 64
                        with self.assertRaises(WorkflowRecordError):
                            validate_post_record(tampered_output, ROOT)
                        tampered_tool = copy.deepcopy(record)
                        tampered_tool["proof_tools"][0]["sha256"] = "0" * 64
                        with self.assertRaises(WorkflowRecordError):
                            validate_post_record(tampered_tool, ROOT)
                    self.assertFalse(Path(record["requested_build"]["path"]).is_absolute())
                    self.assertFalse(Path(record["derived_artifact"]["path"]).is_absolute())
                    self.assertFalse(Path(record["runtime_bundle"]["path"]).is_absolute())
                    altered_provenance = json.loads(provenance.read_text())
                    altered_provenance["runtime_mutation"]["provenance"]["payload"]["digest"] = "0" * 64
                    write_json(provenance, altered_provenance)
                    with self.assertRaises(WorkflowRecordError):
                        validate_post_record(record, ROOT)
                    write_respin_provenance(provenance, board, "0.7.6", context, bundle, artifact)
                    artifact.write_bytes(b"altered")
                    with self.assertRaises(WorkflowRecordError):
                        validate_post_record(record, ROOT)

    def test_post_record_rejects_failed_proof_and_altered_tool_or_companion(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            work = Path(temporary)
            requested_path = work / "requested.json"
            write_json(requested_path, requested("orange-pi-zero-2w"))
            parent = json.loads(MANIFEST.read_text())["image_parents"][0]
            companions = []
            for name in (parent["asset"], *parent["proof_companion_assets"]):
                path = work / name
                path.write_bytes(b"companion")
                companions.append(path)
            bundle = work / "bundle"
            bundle.mkdir()
            for name in ("SHA256SUMS", "octessera-pi", "octessera-runtime.json"):
                (bundle / name).write_bytes(b"bundle")
            artifact = work / "artifact"
            provenance = work / "provenance"
            artifact.write_bytes(b"artifact")
            provenance.write_bytes(b"provenance")
            proof = work / "proof"
            checked = synthetic_manifest("orange-pi-zero-2w", companions)
            context = synthetic_context("orange-pi-zero-2w", checked)
            write_respin_provenance(provenance, "orange-pi-zero-2w", "0.7.6", context, bundle, artifact)
            write_orange_proof(proof, provenance, artifact, context)
            with patch.object(post_proof_record, "load_manifest", return_value=checked), patch.object(post_proof_record, "parent_context_for_board", return_value=context), patch.object(post_proof_record, "load_policy", return_value=synthetic_policy(context)):
                record = build_post_record(
                    root=ROOT,
                    requested_build=requested_path,
                    manifest=MANIFEST,
                    board="orange-pi-zero-2w",
                    runtime_bundle=bundle,
                    artifact=artifact,
                    respin_provenance=provenance,
                    proof_outputs={"orange-image": proof},
                    template_ids={"orange-image": PROOF_TEMPLATE["orange-image"][0]},
                    companions=companions,
                    workflow=ROOT / ".github/workflows/respin-board-image.yml",
                )
                def reject_provenance(mutator: Any) -> None:
                    altered = json.loads(provenance.read_text())
                    mutator(altered)
                    write_json(provenance, altered)
                    record["respin_provenance"] = identity(provenance, ROOT)
                    with self.assertRaises(WorkflowRecordError):
                        validate_post_record(record, ROOT)
                    write_respin_provenance(provenance, "orange-pi-zero-2w", "0.7.6", context, bundle, artifact)
                    record["respin_provenance"] = identity(provenance, ROOT)

                reject_provenance(lambda value: value["runtime_mutation"]["provenance"]["parent"]["identity"].update({"unexpected": True}))
                reject_provenance(lambda value: value["runtime_mutation"]["provenance"]["parent"].update({"digest": "0" * 64}))
                reject_provenance(lambda value: value["runtime_mutation"]["provenance"]["mutation_contract"].update({"digest": "0" * 64}))
                reject_provenance(lambda value: value["disk_invariants"].update({"pre": {"drift": True}}))
                reject_provenance(lambda value: value["disk_invariants"].update({"digest": "0" * 64}))
                reject_provenance(lambda value: value["boot_integrity"]["pre"].update({"digest": "0" * 64}))
                reject_provenance(lambda value: value["boot_integrity"].update({"selected_kernel": "boot/tampered"}))
                reject_provenance(lambda value: value["boot_integrity"]["selectors"].update({"kernel": "tampered"}))
                reject_provenance(lambda value: value["boot_integrity"].update({"protected_paths": []}))
                reject_provenance(lambda value: value["boot_integrity"].update({"changed_paths": ["boot/Image"]}))
                reject_provenance(lambda value: value.update({"policy": {"name": "wrong"}}))
                reject_provenance(lambda value: value["finalizer"].update({"tool_identity": "not-runtime-mutation"}))
                reject_provenance(lambda value: value["finalizer"].update({"runtime_tool_code_digest": "0" * 64}))
                proof_document = json.loads(proof.read_text())
                proof_document["selected_boot"]["selected_kernel"] = "boot/tampered"
                write_json(proof, proof_document)
                with self.assertRaises(WorkflowRecordError):
                    validate_post_record(record, ROOT)
                write_orange_proof(proof, provenance, artifact, context)
                proof.write_text("proof failed\n", encoding="utf-8")
                with self.assertRaises(WorkflowRecordError):
                    validate_post_record(record, ROOT)
                proof.write_text("proof passed\n", encoding="utf-8")
                failed_result = copy.deepcopy(record)
                failed_result["result"]["status"] = "failed"
                with self.assertRaises(WorkflowRecordError):
                    validate_post_record(failed_result, ROOT)
                altered_tool = copy.deepcopy(record)
                altered_tool["proof_tools"][0]["sha256"] = "0" * 64
                with self.assertRaises(WorkflowRecordError):
                    validate_post_record(altered_tool, ROOT)
                companions[0].write_bytes(b"altered companion")
                with self.assertRaises(WorkflowRecordError):
                    validate_post_record(record, ROOT)


if __name__ == "__main__":
    unittest.main()
