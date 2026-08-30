from __future__ import annotations

import fnmatch
import hashlib
import json
import os
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

try:
    from .inventory import Inventory, InventoryError, inventory_digest, virtual_symlink_target
    from .runtime_contract_schema import ContractSchemaError, validate_contract_schema
except ImportError:
    from inventory import Inventory, InventoryError, inventory_digest, virtual_symlink_target
    from runtime_contract_schema import ContractSchemaError, validate_contract_schema


class MutationError(ValueError):
    pass


VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
METADATA_KEYS = {"artifact_kind", "binary_sha256", "name", "profile", "runtime_ready", "version"}
MANIFEST_KEYS = {"schema_version", "updater_protocol", "candidate_health_protocol", "tag", "version", "board_profile", "arch", "binary", "platforms"}
ORANGE_MANIFEST_KEYS = MANIFEST_KEYS | {"updater_supported", "distribution"}
STATE_KEYS = {"schema_version", "phase", "current", "previous", "next", "updated_at", "release", "asset"}
ORANGE_STATE_KEYS = STATE_KEYS - {"next"}
STATE_TIMESTAMP_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?Z$")
BUILD_METADATA_KEY_ORDER = ("OCTESSERA_IMAGE_KIND", "OCTESSERA_IMAGE_MODE", "OCTESSERA_BOARD_PROFILE_ID", "OCTESSERA_IMAGE_BUILT_AT", "OCTESSERA_RUNTIME_ENABLED_DEFAULT", "OCTESSERA_IMAGE_CONTRACT_SHA256", "OCTESSERA_RUNTIME_VERSION", "OCTESSERA_RUNTIME_BINARY_SHA256", "OCTESSERA_RUNTIME_MANIFEST_SHA256", "OCTESSERA_RUNTIME_METADATA_SHA256", "OCTESSERA_SPI1_CS0_DTS_SHA256", "OCTESSERA_SPI1_CS0_DTBO_SHA256", "OCTESSERA_INPUT_ROUTING_DTS_SHA256", "OCTESSERA_INPUT_ROUTING_DTBO_SHA256", "OCTESSERA_PI_DEFAULT_SHA256", "OCTESSERA_SAMPLES_MANIFEST_SHA256")
BUILD_METADATA_KEYS = set(BUILD_METADATA_KEY_ORDER)
BUILD_METADATA_TRANSFORMS = {"OCTESSERA_RUNTIME_VERSION", "OCTESSERA_RUNTIME_BINARY_SHA256", "OCTESSERA_RUNTIME_METADATA_SHA256", "OCTESSERA_RUNTIME_MANIFEST_SHA256"}
BUILD_METADATA_HASH_KEYS = {key for key in BUILD_METADATA_KEYS if key.endswith("_SHA256")}
CONTRACTS = Path(__file__).resolve().parents[2] / "resources" / "image-mutations"
PARENT_CONTEXT_KEYS = {"schema", "repository", "board_profile", "version", "constructor", "artifact", "image", "record"}


@dataclass(frozen=True)
class ParentValidation:
    prior_version: str
    parent_identity: dict[str, Any]
    manifest: dict[str, Any] | None
    state: dict[str, Any] | None
    state_bytes: bytes | None
    state_digest: str | None
    release_hashes: dict[str, str]
    build_metadata: "BuildMetadata | None"


@dataclass(frozen=True)
class BuildMetadata:
    raw: bytes
    lines: tuple[bytes, ...]
    fields: dict[str, str]


def fail(message: str) -> None:
    raise MutationError(message)


def unique_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    document: dict[str, Any] = {}
    for key, value in pairs:
        if key in document:
            raise ValueError(f"duplicate JSON key: {key}")
        document[key] = value
    return document


def read_json_bytes(path: Path) -> tuple[Any, bytes]:
    try:
        raw = path.read_bytes()
        return json.loads(raw.decode("utf-8"), object_pairs_hook=unique_pairs), raw
    except (OSError, UnicodeError, ValueError) as exc:
        raise MutationError(f"invalid JSON: {path}") from exc


