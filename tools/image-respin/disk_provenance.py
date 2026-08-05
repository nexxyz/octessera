from __future__ import annotations

import json
from pathlib import Path
from typing import Any

try:
    from .disk_layout import DiskLayout
    from .disk_packaging import file_digest
    from .provenance import TOOL_IDENTITY, canonical_source_identity, digest_object, tool_code_model
    from .setup_provenance import setup_tool_code_model
except ImportError:
    from disk_layout import DiskLayout
    from disk_packaging import file_digest
    from provenance import TOOL_IDENTITY, canonical_source_identity, digest_object, tool_code_model
    from setup_provenance import setup_tool_code_model


DERIVED_PROOF_SCHEMA = "octessera.image-derived-respin-provenance.v1"


def build_derived_provenance(
    *,
    board_profile: str,
    version: str,
    source_identity: object,
    parent_context: dict[str, Any],
    trust_manifest_digest: str,
    runtime_provenance: dict[str, Any],
    pre_layout: DiskLayout,
    post_layout: DiskLayout,
    image: Path,
    packaged: Path,
    compression_identity: str,
    tool_identity: str = TOOL_IDENTITY,
    boot_integrity: dict[str, Any] | None = None,
    boot_policy: dict[str, Any] | None = None,
    parent_binding: dict[str, Any] | None = None,
    derivation_kind: str | None = None,
    setup_mutation: dict[str, Any] | None = None,
    setup_proof: dict[str, Any] | None = None,
) -> dict[str, Any]:
    source = canonical_source_identity(source_identity)
    image_digest, image_size = file_digest(image)
    package_digest, package_size = file_digest(packaged)
    layouts = {"pre": pre_layout.as_dict(), "post": post_layout.as_dict()}
    runtime_digest = digest_object(runtime_provenance)
    tool_code = tool_code_model()
    if boot_integrity is not None:
        if boot_policy is None or parent_binding is None or derivation_kind not in {"runtime-only", "setup-portal"}:
            raise ValueError("Orange boot-neutral provenance context is incomplete")
        if derivation_kind == "setup-portal" and (setup_mutation is None or setup_proof is None):
            raise ValueError("Orange setup provenance context is incomplete")
        finalizer: dict[str, Any] = {"tool_identity": tool_identity, "compression_identity": compression_identity}
        if derivation_kind == "setup-portal":
            finalizer["setup_tool_code"] = setup_tool_code_model()
        else:
            finalizer.update({"runtime_tool_code_schema": tool_code["schema"], "runtime_tool_code_version": tool_code["version"], "runtime_tool_code_digest": tool_code["digest"], "runtime_tool_code_files": tool_code["files"]})
        value: dict[str, Any] = {
            "proof_schema": f"octessera.image-derived-{ 'setup-' if derivation_kind == 'setup-portal' else '' }respin-provenance.v2",
            "schema_version": 2,
            "proof_mode": boot_policy["proof_mode"],
            "derivation_kind": derivation_kind,
            "board_profile": board_profile,
            "version": version,
            "source_identity": source,
            "boot_mutation": False,
            "phase5_claim": False,
            "policy": boot_policy["policy"],
            "parent": parent_binding,
            "runtime_mutation": {"digest": runtime_digest, "provenance": runtime_provenance},
            "boot_integrity": boot_integrity,
            "disk_invariants": {"pre": layouts["pre"], "post": layouts["post"], "digest": digest_object(layouts)},
            "derived_image": {"sha256": image_digest, "size": image_size},
            "packaged_artifact": {"sha256": package_digest, "size": package_size, "path": packaged.name},
            "finalizer": finalizer,
        }
        if derivation_kind == "setup-portal":
            value["setup_mutation"] = setup_mutation
            value["setup_proof"] = setup_proof
        return value
    return {
        "proof_schema": DERIVED_PROOF_SCHEMA,
        "schema_version": 1,
        "board_profile": board_profile,
        "version": version,
        "source_identity": source,
        "parent": {"context": parent_context, "asset": parent_context["asset"], "trust_manifest_sha256": trust_manifest_digest, "digest": digest_object({"context": parent_context, "trust_manifest_sha256": trust_manifest_digest})},
        "runtime_mutation": {"digest": runtime_digest, "provenance": runtime_provenance},
        "disk_invariants": {"pre": layouts["pre"], "post": layouts["post"], "digest": digest_object(layouts)},
        "derived_image": {"sha256": image_digest, "size": image_size},
        "packaged_artifact": {"sha256": package_digest, "size": package_size, "path": packaged.name},
        "finalizer": {"tool_identity": tool_identity, "compression_identity": compression_identity, "runtime_tool_code_schema": tool_code["schema"], "runtime_tool_code_version": tool_code["version"], "runtime_tool_code_digest": tool_code["digest"], "runtime_tool_code_files": tool_code["files"]},
    }


def provenance_bytes(provenance: dict[str, Any]) -> bytes:
    return (json.dumps(provenance, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode("utf-8")
