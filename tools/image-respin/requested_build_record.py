from __future__ import annotations

from collections import Counter
from pathlib import Path
from typing import Any

try:
    from .workflow_record_common import (
        RecordError,
        identity,
        require,
        require_keys,
        tool_identity,
        verify_docker_digests,
        verify_docker_id,
        verify_identity,
        verify_source,
        verify_tool,
        resolve,
    )
    from .setup_contract import load_contract, validate_tracked_sources
except ImportError:
    from workflow_record_common import (
        RecordError,
        identity,
        require,
        require_keys,
        tool_identity,
        verify_docker_digests,
        verify_docker_id,
        verify_identity,
        verify_source,
        verify_tool,
        resolve,
    )
    from setup_contract import load_contract, validate_tracked_sources


SCHEMA = "octessera.image-respin-requested-build/v1"
TOOL_NAME = "octessera-image-respin-requested-build"
BOARDS = {"orange-pi-zero-2w", "raspberry-pi-zero-2w"}
REQUIRED_FILES = {
    "Cargo.lock",
    "Cargo.toml",
    "package.json",
    "apps/pi-zero/Cargo.toml",
    "Cross.toml",
    "Dockerfile.pi-zero",
    ".github/workflows/respin-board-image.yml",
    "tools/image-respin/runtime_bundle.py",
    "tools/image-respin/boot_neutral.py",
    "resources/legal/notice-bundle.json",
    "tools/legal/stage_notices.py",
    "tools/image-respin/notice_mutation.py",
}
SETUP_TOOL_FILES = tuple(f"tools/image-respin/{name}" for name in ("inventory.py", "provenance.py", "runtime_contract_schema.py", "runtime_contract.py", "runtime_payload.py", "runtime_transaction.py", "runtime_mutation.py", "disk_layout.py", "disk_mount.py", "disk_packaging.py", "disk_provenance.py", "setup_contract_schema.py", "setup_contract.py", "setup_provenance.py", "setup_mutation.py", "setup_proof.py", "disk_setup_respin.py", "boot_neutral.py", "setup_workflow_record.py", "workflow_records.py", "requested_build_record.py", "post_proof_record.py", "trust_manifest.py", "workflow_record_common.py"))
PROOF_PACKAGES = {
    "cpio",
    "zstd",
    "dosfstools",
    "e2fsprogs",
    "initramfs-tools-core",
    "kpartx",
    "unzip",
    "util-linux",
    "xz-utils",
}
TOP_KEYS = {"schema", "schema_version", "record_kind", "source", "inputs", "trust_manifest", "toolchain", "proof_dependencies", "reproducibility", "tool"}
SOURCE_KEYS = {"sha", "version", "board", "feature_command"}
TOOLCHAIN_KEYS = {"host_orchestration", "container", "cross_image", "base_image"}
HOST_TOOLCHAIN_KEYS = {"rustc_vv", "cargo_version", "cross_version"}
CONTAINER_TOOLCHAIN_KEYS = {"rustc_vv", "cargo_version", "image_id"}
IMAGE_KEYS = {"name", "image_id", "repo_digests"}


def _require_exact_setup_inputs(actual_inputs: list[dict[str, Any]], expected_inputs: set[str], label: str) -> None:
    actual_paths = [item["path"] for item in actual_inputs]
    counts = Counter(actual_paths)
    missing = sorted(expected_inputs - set(counts))
    extra = sorted(set(counts) - expected_inputs)
    duplicates = sorted(path for path, count in counts.items() if count > 1)
    require(not missing and not extra and not duplicates and len(actual_paths) == len(expected_inputs), f"{label} mismatch: missing={missing}; extra={extra}; duplicates={duplicates}")