def parse_build_metadata(raw: bytes) -> BuildMetadata:
    if not raw or b"\r" in raw or not raw.endswith(b"\n"):
        fail("Orange build metadata must be canonical LF text")
    fields: dict[str, str] = {}
    lines = tuple(raw.splitlines(keepends=True))
    for line in lines:
        if not line.endswith(b"\n"):
            fail("Orange build metadata contains an unterminated line")
        try:
            text = line[:-1].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise MutationError("Orange build metadata is not UTF-8") from exc
        key, separator, value = text.partition("=")
        if not separator or not re.fullmatch(r"[A-Z][A-Z0-9_]*", key) or key in fields:
            fail("Orange build metadata has malformed or duplicate assignments")
        if key not in BUILD_METADATA_KEYS:
            fail(f"Orange build metadata has an extra assignment: {key}")
        fields[key] = value
    if tuple(fields) != BUILD_METADATA_KEY_ORDER:
        fail("Orange build metadata assignments are not exact")
    return BuildMetadata(raw, lines, fields)


def validate_build_metadata(root: Path, inventory: Inventory, contract: dict[str, Any], prior_version: str, release_hashes: dict[str, str]) -> BuildMetadata:
    relative = contract["build_metadata_contract"]["path"]
    path = managed_lstat(root, relative)
    preimage_spec = dict(contract["build_metadata_contract"])
    preimage_spec["mode"] = preimage_spec.pop("preimage_mode")
    check_spec(metadata(inventory, relative), preimage_spec, "Orange build metadata preimage")
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise MutationError("cannot read Orange build metadata") from exc
    parsed = parse_build_metadata(raw)
    expected = {"OCTESSERA_IMAGE_KIND": "armbian", "OCTESSERA_IMAGE_MODE": "production", "OCTESSERA_BOARD_PROFILE_ID": "orange-pi-zero-2w", "OCTESSERA_RUNTIME_ENABLED_DEFAULT": "true", "OCTESSERA_RUNTIME_VERSION": prior_version, "OCTESSERA_RUNTIME_BINARY_SHA256": release_hashes["octessera-pi"], "OCTESSERA_RUNTIME_METADATA_SHA256": release_hashes["octessera-runtime.json"], "OCTESSERA_RUNTIME_MANIFEST_SHA256": release_hashes["SHA256SUMS"]}
    if not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", parsed.fields["OCTESSERA_IMAGE_BUILT_AT"]):
        fail("Orange build metadata timestamp is invalid")
    for key in BUILD_METADATA_HASH_KEYS:
        if not SHA256_RE.fullmatch(parsed.fields[key]):
            fail(f"Orange build metadata hash is invalid: {key}")
    if any(parsed.fields[key] != value for key, value in expected.items()):
        fail("Orange build metadata preimage is not hash-bound to the prior release")
    return parsed


def transform_build_metadata(preimage: BuildMetadata, version: str, release_hashes: dict[str, str]) -> bytes:
    if not VERSION_RE.fullmatch(version) or any(not SHA256_RE.fullmatch(value) for value in release_hashes.values()):
        fail("Orange build metadata transform identity is invalid")
    replacements = {"OCTESSERA_RUNTIME_VERSION": version, "OCTESSERA_RUNTIME_BINARY_SHA256": release_hashes["octessera-pi"], "OCTESSERA_RUNTIME_METADATA_SHA256": release_hashes["octessera-runtime.json"], "OCTESSERA_RUNTIME_MANIFEST_SHA256": release_hashes["SHA256SUMS"]}
    output: list[bytes] = []
    for line in preimage.lines:
        key = line[:-1].decode("utf-8").split("=", 1)[0]
        output.append(f"{key}={replacements[key]}\n".encode("utf-8") if key in BUILD_METADATA_TRANSFORMS else line)
    return b"".join(output)


