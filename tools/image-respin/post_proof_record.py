from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

try:
    from .requested_build_record import BOARDS as BUILD_BOARDS, validate_record as validate_requested
    from .disk_packaging import compression_identity
    from .current_parent import load_record as load_current_parent, parent_context as current_parent_context, validate_downloaded_directory as validate_current_parent_directory
    from .inventory import build_inventory, ensure_inventory_symlinks_contained, inventory_digest
    from .runtime_contract import MutationError, contract_for_board, load_contract, validate_declared_changed_paths
    from .provenance import RUNTIME_TOOL_IDENTITY, TOOL_IDENTITY, digest_object as provenance_digest, tool_code_model
    from .boot_neutral import load_policy
    from .setup_contract import load_contract as load_setup_contract
    from .setup_provenance import setup_tool_code_model
    from .record_documents import load_json
    from .record_hashing import canonical_bytes
    from .record_paths import identity, project_path, resolve, verify_identity
    from .record_tool_contract import tool_identity, verify_tool
    from .record_validation import RecordError, SHA_RE, require, require_keys, verify_source
except ImportError:
    from requested_build_record import BOARDS as BUILD_BOARDS, validate_record as validate_requested
    from disk_packaging import compression_identity
    from current_parent import load_record as load_current_parent, parent_context as current_parent_context, validate_downloaded_directory as validate_current_parent_directory
    from inventory import build_inventory, ensure_inventory_symlinks_contained, inventory_digest
    from runtime_contract import MutationError, contract_for_board, load_contract, validate_declared_changed_paths
    from provenance import RUNTIME_TOOL_IDENTITY, TOOL_IDENTITY, digest_object as provenance_digest, tool_code_model
    from boot_neutral import load_policy
    from setup_contract import load_contract as load_setup_contract
    from setup_provenance import setup_tool_code_model
    from record_documents import load_json
    from record_hashing import canonical_bytes
    from record_paths import identity, project_path, resolve, verify_identity
    from record_tool_contract import tool_identity, verify_tool
    from record_validation import RecordError, SHA_RE, require, require_keys, verify_source


SCHEMA = "octessera.image-respin-post-proof/v1"
TOOL_NAME = "octessera-image-respin-post-proof"
ORANGE = "orange-pi-zero-2w"
TOP_KEYS = {"schema", "schema_version", "record_kind", "result", "source", "requested_build", "parent", "runtime_bundle", "derived_artifact", "respin_provenance", "companions", "proofs", "proof_tools", "workflow", "tool"}
RESULT_KEYS = {"status", "proofs_succeeded"}
PARENT_KEYS = {"parent_record", "context"}
BUNDLE_KEYS = {"path", "entries", "sha256", "inventory_sha256"}
PROOF_KEYS = {"label", "schema", "command_template_id", "command_template", "result", "output"}
PROOF_RESULT_KEYS = {"result"}
RUNTIME_KEYS = {"proof_schema", "schema_version", "board_profile", "version", "source_identity", "parent", "payload", "mutation_contract", "finalizer", "inventories", "parent_inventory_digest", "post_inventory_digest", "changed_paths"}


PROOF_TEMPLATE = {
    "orange-image": ("orange-production", "sudo bash tools/armbian-image/verify-orange-image.sh --image {artifact} --boot-proof-mode validated-parent --boot-neutral-contract resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json --parent-image parent-assets/$(jq -r '.image.name' resources/image-parents/orange-pi-zero-2w-current.json) --parent-record resources/image-parents/orange-pi-zero-2w-current.json --respin-provenance {artifact}.provenance.json --derivation-kind runtime-only --output {proof_output}", ORANGE + "/production-image-proof/v2"),
}
ORANGE_TOOLS = ("tools/armbian-image/verify-orange-image.sh", "tools/armbian-image/verify-orange-image.py", "tools/armbian-image/orange_boot_contract.py", "tools/armbian-image/orange_boot_inventory.py", "tools/armbian-image/orange_boot_selection.py", "tools/armbian-image/orange_image_mount.py", "tools/armbian-image/orange_initramfs.py", "tools/armbian-image/orange_phase5_proof.py", "tools/armbian-image/orange_trusted_parent_proof.py", "tools/armbian-image/verify_runtime_account.py", "tools/image-respin/boot_neutral.py", "tools/image-respin/current_parent.py", "resources/image-construction/boot-layers/orange-pi-zero-2w.json", "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json", "tools/kernel-patches/orange-midi-interface-manifest.json")


