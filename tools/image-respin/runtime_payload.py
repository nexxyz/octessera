from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

try:
    from .inventory import Inventory, InventoryError, build_inventory, ensure_inventory_symlinks_contained, inventory_digest, remove_path
    from .runtime_contract import BuildMetadata, METADATA_KEYS, MutationError, check_spec, fail, managed_lstat, manifest_for, metadata, mode_matches, read_json_bytes, validate_build_metadata_output
except ImportError:
    from inventory import Inventory, InventoryError, build_inventory, ensure_inventory_symlinks_contained, inventory_digest, remove_path
    from runtime_contract import BuildMetadata, METADATA_KEYS, MutationError, check_spec, fail, managed_lstat, manifest_for, metadata, mode_matches, read_json_bytes, validate_build_metadata_output


@dataclass(frozen=True)
class PayloadValidation:
    inventory: Inventory
    metadata: dict[str, Any]
    digest: str


def set_owner(path: Path, uid: int, gid: int, follow_symlinks: bool = True) -> None:
    chown = getattr(os, "chown", None)
    if chown is None or os.name == "nt":
        return
    try:
        chown(path, uid, gid, follow_symlinks=follow_symlinks)
    except OSError as exc:
        raise MutationError(f"cannot set owner on generated path: {path}") from exc


def validate_bundle(bundle: Path, contract: dict[str, Any], version: str) -> PayloadValidation:
    try:
        metadata = bundle.lstat()
        if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
            fail("runtime bundle is not a real directory")
        inventory = build_inventory(bundle)
        ensure_inventory_symlinks_contained(bundle, inventory)
    except InventoryError as exc:
        raise MutationError(str(exc)) from exc
    expected = set(contract["bundle_contract"]["entries"])
    actual = {path for path in inventory if path != "." and "/" not in path}
    if actual != expected:
        fail("runtime bundle entries are not exact")
    input_modes = contract["bundle_contract"]["input_modes"]
    for name in expected:
        entry = inventory[name]
        if entry["type"] != "file" or entry["symlink"] or not mode_matches(input_modes[name], entry["mode"], entry["type"]) or entry["xattrs"]:
            fail(f"runtime bundle entry is not a clean regular file: {name}")
    metadata_value, _ = read_json_bytes(bundle / "octessera-runtime.json")
    if not isinstance(metadata_value, dict) or set(metadata_value) != METADATA_KEYS:
        fail("runtime metadata keys are not exact")
    binary_hash = inventory["octessera-pi"]["sha256"]
    expected_metadata = {"artifact_kind": "production-runtime", "binary_sha256": binary_hash, "name": "octessera-pi", "profile": contract["board_profile"], "runtime_ready": True, "version": version}
    if metadata_value != expected_metadata:
        fail("runtime metadata is not consistent with the requested bundle")
    try:
        sums = (bundle / "SHA256SUMS").read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise MutationError("runtime checksum manifest cannot be read") from exc
    if sums != f"{binary_hash}  octessera-pi\n":
        fail("runtime checksum manifest is not exact")
    return PayloadValidation(inventory, metadata_value, inventory_digest(inventory))


def _copy_file(source: Path, destination: Path, mode: int) -> None:
    try:
        shutil.copyfile(source, destination)
        os.chmod(destination, mode)
        set_owner(destination, 0, 0)
    except OSError as exc:
        raise MutationError(f"cannot stage runtime file: {destination}") from exc


def stage_release(releases: Path, bundle: Path, contract: dict[str, Any], version: str) -> Path:
    output = {item["name"]: item for item in contract["new_release"]["entries"]}
    stage = Path(tempfile.mkdtemp(prefix=".image-respin-stage-", dir=releases))
    try:
        directory = contract["new_release"]["directory"]
        os.chmod(stage, directory["mode"])
        set_owner(stage, directory["uid"], directory["gid"])
        for name, spec in output.items():
            destination = stage / name
            if name == "update-manifest.json":
                destination.write_text(json.dumps(manifest_for(contract["board_profile"], version), sort_keys=True, indent=2) + "\n", encoding="utf-8")
                os.chmod(destination, spec["mode"])
                set_owner(destination, spec["uid"], spec["gid"])
            else:
                _copy_file(bundle / name, destination, spec["mode"])
        return stage
    except Exception:
        remove_path(stage)
        raise


def validate_output(root: Path, inventory: Inventory, bundle_inventory: Inventory, contract: dict[str, Any], version: str, state: dict[str, Any] | None, build_metadata: BuildMetadata | None = None) -> None:
    base = f"{contract['managed']['releases']}/{version}"
    managed_lstat(root, base)
    check_spec(metadata(inventory, base), contract["new_release"]["directory"], "new release directory")
    expected = {item["name"]: item for item in contract["new_release"]["entries"]}
    actual = {path[len(base) + 1:]: value for path, value in inventory.items() if path.startswith(base + "/") and "/" not in path[len(base) + 1:]}
    if set(actual) != set(expected):
        fail("new release entries are not exact")
    for name, spec in expected.items():
        managed_lstat(root, f"{base}/{name}")
        check_spec(actual[name], spec, f"new release {name}")
        expected_hash = bundle_inventory[name]["sha256"] if spec.get("sha256") == "payload" else actual[name]["sha256"]
        if not expected_hash or actual[name]["sha256"] != expected_hash:
            fail(f"new release {name} is not hash-bound")
    if contract["board_profile"] == "raspberry-pi-zero-2w":
        manifest, _ = read_json_bytes(managed_lstat(root, f"{base}/update-manifest.json"))
        if manifest != manifest_for(contract["board_profile"], version):
            fail("new Raspberry release manifest is not exact")
    current = contract["managed"]["current"]
    managed_lstat(root, current)
    managed_lstat(root, contract["managed"]["binary_link"])
    check_spec(metadata(inventory, current), dict(contract["current_link"], target=contract["current_link"]["target"].format(version=version)), "new current release link")
    check_spec(metadata(inventory, contract["managed"]["binary_link"]), contract["binary_link"], "new runtime binary link")
    state_path = contract["state_contract"]["path"]
    if contract["state_contract"]["owned"]:
        managed_lstat(root, state_path)
        check_spec(metadata(inventory, state_path), contract["state_contract"], "new runtime state")
        if state is None or state.get("current") != version or state.get("release") != manifest_for(contract["board_profile"], version):
            fail("new Raspberry state does not describe the new release")
    elif state_path in inventory:
        fail("Orange runtime state was created")
    if contract["board_profile"] == "orange-pi-zero-2w":
        if build_metadata is None:
            fail("Orange build metadata preimage is missing")
        validate_build_metadata_output(root, inventory, contract, cast(BuildMetadata, build_metadata), version, {"octessera-pi": actual["octessera-pi"]["sha256"], "octessera-runtime.json": actual["octessera-runtime.json"]["sha256"], "SHA256SUMS": actual["SHA256SUMS"]["sha256"]})
    elif build_metadata is not None:
        fail("Raspberry runtime does not own Orange build metadata")