def validate_build_metadata_output(root: Path, inventory: Inventory, contract: dict[str, Any], preimage: BuildMetadata, version: str, release_hashes: dict[str, str]) -> None:
    relative = contract["build_metadata_contract"]["path"]
    path = managed_lstat(root, relative)
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise MutationError("cannot read transformed Orange build metadata") from exc
    check_spec(metadata(inventory, relative), contract["build_metadata_contract"], "new Orange build metadata")
    expected = transform_build_metadata(preimage, version, release_hashes)
    if raw != expected:
        fail("Orange build metadata changed outside the four runtime assignments")
    parsed = parse_build_metadata(raw)
    if parsed.fields["OCTESSERA_RUNTIME_VERSION"] != version or parsed.fields["OCTESSERA_IMAGE_MODE"] != "production" or parsed.fields["OCTESSERA_RUNTIME_ENABLED_DEFAULT"] != "true":
        fail("transformed Orange build metadata identity is invalid")


def load_contract(path: Path) -> tuple[dict[str, Any], str]:
    try:
        raw = Path(path).read_bytes()
        contract = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_pairs)
    except (OSError, UnicodeError, ValueError) as exc:
        raise MutationError(f"mutation contract is invalid: {path}") from exc
    try:
        validate_contract_schema(contract)
    except (ContractSchemaError, TypeError, KeyError) as exc:
        raise MutationError(f"mutation contract schema is invalid: {exc}") from exc
    return contract, hashlib.sha256(raw).hexdigest()


def contract_for_board(board_profile: str) -> Path:
    if board_profile not in {"raspberry-pi-zero-2w", "orange-pi-zero-2w"}:
        fail(f"unsupported board profile: {board_profile}")
    return CONTRACTS / f"{board_profile}.json"


def rooted(root: Path, relative: str) -> Path:
    if not relative or relative.startswith("/") or "\\" in relative:
        fail(f"managed path is not a relative POSIX path: {relative}")
    parts = relative.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        fail(f"managed path contains traversal: {relative}")
    return Path(root).absolute().joinpath(*parts)


def managed_lstat(root: Path, relative: str) -> Path:
    path = rooted(root, relative)
    current = Path(root).absolute()
    parts = relative.split("/")
    try:
        if not current.is_dir() or current.is_symlink():
            fail("managed root is not a real directory")
        for index, part in enumerate(parts):
            current = current / part
            metadata = current.lstat()
            if index != len(parts) - 1 and stat_is_symlink(metadata.st_mode):
                fail(f"managed path has a symlink parent: {relative}")
    except OSError as exc:
        raise MutationError(f"managed path is unavailable: {relative}") from exc
    return path


def stat_is_symlink(mode: int) -> bool:
    return stat.S_ISLNK(mode)


def metadata(inventory: Inventory, relative: str) -> dict[str, Any]:
    try:
        return inventory[relative]
    except KeyError as exc:
        raise MutationError(f"required managed path is missing: {relative}") from exc


def mode_matches(expected: int, actual: int, entry_type: str) -> bool:
    if os.name != "nt" or expected == actual:
        return expected == actual
    windows_modes = {(493, "directory"): {511}, (493, "file"): {438}, (436, "file"): {438}, (420, "file"): {438}, (365, "directory"): {365}, (365, "file"): {292}, (292, "file"): {292}}
    return actual in windows_modes.get((expected, entry_type), set())


def check_spec(entry: dict[str, Any], spec: dict[str, Any], label: str) -> None:
    for key in ("type", "mode", "uid", "gid", "symlink"):
        if key not in spec:
            continue
        matches = mode_matches(int(spec[key]), int(entry.get(key, -1)), str(entry.get("type"))) if key == "mode" else entry.get(key) == spec[key]
        if not matches:
            fail(f"{label} has an unexpected {key}")
    if spec.get("type") == "symlink" and entry.get("target") != spec.get("target"):
        fail(f"{label} has an unexpected symlink target")
    if entry.get("xattrs") != spec.get("xattrs", {}) or entry.get("capability") != spec.get("capability"):
        fail(f"{label} has unexpected xattrs or capability")


