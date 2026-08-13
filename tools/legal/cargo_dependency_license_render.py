"""Stable rendering for the checked Cargo dependency artifacts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from cargo_dependency_license_support import REFERENCE_PATHS, reference_text_bytes

def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def lf_text(value: str) -> str:
    return value.replace("\r\n", "\n").replace("\r", "\n")


def checksum_manifest(output: dict[str, bytes], prefix: str) -> bytes:
    lines = [
        f"{sha256(output[path])}  {path}"
        for path in sorted(output)
        if path != f"{prefix}/SHA256SUMS"
    ]
    return ("\n".join(lines) + "\n").encode("utf-8")


def render_cargo_outputs(
    root: Path,
    inventory: dict[str, Any],
    packages: list[dict[str, Any]],
    documents: dict[str, dict[str, Any]],
    source_index: list[dict[str, Any]],
) -> dict[str, bytes]:
    output = {
        "licenses/cargo/inventory.json": json_bytes(inventory),
        "licenses/cargo/SOURCE_INDEX.json": json_bytes(
            {
                "generated": True,
                "scope": "cargo-lock-overinclusive",
                "cargo_lock_sha256": inventory["cargo_lock_sha256"],
                "release_target_profiles": [
                    {"name": "desktop", "workspace_member": "octessera-desktop", "features": []},
                    {"name": "pi-default", "workspace_member": "octessera-pi", "features": ["default"]},
                    {"name": "pi-hardware-rpi-zero-2w", "workspace_member": "octessera-pi", "features": ["hardware-rpi-zero-2w"]},
                    {"name": "pi-hardware-orange-pi-zero-2w", "workspace_member": "octessera-pi", "features": ["hardware-orange-pi-zero-2w"]}
                ],
                "packages": source_index,
            }
        ),
    }
    text = [
        "# GENERATED FILE: Cargo dependency license text inventory",
        "# Inputs: Cargo.lock, Cargo package manifests, and installed package license files.",
        "# Do not edit; run tools/legal/dependency_license_generate.py.",
        "",
        "## Package index",
        "",
    ]
    for package in packages:
        text.extend(
            [
                f"### {package['name']} {package['version']}",
                f"License: {package['license'] or 'UNKNOWN'}",
                f"Class: {package['license_class']}",
                f"Source: {package['source']}",
            ]
        )
        if package.get("review_status"):
            text.append(f"Review: {package['review_status']} ({package['effective_license']})")
        if package.get("modified"):
            text.extend(
                [
                    "Status: modified local vendoring; this is not an unmodified upstream package.",
                    "Provenance: third_party/cpal-0.15.3/PROVENANCE.md",
                ]
            )
        if package["license_files"]:
            text.append("License files:")
            text.extend(f"- {item['source_file']} ({item['document_sha256']})" for item in package["license_files"])
        else:
            text.append("License files: manifest-license-no-file; informational until source-availability review before public binary release")
        text.append("")
    text.extend(["## License documents", ""])
    for document_id in sorted(documents):
        document = documents[document_id]
        text.extend([f"### SHA-256 {document_id}", "Sources:"])
        text.extend(f"- {source}" for source in sorted(document["sources"]))
        text.extend(["", "----- BEGIN LICENSE TEXT -----", lf_text(document["content"]).rstrip("\n"), "----- END LICENSE TEXT -----", ""])
    for reference in sorted(REFERENCE_PATHS):
        source = reference_text_bytes(root, reference)
        text.extend([f"## Reviewed SPDX reference: {reference}", "", lf_text(source.decode("utf-8")).rstrip("\n"), ""])
        output[REFERENCE_PATHS[reference]] = source
    output["licenses/cargo/THIRD_PARTY_LICENSES.txt"] = "\n".join(text).encode("utf-8")

    status = [
        "# GENERATED FILE: Cargo dependency license status",
        "",
        "This report records reviewed policy and packages requiring source-availability review before public binary release; it is not legal advice.",
        "",
        f"Permissive packages: {inventory['license_classes']['permissive']}",
        f"Reviewed MPL packages: {inventory['reviewed_counts']['mpl']}",
        f"Reviewed license alternatives: {inventory['reviewed_counts']['alternatives']}",
        f"Custom or unknown packages: {inventory['license_classes']['custom-or-unknown']}",
        f"Workspace license metadata missing: {len(inventory['workspace_license_metadata_missing'])}",
        f"manifest-license-no-file informational packages: {len(inventory['manifest_license_no_file'])}",
        f"Packages requiring source-availability review before public binary release: {inventory['source_availability_review_count']}",
        "",
        "## Reviewed MPL packages",
        "",
    ]
    status.extend(
        f"- {item['name']} {item['version']}: file-level-copyleft; reviewed-with-source-obligation"
        for item in packages
        if item.get("review_status") == "reviewed-with-source-obligation"
    )
    status.extend(["", "## Reviewed license alternatives", ""])
    status.extend(
        f"- {item['name']} {item['version']}: {item['license']} -> {item['effective_license']}"
        for item in packages
        if item.get("review_status") == "reviewed-alternative"
    )
    status.extend(["", "## Vendored source note", "", "- cpal 0.15.3 is modified-local-vendoring.", "- Its LICENSE and PROVENANCE.md remain authoritative local references."])
    output["licenses/cargo/REVIEWED_DEPENDENCY_STATUS.txt"] = "\n".join(status).encode("utf-8")
    cpal_root = root / "third_party/cpal-0.15.3"
    output["licenses/cargo/vendored-cpal-0.15.3/LICENSE"] = (cpal_root / "LICENSE").read_bytes()
    output["licenses/cargo/vendored-cpal-0.15.3/PROVENANCE.md"] = (cpal_root / "PROVENANCE.md").read_bytes()
    output["licenses/cargo/SHA256SUMS"] = checksum_manifest(output, "licenses/cargo")
    return output
