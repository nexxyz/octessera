from __future__ import annotations

import fnmatch
import hashlib
import json
import lzma
import shutil
import tempfile
import zipfile
from contextlib import contextmanager
from pathlib import Path
from collections.abc import Iterator
from typing import Any

try:
    from .disk_packaging import compression_identity, file_digest
    from .post_proof_record import ORANGE_TOOLS, RPI_TOOLS, RUNTIME_KEYS, _bundle_identity, _companion_records, _read_proof, _validate_notice, _validate_orange_provenance
    from .provenance import RUNTIME_TOOL_IDENTITY, digest_object, tool_code_model
    from .requested_build_record import validate_record as validate_requested
    from .runtime_contract import _classify, load_contract as load_runtime_contract
    from .setup_contract import load_contract, source_path
    from .setup_mutation import SETUP_TOOL_IDENTITY
    from .setup_provenance import setup_tool_code_model
    from .trust_manifest import load_manifest, parent_context_for_board
    from .record_documents import load_json
    from .record_paths import identity, resolve, verify_identity
    from .record_tool_contract import tool_identity, verify_tool
    from .record_validation import RecordError, SHA_RE, require, require_keys
except ImportError:
    from disk_packaging import compression_identity, file_digest
    from post_proof_record import ORANGE_TOOLS, RPI_TOOLS, RUNTIME_KEYS, _bundle_identity, _companion_records, _read_proof, _validate_notice, _validate_orange_provenance
    from provenance import RUNTIME_TOOL_IDENTITY, digest_object, tool_code_model
    from requested_build_record import validate_record as validate_requested
    from runtime_contract import _classify, load_contract as load_runtime_contract
    from setup_contract import load_contract, source_path
    from setup_mutation import SETUP_TOOL_IDENTITY
    from setup_provenance import setup_tool_code_model
    from trust_manifest import load_manifest, parent_context_for_board
    from record_documents import load_json
    from record_paths import identity, resolve, verify_identity
    from record_tool_contract import tool_identity, verify_tool
    from record_validation import RecordError, SHA_RE, require, require_keys


SCHEMA = "octessera.image-respin-setup-post-proof/v1"
TOOL_NAME = "octessera-image-respin-setup-post-proof"
RPI = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
SETUP_PROOF_TOOLS = {ORANGE: ORANGE_TOOLS, RPI: RPI_TOOLS}
PRODUCTION_PROOF_LABELS = {ORANGE: ("orange-image",), RPI: ("raspberry-sanitized",)}
PROVENANCE_KEYS = {"proof_schema", "schema_version", "board_profile", "version", "source_identity", "parent", "runtime_mutation", "setup_mutation", "setup_proof", "disk_invariants", "derived_image", "packaged_artifact", "finalizer"}
RUNTIME_KEYS = {"proof_schema", "schema_version", "board_profile", "version", "source_identity", "parent", "payload", "mutation_contract", "finalizer", "inventories", "parent_inventory_digest", "post_inventory_digest", "notice", "changed_paths"}
RUNTIME_PARENT_KEYS = {"board_profile", "prior_version", "prior_release_entries", "prior_release_digest", "prior_state_preimage_sha256", "prior_build_metadata_preimage_sha256", "current_target", "parent_context", "parent_context_sha256"}
LAYOUT_KEYS = {"board_profile", "image_size", "table_label", "disk_id", "first_lba", "last_lba", "sector_size", "partitions", "raw_prepartition_sha256", "raw_boot_partition_sha256"}
PARTITION_KEYS = {"index", "start", "size", "partition_type", "partition_uuid", "filesystem_type", "filesystem_uuid", "filesystem_label"}
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
    expected = SETUP_PROOF_TOOLS[board]
    require(isinstance(value, list) and all(isinstance(item, dict) for item in value) and {item["path"] for item in value} == set(expected) and len(value) == len(expected), "setup proof tool set changed")
    for item in value:
        verify_identity(item, root, "setup proof tool")