def _read_proof(path: Path, board: str) -> dict[str, Any] | None:
    require(board == ORANGE, "Raspberry has no current parent record")
    try:
        proof = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RecordError(f"structured Orange proof is unreadable: {path}") from error
    require_keys(proof, {"schema", "schema_version", "proof_mode", "phase5_claim", "boot_state", "artifact", "board_profile", "runtime", "parent", "selected_boot", "contract", "respin_provenance_sha256"}, "Orange proof")
    require(proof["schema"] == "octessera.image-proof/v2" and proof["schema_version"] == 2 and proof["proof_mode"] == "validated-parent" and proof["phase5_claim"] is False and proof["boot_state"] == "current-parent-preserved" and proof["board_profile"] == ORANGE, "Orange structured proof identity changed")
    return proof


def _bundle_identity(path: Path, root: Path) -> dict[str, Any]:
    candidate, relative = project_path(path, root)
    require(candidate.is_dir() and not candidate.is_symlink(), f"runtime bundle is not a real directory: {path}")
    entries = sorted(candidate.iterdir(), key=lambda item: item.name)
    require([item.name for item in entries] == ["SHA256SUMS", "octessera-pi", "octessera-runtime.json"], "runtime bundle entries are not exact")
    files = [identity(item, root) for item in entries]
    bundle_inventory = build_inventory(candidate)
    ensure_inventory_symlinks_contained(candidate, bundle_inventory)
    return {"path": relative, "entries": files, "sha256": hashlib.sha256(canonical_bytes(files)).hexdigest(), "inventory_sha256": inventory_digest(bundle_inventory)}


def _companion_records(companions: list[Path], root: Path, record: dict[str, Any], board: str) -> list[dict[str, Any]]:
    require(board == ORANGE, "Raspberry has no current parent record")
    expected_names = set(record["artifact"]["entries"])
    require(bool(companions) and all(path.parent == companions[0].parent for path in companions), "current parent companions are not co-located")
    validate_current_parent_directory(companions[0].parent, record)
    records = [identity(path, root) for path in companions]
    require(len(records) == len(expected_names) and {Path(item["path"]).name for item in records} == expected_names, "companion set is not exact")
    return sorted(records, key=lambda item: item["path"])


def _expected_labels(board: str) -> set[str]:
    require(board == ORANGE, "Raspberry has no current parent record")
    return {"orange-image"}


def _proof_records(root: Path, board: str, outputs: dict[str, Path], template_ids: dict[str, str]) -> list[dict[str, Any]]:
    labels = _expected_labels(board)
    require(set(outputs) == labels and set(template_ids) == labels, "proof output set is not exact")
    records = []
    for label in sorted(labels):
        expected_id, command, schema = PROOF_TEMPLATE[label]
        require(template_ids[label] == expected_id, f"proof template ID is not exact: {label}")
        structured = _read_proof(outputs[label], board)
        result = {"result": "success"} if structured is None else {"result": "success", "proof": structured}
        records.append({"label": label, "schema": schema, "command_template_id": expected_id, "command_template": command, "result": result, "output": identity(outputs[label], root)})
    return records


def _sha(value: Any, label: str) -> None:
    require(isinstance(value, str) and SHA_RE.fullmatch(value) is not None, f"{label} is not a SHA-256 digest")


