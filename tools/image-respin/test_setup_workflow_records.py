from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any

import sys

sys.path.insert(0, str(Path(__file__).parent))

from disk_layout import DiskLayout, PartitionIdentity
from disk_packaging import compression_identity, file_digest, package_derived
from post_proof_record import ORANGE_TOOLS, RPI_TOOLS, _bundle_identity
from provenance import build_provenance, digest_object, tool_code_model
from setup_contract import contract_for_board, load_contract
from setup_mutation import SETUP_TOOL_IDENTITY
from setup_provenance import setup_tool_code_model
from setup_workflow_record import SETUP_PROOF_TOOLS, _production_proof_identities, _validate_production_proofs, _validate_provenance, _validate_setup_proof_tools
from workflow_record_common import RecordError, identity
from test_workflow_records import notice_record, write_orange_proof, write_respin_provenance
from trust_manifest import load_manifest, parent_context_for_board


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "resources/image-parents/v0.7.5-trust-manifest.json"
BOARD = "raspberry-pi-zero-2w"


def _sha(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _fixture(work: Path) -> tuple[Path, dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any]]:
    contract, contract_digest = load_contract(contract_for_board(BOARD))
    bundle_path = work / "runtime-bundle"
    bundle_path.mkdir()
    for name in ("SHA256SUMS", "octessera-pi", "octessera-runtime.json"):
        (bundle_path / name).write_bytes(name.encode())
    bundle = _bundle_identity(bundle_path, ROOT)
    image = work / "image.img"
    image.write_bytes(b"abcdefgh")
    artifact = work / "octessera-0.7.6-raspberry-pi-zero-2w-derived-setup-respin.zip"
    package_derived(image, artifact, BOARD, "0.7.6", "setup")
    artifact_identity = identity(artifact, ROOT)
    context = {"board": BOARD, "asset": {"name": "parent.zip", "sha256": "a" * 64, "size": 1}}
    parent_identity = {"board_profile": BOARD, "prior_version": "0.7.5", "prior_release_entries": {"octessera-pi": "b" * 64, "update-manifest.json": "c" * 64}, "prior_release_digest": "d" * 64, "prior_state_preimage_sha256": "e" * 64, "prior_build_metadata_preimage_sha256": None, "current_target": "releases/0.7.5", "parent_context": context, "parent_context_sha256": digest_object(context)}
    runtime_contract_digest = identity(ROOT / "resources/image-mutations" / f"{BOARD}.json", ROOT)["sha256"]
    notice = notice_record()
    runtime = build_provenance(board_profile=BOARD, version="0.7.6", source_identity="a" * 40, parent_identity=parent_identity, payload_digest=bundle["inventory_sha256"], mutation_contract_digest=runtime_contract_digest, pre_inventory_digest="f" * 64, post_inventory_digest="0" * 64, changed_paths=notice["changed_paths"], notice=notice)
    prerequisites = {"packages_sha256": "1" * 64, "accounts": {f"user:{item['user']}": "user" for item in contract["prerequisites"]["accounts"]} | {f"group:{item['group']}": "group" for item in contract["prerequisites"]["accounts"]}, "passwd_sha256": "2" * 64, "group_sha256": "3" * 64, "executables": {item: {"path": item, "type": "file", "uid": 0, "gid": 0, "mode": 493, "symlink": False, "target": None, "sha256": "0" * 64, "xattrs": {}, "capability": None} for item in contract["prerequisites"]["executables"]}, "services": {item: "service" for item in contract["prerequisites"]["services"]}}
    setup_parent = {"board_profile": BOARD, "preimage_source": contract["preimage_source"], "prerequisites": prerequisites, "preimage_digest": "4" * 64}
    source_inputs = [identity(ROOT / item["path"], ROOT) for item in contract["source_inputs"]]
    setup_paths = sorted([item["target"] for item in contract["directories"]] + [item["target"] for item in contract["entries"]])
    setup_mutation = {"proof_schema": "octessera.image-setup-mutation-provenance.v1", "schema_version": 1, "board_profile": BOARD, "source_identity": "a" * 40, "parent": {"identity": setup_parent, "digest": digest_object(setup_parent)}, "setup_layer": {"contract_digest": contract_digest, "source_inputs": source_inputs}, "inventories": {"pre": "5" * 64, "post": "6" * 64}, "changed_paths": setup_paths, "finalizer": {"source_identity": "a" * 40, "tool_identity": SETUP_TOOL_IDENTITY, "tool_code_digest": setup_tool_code_model(ROOT / "tools/image-respin")["digest"]}}
    proof = {"proof": "setup-layer-mounted", "schema_version": 1, "board_profile": BOARD, "contract_sha256": contract_digest, "inventory_sha256": "6" * 64, "prerequisites": prerequisites, "verified_paths": setup_paths}
    layout = DiskLayout(BOARD, 8, "dos", "disk", 0, 7, 1, (PartitionIdentity(1, "", 1, 2, "type", "p1", "vfat", "f1", "boot"), PartitionIdentity(2, "", 3, 2, "type", "p2", "ext4", "f2", "root")), _sha(b"a"), _sha(b"bc"))
    disk = {"pre": layout.as_dict(), "post": layout.as_dict()}
    image_digest, image_size = file_digest(image)
    package_digest, package_size = file_digest(artifact)
    provenance = {"proof_schema": "octessera.image-derived-setup-respin-provenance.v1", "schema_version": 1, "board_profile": BOARD, "version": "0.7.6", "source_identity": "a" * 40, "parent": {"context": context, "trust_manifest_sha256": _sha(MANIFEST.read_bytes()), "digest": digest_object({"context": context, "trust_manifest_sha256": _sha(MANIFEST.read_bytes())})}, "runtime_mutation": {"digest": digest_object(runtime), "provenance": runtime}, "setup_mutation": {"digest": digest_object(setup_mutation), "provenance": setup_mutation}, "setup_proof": proof, "disk_invariants": {**disk, "digest": digest_object(disk)}, "derived_image": {"sha256": image_digest, "size": image_size}, "packaged_artifact": {"sha256": package_digest, "size": package_size, "path": artifact.name}, "finalizer": {"tool_identity": "octessera-image-respin-runtime-mutation/1", "compression_identity": compression_identity(BOARD), "setup_tool_code": setup_tool_code_model(ROOT / "tools/image-respin")}}
    provenance_path = work / "artifact.provenance.json"
    provenance_path.write_text(json.dumps(provenance), encoding="utf-8")
    requested = {"source": {"sha": "a" * 40, "version": "0.7.6", "board": BOARD, "feature_command": "test"}}
    return provenance_path, provenance, requested, contract, {"path": str(contract_for_board(BOARD).relative_to(ROOT)).replace("\\", "/"), "sha256": contract_digest, "size": contract_for_board(BOARD).stat().st_size}, proof, context, bundle, artifact_identity