def _validate_layout(value: Any, board: str, label: str) -> None:
    layout = require_keys(value, LAYOUT_KEYS, label)
    require(layout["board_profile"] == board, f"{label} board differs from setup source")
    for field in ("image_size", "first_lba", "last_lba", "sector_size"):
        require(type(layout[field]) is int and layout[field] >= 0, f"{label} {field} is invalid")
    require(layout["image_size"] > 0 and layout["first_lba"] <= layout["last_lba"] and layout["sector_size"] > 0, f"{label} geometry is invalid")
    require(bool(isinstance(layout["table_label"], str) and layout["table_label"]), f"{label} table label is invalid")
    require(layout["disk_id"] is None or isinstance(layout["disk_id"], str), f"{label} disk identity is invalid")
    _sha(layout["raw_prepartition_sha256"], f"{label} raw pre-partition region")
    if board == RPI:
        _sha(layout["raw_boot_partition_sha256"], f"{label} raw boot region")
    else:
        require(layout["raw_boot_partition_sha256"] is None, f"{label} Orange raw boot region is not exact")
    partitions = layout["partitions"]
    expected_filesystems = ["vfat", "ext4"] if board == RPI else ["ext4"]
    require(isinstance(partitions, list) and len(partitions) == len(expected_filesystems), f"{label} partition count is invalid")
    previous_end = -1
    for index, raw_partition in enumerate(partitions, 1):
        partition = require_keys(raw_partition, PARTITION_KEYS, f"{label} partition")
        require(partition["index"] == index and type(partition["start"]) is int and type(partition["size"]) is int, f"{label} partition geometry is invalid")
        require(partition["start"] >= 0 and partition["size"] > 0 and partition["start"] >= previous_end, f"{label} partitions overlap")
        previous_end = partition["start"] + partition["size"]
        require(bool(isinstance(partition["partition_type"], str) and partition["partition_type"]), f"{label} partition type is invalid")
        require(partition["filesystem_type"] == expected_filesystems[index - 1], f"{label} filesystem identity is invalid")
        for field in ("partition_uuid", "filesystem_uuid", "filesystem_label"):
            require(partition[field] is None or isinstance(partition[field], str), f"{label} partition metadata is invalid")


@contextmanager
def _unpacked_image(artifact: Path, board: str) -> Iterator[Path]:
    try:
        with tempfile.TemporaryDirectory(prefix="octessera-setup-proof-") as temporary:
            image = Path(temporary) / "derived.img"
            if board == ORANGE:
                with lzma.open(artifact, "rb") as source, image.open("wb") as destination:
                    shutil.copyfileobj(source, destination, 1024 * 1024)
            else:
                with zipfile.ZipFile(artifact, "r") as archive:
                    entries = archive.infolist()
                    images = [entry for entry in entries if entry.filename.endswith(".img")]
                    require(len(entries) == 1 and len(images) == 1, "Raspberry setup artifact members are not exact")
                    entry = images[0]
                    require(not entry.filename.startswith("/") and "\\" not in entry.filename and all(part not in {"", ".", ".."} for part in Path(entry.filename).parts), "Raspberry setup artifact member path is unsafe")
                    require(not entry.is_dir() and ((entry.external_attr >> 16) & 0o170000) != 0o120000, "Raspberry setup artifact image is not regular")
                    with archive.open(entry, "r") as source, image.open("wb") as destination:
                        shutil.copyfileobj(source, destination, 1024 * 1024)
            yield image
    except (OSError, EOFError, lzma.LZMAError, zipfile.BadZipFile) as exc:
        raise RecordError(f"cannot unpack setup artifact: {artifact}") from exc


def _image_region_digest(image: Path, start: int, length: int) -> str:
    digest = hashlib.sha256()
    remaining = length
    with image.open("rb") as handle:
        handle.seek(start)
        while remaining:
            chunk = handle.read(min(1024 * 1024, remaining))
            require(bool(chunk), "disk invariant region is truncated")
            digest.update(chunk)
            remaining -= len(chunk)
    return digest.hexdigest()


def _validate_image_regions(image: Path, layout: dict[str, Any], board: str, label: str) -> None:
    first = layout["partitions"][0]
    sector_size = layout["sector_size"]
    require(_image_region_digest(image, 0, first["start"] * sector_size) == layout["raw_prepartition_sha256"], f"{label} raw pre-partition region changed")
    if board == RPI:
        require(_image_region_digest(image, first["start"] * sector_size, first["size"] * sector_size) == layout["raw_boot_partition_sha256"], f"{label} raw boot region changed")


