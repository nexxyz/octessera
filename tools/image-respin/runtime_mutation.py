from __future__ import annotations

import argparse
import json
import os
import re
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, cast

try:
    from .inventory import Inventory, InventoryError, build_inventory, inventory_digest, remove_path
    from .provenance import TOOL_IDENTITY, build_provenance, canonical_source_identity
    from .runtime_contract import MutationError, contract_for_board, load_contract, read_json_bytes, rooted, transform_build_metadata, validate_changed_paths, validate_parent
    from .runtime_payload import PayloadValidation, stage_release, validate_bundle, validate_output
    from .runtime_transaction import MutableSnapshot, atomic_bytes, atomic_link
except ImportError:
    from inventory import Inventory, InventoryError, build_inventory, inventory_digest, remove_path
    from provenance import TOOL_IDENTITY, build_provenance, canonical_source_identity
    from runtime_contract import MutationError, contract_for_board, load_contract, read_json_bytes, rooted, transform_build_metadata, validate_changed_paths, validate_parent
    from runtime_payload import PayloadValidation, stage_release, validate_bundle, validate_output
    from runtime_transaction import MutableSnapshot, atomic_bytes, atomic_link


VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


@dataclass(frozen=True)
class MutationResult:
    board_profile: str
    prior_version: str
    version: str
    source_identity: str | dict[str, Any]
    contract_digest: str
    payload_digest: str
    parent_identity: dict[str, Any]
    pre_inventory_digest: str
    post_inventory_digest: str
    changed_paths: list[str]
    provenance: dict[str, Any]


def _root_is_real(path: Path, label: str) -> Path:
    path = Path(path).absolute()
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise MutationError(f"{label} is unavailable") from exc
    if not metadata or not path.is_dir() or path.is_symlink():
        raise MutationError(f"{label} must be a real directory")
    return path.resolve(strict=False)


def _state_payload(contract: dict[str, Any], state: dict[str, Any] | None, version: str) -> tuple[dict[str, Any] | None, bytes | None]:
    if not contract["state_contract"]["owned"]:
        return None, None
    if state is None:
        raise MutationError("Raspberry runtime state is missing")
    result = dict(state)
    manifest = {"schema_version": 2, "updater_protocol": 2, "candidate_health_protocol": 1, "tag": f"v{version}", "version": version, "board_profile": contract["board_profile"], "arch": "aarch64-unknown-linux-gnu", "binary": "octessera-pi", "platforms": [contract["board_profile"], "linux-aarch64-device"]}
    result.update({"current": version, "previous": None, "next": None, "release": manifest})
    return result, (json.dumps(result, sort_keys=True, indent=2) + "\n").encode("utf-8")


def _assert_staged_preimage(root: Path, before: Inventory, stage: Path) -> None:
    current = build_inventory(root)
    stage_relative = stage.relative_to(root).as_posix()
    filtered = {path: entry for path, entry in current.items() if path != stage_relative and not path.startswith(stage_relative + "/")}
    if filtered != before:
        raise MutationError("root changed outside the private staged release before commit")


