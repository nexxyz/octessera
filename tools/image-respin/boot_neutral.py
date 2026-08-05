from __future__ import annotations

import hashlib
import importlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

try:
    from .disk_layout import DiskLayout
    from .inventory import inventory_digest
    from .orange_boot_inventory import OrangeBootInventoryError, capture as capture_protected
except ImportError:
    from disk_layout import DiskLayout
    from inventory import inventory_digest
    _orange_tools = str(Path(__file__).resolve().parents[1] / "armbian-image")
    if _orange_tools not in sys.path:
        sys.path.insert(0, _orange_tools)
    from orange_boot_inventory import OrangeBootInventoryError, capture as capture_protected


class BootNeutralError(ValueError):
    pass


ORANGE = "orange-pi-zero-2w"
PROOF_MODE = "trusted-v0.7.5-boot-neutral"
CONTRACT_RELATIVE = "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json"
CAPTURE_KEYS = {"protected_inventory", "inventory_digest", "inventory_count", "expected_absent_paths", "selected_boot", "selectors", "disk_layout"}
INVENTORY_KEYS = {"digest", "count"}
SELECTOR_KEYS = {"format", "kernel", "initramfs", "dtb"}


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise BootNeutralError(message)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise BootNeutralError(f"cannot hash boot-neutral identity: {path}") from exc
    return digest.hexdigest()