def _validate_orange_provenance(document: dict[str, Any], root: Path, source: dict[str, Any], parent: dict[str, Any], parent_record_digest: str, bundle: dict[str, Any], artifact: dict[str, Any], provenance_identity: dict[str, Any], proof: dict[str, Any]) -> None:
    policy = load_policy(root)
    expected = set(policy.contract["respin_provenance"]["top_level_keys"])
    is_setup = proof["runtime"]["derivation_kind"] == "setup-portal"
    if is_setup:
        expected |= set(policy.contract["respin_provenance"]["setup_top_level_additions"])
    require(set(document) == expected, "Orange respin provenance keys are not exact")
    require(document["proof_schema"] in {policy.contract["respin_provenance"]["runtime_schema"], policy.contract["respin_provenance"]["setup_schema"]} and document["schema_version"] == 2, "Orange respin provenance schema changed")
    derivation_kind = document["derivation_kind"]
    require(derivation_kind in {"runtime-only", "setup-portal"} and (("setup_mutation" in document) == (derivation_kind == "setup-portal")), "Orange derivation kind is not exact")
    require(document["proof_mode"] == policy.proof_mode and document["board_profile"] == ORANGE and document["version"] == source["version"] and document["source_identity"] == source["sha"] and document["boot_mutation"] is False and document["phase5_claim"] is False and document["policy"] == policy.policy, "Orange boot-neutral provenance identity changed")
    parent_document = require_keys(document["parent"], {"record", "context", "image", "digest"}, "Orange provenance parent")
    expected_record = {"path": policy.contract["parent_record"], "sha256": parent_record_digest, "size": parent["parent_record"]["size"]}
    require(parent_document["record"] == expected_record and parent_document["context"] == parent["context"] and parent_document["image"] == parent["context"]["image"] and parent_document["digest"] == provenance_digest({"context": parent["context"], "record": expected_record, "image": parent["context"]["image"]}), "Orange provenance parent changed")
    runtime = require_keys(document["runtime_mutation"], {"digest", "provenance"}, "Orange runtime mutation")
    runtime_value = require_keys(runtime["provenance"], RUNTIME_KEYS, "Orange runtime provenance")
    require(runtime["digest"] == provenance_digest(runtime_value) and runtime_value["proof_schema"] == "octessera.image-mutation-provenance.v2" and runtime_value["schema_version"] == 2 and runtime_value["board_profile"] == ORANGE and runtime_value["version"] == source["version"] and runtime_value["source_identity"] == source["sha"], "Orange runtime provenance changed")
    runtime_parent = require_keys(runtime_value["parent"], {"identity", "digest"}, "Orange runtime parent")
    parent_identity = require_keys(runtime_parent["identity"], {"board_profile", "prior_version", "prior_release_entries", "prior_release_digest", "prior_state_preimage_sha256", "prior_build_metadata_preimage_sha256", "current_target", "parent_context", "parent_context_sha256"}, "Orange runtime parent identity")
    require(runtime_parent["digest"] == provenance_digest(parent_identity) and parent_identity["board_profile"] == ORANGE and parent_identity["prior_version"] == parent["context"]["version"] and parent_identity["parent_context"] == parent["context"], "Orange runtime parent identity changed")
    require(parent_identity["parent_context_sha256"] == provenance_digest(parent["context"]) and set(parent_identity["prior_release_entries"]) == {"octessera-pi", "octessera-runtime.json", "SHA256SUMS", "update-manifest.json"} and isinstance(parent_identity["prior_state_preimage_sha256"], str) and bool(SHA_RE.fullmatch(parent_identity["prior_state_preimage_sha256"])) and isinstance(parent_identity["prior_build_metadata_preimage_sha256"], str), "Orange runtime parent release identity changed")
    require(runtime_value["payload"] == {"digest": bundle["inventory_sha256"]}, "Orange runtime bundle identity changed")
    contract_identity = identity(root / "resources/image-mutations/orange-pi-zero-2w.json", root)
    require(runtime_value["mutation_contract"] == {"digest": contract_identity["sha256"]}, "Orange runtime contract identity changed")
    inventories = require_keys(runtime_value["inventories"], {"pre", "post"}, "Orange runtime inventories")
    _sha(inventories["pre"], "Orange runtime pre-inventory")
    _sha(inventories["post"], "Orange runtime post-inventory")
    require(runtime_value["parent_inventory_digest"] == inventories["pre"] and runtime_value["post_inventory_digest"] == inventories["post"], "Orange runtime inventory aliases changed")
    current_tool = tool_code_model(root / "tools/image-respin")
    runtime_finalizer = require_keys(runtime_value["finalizer"], {"source_identity", "tool_identity", "tool_code_schema", "tool_code_version", "tool_code_digest", "tool_code_files"}, "Orange runtime finalizer")
    require(runtime_finalizer["source_identity"] == source["sha"] and runtime_finalizer["tool_identity"] == RUNTIME_TOOL_IDENTITY and runtime_finalizer["tool_code_schema"] == current_tool["schema"] and runtime_finalizer["tool_code_version"] == current_tool["version"] and runtime_finalizer["tool_code_digest"] == current_tool["digest"] and runtime_finalizer["tool_code_files"] == current_tool["files"], "Orange runtime tool identity changed")
    try:
        runtime_contract, _ = load_contract(contract_for_board(ORANGE))
        changed = validate_declared_changed_paths(runtime_value["changed_paths"], runtime_contract, parent["context"]["version"], source["version"])
    except MutationError as error:
        raise RecordError(str(error)) from error
    protected = policy.contract["protected_paths"]
    require(not any(path == item or path.startswith(f"{item}/") or item.startswith(f"{path}/") for path in changed for item in protected), "Orange runtime changed a protected boot path")
    boot = require_keys(document["boot_integrity"], set(policy.contract["respin_provenance"]["boot_integrity_keys"]), "Orange boot integrity")
    for inventory in (boot["pre"], boot["post"]):
        item = require_keys(inventory, set(policy.contract["respin_provenance"]["inventory_keys"]), "Orange boot inventory")
        _sha(item["digest"], "Orange boot inventory digest")
        require(type(item["count"]) is int and item["count"] >= 0, "Orange boot inventory count changed")
    require(boot["pre"] == boot["post"] and boot["protected_scopes"] == policy.contract["protected_scopes"] and boot["protected_paths"] == protected and boot["expected_absent_paths"] == policy.contract["expected_absent_paths"] and boot["changed_paths"] == [], "Orange boot integrity changed")
    require({key: boot[key] for key in ("selected_kernel", "selected_initramfs", "selected_dtb")} == {key: proof["selected_boot"][key] for key in ("selected_kernel", "selected_initramfs", "selected_dtb")} and set(boot["selectors"]) == set(policy.contract["respin_provenance"]["selector_keys"]), "Orange selected boot identity changed")
    disk = require_keys(document["disk_invariants"], {"pre", "post", "digest"}, "Orange disk invariants")
    require(disk["pre"] == disk["post"] and disk["digest"] == provenance_digest({"pre": disk["pre"], "post": disk["post"]}) and disk["pre"]["board_profile"] == ORANGE and len(disk["pre"]["partitions"]) == 1 and [item["filesystem_type"] for item in disk["pre"]["partitions"]] == ["ext4"] and disk["pre"]["raw_boot_partition_sha256"] is None, "Orange disk identity changed")
    derived = require_keys(document["derived_image"], {"sha256", "size"}, "Orange derived image")
    artifact_record = require_keys(document["packaged_artifact"], {"sha256", "size", "path"}, "Orange packaged artifact")
    require(artifact_record == {"sha256": artifact["sha256"], "size": artifact["size"], "path": Path(artifact["path"]).name} and isinstance(derived["size"], int), "Orange artifact identity changed")
    finalizer = document["finalizer"]
    if derivation_kind == "runtime-only":
        require_keys(finalizer, {"tool_identity", "compression_identity", "runtime_tool_code_schema", "runtime_tool_code_version", "runtime_tool_code_digest", "runtime_tool_code_files"}, "Orange runtime finalizer")
        require(finalizer["tool_identity"] == TOOL_IDENTITY and finalizer["compression_identity"] == compression_identity(ORANGE) and finalizer["runtime_tool_code_digest"] == current_tool["digest"] and finalizer["runtime_tool_code_files"] == current_tool["files"], "Orange runtime finalizer changed")
    else:
        setup_tool = setup_tool_code_model(root / "tools/image-respin")
        require_keys(finalizer, {"tool_identity", "compression_identity", "setup_tool_code"}, "Orange setup finalizer")
        require(finalizer["compression_identity"] == compression_identity(ORANGE) and finalizer["setup_tool_code"] == setup_tool, "Orange setup finalizer changed")
        setup = require_keys(document["setup_mutation"], {"digest", "provenance"}, "Orange setup mutation")
        setup_value = require_keys(setup["provenance"], {"proof_schema", "schema_version", "board_profile", "source_identity", "parent", "setup_layer", "inventories", "changed_paths", "finalizer"}, "Orange setup provenance")
        require(setup["digest"] == provenance_digest(setup_value) and setup_value["board_profile"] == ORANGE and setup_value["source_identity"] == source["sha"], "Orange setup mutation changed")
        require(not any(path == item or path.startswith(f"{item}/") or item.startswith(f"{path}/") for path in setup_value["changed_paths"] for item in protected), "Orange setup changed a protected boot path")
        setup_proof = require_keys(document["setup_proof"], {"proof", "schema_version", "board_profile", "contract_sha256", "inventory_sha256", "prerequisites", "verified_paths"}, "Orange setup proof")
        require(setup_proof["proof"] == "setup-layer-mounted" and setup_proof["schema_version"] == 1 and setup_proof["board_profile"] == ORANGE and setup_proof["contract_sha256"] == identity(root / "resources/image-mutations/orange-pi-zero-2w-setup.json", root)["sha256"], "Orange setup proof changed")
        setup_contract, setup_digest = load_setup_contract(root / "resources/image-mutations/orange-pi-zero-2w-setup.json")
        require(setup_digest == setup_proof["contract_sha256"] and all(value is False for key, value in setup_contract["recipe"].items() if key.endswith("_mutation")), "Orange setup contract permits boot mutation")
    require(proof["artifact"] == {"name": Path(artifact["path"]).name, "sha256": artifact["sha256"], "size": artifact["size"]} and proof["runtime"] == {"derivation_kind": derivation_kind, "setup_proof": derivation_kind == "setup-portal", "boot_mutation": False}, "Orange structured proof is not bound")
    require(proof["parent"] == {"record": parent["parent_record"], "image": parent["context"]["image"]}, "Orange structured parent proof changed")
    require(proof["contract"] == {"path": "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json", "sha256": policy.sha256} and proof["respin_provenance_sha256"] == provenance_identity["sha256"], "Orange structured policy/provenance binding changed")