def mutate_runtime(
    root: Path,
    bundle: Path,
    board_profile: str,
    version: str,
    source_identity: object,
    parent_context: object,
    *,
    contract_path: Path | None = None,
    mutation_hook: Callable[[str], None] | None = None,
    tool_identity: str = TOOL_IDENTITY,
) -> MutationResult:
    if not VERSION_RE.fullmatch(version):
        raise MutationError("runtime version must be strict semver")
    source = canonical_source_identity(source_identity)
    root = _root_is_real(root, "mounted root")
    bundle = Path(bundle).absolute()
    try:
        bundle_metadata = bundle.lstat()
    except OSError as exc:
        raise MutationError("runtime bundle is unavailable") from exc
    if stat.S_ISLNK(bundle_metadata.st_mode) or not stat.S_ISDIR(bundle_metadata.st_mode):
        raise MutationError("runtime bundle must be a real directory")
    contract, contract_digest = load_contract(contract_path or contract_for_board(board_profile))
    if contract["board_profile"] != board_profile or contract["binary"] != "octessera-pi":
        raise MutationError("mutation contract does not match the requested board")
    try:
        before = build_inventory(root)
        parent = validate_parent(root, before, contract, parent_context)
        payload: PayloadValidation = validate_bundle(bundle, contract, version)
    except InventoryError as exc:
        raise MutationError(str(exc)) from exc
    if version != parent.prior_version and f"{contract['managed']['releases']}/{version}" in before:
        raise MutationError("requested release already exists outside the exact prior release")
    releases = rooted(root, contract["managed"]["releases"])
    target = rooted(root, f"{contract['managed']['releases']}/{version}")
    prior_path = rooted(root, f"{contract['managed']['releases']}/{parent.prior_version}")
    current_path = rooted(root, contract["managed"]["current"])
    binary_path = rooted(root, contract["managed"]["binary_link"])
    state_path = rooted(root, contract["state_contract"]["path"])
    state, state_bytes = _state_payload(contract, parent.state, version)
    build_metadata_path = rooted(root, contract["managed"]["build_metadata"]) if parent.build_metadata is not None else None
    new_release_hashes = {name: payload.inventory[name]["sha256"] for name in ("octessera-pi", "octessera-runtime.json", "SHA256SUMS")} if parent.build_metadata is not None else None
    build_metadata_bytes = transform_build_metadata(cast(Any, parent.build_metadata), version, cast(dict[str, str], new_release_hashes)) if parent.build_metadata is not None and new_release_hashes is not None else None
    mutable_roots = [f"{contract['managed']['releases']}/{parent.prior_version}", f"{contract['managed']['releases']}/{version}", contract["managed"]["current"], contract["managed"]["binary_link"]]
    if contract["state_contract"]["owned"]:
        mutable_roots.append(contract["state_contract"]["path"])
    if parent.build_metadata is not None:
        mutable_roots.append(contract["managed"]["build_metadata"])
    mutable_roots = list(dict.fromkeys(mutable_roots))
    snapshot = MutableSnapshot.capture(root, before, tuple(mutable_roots), (f"{contract['managed']['releases']}/.image-respin-stage-*", f"{contract['managed']['releases']}/.image-respin-backup-*"))
    stage: Path | None = None
    backup: Path | None = None
    try:
        stage = stage_release(releases, bundle, contract, version)
        if mutation_hook:
            mutation_hook("staged")
        _assert_staged_preimage(root, before, stage)
        backup = Path(tempfile.mkdtemp(prefix=".image-respin-backup-", dir=releases))
        backup.rmdir()
        os.replace(prior_path, backup)
        if mutation_hook:
            mutation_hook("prior-release-moved")
        os.replace(stage, target)
        stage = None
        if mutation_hook:
            mutation_hook("release-installed")
        atomic_link(current_path, contract["current_link"]["target"].format(version=version))
        if mutation_hook:
            mutation_hook("current-replaced")
        atomic_link(binary_path, contract["binary_link"]["target"])
        if mutation_hook:
            mutation_hook("binary-replaced")
        if build_metadata_bytes is not None and build_metadata_path is not None:
            atomic_bytes(build_metadata_path, build_metadata_bytes, contract["build_metadata_contract"]["mode"])
            if mutation_hook:
                mutation_hook("build-metadata-replaced")
        if state_bytes is not None:
            atomic_bytes(state_path, state_bytes, contract["state_contract"]["mode"])
            if mutation_hook:
                mutation_hook("state-replaced")
        after_with_backup = build_inventory(root)
        backup_relative = backup.relative_to(root).as_posix() if backup is not None else ""
        after = {path: entry for path, entry in after_with_backup.items() if not backup_relative or (path != backup_relative and not path.startswith(backup_relative + "/"))}
        validate_output(root, after_with_backup, payload.inventory, contract, version, state, parent.build_metadata)
        changed = validate_changed_paths(before, after, contract, parent.prior_version, version)
        post_digest = inventory_digest(after)
        provenance = build_provenance(board_profile=board_profile, version=version, source_identity=source, parent_identity=parent.parent_identity, payload_digest=payload.digest, mutation_contract_digest=contract_digest, pre_inventory_digest=inventory_digest(before), post_inventory_digest=post_digest, changed_paths=changed, tool_identity=tool_identity)
        if backup is not None:
            remove_path(backup)
            backup = None
        return MutationResult(board_profile, parent.prior_version, version, source, contract_digest, payload.digest, parent.parent_identity, inventory_digest(before), post_digest, changed, provenance)
    except Exception as exc:
        try:
            snapshot.restore()
        except Exception as rollback_error:
            raise MutationError(f"runtime mutation failed and rollback failed: {rollback_error}") from exc
        raise exc if isinstance(exc, MutationError) else MutationError(str(exc)) from exc
    finally:
        if stage is not None:
            remove_path(stage)
        if backup is not None:
            remove_path(backup)
        snapshot.close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Apply a strict mounted-root runtime mutation")
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--board", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-identity", dest="source_identity", required=True)
    parser.add_argument("--parent-context", type=Path, required=True)
    parser.add_argument("--contract", type=Path)
    args = parser.parse_args()
    parent_context, _ = read_json_bytes(args.parent_context)
    result = mutate_runtime(args.root, args.bundle, args.board, args.version, args.source_identity, parent_context, contract_path=args.contract)
    print(json.dumps(result.provenance, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (MutationError, InventoryError) as exc:
        print(f"runtime mutation rejected: {exc}", flush=True)
        raise SystemExit(2) from exc