def validate_parent_context(context: object, board_profile: str) -> dict[str, Any]:
    if not isinstance(context, dict):
        fail("parent_context must be an object")
    document = cast(dict[str, Any], context)
    if set(document) != PARENT_CONTEXT_KEYS:
        fail("parent_context keys are not exact")
    if document["schema"] != "octessera.image-current-parent/v1" or document["repository"] != "nexxyz/octessera" or document["board_profile"] != "orange-pi-zero-2w" or board_profile != "orange-pi-zero-2w":
        fail("current parent context identity is invalid")
    version = document["version"]
    if not isinstance(version, str) or not VERSION_RE.fullmatch(version):
        fail("current parent context version is invalid")
    constructor = document["constructor"]
    if not isinstance(constructor, dict) or set(constructor) != {"run_id", "source_sha"} or type(constructor["run_id"]) is not int or not COMMIT_RE.fullmatch(constructor["source_sha"]):
        fail("current parent constructor identity is invalid")
    artifact = document["artifact"]
    if not isinstance(artifact, dict) or set(artifact) != {"id", "name", "size", "digest", "expires_at", "entries"}:
        fail("current parent artifact identity is invalid")
    if type(artifact["id"]) is not int or not isinstance(artifact["name"], str) or type(artifact["size"]) is not int or artifact["size"] <= 0 or not isinstance(artifact["digest"], str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", artifact["digest"]):
        fail("current parent artifact identity is invalid")
    image = document["image"]
    if not isinstance(image, dict) or set(image) != {"name", "size", "sha256"} or image["name"] != f"octessera-{version}-orange-pi-zero-2w.img.xz" or type(image["size"]) is not int or image["size"] <= 0 or not isinstance(image["sha256"], str) or not SHA256_RE.fullmatch(image["sha256"]):
        fail("current parent image identity is invalid")
    record = document["record"]
    if not isinstance(record, dict) or set(record) != {"path", "sha256", "size"} or record["path"] != "resources/image-parents/orange-pi-zero-2w-current.json" or not isinstance(record["sha256"], str) or not SHA256_RE.fullmatch(record["sha256"]) or type(record["size"]) is not int or record["size"] <= 0:
        fail("current parent record identity is invalid")
    return json.loads(json.dumps(document, sort_keys=True, separators=(",", ":")))


def manifest_for(board: str, version: str) -> dict[str, Any]:
    manifest = {"schema_version": 2, "updater_protocol": 2, "candidate_health_protocol": 1, "tag": f"v{version}", "version": version, "board_profile": board, "arch": "aarch64-unknown-linux-gnu", "binary": "octessera-pi", "platforms": [board, "linux-aarch64-device"]}
    if board == "orange-pi-zero-2w":
        manifest.update({"updater_supported": True, "distribution": "runtime-updater"})
    return manifest


def read_manifest(root: Path, relative: str, version: str, board: str) -> dict[str, Any]:
    manifest, _ = read_json_bytes(managed_lstat(root, relative))
    expected_keys = ORANGE_MANIFEST_KEYS if board == "orange-pi-zero-2w" else MANIFEST_KEYS
    if not isinstance(manifest, dict) or set(manifest) != expected_keys or manifest != manifest_for(board, version):
        fail("Orange release manifest is not exact" if board == "orange-pi-zero-2w" else "Raspberry release manifest is not exact")
    return manifest


def _check_parents(root: Path, inventory: Inventory, contract: dict[str, Any]) -> None:
    for spec in contract["real_parents"]:
        relative = spec["path"]
        path = managed_lstat(root, relative)
        entry = metadata(inventory, relative)
        if entry["type"] != "directory" or path.is_symlink():
            fail(f"managed parent is not a real directory: {relative}")
        check_spec(entry, spec, relative)
        parts = relative.split("/")
        for index in range(1, len(parts) + 1):
            cumulative = "/".join(parts[:index])
            if inventory.get(cumulative, {}).get("type") == "symlink":
                fail(f"managed parent component is a symlink: {cumulative}")


def _check_release(root: Path, inventory: Inventory, contract: dict[str, Any], version: str) -> tuple[dict[str, Any] | None, dict[str, str]]:
    base = f"{contract['managed']['releases']}/{version}"
    managed_lstat(root, base)
    check_spec(metadata(inventory, base), contract["prior_release"]["directory"], "prior release directory")
    expected = {item["name"]: item for item in contract["prior_release"]["entries"]}
    actual = {path[len(base) + 1:]: value for path, value in inventory.items() if path.startswith(base + "/") and "/" not in path[len(base) + 1:]}
    if set(actual) != set(expected):
        fail("prior release entries are not exact")
    hashes: dict[str, str] = {}
    for name, spec in expected.items():
        managed_lstat(root, f"{base}/{name}")
        check_spec(actual[name], spec, f"prior release {name}")
        digest = actual[name].get("sha256")
        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            fail(f"prior release {name} has no content digest")
        hashes[name] = str(digest)
    manifest = read_manifest(root, f"{base}/update-manifest.json", version, contract["board_profile"])
    return manifest, hashes


def _check_state(root: Path, inventory: Inventory, contract: dict[str, Any], prior_version: str, manifest: dict[str, Any] | None) -> tuple[dict[str, Any] | None, bytes | None, str | None]:
    relative = contract["state_contract"]["path"]
    entry = inventory.get(relative)
    if not contract["state_contract"]["owned"]:
        if entry is not None:
            fail("Orange runtime state is not allowed")
        return None, None, None
    check_spec(entry or {}, contract["state_contract"], "runtime state")
    state, raw = read_json_bytes(managed_lstat(root, relative))
    orange = contract["board_profile"] == "orange-pi-zero-2w"
    expected_keys = ORANGE_STATE_KEYS if orange else STATE_KEYS
    if not isinstance(state, dict) or set(state) != expected_keys or state["schema_version"] != 2 or state["phase"] != "committed" or state["current"] != prior_version:
        fail("Orange runtime state shape is not exact" if orange else "Raspberry runtime state shape is not exact")
    timestamp_valid = isinstance(state["updated_at"], str) and bool(state["updated_at"].strip())
    if state["previous"] is not None or (not orange and state["next"] is not None) or state["asset"] is not None or state["release"] != manifest or not timestamp_valid or (orange and not STATE_TIMESTAMP_RE.fullmatch(state["updated_at"])):
        fail("Orange runtime state does not describe the current release" if orange else "Raspberry runtime state does not describe the current release")
    return state, raw, hashlib.sha256(raw).hexdigest()


def _check_links(root: Path, inventory: Inventory, contract: dict[str, Any], prior_version: str) -> None:
    managed = contract["managed"]
    current, binary = managed["current"], managed["binary_link"]
    current_spec = dict(contract["current_link"], target=contract["current_link"]["target"].format(version=prior_version))
    check_spec(metadata(inventory, current), current_spec, "current release link")
    check_spec(metadata(inventory, binary), contract["binary_link"], "runtime binary link")
    current_path, binary_path = managed_lstat(root, current), managed_lstat(root, binary)
    if virtual_symlink_target(root, current_path, str(inventory[current]["target"])) != rooted(root, f"{managed['releases']}/{prior_version}"):
        fail("current release link resolves outside the exact prior release")
    if virtual_symlink_target(root, binary_path, str(inventory[binary]["target"])) != rooted(root, f"{current}/octessera-pi"):
        fail("runtime binary link resolves outside current")


def validate_parent(root: Path, inventory: Inventory, contract: dict[str, Any], parent_context: object) -> ParentValidation:
    context = validate_parent_context(parent_context, contract["board_profile"])
    _check_parents(root, inventory, contract)
    releases = contract["managed"]["releases"]
    children = [path[len(releases) + 1:] for path in inventory if path.startswith(releases + "/") and "/" not in path[len(releases) + 1:]]
    if len(children) != 1 or not VERSION_RE.fullmatch(children[0]):
        fail("managed releases must contain exactly one prior semver release")
    prior = children[0]
    managed_lstat(root, contract["managed"]["current"])
    managed_lstat(root, contract["managed"]["binary_link"])
    _check_links(root, inventory, contract, prior)
    manifest, hashes = _check_release(root, inventory, contract, prior)
    state, state_bytes, state_digest = _check_state(root, inventory, contract, prior, manifest)
    build_metadata = validate_build_metadata(root, inventory, contract, prior, hashes) if contract["board_profile"] == "orange-pi-zero-2w" else None
    for pattern in contract["mutation_contract"]["forbidden"]:
        if any(fnmatch.fnmatchcase(path, pattern) for path in inventory):
            fail(f"forbidden staging path exists: {pattern}")
    context_digest = hashlib.sha256(json.dumps(context, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    parent_identity = {"board_profile": contract["board_profile"], "prior_version": prior, "prior_release_entries": hashes, "prior_release_digest": inventory_digest({path: value for path, value in inventory.items() if path == f"{releases}/{prior}" or path.startswith(f"{releases}/{prior}/")}), "prior_state_preimage_sha256": state_digest, "prior_build_metadata_preimage_sha256": hashlib.sha256(build_metadata.raw).hexdigest() if build_metadata is not None else None, "current_target": inventory[contract["managed"]["current"]]["target"], "parent_context": context, "parent_context_sha256": context_digest}
    return ParentValidation(prior, parent_identity, manifest, state, state_bytes, state_digest, hashes, build_metadata)


def _classify(path: str, old_base: str, new_base: str, contract: dict[str, Any], same_version: bool) -> str | None:
    mutation = contract["mutation_contract"]
    version_values = {"prior_version": old_base.rsplit("/", 1)[-1], "version": new_base.rsplit("/", 1)[-1]}
    replace = {item.format(**version_values) for item in mutation["replace"]}
    remove = {item.format(**version_values) for item in mutation["remove"]}
    generated = {item.format(**version_values) for item in mutation["generated"]}
    if path in replace:
        return "replace"
    if contract["state_contract"]["owned"] and path == contract["state_contract"]["path"]:
        return "structured_transform"
    if path == contract["managed"].get("build_metadata"):
        return "structured_transform"
    if path == old_base:
        return "replace" if same_version and old_base in replace else "remove" if old_base in remove else None
    if path.startswith(old_base + "/"):
        return "generated" if same_version and path in generated else "remove" if old_base in remove else None
    if path == new_base:
        return "replace" if same_version and new_base in replace else "generated" if new_base in generated else None
    if path.startswith(new_base + "/"):
        return "generated" if path in generated else None
    return None


def validate_changed_paths(before: Inventory, after: Inventory, contract: dict[str, Any], prior: str, version: str, extra_allowed_paths: set[str] | None = None) -> list[str]:
    old_base = f"{contract['managed']['releases']}/{prior}"
    new_base = f"{contract['managed']['releases']}/{version}"
    changed = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
    extra_allowed_paths = extra_allowed_paths or set()
    for path in changed:
        if path in extra_allowed_paths:
            continue
        if any(fnmatch.fnmatchcase(path, pattern) for pattern in contract["mutation_contract"]["forbidden"]):
            fail(f"forbidden path changed: {path}")
        if _classify(path, old_base, new_base, contract, prior == version) is None:
            fail(f"unauthorized root mutation: {path}")
    return changed
