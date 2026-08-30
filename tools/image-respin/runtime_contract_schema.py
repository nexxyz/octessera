from __future__ import annotations

import re
import string
from typing import Any


class ContractSchemaError(ValueError):
    pass


BOARD_NAMES = {"raspberry-pi-zero-2w", "orange-pi-zero-2w"}
BUILD_KEYS = ("OCTESSERA_IMAGE_KIND", "OCTESSERA_IMAGE_MODE", "OCTESSERA_BOARD_PROFILE_ID", "OCTESSERA_IMAGE_BUILT_AT", "OCTESSERA_RUNTIME_ENABLED_DEFAULT", "OCTESSERA_IMAGE_CONTRACT_SHA256", "OCTESSERA_RUNTIME_VERSION", "OCTESSERA_RUNTIME_BINARY_SHA256", "OCTESSERA_RUNTIME_MANIFEST_SHA256", "OCTESSERA_RUNTIME_METADATA_SHA256", "OCTESSERA_SPI1_OLED_SD2_DTS_SHA256", "OCTESSERA_SPI1_OLED_SD2_DTBO_SHA256", "OCTESSERA_INPUT_ROUTING_DTS_SHA256", "OCTESSERA_INPUT_ROUTING_DTBO_SHA256", "OCTESSERA_AHUB0_PCM5102_DTS_SHA256", "OCTESSERA_AHUB0_PCM5102_DTBO_SHA256", "OCTESSERA_PI_DEFAULT_SHA256", "OCTESSERA_SAMPLES_MANIFEST_SHA256")
TRANSFORM_KEYS = ("OCTESSERA_RUNTIME_VERSION", "OCTESSERA_RUNTIME_BINARY_SHA256", "OCTESSERA_RUNTIME_METADATA_SHA256", "OCTESSERA_RUNTIME_MANIFEST_SHA256")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
SPEC_KEYS = {"type", "mode", "uid", "gid", "symlink", "xattrs", "capability"}
LINK_SPEC_KEYS = SPEC_KEYS - {"mode"}