def _validate_respin_provenance(path: Path, root: Path, source: dict[str, Any], parent: dict[str, Any], parent_record_digest: str, bundle: dict[str, Any], artifact: dict[str, Any], provenance_identity: dict[str, Any] | None = None, structured_proof: dict[str, Any] | None = None) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RecordError(f"respin provenance is invalid: {path}") from error
    if source["board"] == ORANGE:
        require(provenance_identity is not None and structured_proof is not None, "Orange structured proof binding is required")
        if provenance_identity is None or structured_proof is None:
            raise RecordError("Orange structured proof binding is required")
        policy = load_policy(root)
        require(parent["parent_record"]["path"] == policy.contract["parent_record"], "Orange current parent record path is not canonical")
        _validate_orange_provenance(document, root, source, parent, parent_record_digest, bundle, artifact, provenance_identity, structured_proof)
        return
    raise RecordError("Raspberry has no current parent record")


def build_record(*, root: Path, requested_build: Path, parent_record: Path, board: str, runtime_bundle: Path, artifact: Path, respin_provenance: Path, proof_outputs: dict[str, Path], template_ids: dict[str, str], companions: list[Path], workflow: Path) -> dict[str, Any]:
    requested = _load_requested(requested_build, root)
    require(requested["source"]["board"] == board, "post-proof board differs from requested build")
    require(board == ORANGE, "Raspberry has no current parent record")
    checked_parent, _ = load_current_parent(root, parent_record)
    parent_context = current_parent_context(root, parent_record)
    tools = ORANGE_TOOLS
    parent_record_identity = identity(parent_record, root)
    parent = {"parent_record": parent_record_identity, "context": parent_context}
    bundle_identity = _bundle_identity(runtime_bundle, root)
    artifact_identity = identity(artifact, root)
    provenance_identity = identity(respin_provenance, root)
    proof_records = _proof_records(root, board, proof_outputs, template_ids)
    structured_proof = proof_records[0]["result"].get("proof") if board == ORANGE else None
    _validate_respin_provenance(respin_provenance, root, requested["source"], parent, parent_record_identity["sha256"], bundle_identity, artifact_identity, provenance_identity, structured_proof)
    return {
        "schema": SCHEMA,
        "schema_version": 1,
        "record_kind": "post-proof",
        "result": {"status": "success", "proofs_succeeded": True},
        "source": requested["source"],
        "requested_build": identity(requested_build, root),
        "parent": parent,
        "runtime_bundle": bundle_identity,
        "derived_artifact": artifact_identity,
        "respin_provenance": provenance_identity,
        "companions": _companion_records(companions, root, checked_parent, board),
        "proofs": proof_records,
        "proof_tools": [identity(resolve(root, path), root) for path in tools],
        "workflow": identity(workflow, root),
        "tool": tool_identity(Path(__file__).resolve(), root, "tools/image-respin/post_proof_record.py", TOOL_NAME),
    }


