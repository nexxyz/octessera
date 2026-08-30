from __future__ import annotations

import json
from pathlib import Path
from typing import Any

try:
    from .post_proof_record import ORANGE_TOOLS, _bundle_identity, _companion_records, _read_proof, _validate_orange_provenance
    from .requested_build_record import validate_record as validate_requested
    from .setup_contract import load_contract
    from .current_parent import load_record as load_current_parent, parent_context as current_parent_context
    from .record_documents import load_json
    from .record_paths import identity, resolve, verify_identity
    from .record_tool_contract import tool_identity, verify_tool
    from .record_validation import RecordError, SHA_RE, require, require_keys
except ImportError:
    from post_proof_record import ORANGE_TOOLS, _bundle_identity, _companion_records, _read_proof, _validate_orange_provenance
    from requested_build_record import validate_record as validate_requested
    from setup_contract import load_contract
    from current_parent import load_record as load_current_parent, parent_context as current_parent_context
    from record_documents import load_json
    from record_paths import identity, resolve, verify_identity
    from record_tool_contract import tool_identity, verify_tool
    from record_validation import RecordError, SHA_RE, require, require_keys


SCHEMA = "octessera.image-respin-setup-post-proof/v1"
TOOL_NAME = "octessera-image-respin-setup-post-proof"
ORANGE = "orange-pi-zero-2w"
SETUP_PROOF_TOOLS = {ORANGE: ORANGE_TOOLS}
PRODUCTION_PROOF_LABELS = {ORANGE: ("orange-image",)}
INVENTORY_ENTRY_KEYS = {"path", "type", "uid", "gid", "mode", "symlink", "target", "sha256", "xattrs", "capability"}