def _keys(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise ContractSchemaError(f"{label} keys are not exact")
    return value


def _safe_path(value: Any, label: str, *, pattern: bool = False) -> None:
    if not isinstance(value, str) or not value or value.startswith("/") or "\\" in value:
        raise ContractSchemaError(f"{label} is not a safe relative path")
    if value == "**":
        if not pattern:
            raise ContractSchemaError(f"{label} has an invalid wildcard")
        return
    if pattern and value.endswith("*"):
        value = value[:-1]
    for part in value.split("/"):
        if not part or part in {".", ".."}:
            raise ContractSchemaError(f"{label} contains traversal")
    placeholders = {field_name for _, field_name, _, _ in string.Formatter().parse(value) if field_name is not None}
    if placeholders - {"version", "prior_version"}:
        raise ContractSchemaError(f"{label} has an unsupported placeholder")


def _xattrs(spec: dict[str, Any], label: str) -> None:
    values = spec["xattrs"]
    if not isinstance(values, dict):
        raise ContractSchemaError(f"{label}.xattrs is not an object")
    for name, value in values.items():
        if not isinstance(name, str) or not name or not isinstance(value, str) or len(value) % 2 or not re.fullmatch(r"[0-9a-f]*", value):
            raise ContractSchemaError(f"{label}.xattrs is not canonical hex")
    capability = spec["capability"]
    if capability is not None and (not isinstance(capability, str) or len(capability) % 2 or not re.fullmatch(r"[0-9a-f]+", capability) or values.get("security.capability") != capability):
        raise ContractSchemaError(f"{label}.capability is inconsistent")
    if capability is None and "security.capability" in values:
        raise ContractSchemaError(f"{label}.capability is missing")


def _spec(spec: Any, label: str, *, link: bool = False, release_file: bool = False) -> dict[str, Any]:
    expected = (LINK_SPEC_KEYS if link else SPEC_KEYS) | ({"target"} if link else set()) | ({"sha256"} if release_file else set())
    result = _keys(spec, expected, label)
    if result["type"] not in {"directory", "file", "symlink"} or not isinstance(result["symlink"], bool) or result["symlink"] != (result["type"] == "symlink"):
        raise ContractSchemaError(f"{label} type/symlink fields are inconsistent")
    for field in (("uid", "gid") if link else ("mode", "uid", "gid")):
        if isinstance(result[field], bool) or not isinstance(result[field], int) or result[field] < 0:
            raise ContractSchemaError(f"{label}.{field} is invalid")
    if link and (not isinstance(result["target"], str) or not result["target"].startswith("/") or {field_name for _, field_name, _, _ in string.Formatter().parse(result["target"]) if field_name is not None} - {"version"}):
        raise ContractSchemaError(f"{label}.target is invalid")
    if release_file and result["sha256"] not in {"preimage", "payload", "derived"}:
        raise ContractSchemaError(f"{label}.sha256 placeholder is invalid")
    _xattrs(result, label)
    return result


def _paths(values: Any, label: str, *, pattern: bool = False) -> list[str]:
    if not isinstance(values, list) or not values or any(not isinstance(value, str) for value in values) or len(set(values)) != len(values):
        raise ContractSchemaError(f"{label} is not a unique nonempty path list")
    for value in values:
        _safe_path(value, label, pattern=pattern)
    return values


def validate_contract_schema(contract: Any) -> None:
    if not isinstance(contract, dict) or contract.get("schema_version") != 1:
        raise ContractSchemaError("contract schema_version is unsupported")
    board = contract.get("board_profile")
    if board not in BOARD_NAMES:
        raise ContractSchemaError("contract board_profile is unsupported")
    top = {"schema_version", "board_profile", "binary", "managed", "real_parents", "current_link", "binary_link", "prior_release", "new_release", "state_contract", "bundle_contract", "mutation_contract"}
    if board == "orange-pi-zero-2w":
        top.add("build_metadata_contract")
    _keys(contract, top, "contract")
    if contract["binary"] != "octessera-pi":
        raise ContractSchemaError("contract binary is invalid")
    managed_keys = {"runtime_root", "releases", "current", "binary_link", "state"} | ({"build_metadata"} if board == "orange-pi-zero-2w" else set())
    managed = _keys(contract["managed"], managed_keys, "managed")
    expected_managed = {"runtime_root": "opt/octessera", "releases": "opt/octessera/releases", "current": "opt/octessera/current", "binary_link": "usr/local/bin/octessera-pi", "state": "opt/octessera/update-state.json"}
    if board == "orange-pi-zero-2w":
        expected_managed["build_metadata"] = "etc/octessera/build-metadata.env"
    if managed != expected_managed:
        raise ContractSchemaError("managed paths are not exact")
    for name, value in managed.items():
        _safe_path(value, f"managed.{name}")
    parents = contract["real_parents"]
    if not isinstance(parents, list) or not parents:
        raise ContractSchemaError("real_parents is invalid")
    parent_paths: list[str] = []
    for index, spec in enumerate(parents):
        parent = _keys(spec, {"path"} | SPEC_KEYS, f"real_parents[{index}]")
        _safe_path(parent["path"], f"real_parents[{index}].path")
        _spec({key: parent[key] for key in SPEC_KEYS}, f"real_parents[{index}]")
        parent_paths.append(parent["path"])
    if len(set(parent_paths)) != len(parent_paths):
        raise ContractSchemaError("real_parents contains duplicates")
    expected_parents = ["opt", "opt/octessera", "opt/octessera/releases", "usr", "usr/local", "usr/local/bin"]
    if board == "orange-pi-zero-2w":
        expected_parents.extend(("etc", "etc/octessera"))
    if parent_paths != expected_parents:
        raise ContractSchemaError("real parent paths are not exact")
    _spec(contract["current_link"], "current_link", link=True)
    _spec(contract["binary_link"], "binary_link", link=True)
    expected_entries = ["octessera-pi", "update-manifest.json"] if board == "raspberry-pi-zero-2w" else ["octessera-pi", "octessera-runtime.json", "SHA256SUMS", "update-manifest.json"]
    for section in ("prior_release", "new_release"):
        release = _keys(contract[section], {"directory", "entries"}, section)
        _spec(release["directory"], f"{section}.directory")
        entries = release["entries"]
        if not isinstance(entries, list) or [item.get("name") for item in entries if isinstance(item, dict)] != expected_entries:
            raise ContractSchemaError(f"{section}.entries are not exact")
        for index, item in enumerate(entries):
            entry = _keys(item, {"name", "sha256"} | SPEC_KEYS, f"{section}.entries[{index}]")
            _spec({**{key: entry[key] for key in SPEC_KEYS}, "sha256": entry["sha256"]}, f"{section}.entries[{index}]", release_file=True)
    state = contract["state_contract"]
    if board == "raspberry-pi-zero-2w":
        _keys(state, {"owned", "path", "type", "symlink", "mode", "uid", "gid", "preimage", "transform", "xattrs", "capability"}, "state_contract")
        if state["owned"] is not True or state["type"] != "file" or state["preimage"] != "exact-committed-state" or state["transform"] != "committed-current-release":
            raise ContractSchemaError("Raspberry state ownership is invalid")
        _spec({key: state[key] for key in SPEC_KEYS}, "state_contract")
    else:
        _keys(state, {"owned", "path", "type", "symlink", "mode", "uid", "gid", "preimage", "transform", "xattrs", "capability"}, "state_contract")
        if state["owned"] is not True or state["type"] != "file" or state["preimage"] != "exact-committed-state" or state["transform"] != "committed-current-release":
            raise ContractSchemaError("Orange state ownership is invalid")
        _spec({key: state[key] for key in SPEC_KEYS}, "state_contract")
    if board == "orange-pi-zero-2w":
        metadata = _keys(contract["build_metadata_contract"], {"path", "type", "preimage_mode", "mode", "uid", "gid", "symlink", "required_keys", "transform_keys", "line_endings", "xattrs", "capability"}, "build_metadata_contract")
        _safe_path(metadata["path"], "build_metadata_contract.path")
        _spec({key: metadata[key] for key in SPEC_KEYS}, "build_metadata_contract")
        if isinstance(metadata["preimage_mode"], bool) or not isinstance(metadata["preimage_mode"], int) or metadata["preimage_mode"] != 420 or metadata["mode"] != 420 or not isinstance(metadata["required_keys"], list) or not isinstance(metadata["transform_keys"], list) or tuple(metadata["required_keys"]) != BUILD_KEYS or tuple(metadata["transform_keys"]) != TRANSFORM_KEYS or metadata["line_endings"] != "LF":
            raise ContractSchemaError("Orange build metadata contract fields are not exact")
    bundle = _keys(contract["bundle_contract"], {"entries", "input_modes", "metadata"}, "bundle_contract")
    if bundle["entries"] != ["SHA256SUMS", "octessera-pi", "octessera-runtime.json"] or bundle["input_modes"] != {"octessera-pi": 493, "octessera-runtime.json": 420, "SHA256SUMS": 420} or bundle["metadata"] != "production-runtime":
        raise ContractSchemaError("bundle contract entries are not exact")
    mutation = _keys(contract["mutation_contract"], {"replace", "remove", "generated", "structured_transform", "preserve", "forbidden", "parent_metadata"}, "mutation_contract")
    for name in ("replace", "remove", "generated", "structured_transform"):
        _paths(mutation[name], f"mutation_contract.{name}")
    _paths(mutation["preserve"], "mutation_contract.preserve", pattern=True)
    _paths(mutation["forbidden"], "mutation_contract.forbidden", pattern=True)
    if mutation["preserve"] != ["**"] or mutation["parent_metadata"] != "preserve-exactly":
        raise ContractSchemaError("mutation preserve contract is invalid")
    expected_replace = ["opt/octessera/releases/{version}", "opt/octessera/current", "usr/local/bin/octessera-pi"]
    expected_remove = ["opt/octessera/releases/{prior_version}"]
    expected_generated = ["opt/octessera/releases/{version}", "opt/octessera/releases/{version}/octessera-pi", "opt/octessera/releases/{version}/update-manifest.json"] if board == "raspberry-pi-zero-2w" else ["opt/octessera/releases/{version}", "opt/octessera/releases/{version}/octessera-pi", "opt/octessera/releases/{version}/octessera-runtime.json", "opt/octessera/releases/{version}/SHA256SUMS", "opt/octessera/releases/{version}/update-manifest.json"]
    expected_structured = [managed["state"]] if board == "raspberry-pi-zero-2w" else [managed["state"], managed["build_metadata"]]
    if mutation["replace"] != expected_replace or mutation["remove"] != expected_remove or mutation["generated"] != expected_generated or mutation["structured_transform"] != expected_structured:
        raise ContractSchemaError("structured transform ownership is invalid")
    if mutation["forbidden"] != ["opt/octessera/releases/.image-respin-*", "opt/octessera/.image-respin-*", "usr/local/bin/.image-respin-*"]:
        raise ContractSchemaError("mutation forbidden paths are not exact")