def _validate_runtime_provenance(value: Any, root: Path, source: dict[str, Any], parent: dict[str, Any], manifest_digest: str, bundle: dict[str, Any]) -> None:
    runtime = require_keys(value, {"digest", "provenance"}, "runtime mutation")
    runtime_value = require_keys(runtime["provenance"], RUNTIME_KEYS, "runtime provenance")
    require(runtime["digest"] == digest_object(runtime_value), "runtime provenance digest changed")
    require(runtime_value["proof_schema"] == "octessera.image-mutation-provenance.v2" and runtime_value["schema_version"] == 2 and runtime_value["board_profile"] == source["board"] and runtime_value["version"] == source["version"] and runtime_value["source_identity"] == source["sha"], "runtime provenance source changed")
    _validate_notice(runtime_value["notice"], root)
    runtime_parent = require_keys(runtime_value["parent"], {"identity", "digest"}, "runtime provenance parent")
    parent_identity = require_keys(runtime_parent["identity"], RUNTIME_PARENT_KEYS, "runtime parent identity")
    expected_entries = {"octessera-pi", "update-manifest.json"} if source["board"] == RPI else {"octessera-pi", "octessera-runtime.json", "SHA256SUMS"}
    require(parent_identity["board_profile"] == source["board"] and parent_identity["parent_context"] == parent["context"] and parent_identity["prior_version"] == "0.7.5", "runtime parent context changed")
    require(isinstance(parent_identity["prior_release_entries"], dict) and set(parent_identity["prior_release_entries"]) == expected_entries, "runtime parent release identity changed")
    for entry in parent_identity["prior_release_entries"].values():
        _sha(entry, "runtime parent release entry")
    _sha(parent_identity["prior_release_digest"], "runtime parent release")
    if source["board"] == RPI:
        _sha(parent_identity["prior_state_preimage_sha256"], "runtime parent state")
        require(parent_identity["prior_build_metadata_preimage_sha256"] is None, "runtime parent metadata identity changed")
    else:
        require(parent_identity["prior_state_preimage_sha256"] is None, "runtime parent state identity changed")
        _sha(parent_identity["prior_build_metadata_preimage_sha256"], "runtime parent metadata")
    require(bool(isinstance(parent_identity["current_target"], str) and parent_identity["current_target"].strip()), "runtime parent current target is invalid")
    require(parent_identity["parent_context_sha256"] == digest_object(parent["context"]), "runtime parent context digest changed")
    require(runtime_parent["digest"] == digest_object(parent_identity), "runtime parent digest changed")
    payload = require_keys(runtime_value["payload"], {"digest"}, "runtime payload")
    require(payload["digest"] == bundle["inventory_sha256"], "runtime payload does not match bundle inventory")
    _sha(payload["digest"], "runtime payload")
    mutation_contract = require_keys(runtime_value["mutation_contract"], {"digest"}, "runtime mutation contract")
    runtime_contract_path = root / "resources/image-mutations" / f"{source['board']}.json"
    runtime_contract, runtime_contract_digest = load_runtime_contract(runtime_contract_path)
    require(mutation_contract["digest"] == runtime_contract_digest, "runtime mutation contract changed")
    inventories = require_keys(runtime_value["inventories"], {"pre", "post"}, "runtime inventories")
    _sha(inventories["pre"], "runtime pre-inventory")
    _sha(inventories["post"], "runtime post-inventory")
    require(runtime_value["parent_inventory_digest"] == inventories["pre"] and runtime_value["post_inventory_digest"] == inventories["post"], "runtime inventory aliases changed")
    finalizer = require_keys(runtime_value["finalizer"], {"source_identity", "tool_identity", "tool_code_schema", "tool_code_version", "tool_code_digest", "tool_code_files"}, "runtime finalizer")
    current_tool = tool_code_model(root / "tools/image-respin")
    require(finalizer["source_identity"] == source["sha"] and finalizer["tool_identity"] == RUNTIME_TOOL_IDENTITY and finalizer["tool_code_schema"] == current_tool["schema"] and finalizer["tool_code_version"] == current_tool["version"] and finalizer["tool_code_digest"] == current_tool["digest"] and finalizer["tool_code_files"] == current_tool["files"], "runtime tool code changed")
    changed = runtime_value["changed_paths"]
    require(isinstance(changed, list) and changed == sorted(set(changed)) and all(isinstance(path, str) and path and not path.startswith("/") and "\\" not in path for path in changed), "runtime changed paths are not exact")
    notice_paths = set(runtime_value["notice"]["changed_paths"])
    global_notice = {path for path in changed if path == "usr/share/doc/octessera" or path.startswith("usr/share/doc/octessera/")}
    require(global_notice == notice_paths, "notice paths are not the exact runtime subset")
    prior = parent_identity["prior_version"]
    for path in changed:
        if path in notice_paths:
            continue
        require(not any(fnmatch.fnmatchcase(path, pattern) for pattern in runtime_contract["mutation_contract"]["forbidden"]), f"runtime forbidden path changed: {path}")
        require(_classify(path, f'{runtime_contract["managed"]["releases"]}/{prior}', f'{runtime_contract["managed"]["releases"]}/{source["version"]}', runtime_contract, prior == source["version"]) is not None, f"runtime unauthorized path changed: {path}")