def build_record(
    *,
    root: Path,
    source_sha: str,
    version: str,
    board: str,
    feature_command: str,
    input_files: list[Path],
    trust_manifest: Path,
    rustc_vv: str,
    cargo_version: str,
    cross_version: str,
    container_rustc_vv: str,
    container_cargo_version: str,
    cross_image_id: str,
    cross_repo_digests: list[str],
    base_image_id: str,
    base_repo_digests: list[str],
    proof_packages: dict[str, str],
    setup_layer: str | None = None,
    setup_contract: Path | None = None,
    setup_input_files: list[Path] | None = None,
    require_setup_sources_tracked: bool = False,
) -> dict[str, Any]:
    verify_source(source_sha, version, board, BOARDS)
    require(bool(feature_command.strip()), "feature command is empty")
    inputs = [identity(path, root) for path in input_files]
    require({item["path"] for item in inputs} == REQUIRED_FILES and len(inputs) == len(REQUIRED_FILES), "requested input set is not exact")
    verify_docker_id(cross_image_id, "custom cross image")
    verify_docker_id(base_image_id, "base image")
    verify_docker_digests(cross_repo_digests, "custom cross image", required=False)
    verify_docker_digests(base_repo_digests, "base image", required=True)
    require(set(proof_packages) == PROOF_PACKAGES, "proof package set is not exact")
    require(all(isinstance(value, str) and value.strip() for value in proof_packages.values()), "proof package version is missing")
    require(all(value.strip() for value in (rustc_vv, cargo_version, cross_version, container_rustc_vv, container_cargo_version)), "toolchain identity is incomplete")
    record = {
        "schema": SCHEMA,
        "schema_version": 1,
        "record_kind": "requested-build",
        "source": {"sha": source_sha, "version": version, "board": board, "feature_command": feature_command},
        "inputs": sorted(inputs, key=lambda item: item["path"]),
        "trust_manifest": identity(trust_manifest, root),
        "toolchain": {
            "host_orchestration": {"rustc_vv": rustc_vv.rstrip("\n"), "cargo_version": cargo_version.rstrip("\n"), "cross_version": cross_version.rstrip("\n")},
            "container": {"rustc_vv": container_rustc_vv.rstrip("\n"), "cargo_version": container_cargo_version.rstrip("\n"), "image_id": cross_image_id},
            "cross_image": {"name": "octessera-pi-cross", "image_id": cross_image_id, "repo_digests": sorted(cross_repo_digests)},
            "base_image": {"name": "rust:1-bookworm", "image_id": base_image_id, "repo_digests": sorted(base_repo_digests)},
        },
        "proof_dependencies": [{"name": name, "version": proof_packages[name]} for name in sorted(proof_packages)],
        "reproducibility": {"claim": "not-claimed", "mutable_components": ["github-hosted-runner", "rust:1-bookworm tag", "local cross image tag", "cross toolchain release environment"]},
        "tool": tool_identity(Path(__file__).resolve(), root, "tools/image-respin/requested_build_record.py", TOOL_NAME),
    }
    if setup_layer is None:
        require(setup_contract is None and not setup_input_files, "setup inputs require the setup-portal layer choice")
    else:
        require(setup_layer == "setup-portal", "setup layer choice is not exact")
        require(setup_contract is not None and setup_input_files is not None, "setup layer record inputs are incomplete")
        contract_path = setup_contract
        input_paths = setup_input_files
        assert contract_path is not None and input_paths is not None
        contract, _ = load_contract(contract_path)
        validate_tracked_sources(contract, root, strict=True if require_setup_sources_tracked else None)
        expected_sources = {item["path"] for item in contract["source_inputs"]}
        contract_identity = identity(contract_path, root)
        expected_inputs = expected_sources | {contract_identity["path"], *SETUP_TOOL_FILES}
        actual_inputs = [identity(path, root) for path in input_paths]
        _require_exact_setup_inputs(actual_inputs, expected_inputs, "setup layer input set")
        record["setup"] = {"mode": setup_layer, "contract": contract_identity, "inputs": sorted(actual_inputs, key=lambda item: item["path"]), "tool_files": list(SETUP_TOOL_FILES)}
    return record


