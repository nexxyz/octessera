from __future__ import annotations

import hashlib
import importlib
import json
import sys
from pathlib import Path
from typing import Any

from orange_boot_inventory import OrangeBootInventoryError, capture as capture_protected
from orange_boot_selection import EXPECTED_DTB_NAME, parse_boot_selectors, read_fdtfile


class TrustedParentProofError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise TrustedParentProofError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def digest_object(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")).hexdigest()


def _respin_module(repository_root: Path, name: str) -> Any:
    directory = str(repository_root / "tools/image-respin")
    inserted = directory not in sys.path
    if inserted:
        sys.path.insert(0, directory)
    try:
        return importlib.import_module(name)
    finally:
        if inserted:
            sys.path.remove(directory)


def _tree_identity(path: Path) -> tuple[str, int]:
    records: list[dict[str, Any]] = []
    for child in sorted(path.rglob("*")):
        relative = child.relative_to(path).as_posix()
        if child.is_symlink():
            records.append({"path": relative, "type": "symlink", "target": child.readlink().as_posix()})
        elif child.is_file():
            records.append({"path": relative, "type": "file", "sha256": sha256_file(child), "size": child.stat().st_size})
        elif child.is_dir():
            records.append({"path": relative, "type": "directory"})
        else:
            raise TrustedParentProofError(f"unsupported trusted image entry: {child}")
    encoded = (json.dumps(records, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("utf-8")
    return hashlib.sha256(encoded).hexdigest(), len(encoded)


def artifact_identity(path: Path) -> tuple[str, int]:
    if path.is_file() and not path.is_symlink():
        return sha256_file(path), path.stat().st_size
    if path.is_dir() and not path.is_symlink():
        return _tree_identity(path)
    raise TrustedParentProofError(f"trusted image artifact is not a regular file or directory: {path}")


def load_contract(path: Path, repository_root: Path) -> tuple[Path, dict[str, Any]]:
    expected = repository_root / "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json"
    require(path.name == expected.name and path.resolve(strict=True) == expected.resolve(strict=True), "Orange boot-neutral contract path is not canonical")
    require(path.is_file() and not path.is_symlink(), "Orange boot-neutral contract is missing or symlinked")
    try:
        contract = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TrustedParentProofError("Orange trusted boot-neutral contract is unreadable") from error
    require(set(contract) == {"schema", "schema_version", "proof_mode", "board_profile", "parent_record", "allowed_derivations", "mutation_authority", "boot_mutation", "protected_scopes", "protected_paths", "expected_absent_paths", "selected_boot", "respin_provenance", "setup_proof", "proofs"}, "Orange validated-parent contract changed")
    require(contract["schema"] == "octessera.image-derivation/boot-neutral/v1" and contract["schema_version"] == 1, "Orange trusted boot-neutral schema is invalid")
    require(contract["proof_mode"] == "validated-parent" and contract["board_profile"] == "orange-pi-zero-2w", "Orange validated-parent identity is invalid")
    require(contract["mutation_authority"] == "none" and contract["boot_mutation"] is False, "Orange trusted boot-neutral policy is executable")
    require(contract["allowed_derivations"] == ["runtime-only", "setup-portal"], "Orange trusted derivations changed")
    require(bool(isinstance(contract["protected_scopes"], list) and contract["protected_scopes"] and all(isinstance(item, dict) and set(item) == {"name", "prefix", "kind"} and item["kind"] == "recursive" for item in contract["protected_scopes"])), "Orange trusted protected scopes are not exact")
    require(isinstance(contract["protected_paths"], list) and contract["protected_paths"], "Orange trusted protected inventory is empty")
    require(isinstance(contract["expected_absent_paths"], list) and len(contract["expected_absent_paths"]) == len(set(contract["expected_absent_paths"])) and all(isinstance(item, str) and item and not item.startswith("/") and ".." not in Path(item).parts for item in contract["expected_absent_paths"]), "Orange trusted expected-absent inventory is not exact")
    require(contract["parent_record"] == "resources/image-parents/orange-pi-zero-2w-current.json", "Orange current parent record path changed")
    provenance_contract = contract["respin_provenance"]
    require(set(provenance_contract) == {"runtime_schema", "setup_schema", "schema_version", "required_disk_invariants", "required_parent_binding", "required_runtime_mutation", "policy", "top_level_keys", "setup_top_level_additions", "policy_keys", "boot_integrity_keys", "inventory_keys", "selector_keys"} and provenance_contract["schema_version"] == 2, "Orange trusted provenance contract changed")
    require(provenance_contract["top_level_keys"] == ["proof_schema", "schema_version", "proof_mode", "derivation_kind", "board_profile", "version", "source_identity", "boot_mutation", "phase5_claim", "policy", "parent", "runtime_mutation", "boot_integrity", "disk_invariants", "derived_image", "packaged_artifact", "finalizer"], "Orange trusted provenance top-level contract changed")
    require(provenance_contract["setup_top_level_additions"] == ["setup_mutation", "setup_proof"], "Orange trusted setup provenance contract changed")
    require(provenance_contract["policy_keys"] == ["name", "version", "mutation_authority", "parent_finalization"], "Orange validated-parent policy contract changed")
    require(provenance_contract["boot_integrity_keys"] == ["pre", "post", "selected_kernel", "selected_initramfs", "selected_dtb", "selectors", "protected_scopes", "protected_paths", "expected_absent_paths", "changed_paths"], "Orange trusted boot integrity contract changed")
    require(provenance_contract["inventory_keys"] == ["digest", "count"] and provenance_contract["selector_keys"] == ["format", "kernel", "initramfs", "dtb"], "Orange trusted boot subcontracts changed")
    require(set(contract["setup_proof"]) == {"proof", "schema_version", "required_for", "boot_mutation", "mutation_authority", "source_contract"} and contract["setup_proof"]["proof"] == "setup-layer-mounted", "Orange trusted setup proof contract changed")
    require(len(contract["protected_paths"]) == len(set(contract["protected_paths"])) and contract["expected_absent_paths"] == ["lib/systemd/system-sleep/octessera-orange-oled", "usr/lib/systemd/system-sleep/octessera-orange-oled"] and not set(contract["expected_absent_paths"]) & set(contract["protected_paths"]) and {"etc/systemd/system/octessera-orange-oled-suspend.service", "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service", "usr/local/sbin/octessera-orange-oled-suspend", "usr/local/sbin/octessera-orange-oled-handoff.py"} <= set(contract["protected_paths"]), "Orange trusted OLED protected inventory changed")
    require(contract["proofs"] == ["tools/armbian-image/verify-orange-image.sh", "tools/armbian-image/verify-orange-image.py", "tools/armbian-image/orange_boot_contract.py", "tools/armbian-image/orange_boot_inventory.py", "tools/armbian-image/orange_boot_selection.py", "tools/armbian-image/orange_image_mount.py", "tools/armbian-image/orange_initramfs.py", "tools/armbian-image/orange_phase5_proof.py", "tools/armbian-image/orange_trusted_parent_proof.py", "tools/armbian-image/verify_runtime_account.py", "tools/image-respin/current_parent.py", "tools/image-respin/boot_neutral.py", "resources/image-construction/boot-layers/orange-pi-zero-2w.json", "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json", "tools/kernel-patches/orange-midi-interface-manifest.json"], "Orange validated-parent production proof tool set changed")
    return path, contract


def _load_parent_record(path: Path, contract: dict[str, Any], repository_root: Path) -> tuple[dict[str, Any], dict[str, Any], str]:
    expected = repository_root / contract["parent_record"]
    require(path.resolve(strict=True) == expected.resolve(strict=True), "Orange current parent record path is not canonical")
    require(path.is_file() and not path.is_symlink() and not expected.is_symlink(), "Orange current parent record is not a canonical regular file")
    try:
        module = _respin_module(repository_root, "current_parent")
        record, record_digest = module.load_record(repository_root, path)
        context = module.parent_context(repository_root, path)
    except (OSError, ValueError) as error:
        raise TrustedParentProofError("Orange current parent record is invalid") from error
    return record, context, record_digest


def _protected_state(root: Path, contract: dict[str, Any]) -> dict[str, Any]:
    try:
        inventory, absent = capture_protected(root, contract)
    except OrangeBootInventoryError as error:
        raise TrustedParentProofError(str(error)) from error
    return {"inventory": inventory, "digest": digest_object(inventory), "count": len(inventory), "expected_absent_paths": absent}


def _selected_boot(root: Path, release: str) -> dict[str, str]:
    selected = parse_boot_selectors(root, release)
    return {"selected_kernel": selected["linux"].relative_to(root).as_posix(), "selected_initramfs": selected["initrd"].relative_to(root).as_posix(), "selected_dtb": selected["fdt"].relative_to(root).as_posix()}


def _boot_selectors(root: Path, release: str) -> dict[str, str]:
    boot = root / "boot"
    extlinux = boot / "extlinux/extlinux.conf"
    if extlinux.is_file():
        values: dict[str, str] = {}
        for line in extlinux.read_text(encoding="utf-8").splitlines():
            parts = line.split(None, 1)
            if len(parts) == 2 and parts[0].upper() in {"LINUX", "INITRD", "FDT"}:
                values[parts[0].lower()] = parts[1].strip()
        return {"format": "extlinux.conf", "kernel": values["linux"], "initramfs": values["initrd"], "dtb": values["fdt"]}
    environment = boot / "armbianEnv.txt"
    fdtfile = read_fdtfile(environment)
    return {"format": "armbianEnv.txt", "kernel": "Image", "initramfs": "uInitrd" if (boot / "uInitrd").exists() else f"initrd.img-{release}", "dtb": EXPECTED_DTB_NAME if fdtfile is None else fdtfile}


def _require_unprotected_paths(paths: object, protected_paths: list[str], message: str) -> None:
    require(isinstance(paths, list) and paths == sorted(set(paths)) and all(isinstance(path, str) for path in paths), message)
    if not isinstance(paths, list):
        raise TrustedParentProofError(message)
    normalized = [path.lstrip("/") for path in protected_paths]
    require(not any(protected == changed or protected.startswith(f"{changed}/") or changed.startswith(f"{protected}/") for changed in paths for protected in normalized), message)


def _verify_provenance(path: Path, record_digest: str, contract: dict[str, Any], parent_record: dict[str, Any], parent_context: dict[str, Any], derived_root: Path, derived_identity: tuple[str, int], artifact_identity_value: tuple[str, int], artifact_name: str, derivation_kind: str, repository_root: Path, parent_state: dict[str, Any], derived_state: dict[str, Any], parent_layout: dict[str, Any], derived_layout: dict[str, Any]) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TrustedParentProofError("Orange trusted respin provenance is unreadable") from error
    expected = set(contract["respin_provenance"]["top_level_keys"])
    if derivation_kind == "setup-portal":
        expected |= set(contract["respin_provenance"]["setup_top_level_additions"])
    expected_schema = contract["respin_provenance"]["runtime_schema"] if derivation_kind == "runtime-only" else contract["respin_provenance"]["setup_schema"]
    require(set(document) == expected and document["proof_schema"] == expected_schema and document["schema_version"] == contract["respin_provenance"]["schema_version"], "Orange trusted respin provenance schema changed")
    require(document["proof_mode"] == contract["proof_mode"] and document["derivation_kind"] == derivation_kind and document["board_profile"] == contract["board_profile"], "Orange trusted respin provenance identity changed")
    require(document["boot_mutation"] is False and document["phase5_claim"] is False and document["policy"] == contract["respin_provenance"]["policy"], "Orange trusted boot-neutral policy changed")
    parent = document["parent"]
    expected_record = {"path": contract["parent_record"], "sha256": record_digest, "size": (repository_root / contract["parent_record"]).stat().st_size}
    require(set(parent) == {"record", "context", "image", "digest"}, "Orange validated provenance parent changed")
    require(parent["record"] == expected_record and parent["context"] == parent_context and parent["image"] == parent_record["image"], "Orange validated provenance parent binding changed")
    require(parent["digest"] == digest_object({"context": parent["context"], "record": parent["record"], "image": parent["image"]}), "Orange validated provenance parent digest changed")
    runtime = document["runtime_mutation"]
    require(set(runtime) == {"digest", "provenance"} and isinstance(runtime["provenance"], dict), "Orange trusted runtime provenance changed")
    runtime_provenance = runtime["provenance"]
    require(set(runtime_provenance) == {"proof_schema", "schema_version", "board_profile", "version", "source_identity", "parent", "payload", "mutation_contract", "finalizer", "inventories", "parent_inventory_digest", "post_inventory_digest", "notice", "changed_paths"}, "Orange trusted runtime provenance fields changed")
    require(runtime_provenance["proof_schema"] == "octessera.image-mutation-provenance.v2" and runtime_provenance["schema_version"] == 2, "Orange trusted runtime provenance schema changed")
    require(runtime_provenance["board_profile"] == document["board_profile"] and runtime_provenance["version"] == document["version"] and runtime_provenance["source_identity"] == document["source_identity"], "Orange trusted runtime source binding changed")
    require(runtime["digest"] == digest_object(runtime_provenance), "Orange trusted runtime provenance digest changed")
    notice_module = _respin_module(repository_root, "notice_mutation")
    try:
        notice_module.validate_notice_record(runtime_provenance["notice"], repository_root)
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise TrustedParentProofError(f"Orange trusted notice provenance changed: {error}") from error
    notice_target = notice_module.NOTICE_TARGET
    notice_paths = set(runtime_provenance["notice"]["changed_paths"])
    global_notice_paths = {path for path in runtime_provenance["changed_paths"] if path == notice_target or path.startswith(f"{notice_target}/")}
    require(global_notice_paths == notice_paths, "Orange trusted notice changed paths are not the exact runtime subset")
    runtime_parent = runtime_provenance["parent"]
    require(set(runtime_parent) == {"identity", "digest"} and runtime_parent["digest"] == digest_object(runtime_parent["identity"]), "Orange trusted runtime parent identity changed")
    parent_identity = runtime_parent["identity"]
    require(set(parent_identity) == {"board_profile", "prior_version", "prior_release_entries", "prior_release_digest", "prior_state_preimage_sha256", "prior_build_metadata_preimage_sha256", "current_target", "parent_context", "parent_context_sha256"}, "Orange trusted runtime parent fields changed")
    require(parent_identity["board_profile"] == contract["board_profile"] and parent_identity["prior_version"] == parent_context["version"] and parent_identity["parent_context"] == document["parent"]["context"], "Orange validated runtime parent binding changed")
    require(parent_identity["parent_context_sha256"] == digest_object(document["parent"]["context"]), "Orange validated runtime parent context digest changed")
    require(set(parent_identity["prior_release_entries"]) == {"octessera-pi", "octessera-runtime.json", "SHA256SUMS", "update-manifest.json"}, "Orange trusted runtime release inventory changed")
    require(isinstance(parent_identity["prior_state_preimage_sha256"], str) and len(parent_identity["prior_state_preimage_sha256"]) == 64 and isinstance(parent_identity["prior_build_metadata_preimage_sha256"], str), "Orange trusted runtime parent preimage changed")
    payload = runtime_provenance["payload"]
    require(set(payload) == {"digest"} and isinstance(payload["digest"], str) and len(payload["digest"]) == 64, "Orange trusted runtime payload changed")
    protected_paths = contract["protected_paths"]
    _require_unprotected_paths(runtime_provenance.get("changed_paths", []), protected_paths, "Orange trusted runtime changed paths are not exact")
    mutation_contract = runtime_provenance["mutation_contract"]
    mutation_path = repository_root / "resources/image-mutations/orange-pi-zero-2w.json"
    require(set(mutation_contract) == {"digest"} and mutation_contract["digest"] == sha256_file(mutation_path), "Orange trusted runtime mutation contract changed")
    inventories = runtime_provenance["inventories"]
    require(set(inventories) == {"pre", "post"} and runtime_provenance["parent_inventory_digest"] == inventories["pre"] and runtime_provenance["post_inventory_digest"] == inventories["post"], "Orange trusted runtime inventory aliases changed")
    require(all(isinstance(value, str) and len(value) == 64 for value in inventories.values()), "Orange trusted runtime inventory digest changed")
    runtime_tool = _respin_module(repository_root, "provenance").tool_code_model(repository_root / "tools/image-respin")
    compression = _respin_module(repository_root, "disk_packaging").compression_identity(contract["board_profile"])
    require(runtime_provenance["finalizer"] == {"source_identity": document["source_identity"], "tool_identity": "octessera-image-respin-runtime-mutation/2", "tool_code_schema": runtime_tool["schema"], "tool_code_version": runtime_tool["version"], "tool_code_digest": runtime_tool["digest"], "tool_code_files": runtime_tool["files"]}, "Orange trusted runtime finalizer changed")
    disk = document["disk_invariants"]
    require(set(disk) == {"pre", "post", "digest"} and disk["pre"] == disk["post"] and parent_layout == derived_layout == disk["pre"], "Orange trusted disk invariants drifted")
    require(disk["digest"] == digest_object({"pre": disk["pre"], "post": disk["post"]}), "Orange trusted disk invariant digest changed")
    layout = disk["pre"]
    selected = contract["selected_boot"]
    require(set(layout) == {"board_profile", "image_size", "table_label", "disk_id", "first_lba", "last_lba", "sector_size", "partitions", "raw_prepartition_sha256", "raw_boot_partition_sha256"}, "Orange trusted disk layout fields changed")
    require(layout["board_profile"] == contract["board_profile"] and len(layout["partitions"]) == selected["partition_count"], "Orange trusted disk layout changed")
    require([part["filesystem_type"] for part in layout["partitions"]] == selected["filesystem_types"], "Orange trusted filesystem layout changed")
    require(layout["raw_boot_partition_sha256"] == selected["raw_boot_partition_sha256"], "Orange trusted raw boot partition changed")
    require(document["derived_image"] == {"sha256": derived_identity[0], "size": derived_identity[1]}, "Orange derived image identity is not provenance-bound")
    require(document["packaged_artifact"] == {"sha256": artifact_identity_value[0], "size": artifact_identity_value[1], "path": artifact_name}, "Orange trusted packaging proof changed")
    expected_finalizer = {"tool_identity", "compression_identity", "runtime_tool_code_schema", "runtime_tool_code_version", "runtime_tool_code_digest", "runtime_tool_code_files"} if derivation_kind == "runtime-only" else {"tool_identity", "compression_identity", "setup_tool_code"}
    require(set(document["finalizer"]) == expected_finalizer, "Orange trusted finalizer identity changed")
    boot_integrity = document["boot_integrity"]
    require(set(boot_integrity) == set(contract["respin_provenance"]["boot_integrity_keys"]), "Orange trusted boot integrity keys changed")
    for inventory in (boot_integrity["pre"], boot_integrity["post"]):
        require(set(inventory) == set(contract["respin_provenance"]["inventory_keys"]) and isinstance(inventory["digest"], str) and len(inventory["digest"]) == 64 and isinstance(inventory["count"], int) and inventory["count"] >= 0, "Orange trusted boot inventory changed")
    require(boot_integrity["pre"] == {"digest": derived_state["digest"], "count": derived_state["count"]} and boot_integrity["post"] == boot_integrity["pre"] and parent_state == derived_state, "Orange trusted boot inventory drifted")
    selected_boot = _selected_boot(derived_root, contract["selected_boot"]["kernel_release"])
    require({key: boot_integrity[key] for key in ("selected_kernel", "selected_initramfs", "selected_dtb")} == selected_boot, "Orange trusted selected boot changed")
    require(boot_integrity["selectors"] == _boot_selectors(derived_root, contract["selected_boot"]["kernel_release"]), "Orange trusted boot selectors changed")
    require(boot_integrity["protected_scopes"] == contract["protected_scopes"] and boot_integrity["protected_paths"] == contract["protected_paths"] and boot_integrity["expected_absent_paths"] == contract["expected_absent_paths"] and derived_state["expected_absent_paths"] == contract["expected_absent_paths"] and boot_integrity["changed_paths"] == [], "Orange trusted protected boot inventory changed")
    if derivation_kind == "setup-portal":
        setup_mutation = document["setup_mutation"]
        require(set(setup_mutation) == {"digest", "provenance"} and setup_mutation["digest"] == digest_object(setup_mutation["provenance"]), "Orange setup mutation proof changed")
        require(set(setup_mutation["provenance"]) == {"proof_schema", "schema_version", "board_profile", "source_identity", "parent", "setup_layer", "inventories", "changed_paths", "finalizer"}, "Orange setup mutation provenance changed")
        require(setup_mutation["provenance"]["board_profile"] == document["board_profile"] and setup_mutation["provenance"]["source_identity"] == document["source_identity"], "Orange setup source binding changed")
        _require_unprotected_paths(setup_mutation["provenance"]["changed_paths"], protected_paths, "Orange trusted setup changed paths touch protected boot paths")
        setup_tool = _respin_module(repository_root, "setup_provenance").setup_tool_code_model(repository_root / "tools/image-respin")
        require(document["finalizer"] == {"tool_identity": "octessera-image-respin-runtime-mutation/1", "compression_identity": compression, "setup_tool_code": setup_tool}, "Orange trusted setup finalizer changed")
    if derivation_kind == "runtime-only":
        require(document["finalizer"] == {"tool_identity": "octessera-image-respin-runtime-mutation/1", "compression_identity": compression, "runtime_tool_code_schema": runtime_tool["schema"], "runtime_tool_code_version": runtime_tool["version"], "runtime_tool_code_digest": runtime_tool["digest"], "runtime_tool_code_files": runtime_tool["files"]}, "Orange trusted runtime finalizer changed")
    return document


def _verify_setup_proof(path: Path, contract: dict[str, Any], repository_root: Path, embedded: dict[str, Any]) -> None:
    try:
        proof = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TrustedParentProofError("Orange setup proof is unreadable") from error
    expected = {"proof", "schema_version", "board_profile", "contract_sha256", "inventory_sha256", "prerequisites", "verified_paths"}
    require(set(proof) == expected and proof["proof"] == contract["setup_proof"]["proof"] and proof["schema_version"] == contract["setup_proof"]["schema_version"], "Orange setup proof schema changed")
    require(proof["board_profile"] == contract["board_profile"] and proof == embedded, "Orange setup proof identity changed")
    source_contract = repository_root / contract["setup_proof"]["source_contract"]
    require(proof["contract_sha256"] == sha256_file(source_contract), "Orange setup source proof is not exact")
    setup_module = _respin_module(repository_root, "setup_contract")
    setup_contract, setup_digest = setup_module.load_contract(source_contract)
    require(setup_contract["board_profile"] == contract["board_profile"] and setup_digest == proof["contract_sha256"] and all(value is False for key, value in setup_contract["recipe"].items() if key.endswith("_mutation")), "Orange setup contract permits boot mutation")
    require(isinstance(proof["prerequisites"], dict) and proof["verified_paths"] == sorted(set(proof["verified_paths"])), "Orange setup proof paths are not exact")


def verify_trusted_roots(parent_root: Path, derived_root: Path, parent_record: dict[str, Any], parent_context: dict[str, Any], record_digest: str, contract: dict[str, Any], provenance_path: Path, derivation_kind: str, setup_proof: Path | None, repository_root: Path, derived_identity: tuple[str, int], artifact_identity_value: tuple[str, int], artifact_name: str, parent_layout: dict[str, Any], derived_layout: dict[str, Any], record_path: Path | None = None, contract_path: Path | None = None) -> dict[str, Any]:
    parent_asset = parent_record["image"]
    require(parent_record["board_profile"] == contract["board_profile"] and parent_context["image"] == parent_asset, "Orange validated parent record binding changed")
    parent_state = _protected_state(parent_root, contract)
    derived_state = _protected_state(derived_root, contract)
    require(parent_state == derived_state, "Orange trusted protected inventory changed")
    require(parent_layout == derived_layout, "Orange trusted raw disk identity changed")
    provenance = _verify_provenance(provenance_path, record_digest, contract, parent_record, parent_context, derived_root, derived_identity, artifact_identity_value, artifact_name, derivation_kind, repository_root, parent_state, derived_state, parent_layout, derived_layout)
    notice_module = _respin_module(repository_root, "notice_mutation")
    try:
        notice_module.verify_mounted_notice_tree(derived_root, provenance["runtime_mutation"]["provenance"]["notice"])
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise TrustedParentProofError(f"Orange mounted notice tree changed: {error}") from error
    inventory_module = _respin_module(repository_root, "inventory")
    parent_inventory = inventory_module.build_inventory(parent_root)
    derived_inventory = inventory_module.build_inventory(derived_root)
    for sentinel in ("usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"):
        require(sentinel in parent_inventory and sentinel in derived_inventory and parent_inventory[sentinel] == derived_inventory[sentinel], f"Orange vendor legal sentinel changed: {sentinel}")
    if derivation_kind == "setup-portal":
        require(setup_proof is not None, "setup-portal derivation requires setup proof")
        if setup_proof is None:
            raise TrustedParentProofError("setup-portal derivation requires setup proof")
        _verify_setup_proof(setup_proof, contract, repository_root, provenance["setup_proof"])
    else:
        require(setup_proof is None, "runtime-only derivation rejects setup proof")
    parent_selected = _selected_boot(parent_root, contract["selected_boot"]["kernel_release"])
    derived_selected = _selected_boot(derived_root, contract["selected_boot"]["kernel_release"])
    require(parent_selected == derived_selected and Path(parent_selected["selected_dtb"]).name == contract["selected_boot"]["dtb_name"], "Orange trusted selected boot changed")
    record_relative = str(record_path.relative_to(repository_root)) if record_path is not None and record_path.is_relative_to(repository_root) else contract["parent_record"]
    contract_relative = str(contract_path.relative_to(repository_root)) if contract_path is not None and contract_path.is_relative_to(repository_root) else "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json"
    return {"schema": "octessera.image-proof/v2", "schema_version": 2, "proof_mode": contract["proof_mode"], "phase5_claim": False, "boot_state": "current-parent-preserved", "artifact": {"name": artifact_name, "sha256": artifact_identity_value[0], "size": artifact_identity_value[1]}, "board_profile": contract["board_profile"], "runtime": {"derivation_kind": derivation_kind, "setup_proof": derivation_kind == "setup-portal", "boot_mutation": False}, "parent": {"record": {"path": record_relative, "sha256": record_digest, "size": (repository_root / contract["parent_record"]).stat().st_size}, "image": parent_asset}, "selected_boot": derived_selected, "contract": {"path": contract_relative, "sha256": sha256_file(contract_path) if contract_path is not None else digest_object(contract)}, "respin_provenance_sha256": sha256_file(provenance_path)}


def verify_trusted(parent_root: Path, derived_root: Path, parent_image: Path, contract_path: Path, record_path: Path, provenance_path: Path, derivation_kind: str, setup_proof: Path | None, repository_root: Path, derived_identity: tuple[str, int], artifact_identity_value: tuple[str, int], artifact_name: str, parent_layout: dict[str, Any], derived_layout: dict[str, Any]) -> dict[str, Any]:
    contract_path, contract = load_contract(contract_path, repository_root)
    parent_record, parent_context, record_digest = _load_parent_record(record_path, contract, repository_root)
    parent_identity = artifact_identity(parent_image)
    require(parent_image.name == parent_record["image"]["name"] and parent_identity == (parent_record["image"]["sha256"], parent_record["image"]["size"]), "Orange validated parent path, hash, or size changed")
    return verify_trusted_roots(parent_root, derived_root, parent_record, parent_context, record_digest, contract, provenance_path, derivation_kind, setup_proof, repository_root, derived_identity, artifact_identity_value, artifact_name, parent_layout, derived_layout, record_path, contract_path)