class SetupWorkflowRecordTests(unittest.TestCase):
    def test_setup_proof_tools_are_board_explicit(self) -> None:
        self.assertEqual(SETUP_PROOF_TOOLS["orange-pi-zero-2w"], ORANGE_TOOLS)
        self.assertEqual(SETUP_PROOF_TOOLS["raspberry-pi-zero-2w"], RPI_TOOLS)
        self.assertIn("tools/pi-image/rpi_initramfs_proof.py", RPI_TOOLS)
        self.assertIn("resources/image-construction/boot-layers/raspberry-pi-zero-2w.json", RPI_TOOLS)
        tools = [identity(ROOT / path, ROOT) for path in SETUP_PROOF_TOOLS[BOARD]]
        _validate_setup_proof_tools(tools, ROOT, BOARD)
        with self.assertRaises(RecordError):
            _validate_setup_proof_tools(tools[:-1], ROOT, BOARD)
        altered_tool = copy.deepcopy(tools)
        altered_tool[0]["sha256"] = "0" * 64
        with self.assertRaises(RecordError):
            _validate_setup_proof_tools(altered_tool, ROOT, BOARD)

    def test_raspberry_setup_production_proofs_are_exact_and_tamper_evident(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            work = Path(temporary)
            outputs = {label: work / f"{label}.txt" for label in ("raspberry-sanitized", "raspberry-kernel")}
            for path in outputs.values():
                path.write_text("proof passed\n", encoding="utf-8")
            identities, structured = _production_proof_identities(ROOT, BOARD, outputs)
            self.assertEqual(structured, {})
            self.assertEqual(set(identities), set(outputs))
            _validate_production_proofs(identities, ROOT, BOARD)
            with self.assertRaises(RecordError):
                _validate_production_proofs({"raspberry-sanitized": identities["raspberry-sanitized"]}, ROOT, BOARD)
            extra = dict(identities)
            extra["unexpected"] = identities["raspberry-sanitized"]
            with self.assertRaises(RecordError):
                _validate_production_proofs(extra, ROOT, BOARD)
            outputs["raspberry-kernel"].write_text("tampered\n", encoding="utf-8")
            with self.assertRaises(RecordError):
                _validate_production_proofs(identities, ROOT, BOARD)

    def test_orange_setup_provenance_and_both_structured_proofs_are_bound(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            work = Path(temporary)
            bundle = work / "bundle"
            bundle.mkdir()
            for name in ("SHA256SUMS", "octessera-pi", "octessera-runtime.json"):
                (bundle / name).write_bytes(name.encode())
            artifact = work / "derived.img.xz"
            artifact.write_bytes(b"artifact")
            provenance = work / "derived.img.xz.provenance.json"
            parent_context = parent_context_for_board(load_manifest(MANIFEST), "orange-pi-zero-2w")
            write_respin_provenance(provenance, "orange-pi-zero-2w", "0.7.6", parent_context, bundle, artifact)
            document = json.loads(provenance.read_text())
            setup_value = {"proof_schema": "octessera.image-setup-mutation-provenance.v1", "schema_version": 1, "board_profile": "orange-pi-zero-2w", "source_identity": "a" * 40, "parent": {}, "setup_layer": {}, "inventories": {}, "changed_paths": [], "finalizer": {}}
            setup_proof = {"proof": "setup-layer-mounted", "schema_version": 1, "board_profile": "orange-pi-zero-2w", "contract_sha256": hashlib.sha256((ROOT / "resources/image-mutations/orange-pi-zero-2w-setup.json").read_bytes()).hexdigest(), "inventory_sha256": "b" * 64, "prerequisites": {}, "verified_paths": []}
            document["derivation_kind"] = "setup-portal"
            document["proof_schema"] = "octessera.image-derived-setup-respin-provenance.v2"
            document["runtime_mutation"]["provenance"]["source_identity"] = "a" * 40
            document["finalizer"] = {"tool_identity": "octessera-image-respin-runtime-mutation/1", "compression_identity": compression_identity("orange-pi-zero-2w"), "setup_tool_code": setup_tool_code_model(ROOT / "tools/image-respin")}
            document["setup_mutation"] = {"digest": digest_object(setup_value), "provenance": setup_value}
            document["setup_proof"] = setup_proof
            provenance.write_text(json.dumps(document), encoding="utf-8")
            orange_proof = work / "orange-image-proof.json"
            write_orange_proof(orange_proof, provenance, artifact, parent_context, "setup-portal")
            contract, contract_digest = load_contract(contract_for_board("orange-pi-zero-2w"))
            requested = {"source": {"sha": "a" * 40, "version": "0.7.6", "board": "orange-pi-zero-2w", "feature_command": "test"}}
            contract_identity = {"path": str(contract_for_board("orange-pi-zero-2w").relative_to(ROOT)).replace("\\", "/"), "sha256": contract_digest, "size": contract_for_board("orange-pi-zero-2w").stat().st_size}
            proof = setup_proof
            _validate_provenance(provenance, ROOT, requested, contract, contract_identity, proof, parent_context, hashlib.sha256(MANIFEST.read_bytes()).hexdigest(), _bundle_identity(bundle, ROOT), identity(artifact, ROOT), json.loads(orange_proof.read_text()))
            baseline = json.loads(provenance.read_text())
            baseline_proof = json.loads(orange_proof.read_text())
            def reject(mutator: Any, proof_mutator: Any | None = None) -> None:
                altered = copy.deepcopy(baseline)
                mutator(altered)
                altered_proof = copy.deepcopy(baseline_proof)
                if proof_mutator is not None:
                    proof_mutator(altered_proof)
                provenance.write_text(json.dumps(altered), encoding="utf-8")
                orange_proof.write_text(json.dumps(altered_proof), encoding="utf-8")
                with self.assertRaises(RecordError):
                    _validate_provenance(provenance, ROOT, requested, contract, contract_identity, proof, parent_context, hashlib.sha256(MANIFEST.read_bytes()).hexdigest(), _bundle_identity(bundle, ROOT), identity(artifact, ROOT), altered_proof)
                provenance.write_text(json.dumps(baseline), encoding="utf-8")
                orange_proof.write_text(json.dumps(baseline_proof), encoding="utf-8")
            reject(lambda value: value["boot_integrity"]["pre"].update({"digest": "0" * 64}))
            reject(lambda value: value["boot_integrity"].update({"selected_kernel": "boot/tampered"}))
            reject(lambda value: value["boot_integrity"]["selectors"].update({"kernel": "tampered"}))
            reject(lambda value: value["boot_integrity"].update({"protected_paths": []}))
            reject(lambda value: value["boot_integrity"].update({"changed_paths": ["boot/Image"]}))
            reject(lambda value: value.update({"policy": {"name": "wrong"}}))
            reject(lambda value: value["parent"].update({"trust_manifest_sha256": "0" * 64}))
            reject(lambda value: value["setup_proof"].update({"inventory_sha256": "0" * 64}))
            reject(lambda value: value["finalizer"]["setup_tool_code"].update({"digest": "0" * 64}))
            reject(lambda value: value, lambda proof_value: proof_value["selected_boot"].update({"selected_kernel": "boot/tampered"}))

    def test_provenance_links_are_recomputed_and_tamper_evident(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            work = Path(temporary)
            provenance_path, baseline, requested, contract, contract_identity, proof, context, bundle, artifact = _fixture(work)
            manifest_digest = _sha(MANIFEST.read_bytes())

            def validate(value: dict) -> None:
                provenance_path.write_text(json.dumps(value), encoding="utf-8")
                _validate_provenance(provenance_path, ROOT, requested, contract, contract_identity, proof, context, manifest_digest, bundle, artifact)

            validate(baseline)
            mutations = [
                lambda value: value.update(source_identity="b" * 40),
                lambda value: value["runtime_mutation"]["provenance"]["payload"].update(digest="0" * 64),
                lambda value: value["runtime_mutation"]["provenance"]["finalizer"].update(tool_code_digest="0" * 64),
                lambda value: value["runtime_mutation"]["provenance"].update(changed_paths=["etc/unowned"]),
                lambda value: value["setup_mutation"]["provenance"]["setup_layer"]["source_inputs"][0].update(sha256="0" * 64),
                lambda value: value["setup_mutation"]["provenance"].update(changed_paths=["etc/unowned"]),
                lambda value: value["disk_invariants"]["pre"].update(raw_prepartition_sha256="0" * 64),
                lambda value: value["derived_image"].update(sha256="0" * 64),
                lambda value: value["packaged_artifact"].update(sha256="0" * 64),
                lambda value: value["finalizer"]["setup_tool_code"].update(digest="0" * 64),
                lambda value: value["finalizer"].update(setup_tool_code=tool_code_model(ROOT / "tools/image-respin")),
            ]
            for mutate in mutations:
                altered = copy.deepcopy(baseline)
                mutate(altered)
                with self.assertRaises(RecordError):
                    validate(altered)


if __name__ == "__main__":
    unittest.main()
