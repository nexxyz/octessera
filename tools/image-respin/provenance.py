from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Any


class ProvenanceError(ValueError):
    pass


PROOF_SCHEMA = "octessera.image-mutation-provenance.v2"
TOOL_IDENTITY = "octessera-image-respin-runtime-mutation/1"
RUNTIME_TOOL_IDENTITY = "octessera-image-respin-runtime-mutation/2"
TOOL_CODE_SCHEMA = "octessera-image-respin-tool-code/v1"
TOOL_CODE_FILES = ("inventory.py", "provenance.py", "trust_manifest.py", "runtime_bundle.py", "runtime_contract.py", "runtime_contract_schema.py", "runtime_payload.py", "runtime_transaction.py", "runtime_mutation.py", "notice_mutation.py", "disk_layout.py", "disk_mount.py", "disk_packaging.py", "disk_provenance.py", "disk_respin.py", "boot_neutral.py")
TOOL_CODE_EXTERNAL_FILES = ("tools/armbian-image/orange_boot_contract.py", "tools/armbian-image/orange_boot_inventory.py", "tools/armbian-image/orange_boot_selection.py", "tools/armbian-image/orange_image_mount.py", "tools/armbian-image/orange_initramfs.py", "tools/armbian-image/orange_phase5_proof.py", "tools/armbian-image/orange_trusted_parent_proof.py", "tools/armbian-image/verify-orange-image.py", "tools/armbian-image/verify-orange-image.sh", "tools/armbian-image/verify_runtime_account.py", "tools/legal/stage_notices.py", "resources/legal/notice-bundle.json", "resources/image-construction/boot-layers/orange-pi-zero-2w.json", "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json")


def canonical_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def digest_object(value: object) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def tool_code_model(module_directory: Path | None = None) -> dict[str, Any]:
    directory = Path(module_directory) if module_directory is not None else Path(__file__).resolve().parent
    files: list[dict[str, Any]] = []
    repository = directory.parents[1]
    for name in (*TOOL_CODE_FILES, *TOOL_CODE_EXTERNAL_FILES):
        path = directory / name if name in TOOL_CODE_FILES else repository / name
        if path.is_symlink() or not path.is_file():
            raise ProvenanceError(f"tool-code file is not a regular file: {name}")
        raw = path.read_bytes()
        files.append({"path": name, "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)})
    body = {"schema": TOOL_CODE_SCHEMA, "version": 1, "files": files}
    return {**body, "digest": digest_object(body)}


def tool_code_digest(module_directory: Path | None = None) -> str:
    return str(tool_code_model(module_directory)["digest"])


def canonical_source_identity(value: object) -> str | dict[str, Any]:
    if isinstance(value, str):
        if not value or value != value.strip() or len(value) > 512:
            raise ProvenanceError("source identity must be a nonempty trimmed value")
        return value
    if isinstance(value, dict):
        try:
            encoded = canonical_json(value)
        except (TypeError, ValueError) as exc:
            raise ProvenanceError("source identity is not canonical JSON") from exc
        if not value or len(encoded) > 4096:
            raise ProvenanceError("source identity is empty or too large")
        return json.loads(encoded)
    raise ProvenanceError("source identity must be a string or object")


def _digest_text(value: str, label: str) -> str:
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ProvenanceError(f"{label} is not a lowercase SHA-256")
    return value


def build_provenance(
    *,
    board_profile: str,
    version: str,
    source_identity: object,
    parent_identity: dict[str, Any],
    payload_digest: str,
    mutation_contract_digest: str,
    pre_inventory_digest: str,
    post_inventory_digest: str,
    changed_paths: list[str],
    notice: dict[str, Any] | None = None,
    tool_identity: str = RUNTIME_TOOL_IDENTITY,
    tool_code_directory: Path | None = None,
) -> dict[str, Any]:
    if not board_profile or not version:
        raise ProvenanceError("board profile and version are required")
    if not isinstance(parent_identity, dict) or not parent_identity:
        raise ProvenanceError("parent identity must be a nonempty object")
    if not tool_identity or tool_identity != tool_identity.strip():
        raise ProvenanceError("tool identity is invalid")
    source = canonical_source_identity(source_identity)
    payload_digest = _digest_text(payload_digest, "payload digest")
    mutation_contract_digest = _digest_text(mutation_contract_digest, "mutation-contract digest")
    pre_inventory_digest = _digest_text(pre_inventory_digest, "pre-inventory digest")
    post_inventory_digest = _digest_text(post_inventory_digest, "post-inventory digest")
    if changed_paths != sorted(set(changed_paths)) or any(not path or path.startswith("/") for path in changed_paths):
        raise ProvenanceError("changed paths must be sorted, unique, relative paths")
    parent = json.loads(canonical_json(parent_identity))
    parent_digest = digest_object(parent)
    tool_code = tool_code_model(tool_code_directory)
    result = {
        "proof_schema": PROOF_SCHEMA,
        "schema_version": 2,
        "board_profile": board_profile,
        "version": version,
        "source_identity": source,
        "parent": {"identity": parent, "digest": parent_digest},
        "payload": {"digest": payload_digest},
        "mutation_contract": {"digest": mutation_contract_digest},
        "finalizer": {"source_identity": source, "tool_identity": tool_identity, "tool_code_schema": tool_code["schema"], "tool_code_version": tool_code["version"], "tool_code_digest": tool_code["digest"], "tool_code_files": tool_code["files"]},
        "inventories": {"pre": pre_inventory_digest, "post": post_inventory_digest},
        "parent_inventory_digest": pre_inventory_digest,
        "post_inventory_digest": post_inventory_digest,
        "changed_paths": changed_paths,
    }
    if notice is not None:
        result["notice"] = json.loads(canonical_json(notice))
    return result


def provenance_bytes(provenance: dict[str, Any]) -> bytes:
    return (json.dumps(provenance, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode("utf-8")


def write_provenance(path: Path, provenance: dict[str, Any]) -> None:
    path = Path(path)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    try:
        temporary.write_bytes(provenance_bytes(provenance))
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)