def _exact(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    _require(isinstance(value, dict) and set(value) == keys, f"{label} keys changed")
    return value


def _relative_contract_path(root: Path, path: Path, label: str) -> str:
    try:
        return path.resolve(strict=True).relative_to(root.resolve(strict=True)).as_posix()
    except (OSError, ValueError) as exc:
        raise BootNeutralError(f"{label} is outside the repository") from exc


@dataclass(frozen=True)
class BootNeutralPolicy:
    root: Path
    path: Path
    contract: dict[str, Any]
    sha256: str

    @property
    def proof_mode(self) -> str:
        return str(self.contract["proof_mode"])

    @property
    def policy(self) -> dict[str, Any]:
        return dict(self.contract["respin_provenance"]["policy"])


def load_policy(repository_root: Path, path: Path | None = None) -> BootNeutralPolicy:
    root = Path(repository_root).resolve(strict=True)
    expected = root / CONTRACT_RELATIVE
    candidate = expected if path is None else Path(path)
    _require(candidate.resolve(strict=True) == expected, "Orange boot-neutral contract path is not canonical")
    _require(candidate.is_file() and not candidate.is_symlink(), "Orange boot-neutral contract is missing or symlinked")
    try:
        raw = candidate.read_bytes()
        contract = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise BootNeutralError("Orange boot-neutral contract is unreadable") from exc
    top = _exact(contract, {"schema", "schema_version", "proof_mode", "board_profile", "parent_trust_manifest", "parent_release", "parent_asset", "allowed_derivations", "mutation_authority", "boot_mutation", "protected_scopes", "protected_paths", "expected_absent_paths", "selected_boot", "respin_provenance", "setup_proof", "proofs"}, "Orange boot-neutral contract")
    _require(top["schema"] == "octessera.image-derivation/boot-neutral/v1" and top["schema_version"] == 1 and top["proof_mode"] == PROOF_MODE and top["board_profile"] == ORANGE, "Orange boot-neutral contract identity changed")
    _require(top["parent_trust_manifest"] == "resources/image-parents/v0.7.5-trust-manifest.json" and top["mutation_authority"] == "none" and top["boot_mutation"] is False, "Orange boot-neutral policy changed")
    _require(top["allowed_derivations"] == ["runtime-only", "setup-portal"], "Orange boot-neutral derivation kinds changed")
    scopes = top["protected_scopes"]
    _require(bool(isinstance(scopes, list) and scopes and all(isinstance(item, dict) and set(item) == {"name", "prefix", "kind"} for item in scopes)), "Orange protected scopes are not exact")
    _require(bool(len({item["name"] for item in scopes}) == len(scopes) and all(item["kind"] == "recursive" and isinstance(item["prefix"], str) and item["prefix"] and not item["prefix"].startswith("/") and "\\" not in item["prefix"] and ".." not in PurePosixPath(item["prefix"]).parts for item in scopes)), "Orange protected scope prefixes are not exact")
    paths = top["protected_paths"]
    _require(isinstance(paths, list) and len(paths) == len(set(paths)) and all(isinstance(item, str) and item and not item.startswith("/") and "\\" not in item and ".." not in PurePosixPath(item).parts for item in paths), "Orange protected paths are not exact")
    absent = top["expected_absent_paths"]
    _require(bool(isinstance(absent, list) and len(absent) == len(set(absent)) and all(isinstance(item, str) and item and not item.startswith("/") and "\\" not in item and ".." not in PurePosixPath(item).parts for item in absent)), "Orange expected-absent paths are not exact")
    release = _exact(top["parent_release"], {"repository", "tag", "url", "published_at", "source_commit", "asset_count", "is_draft", "is_prerelease"}, "Orange parent release")
    _require(release["repository"] == "nexxyz/octessera" and release["tag"] == "v0.7.5" and release["source_commit"] == "4eec2b7edf6619fa22c709d4a589237a5748de78" and release["is_draft"] is False and release["is_prerelease"] is False, "Orange parent release identity changed")
    asset = _exact(top["parent_asset"], {"artifact_class", "name", "content_type", "size", "sha256"}, "Orange parent asset")
    _require(asset == {"artifact_class": "trusted-production-parent", "name": "octessera-0.7.5-orange-pi-zero-2w.img.xz", "content_type": "application/x-xz", "size": 353061152, "sha256": "ecf1cb7e4174ef6a149be306854ebcb1667ed55f6ab5de583af62a1c147d9517"}, "Orange parent asset identity changed")
    selected = _exact(top["selected_boot"], {"kernel_release", "dtb_name", "filesystem_types", "partition_count", "raw_boot_partition_sha256"}, "Orange selected boot policy")
    _require(selected == {"kernel_release": "6.18.38-current-sunxi64", "dtb_name": "sun50i-h618-orangepi-zero2w.dtb", "filesystem_types": ["ext4"], "partition_count": 1, "raw_boot_partition_sha256": None}, "Orange selected boot policy changed")
    provenance = _exact(top["respin_provenance"], {"runtime_schema", "setup_schema", "schema_version", "required_disk_invariants", "required_parent_binding", "required_runtime_mutation", "policy", "top_level_keys", "setup_top_level_additions", "policy_keys", "boot_integrity_keys", "inventory_keys", "selector_keys"}, "Orange provenance policy")
    _require(provenance["runtime_schema"] == "octessera.image-derived-respin-provenance.v2" and provenance["setup_schema"] == "octessera.image-derived-setup-respin-provenance.v2" and provenance["schema_version"] == 2, "Orange provenance schema policy changed")
    _require(provenance["required_disk_invariants"] is True and provenance["required_parent_binding"] is True and provenance["required_runtime_mutation"] is True, "Orange provenance requirements changed")
    _require(provenance["top_level_keys"] == ["proof_schema", "schema_version", "proof_mode", "derivation_kind", "board_profile", "version", "source_identity", "boot_mutation", "phase5_claim", "policy", "parent", "runtime_mutation", "boot_integrity", "disk_invariants", "derived_image", "packaged_artifact", "finalizer"], "Orange provenance top-level keys changed")
    _require(provenance["setup_top_level_additions"] == ["setup_mutation", "setup_proof"] and provenance["policy_keys"] == ["name", "version", "mutation_authority", "trusted_parent_finalization"] and provenance["boot_integrity_keys"] == ["pre", "post", "selected_kernel", "selected_initramfs", "selected_dtb", "selectors", "protected_scopes", "protected_paths", "expected_absent_paths", "changed_paths"] and provenance["inventory_keys"] == ["digest", "count"] and provenance["selector_keys"] == ["format", "kernel", "initramfs", "dtb"], "Orange provenance field policy changed")
    _require(provenance["policy"] == {"name": PROOF_MODE, "version": 1, "mutation_authority": "none", "trusted_parent_finalization": "forbidden"}, "Orange provenance policy identity changed")
    setup = _exact(top["setup_proof"], {"proof", "schema_version", "required_for", "boot_mutation", "mutation_authority", "source_contract"}, "Orange setup proof policy")
    _require(setup["proof"] == "setup-layer-mounted" and setup["schema_version"] == 1 and setup["required_for"] == "setup-portal" and setup["boot_mutation"] is False and setup["mutation_authority"] == "none" and setup["source_contract"] == "resources/image-mutations/orange-pi-zero-2w-setup.json", "Orange setup proof policy changed")
    _require(top["expected_absent_paths"] == ["usr/local/sbin/octessera-orange-oled-handoff.py"], "Orange expected-absent paths changed")
    _require(not set(top["expected_absent_paths"]) & set(top["protected_paths"]), "Orange expected-absent path is protected as present")
    _require(isinstance(top["proofs"], list) and top["proofs"] == ["tools/armbian-image/verify-orange-image.sh", "tools/armbian-image/verify-orange-image.py", "tools/armbian-image/orange_boot_contract.py", "tools/armbian-image/orange_boot_inventory.py", "tools/armbian-image/orange_boot_selection.py", "tools/armbian-image/orange_image_mount.py", "tools/armbian-image/orange_initramfs.py", "tools/armbian-image/orange_phase5_proof.py", "tools/armbian-image/orange_trusted_parent_proof.py", "tools/armbian-image/verify_runtime_account.py", "tools/image-respin/boot_neutral.py", "resources/image-construction/boot-layers/orange-pi-zero-2w.json", "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json", "tools/kernel-patches/orange-midi-interface-manifest.json"], "Orange proof list changed")
    return BootNeutralPolicy(root, expected, contract, hashlib.sha256(raw).hexdigest())


def _orange_proof_module(policy: BootNeutralPolicy) -> Any:
    tools = policy.root / "tools/armbian-image"
    value = str(tools)
    inserted = value not in sys.path
    if inserted:
        sys.path.insert(0, value)
    try:
        return importlib.import_module("orange_trusted_parent_proof")
    finally:
        if inserted:
            sys.path.remove(value)


def _validate_layout(policy: BootNeutralPolicy, layout: DiskLayout) -> dict[str, Any]:
    value = layout.as_dict()
    selected = policy.contract["selected_boot"]
    _require(value["board_profile"] == ORANGE and len(value["partitions"]) == selected["partition_count"], "Orange disk layout is not boot-neutral")
    _require([item["filesystem_type"] for item in value["partitions"]] == selected["filesystem_types"] and value["raw_boot_partition_sha256"] == selected["raw_boot_partition_sha256"], "Orange filesystem or raw disk identity changed")
    return value


def capture_state(policy: BootNeutralPolicy, root: Path, layout: DiskLayout) -> dict[str, Any]:
    image_root = Path(root).resolve(strict=True)
    _require(image_root.is_dir() and not image_root.is_symlink(), "Orange boot-neutral root is not a real directory")
    module = _orange_proof_module(policy)
    selected_boot = module._selected_boot(image_root, policy.contract["selected_boot"]["kernel_release"])
    selectors = module._boot_selectors(image_root, policy.contract["selected_boot"]["kernel_release"])
    _require(set(selectors) == set(policy.contract["respin_provenance"]["selector_keys"]), "Orange boot selectors are not exact")
    try:
        inventory, absent = capture_protected(image_root, policy.contract)
    except OrangeBootInventoryError as error:
        raise BootNeutralError(str(error)) from error
    return {"protected_inventory": inventory, "inventory_digest": inventory_digest(inventory), "inventory_count": len(inventory), "expected_absent_paths": absent, "selected_boot": selected_boot, "selectors": selectors, "disk_layout": _validate_layout(policy, layout)}


def assert_unchanged(policy: BootNeutralPolicy, before: dict[str, Any], after: dict[str, Any], post_layout: DiskLayout) -> None:
    _require(set(before) == CAPTURE_KEYS and set(after) == CAPTURE_KEYS, "Orange boot-neutral capture keys changed")
    for value in (before, after):
        _require(value["inventory_digest"] == inventory_digest(value["protected_inventory"]) and value["inventory_count"] == len(value["protected_inventory"]), "Orange protected inventory digest changed")
        _require(value["expected_absent_paths"] == policy.contract["expected_absent_paths"], "Orange expected-absent path inventory changed")
    _require(before["protected_inventory"] == after["protected_inventory"] and before["inventory_digest"] == after["inventory_digest"] and before["inventory_count"] == after["inventory_count"], "Orange protected boot inventory drifted")
    _require(before["expected_absent_paths"] == after["expected_absent_paths"], "Orange expected-absent path drifted")
    _require(before["selected_boot"] == after["selected_boot"] and before["selectors"] == after["selectors"], "Orange selected boot or selectors changed")
    post = _validate_layout(policy, post_layout)
    _require(before["disk_layout"] == after["disk_layout"] == post, "Orange disk or filesystem identity drifted")


def parent_binding(policy: BootNeutralPolicy, manifest_path: Path, manifest_digest: str, parent_context: dict[str, Any]) -> dict[str, Any]:
    expected_manifest = policy.root / policy.contract["parent_trust_manifest"]
    _require(Path(manifest_path).resolve(strict=True) == expected_manifest.resolve(strict=True), "Orange trusted manifest path is not canonical")
    _require(Path(manifest_path).is_file() and not Path(manifest_path).is_symlink() and not expected_manifest.is_symlink(), "Orange trusted manifest is not a canonical regular file")
    _require(_sha256(expected_manifest) == manifest_digest, "Orange trusted manifest digest changed")
    _require(set(parent_context) == {"schema", "repository", "tag", "source_commit", "asset"}, "Orange parent context keys changed")
    _require(parent_context["schema"] == "octessera.image-parent-trust/v1" and parent_context["repository"] == policy.contract["parent_release"]["repository"] and parent_context["tag"] == policy.contract["parent_release"]["tag"] and parent_context["source_commit"] == policy.contract["parent_release"]["source_commit"], "Orange parent context changed")
    asset = parent_context["asset"]
    _require(set(asset) == {"name", "node_id", "size", "sha256"} and asset["name"] == policy.contract["parent_asset"]["name"] and asset["size"] == policy.contract["parent_asset"]["size"] and asset["sha256"] == policy.contract["parent_asset"]["sha256"], "Orange parent asset identity changed")
    return {"context": parent_context, "asset": asset, "trust_manifest_sha256": manifest_digest, "digest": _digest_parent(parent_context, manifest_digest)}


def _digest_parent(context: dict[str, Any], manifest_digest: str) -> str:
    payload = json.dumps({"context": context, "trust_manifest_sha256": manifest_digest}, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def build_integrity(policy: BootNeutralPolicy, before: dict[str, Any], after: dict[str, Any], post_layout: DiskLayout) -> dict[str, Any]:
    assert_unchanged(policy, before, after, post_layout)
    return {"pre": {"digest": before["inventory_digest"], "count": before["inventory_count"]}, "post": {"digest": after["inventory_digest"], "count": after["inventory_count"]}, "selected_kernel": after["selected_boot"]["selected_kernel"], "selected_initramfs": after["selected_boot"]["selected_initramfs"], "selected_dtb": after["selected_boot"]["selected_dtb"], "selectors": after["selectors"], "protected_scopes": list(policy.contract["protected_scopes"]), "protected_paths": list(policy.contract["protected_paths"]), "expected_absent_paths": list(policy.contract["expected_absent_paths"]), "changed_paths": []}


__all__ = ["BootNeutralError", "BootNeutralPolicy", "CONTRACT_RELATIVE", "PROOF_MODE", "assert_unchanged", "build_integrity", "capture_state", "load_policy", "parent_binding"]