def _validate_setup_mutation(value: Any, root: Path, source: dict[str, Any], contract: dict[str, Any], contract_identity: dict[str, Any], proof: dict[str, Any]) -> None:
    setup = require_keys(value, {"digest", "provenance"}, "setup mutation")
    setup_value = require_keys(setup["provenance"], {"proof_schema", "schema_version", "board_profile", "source_identity", "parent", "setup_layer", "inventories", "changed_paths", "finalizer"}, "setup mutation provenance")
    require(setup["digest"] == digest_object(setup_value) and setup_value["proof_schema"] == "octessera.image-setup-mutation-provenance.v1" and setup_value["schema_version"] == 1 and setup_value["board_profile"] == source["board"] and setup_value["source_identity"] == source["sha"], "setup mutation provenance changed")
    parent = require_keys(setup_value["parent"], {"identity", "digest"}, "setup mutation parent")
    parent_identity = require_keys(parent["identity"], {"board_profile", "preimage_source", "prerequisites", "preimage_digest"}, "setup parent identity")
    _sha(parent_identity["preimage_digest"], "setup preimage inventory")
    require(parent_identity["board_profile"] == source["board"] and parent_identity["preimage_source"] == contract["preimage_source"] and parent["digest"] == digest_object(parent_identity), "setup parent identity changed")
    require(_validate_prerequisites(parent_identity["prerequisites"], contract, "setup parent prerequisites") == proof["prerequisites"], "setup prerequisites changed")
    layer = require_keys(setup_value["setup_layer"], {"contract_digest", "source_inputs"}, "setup provenance layer")
    require(layer["contract_digest"] == contract_identity["sha256"], "setup contract digest changed")
    expected_sources = [identity(root / item["path"], root) for item in contract["source_inputs"]]
    sources = layer["source_inputs"]
    require(isinstance(sources, list) and sources == expected_sources, "setup source input set changed")
    for item in contract["entries"]:
        source_file = source_path(contract, item["source"], root)
        source_digest, _ = file_digest(source_file)
        require(source_digest == item["sha256"], f"setup payload source changed: {item['source']}")
    inventories = require_keys(setup_value["inventories"], {"pre", "post"}, "setup inventories")
    _sha(inventories["pre"], "setup pre-inventory")
    _sha(inventories["post"], "setup post-inventory")
    require(inventories["post"] == proof["inventory_sha256"], "setup proof inventory is not linked")
    changed = setup_value["changed_paths"]
    allowed = {item["target"] for item in contract["directories"]} | {item["target"] for item in contract["entries"]} | {item["target"] for item in contract["symlinks"]} | set(contract["stale_runtime_markers"])
    required_directories = {item["target"] for item in contract["directories"]}
    require(isinstance(changed, list) and changed == sorted(set(changed)) and required_directories <= set(changed) and all(path in allowed for path in changed), "setup changed paths are not exact")
    finalizer = require_keys(setup_value["finalizer"], {"source_identity", "tool_identity", "tool_code_digest"}, "setup mutation finalizer")
    setup_tool = setup_tool_code_model(root / "tools/image-respin")
    require(finalizer["source_identity"] == source["sha"] and finalizer["tool_identity"] == SETUP_TOOL_IDENTITY and finalizer["tool_code_digest"] == setup_tool["digest"], "setup finalizer identity changed")


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
    expected_paths = sorted([item["target"] for item in contract["directories"]] + [item["target"] for item in contract["entries"]])
    require(proof["verified_paths"] == expected_paths, "setup proof paths are not exact")