def _document(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RecordError(f"{label} is invalid: {path}") from exc
    require(isinstance(value, dict), f"{label} is not an object")
    return value


def _sha(value: Any, label: str) -> None:
    require(isinstance(value, str) and SHA_RE.fullmatch(value) is not None, f"{label} is not a SHA-256 digest")


def _production_proof_identities(root: Path, board: str, outputs: dict[str, Path]) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    require(board == ORANGE, "Raspberry has no current parent record")
    labels = set(PRODUCTION_PROOF_LABELS[board])
    require(set(outputs) == labels, "production proof output set is not exact")
    identities: dict[str, dict[str, Any]] = {}
    structured: dict[str, Any] = {}
    for label in sorted(labels):
        proof_path = outputs[label]
        identities[label] = identity(proof_path, root)
        proof = _read_proof(proof_path, board)
        if proof is not None:
            structured[label] = proof
    return identities, structured


def _validate_production_proofs(value: Any, root: Path, board: str) -> dict[str, Any]:
    require(board == ORANGE, "Raspberry has no current parent record")
    labels = set(PRODUCTION_PROOF_LABELS[board])
    require(isinstance(value, dict) and set(value) == labels, "production proof output set is not exact")
    structured: dict[str, Any] = {}
    for label in sorted(labels):
        proof_identity = verify_identity(value[label], root, f"{board} production proof")
        proof = _read_proof(resolve(root, proof_identity["path"]), board)
        if proof is not None:
            structured[label] = proof
    return structured


def _validate_setup_proof_tools(value: Any, root: Path, board: str) -> None:
    require(board == ORANGE, "Raspberry has no current parent record")
    expected = SETUP_PROOF_TOOLS[board]
    require(isinstance(value, list) and all(isinstance(item, dict) for item in value) and {item["path"] for item in value} == set(expected) and len(value) == len(expected), "setup proof tool set changed")
    for item in value:
        verify_identity(item, root, "setup proof tool")


def _validate_prerequisites(value: Any, contract: dict[str, Any], label: str) -> dict[str, Any]:
    prerequisites = require_keys(value, {"packages_sha256", "accounts", "passwd_sha256", "group_sha256", "executables", "services"}, label)
    for field in ("packages_sha256", "passwd_sha256", "group_sha256"):
        _sha(prerequisites[field], f"{label} {field}")
    expected_accounts = {f"user:{item['user']}" for item in contract["prerequisites"]["accounts"]} | {f"group:{item['group']}" for item in contract["prerequisites"]["accounts"]}
    require(isinstance(prerequisites["accounts"], dict) and set(prerequisites["accounts"]) == expected_accounts and all(isinstance(item, str) and item for item in prerequisites["accounts"].values()), f"{label} accounts are not exact")
    require(isinstance(prerequisites["executables"], dict) and set(prerequisites["executables"]) == set(contract["prerequisites"]["executables"]), f"{label} executables are not exact")
    for path, item in prerequisites["executables"].items():
        entry = require_keys(item, INVENTORY_ENTRY_KEYS, f"{label} executable identity")
        require(entry["path"] == path and entry["type"] in {"file", "symlink"} and entry["uid"] == 0 and entry["gid"] == 0 and isinstance(entry["xattrs"], dict), f"{label} executable identity is invalid")
    require(isinstance(prerequisites["services"], dict) and set(prerequisites["services"]) == set(contract["prerequisites"]["services"]) and all(isinstance(item, str) and item for item in prerequisites["services"].values()), f"{label} services are not exact")
    return prerequisites


def _validate_proof(proof: dict[str, Any], board: str, contract_identity: dict[str, Any], contract: dict[str, Any]) -> None:
    require(set(proof) == {"proof", "schema_version", "board_profile", "contract_sha256", "inventory_sha256", "prerequisites", "verified_paths"}, "setup proof keys are not exact")
    require(proof["proof"] == "setup-layer-mounted" and proof["schema_version"] == 1 and proof["board_profile"] == board and proof["contract_sha256"] == contract_identity["sha256"], "setup proof identity is not exact")
    _sha(proof["inventory_sha256"], "setup proof inventory")
    _validate_prerequisites(proof["prerequisites"], contract, "setup proof prerequisites")
    expected_paths = sorted(
        [item["target"] for item in contract["directories"]]
        + [item["target"] for item in contract["entries"]]
        + [item["target"] for item in contract["symlinks"] if item["postimage"] == "absent"]
    )
    require(proof["verified_paths"] == expected_paths, "setup proof paths are not exact")


def _validate_provenance(path: Path, root: Path, requested: dict[str, Any], contract: dict[str, Any], contract_identity: dict[str, Any], proof: dict[str, Any], parent_context: dict[str, Any], parent_record_digest: str, bundle: dict[str, Any], artifact: dict[str, Any], orange_proof: dict[str, Any] | None = None) -> dict[str, Any]:
    value = _document(path, "setup provenance")
    require(all(item is False for key, item in contract["recipe"].items() if key.endswith("_mutation")), "setup contract permits mutation")
    require(requested["source"]["board"] == ORANGE, "Raspberry has no current parent record")
    if orange_proof is None:
        raise RecordError("Orange setup proof binding is required")
    parent = {"parent_record": {"path": "resources/image-parents/orange-pi-zero-2w-current.json", "sha256": parent_record_digest, "size": (root / "resources/image-parents/orange-pi-zero-2w-current.json").stat().st_size}, "context": parent_context}
    _validate_orange_provenance(value, root, requested["source"], parent, parent_record_digest, bundle, artifact, identity(path, root), orange_proof)
    return value


def build_record(*, root: Path, requested_build: Path, parent_record: Path, board: str, runtime_bundle: Path, artifact: Path, respin_provenance: Path, setup_proof: Path, production_proofs: dict[str, Path], companions: list[Path], workflow: Path) -> dict[str, Any]:
    requested = load_json(requested_build)
    validate_requested(requested, root)
    setup_identity = require_keys(requested.get("setup"), {"mode", "contract", "inputs", "tool_files"}, "requested setup layer")
    require(setup_identity["mode"] == "setup-portal" and requested["source"]["board"] == board, "requested setup build is not exact")
    contract_identity = verify_identity(setup_identity["contract"], root, "setup contract")
    contract, _ = load_contract(resolve(root, contract_identity["path"]))
    require(board == ORANGE, "Raspberry has no current parent record")
    checked, _ = load_current_parent(root, parent_record)
    parent = current_parent_context(root, parent_record)
    parent_record_identity = identity(parent_record, root)
    proof = _document(setup_proof, "setup proof")
    _validate_proof(proof, board, contract_identity, contract)
    production_proof_identities, structured_proofs = _production_proof_identities(root, board, production_proofs)
    orange_proof_value = structured_proofs.get("orange-image")
    bundle = _bundle_identity(runtime_bundle, root)
    artifact_identity = identity(artifact, root)
    _validate_provenance(respin_provenance, root, requested, contract, contract_identity, proof, parent, parent_record_identity["sha256"], bundle, artifact_identity, orange_proof_value)
    proof_tools = [identity(resolve(root, path), root) for path in SETUP_PROOF_TOOLS[board]]
    return {"schema": SCHEMA, "schema_version": 1, "record_kind": "setup-post-proof", "result": {"status": "success", "setup_proof_succeeded": True}, "source": requested["source"], "requested_build": identity(requested_build, root), "parent": {"context": parent, "parent_record": parent_record_identity}, "runtime_bundle": bundle, "derived_artifact": artifact_identity, "setup_provenance": identity(respin_provenance, root), "setup_proof": identity(setup_proof, root), "production_proofs": production_proof_identities, "proof_tools": proof_tools, "companions": _companion_records(companions, root, checked, board), "workflow": identity(workflow, root), "tool": tool_identity(Path(__file__).resolve(), root, "tools/image-respin/setup_workflow_record.py", TOOL_NAME)}


def validate_record(record: dict[str, Any], root: Path) -> None:
    top = require_keys(record, {"schema", "schema_version", "record_kind", "result", "source", "requested_build", "parent", "runtime_bundle", "derived_artifact", "setup_provenance", "setup_proof", "production_proofs", "proof_tools", "companions", "workflow", "tool"}, "setup post-proof")
    require(top["schema"] == SCHEMA and top["schema_version"] == 1 and top["record_kind"] == "setup-post-proof" and top["result"] == {"status": "success", "setup_proof_succeeded": True}, "setup post-proof identity is not exact")
    verify_tool(top["tool"], Path(__file__).resolve(), root, "tools/image-respin/setup_workflow_record.py", TOOL_NAME)
    requested_identity = verify_identity(top["requested_build"], root, "requested build")
    requested = load_json(resolve(root, requested_identity["path"]))
    validate_requested(requested, root)
    source = require_keys(top["source"], {"sha", "version", "board", "feature_command"}, "setup post-proof source")
    require(requested["source"] == source and requested.get("setup", {}).get("mode") == "setup-portal", "setup post-proof source changed")
    require(source["board"] == ORANGE, "Raspberry has no current parent record")
    parent_record = require_keys(top["parent"], {"context", "parent_record"}, "setup post-proof parent")
    parent_identity = verify_identity(parent_record["parent_record"], root, "parent record")
    checked, _ = load_current_parent(root, resolve(root, parent_identity["path"]))
    require(parent_record["context"] == current_parent_context(root, resolve(root, parent_identity["path"])), "setup post-proof parent changed")
    contract_identity = verify_identity(requested["setup"]["contract"], root, "setup contract")
    contract, _ = load_contract(resolve(root, contract_identity["path"]))
    bundle_record = require_keys(top["runtime_bundle"], {"path", "entries", "sha256", "inventory_sha256"}, "runtime bundle")
    bundle = _bundle_identity(resolve(root, bundle_record["path"]), root)
    require(bundle == bundle_record, "runtime bundle identity changed")
    artifact = verify_identity(top["derived_artifact"], root, "derived setup artifact")
    proof_identity = verify_identity(top["setup_proof"], root, "setup proof")
    proof = _document(resolve(root, proof_identity["path"]), "setup proof")
    _validate_proof(proof, source["board"], contract_identity, contract)
    structured_proofs = _validate_production_proofs(top["production_proofs"], root, source["board"])
    orange_proof_value = structured_proofs.get("orange-image")
    provenance_identity = verify_identity(top["setup_provenance"], root, "setup provenance")
    _validate_provenance(resolve(root, provenance_identity["path"]), root, requested, contract, contract_identity, proof, parent_record["context"], parent_identity["sha256"], bundle, artifact, orange_proof_value)
    _validate_setup_proof_tools(top["proof_tools"], root, source["board"])
    require(isinstance(top["companions"], list), "setup companion records are invalid")
    actual_companions = _companion_records([resolve(root, item["path"]) for item in top["companions"]], root, checked, source["board"])
    require(actual_companions == top["companions"], "setup companion identities changed")
    verify_identity(top["workflow"], root, "workflow")


__all__ = ["RecordError", "build_record", "validate_record"]