def _load_requested(path: Path, root: Path) -> dict[str, Any]:
    record = load_json(path)
    validate_requested(record, root)
    return record


def validate_record(record: dict[str, Any], root: Path) -> None:
    require_keys(record, TOP_KEYS, "post-proof")
    verify_tool(record["tool"], Path(__file__).resolve(), root, "tools/image-respin/post_proof_record.py", TOOL_NAME)
    result = require_keys(record["result"], RESULT_KEYS, "post-proof result")
    require(result == {"status": "success", "proofs_succeeded": True}, "post-proof did not succeed")
    source = require_keys(record["source"], {"sha", "version", "board", "feature_command"}, "post-proof source")
    verify_source(source["sha"], source["version"], source["board"], BUILD_BOARDS)
    require(source["board"] == ORANGE, "Raspberry has no current parent record")
    requested_identity = verify_identity(record["requested_build"], root, "requested build")
    requested = _load_requested(resolve(root, requested_identity["path"]), root)
    require(requested["source"] == source, "post-proof source differs from requested build")
    parent = require_keys(record["parent"], PARENT_KEYS, "post-proof parent")
    parent_record_identity = verify_identity(parent["parent_record"], root, "post-proof parent record")
    parent_record_path = resolve(root, parent_record_identity["path"])
    checked_parent, _ = load_current_parent(root, parent_record_path)
    require(parent["context"] == current_parent_context(root, parent_record_path), "post-proof parent context changed")
    companion_paths = record["companions"]
    require(isinstance(companion_paths, list) and all(isinstance(item, dict) for item in companion_paths), "post-proof companions are invalid")
    for item in companion_paths:
        require_keys(item, {"path", "sha256", "size"}, "post-proof companion")
    actual_companions = _companion_records([resolve(root, item["path"]) for item in companion_paths], root, checked_parent, source["board"])
    require(sorted(companion_paths, key=lambda item: item["path"]) == actual_companions, "post-proof companion hash changed")
    bundle = require_keys(record["runtime_bundle"], BUNDLE_KEYS, "runtime bundle")
    require(_bundle_identity(resolve(root, bundle["path"]), root) == bundle, "runtime bundle changed")
    artifact = verify_identity(record["derived_artifact"], root, "derived artifact")
    provenance = verify_identity(record["respin_provenance"], root, "respin provenance")
    proof_items = record["proofs"]
    require(isinstance(proof_items, list) and all(isinstance(item, dict) for item in proof_items) and len(proof_items) == len(_expected_labels(source["board"])), "proof set is invalid")
    expected_tools = set(ORANGE_TOOLS)
    require({item["label"] for item in proof_items} == _expected_labels(source["board"]), "proof labels changed")
    structured_proof = None
    for item in proof_items:
        proof = require_keys(item, PROOF_KEYS, "proof")
        expected_id, command, schema = PROOF_TEMPLATE[proof["label"]]
        require(proof["schema"] == schema and proof["command_template_id"] == expected_id and proof["command_template"] == command, f"proof template changed: {proof['label']}")
        output = verify_identity(proof["output"], root, "proof output")
        actual_structured = _read_proof(resolve(root, output["path"]), source["board"])
        expected_result = {"result": "success"} if actual_structured is None else {"result": "success", "proof": actual_structured}
        require(proof["result"] == expected_result, "proof result is not successful")
        if actual_structured is not None:
            structured_proof = actual_structured
    _validate_respin_provenance(resolve(root, provenance["path"]), root, source, parent, parent_record_identity["sha256"], bundle, artifact, provenance, structured_proof)
    tool_items = record["proof_tools"]
    require(isinstance(tool_items, list) and all(isinstance(item, dict) for item in tool_items) and {item["path"] for item in tool_items} == expected_tools and len(tool_items) == len(expected_tools), "proof tool set changed")
    for item in tool_items:
        verify_identity(item, root, "proof tool")
    verify_identity(record["workflow"], root, "workflow")


__all__ = ["ORANGE_TOOLS", "PROOF_TEMPLATE", "RecordError", "build_record", "validate_record"]