def _validate_provenance(path: Path, root: Path, requested: dict[str, Any], contract: dict[str, Any], contract_identity: dict[str, Any], proof: dict[str, Any], parent_context: dict[str, Any], manifest_digest: str, bundle: dict[str, Any], artifact: dict[str, Any], orange_proof: dict[str, Any] | None = None) -> dict[str, Any]:
    value = _document(path, "setup provenance")
    require(all(item is False for key, item in contract["recipe"].items() if key.endswith("_mutation")), "setup contract permits mutation")
    if requested["source"]["board"] == ORANGE:
        if orange_proof is None:
            raise RecordError("Orange setup proof binding is required")
        _validate_orange_provenance(value, root, requested["source"], {"trust_manifest": {"path": "resources/image-parents/v0.7.5-trust-manifest.json", "sha256": manifest_digest}, "context": parent_context}, manifest_digest, bundle, artifact, identity(path, root), orange_proof)
        return value
    require_keys(value, PROVENANCE_KEYS, "setup provenance")
    source = requested["source"]
    require(value["proof_schema"] == "octessera.image-derived-setup-respin-provenance.v1" and value["schema_version"] == 1 and value["board_profile"] == source["board"] and value["version"] == source["version"] and value["source_identity"] == source["sha"], "setup provenance source changed")
    parent = require_keys(value["parent"], {"context", "trust_manifest_sha256", "digest"}, "setup provenance parent")
    require(parent["context"] == parent_context and parent["trust_manifest_sha256"] == manifest_digest and parent["digest"] == digest_object({"context": parent_context, "trust_manifest_sha256": manifest_digest}), "setup provenance parent changed")
    _sha(parent["trust_manifest_sha256"], "setup trust manifest")
    _validate_runtime_provenance(value["runtime_mutation"], root, source, {"context": parent_context}, manifest_digest, bundle)
    _validate_setup_mutation(value["setup_mutation"], root, source, contract, contract_identity, proof)
    require(value["setup_proof"] == proof, "setup proof changed between generation and record")
    _validate_proof(proof, source["board"], contract_identity, contract)
    disk = require_keys(value["disk_invariants"], {"pre", "post", "digest"}, "disk invariants")
    _validate_layout(disk["pre"], source["board"], "disk pre-invariants")
    _validate_layout(disk["post"], source["board"], "disk post-invariants")
    require(disk["pre"] == disk["post"] and disk["digest"] == digest_object({"pre": disk["pre"], "post": disk["post"]}), "disk invariants changed")
    derived = require_keys(value["derived_image"], {"sha256", "size"}, "derived image")
    _sha(derived["sha256"], "derived image")
    with _unpacked_image(resolve(root, artifact["path"]), source["board"]) as image:
        image_digest, image_size = file_digest(image)
        actual_image = {"sha256": image_digest, "size": image_size}
        require(type(derived["size"]) is int and derived["size"] > 0 and derived == actual_image, "derived image identity changed")
        _validate_image_regions(image, disk["pre"], source["board"], "disk pre-invariants")
        _validate_image_regions(image, disk["post"], source["board"], "disk post-invariants")
    require(derived["size"] == disk["pre"]["image_size"] and derived["size"] == disk["post"]["image_size"], "derived image size differs from disk invariants")
    packaged = require_keys(value["packaged_artifact"], {"sha256", "size", "path"}, "packaged artifact")
    require(packaged == {"sha256": artifact["sha256"], "size": artifact["size"], "path": Path(artifact["path"]).name}, "packaged artifact identity changed")
    finalizer = require_keys(value["finalizer"], {"tool_identity", "compression_identity", "setup_tool_code"}, "setup respin finalizer")
    require(finalizer["tool_identity"] == "octessera-image-respin-runtime-mutation/1" and finalizer["compression_identity"] == compression_identity(source["board"]) and finalizer["setup_tool_code"] == setup_tool_code_model(root / "tools/image-respin"), "setup respin finalizer identity changed")
    return value