def validate_record(record: dict[str, Any], root: Path) -> None:
    keys = set(record)
    require(keys in (TOP_KEYS, TOP_KEYS | {"setup"}), "requested-build keys are not exact")
    verify_tool(record["tool"], Path(__file__).resolve(), root, "tools/image-respin/requested_build_record.py", TOOL_NAME)
    source = require_keys(record["source"], SOURCE_KEYS, "requested-build source")
    verify_source(source["sha"], source["version"], source["board"], BOARDS)
    feature_command = source["feature_command"]
    require(isinstance(feature_command, str), "requested feature command is invalid")
    require(len(feature_command.strip()) > 0, "requested feature command is empty")
    inputs = record["inputs"]
    require(isinstance(inputs, list) and all(isinstance(item, dict) for item in inputs), "requested inputs are invalid")
    require({item["path"] for item in inputs} == REQUIRED_FILES and len(inputs) == len(REQUIRED_FILES), "requested input set changed")
    for item in inputs:
        verify_identity(item, root, "requested input")
    verify_identity(record["trust_manifest"], root, "requested trust manifest")
    toolchain = require_keys(record["toolchain"], TOOLCHAIN_KEYS, "requested toolchain")
    host = require_keys(toolchain["host_orchestration"], HOST_TOOLCHAIN_KEYS, "host orchestration toolchain")
    container = require_keys(toolchain["container"], CONTAINER_TOOLCHAIN_KEYS, "container toolchain")
    for field in HOST_TOOLCHAIN_KEYS:
        require(isinstance(host[field], str) and host[field].strip(), f"missing host toolchain identity: {field}")
    require(isinstance(container["rustc_vv"], str) and isinstance(container["cargo_version"], str), "missing container toolchain identity")
    require(len(container["rustc_vv"].strip()) > 0 and len(container["cargo_version"].strip()) > 0, "missing container toolchain identity")
    verify_docker_id(container["image_id"], "container cross image")
    for field, name in (("cross_image", "octessera-pi-cross"), ("base_image", "rust:1-bookworm")):
        image = require_keys(toolchain[field], IMAGE_KEYS, field)
        require(image["name"] == name and isinstance(image["image_id"], str), f"{field} identity is invalid")
        verify_docker_id(image["image_id"], field)
        require(isinstance(image["repo_digests"], list), f"{field} repo digests are invalid")
        verify_docker_digests(image["repo_digests"], field, required=field == "base_image")
    dependencies = record["proof_dependencies"]
    require(isinstance(dependencies, list) and len(dependencies) == len(PROOF_PACKAGES), "proof dependencies are invalid")
    for dependency in dependencies:
        item = require_keys(dependency, {"name", "version"}, "proof dependency")
        dependency_version = item["version"]
        require(item["name"] in PROOF_PACKAGES and isinstance(dependency_version, str), "proof dependency identity is invalid")
        require(len(dependency_version.strip()) > 0, "proof dependency version is missing")
    require({item["name"] for item in dependencies} == PROOF_PACKAGES, "proof dependency set changed")
    disclosure = require_keys(record["reproducibility"], {"claim", "mutable_components"}, "reproducibility")
    require(disclosure["claim"] == "not-claimed" and isinstance(disclosure["mutable_components"], list), "reproducibility disclosure is invalid")
    if "setup" in record:
        setup = require_keys(record["setup"], {"mode", "contract", "inputs", "tool_files"}, "requested setup layer")
        require(setup["mode"] == "setup-portal", "requested setup layer mode is invalid")
        contract_identity = verify_identity(setup["contract"], root, "setup contract")
        contract, _ = load_contract(resolve(root, contract_identity["path"]))
        require(contract["board_profile"] == source["board"], "setup contract board differs from requested build")
        expected_sources = {item["path"] for item in contract["source_inputs"]}
        expected_inputs = expected_sources | {contract_identity["path"], *SETUP_TOOL_FILES}
        inputs = setup["inputs"]
        require(isinstance(inputs, list), "requested setup inputs are not a list")
        _require_exact_setup_inputs(inputs, expected_inputs, "requested setup inputs")
        for item in inputs:
            verify_identity(item, root, "requested setup input")
        require(setup["tool_files"] == list(SETUP_TOOL_FILES), "requested setup tool set changed")


__all__ = ["BOARDS", "PROOF_PACKAGES", "RecordError", "build_record", "validate_record"]