def build_record(*, root: Path, requested_build: Path, manifest: Path, board: str, runtime_bundle: Path, artifact: Path, respin_provenance: Path, setup_proof: Path, production_proofs: dict[str, Path], companions: list[Path], workflow: Path) -> dict[str, Any]:
    requested = load_json(requested_build)
    validate_requested(requested, root)
    setup_identity = require_keys(requested.get("setup"), {"mode", "contract", "inputs", "tool_files"}, "requested setup layer")
    require(setup_identity["mode"] == "setup-portal" and requested["source"]["board"] == board, "requested setup build is not exact")
    contract_identity = verify_identity(setup_identity["contract"], root, "setup contract")
    contract, _ = load_contract(resolve(root, contract_identity["path"]))
    checked = load_manifest(manifest)
    manifest_identity = identity(manifest, root)
    if board == ORANGE:
        require(manifest_identity["path"] == "resources/image-parents/v0.7.5-trust-manifest.json", "Orange trusted manifest path is not canonical")
    parent = parent_context_for_board(checked, board)
    proof = _document(setup_proof, "setup proof")
    _validate_proof(proof, board, contract_identity, contract)
    production_proof_identities, structured_proofs = _production_proof_identities(root, board, production_proofs)
    orange_proof_value = structured_proofs.get("orange-image")
    bundle = _bundle_identity(runtime_bundle, root)
    artifact_identity = identity(artifact, root)
    _validate_provenance(respin_provenance, root, requested, contract, contract_identity, proof, parent, manifest_identity["sha256"], bundle, artifact_identity, orange_proof_value)
    proof_tools = [identity(resolve(root, path), root) for path in SETUP_PROOF_TOOLS[board]]
    return {"schema": SCHEMA, "schema_version": 1, "record_kind": "setup-post-proof", "result": {"status": "success", "setup_proof_succeeded": True}, "source": requested["source"], "requested_build": identity(requested_build, root), "parent": {"context": parent, "trust_manifest": identity(manifest, root)}, "runtime_bundle": bundle, "derived_artifact": artifact_identity, "setup_provenance": identity(respin_provenance, root), "setup_proof": identity(setup_proof, root), "production_proofs": production_proof_identities, "proof_tools": proof_tools, "companions": _companion_records(companions, root, checked, board), "workflow": identity(workflow, root), "tool": tool_identity(Path(__file__).resolve(), root, "tools/image-respin/setup_workflow_record.py", TOOL_NAME)}


def validate_record(record: dict[str, Any], root: Path) -> None:
    top = require_keys(record, {"schema", "schema_version", "record_kind", "result", "source", "requested_build", "parent", "runtime_bundle", "derived_artifact", "setup_provenance", "setup_proof", "production_proofs", "proof_tools", "companions", "workflow", "tool"}, "setup post-proof")
    require(top["schema"] == SCHEMA and top["schema_version"] == 1 and top["record_kind"] == "setup-post-proof" and top["result"] == {"status": "success", "setup_proof_succeeded": True}, "setup post-proof identity is not exact")
    verify_tool(top["tool"], Path(__file__).resolve(), root, "tools/image-respin/setup_workflow_record.py", TOOL_NAME)
    requested_identity = verify_identity(top["requested_build"], root, "requested build")
    requested = load_json(resolve(root, requested_identity["path"]))
    validate_requested(requested, root)
    source = require_keys(top["source"], {"sha", "version", "board", "feature_command"}, "setup post-proof source")
    require(requested["source"] == source and requested.get("setup", {}).get("mode") == "setup-portal", "setup post-proof source changed")
    parent_record = require_keys(top["parent"], {"context", "trust_manifest"}, "setup post-proof parent")
    manifest_identity = verify_identity(parent_record["trust_manifest"], root, "trust manifest")
    if source["board"] == ORANGE:
        require(manifest_identity["path"] == "resources/image-parents/v0.7.5-trust-manifest.json", "Orange trusted manifest path is not canonical")
    checked = load_manifest(resolve(root, manifest_identity["path"]))
    require(parent_record["context"] == parent_context_for_board(checked, source["board"]), "setup post-proof parent changed")
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
    _validate_provenance(resolve(root, provenance_identity["path"]), root, requested, contract, contract_identity, proof, parent_record["context"], manifest_identity["sha256"], bundle, artifact, orange_proof_value)
    _validate_setup_proof_tools(top["proof_tools"], root, source["board"])
    require(isinstance(top["companions"], list), "setup companion records are invalid")
    actual_companions = _companion_records([resolve(root, item["path"]) for item in top["companions"]], root, checked, source["board"])
    require(actual_companions == top["companions"], "setup companion identities changed")
    verify_identity(top["workflow"], root, "workflow")


__all__ = ["RecordError", "build_record", "validate_record"]
