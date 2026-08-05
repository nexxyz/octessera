from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

try:
    from .inventory import Inventory, InventoryError, build_inventory, inventory_digest, remove_path, virtual_symlink_target
    from .provenance import canonical_source_identity, digest_object
    from .runtime_contract import MutationError, check_spec, mode_matches, rooted
    from .runtime_transaction import MutableSnapshot, atomic_bytes, atomic_link
    from .setup_contract import SetupContractError, load_contract, source_path, target_spec, validate_sources
    from .setup_provenance import setup_tool_code_digest
except ImportError:
    from inventory import Inventory, InventoryError, build_inventory, inventory_digest, remove_path, virtual_symlink_target
    from provenance import canonical_source_identity, digest_object
    from runtime_contract import MutationError, check_spec, mode_matches, rooted
    from runtime_transaction import MutableSnapshot, atomic_bytes, atomic_link
    from setup_contract import SetupContractError, load_contract, source_path, target_spec, validate_sources
    from setup_provenance import setup_tool_code_digest


class ConstructorRequired(MutationError):
    pass


SETUP_TOOL_IDENTITY = "octessera-image-respin-setup-finalizer/1"
REPOSITORY = Path(__file__).resolve().parents[2]
STALE_FILE_MODES = {
    "setup-complete": 420,
    "setup-force": 420,
    "setup-finalize-failed": 420,
    "setup-portal.request": 384,
    "nonce": 384,
    "readiness": 384,
    "status.json": 384,
}


@dataclass(frozen=True)
class SetupMutationResult:
    board_profile: str
    contract_digest: str
    source_inputs: list[dict[str, Any]]
    pre_inventory_digest: str
    post_inventory_digest: str
    changed_paths: list[str]
    parent_identity: dict[str, Any]
    provenance: dict[str, Any]


def _root_is_real(path: Path) -> Path:
    path = Path(path).absolute()
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise MutationError("mounted root is unavailable") from exc
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise MutationError("mounted root must be a real directory")
    return path.resolve(strict=False)


def _entry(inventory: Inventory, relative: str) -> dict[str, Any] | None:
    return inventory.get(relative)


def _check_exact(entry: dict[str, Any] | None, expected: dict[str, Any], label: str) -> None:
    if entry is None:
        raise ConstructorRequired(f"required setup preimage is missing: {label}")
    if expected["type"] == "symlink":
        if entry.get("type") != "symlink" or entry.get("mode") != expected["mode"] or entry.get("uid") != expected["uid"] or entry.get("gid") != expected["gid"] or entry.get("symlink") is not True or entry.get("xattrs") != expected["xattrs"] or entry.get("capability") != expected["capability"]:
            raise ConstructorRequired(f"{label} metadata does not match the trusted parent preimage")
        if entry.get("target") != expected["link_target"]:
            raise ConstructorRequired(f"{label} has the wrong symlink target")
        return
    try:
        check_spec(entry, expected, label)
    except MutationError as exc:
        raise ConstructorRequired(str(exc)) from exc
    if expected["type"] == "file" and entry.get("sha256") != expected["sha256"]:
        raise ConstructorRequired(f"{label} content does not match the trusted parent preimage")


def _check_preimage(root: Path, inventory: Inventory, relative: str, preimage: dict[str, Any], label: str) -> None:
    kind = preimage["kind"]
    entry = _entry(inventory, relative)
    if kind == "absent":
        if entry is not None:
            raise ConstructorRequired(f"{label} must be absent in the trusted parent")
    elif kind == "unresolved":
        raise ConstructorRequired(f"{label} is unresolved: {preimage['reason']}")
    else:
        _check_exact(entry, preimage, label)


def _package_status(root: Path, packages: list[str]) -> bytes:
    path = rooted(root, "var/lib/dpkg/status")
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise ConstructorRequired("trusted parent has no dpkg status database") from exc
    blocks = raw.decode("utf-8", errors="strict").split("\n\n")
    installed = {line[9:] for block in blocks for line in block.splitlines() if line.startswith("Package: ") and any(candidate == line[9:] and "Status: install ok installed" in block for candidate in packages)}
    missing = sorted(set(packages) - installed)
    if missing:
        raise ConstructorRequired(f"trusted parent is missing required packages: {', '.join(missing)}")
    return raw


def _account_records(root: Path, requirements: list[dict[str, str]]) -> tuple[dict[str, str], bytes, bytes]:
    try:
        passwd = rooted(root, "etc/passwd").read_bytes()
        group = rooted(root, "etc/group").read_bytes()
        passwd_lines = passwd.decode("utf-8").splitlines()
        group_lines = group.decode("utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise ConstructorRequired("trusted parent account databases are unavailable") from exc
    users = {line.split(":", 1)[0]: line.split(":") for line in passwd_lines if line and ":" in line}
    groups = {line.split(":", 1)[0]: line.split(":") for line in group_lines if line and ":" in line}
    identities: dict[str, str] = {}
    for requirement in requirements:
        user = requirement["user"]
        group_name = requirement["group"]
        fields = users.get(user)
        group_fields = groups.get(group_name)
        if fields is None or len(fields) != 7 or group_fields is None or len(group_fields) != 4:
            raise ConstructorRequired(f"trusted parent account prerequisite is missing: {user}/{group_name}")
        if fields[5] != requirement["home"] or fields[6] != requirement["shell"]:
            raise ConstructorRequired(f"trusted parent account prerequisite is mismatched: {user}")
        if fields[3] != group_fields[2]:
            raise ConstructorRequired(f"trusted parent account and group do not match: {user}")
        identities[f"user:{user}"] = ":".join(fields)
        identities[f"group:{group_name}"] = ":".join(group_fields)
    return identities, passwd, group


def _service_path(root: Path, name: str) -> Path:
    for directory in ("etc/systemd/system", "lib/systemd/system", "usr/lib/systemd/system"):
        candidate = rooted(root, f"{directory}/{name}")
        try:
            metadata = candidate.lstat()
        except OSError:
            continue
        if stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
            return candidate
    raise ConstructorRequired(f"trusted parent service prerequisite is missing: {name}")


def _validate_prerequisites(root: Path, inventory: Inventory, contract: dict[str, Any]) -> dict[str, Any]:
    package_bytes = _package_status(root, contract["prerequisites"]["packages"])
    account_identity, passwd_bytes, group_bytes = _account_records(root, contract["prerequisites"]["accounts"])
    executable_identity: dict[str, Any] = {}
    for relative in contract["prerequisites"]["executables"]:
        path = rooted(root, relative)
        entry = inventory.get(relative)
        if entry is None or entry["uid"] != 0 or entry["gid"] != 0 or entry["xattrs"] or entry["type"] not in {"file", "symlink"} or (entry["type"] == "file" and not mode_matches(493, entry["mode"], "file")):
            raise ConstructorRequired(f"trusted parent executable prerequisite is missing or mismatched: {relative}")
        executable_identity[relative] = entry
        if entry["type"] == "symlink":
            try:
                if not virtual_symlink_target(root, path, str(entry["target"])).is_file():
                    raise ConstructorRequired(f"trusted parent executable prerequisite escapes the root: {relative}")
            except (OSError, ValueError) as exc:
                raise ConstructorRequired(f"trusted parent executable prerequisite is unsafe: {relative}") from exc
    services = {name: _service_path(root, name).relative_to(root).as_posix() for name in contract["prerequisites"]["services"]}
    return {"packages_sha256": hashlib.sha256(package_bytes).hexdigest(), "accounts": account_identity, "passwd_sha256": hashlib.sha256(passwd_bytes).hexdigest(), "group_sha256": hashlib.sha256(group_bytes).hexdigest(), "executables": executable_identity, "services": services}


def _validate_markers(root: Path, inventory: Inventory, markers: list[str], request_uids: set[int]) -> None:
    for relative in markers:
        entry = inventory.get(relative)
        if entry is None:
            continue
        if entry["type"] == "directory":
            children = [path for path in inventory if path.startswith(relative + "/")]
            if children:
                raise ConstructorRequired(f"stale setup directory is not empty: {relative}")
            if entry["uid"] != 0 or entry["gid"] != 0 or entry["mode"] != 448 or entry["xattrs"]:
                raise ConstructorRequired(f"stale setup directory metadata is not exact: {relative}")
            continue
        expected_uids = request_uids if relative.endswith("/setup-portal.request") else {0}
        if entry["type"] != "file" or entry["uid"] not in expected_uids or entry["xattrs"]:
            raise ConstructorRequired(f"stale setup marker is not a root-owned regular file: {relative}")
        expected_mode = STALE_FILE_MODES.get(relative.rsplit("/", 1)[-1])
        if expected_mode is not None and not mode_matches(expected_mode, entry["mode"], "file"):
            raise ConstructorRequired(f"stale setup marker mode is not exact: {relative}")


def _validate_parent(root: Path, inventory: Inventory, contract: dict[str, Any]) -> dict[str, Any]:
    parent_paths: set[str] = set()
    for entry in contract["entries"]:
        _check_preimage(root, inventory, entry["target"], entry["preimage"], entry["target"])
        rooted(root, entry["target"])
        parent_paths.add(entry["target"].rsplit("/", 1)[0])
    for item in contract["symlinks"]:
        if item["type"] == "absent":
            _check_preimage(root, inventory, item["target"], item["preimage"], item["target"])
        else:
            _check_preimage(root, inventory, item["target"], item["preimage"], item["target"])
        rooted(root, item["target"])
        parent_paths.add(item["target"].rsplit("/", 1)[0])
    for relative in sorted(parent_paths):
        current = root
        for part in relative.split("/"):
            current /= part
            try:
                metadata = current.lstat()
            except OSError as exc:
                raise ConstructorRequired(f"setup parent directory is missing: {relative}") from exc
            if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
                raise ConstructorRequired(f"setup parent directory is not a real directory: {relative}")
    for item in contract["preserved_paths"]:
        _check_preimage(root, inventory, item["target"], item["preimage"], item["target"])
    prerequisites = _validate_prerequisites(root, inventory, contract)
    request_uids = {0}
    for identity in prerequisites["accounts"].values():
        fields = identity.split(":")
        if len(fields) >= 4 and fields[2].isdigit():
            request_uids.add(int(fields[2]))
    _validate_markers(root, inventory, contract["stale_runtime_markers"], request_uids)
    return {"board_profile": contract["board_profile"], "preimage_source": contract["preimage_source"], "prerequisites": prerequisites, "preimage_digest": inventory_digest(inventory)}


def _remove_markers(root: Path, markers: list[str]) -> None:
    for relative in sorted(markers, key=lambda item: item.count("/"), reverse=True):
        remove_path(rooted(root, relative))


def _set_xattrs(path: Path, xattrs: dict[str, str]) -> None:
    setter = getattr(os, "setxattr", None)
    remover = getattr(os, "removexattr", None)
    lister = getattr(os, "listxattr", None)
    if setter is None or remover is None or lister is None:
        if xattrs:
            raise MutationError(f"cannot install required extended attributes: {path}")
        return
    try:
        for name in set(lister(path, follow_symlinks=False)) - set(xattrs):
            remover(path, name, follow_symlinks=False)
        for name, value in xattrs.items():
            setter(path, name, bytes.fromhex(value), follow_symlinks=False)
    except OSError as exc:
        raise MutationError(f"cannot install extended attributes: {path}") from exc


def _set_metadata(path: Path, spec: dict[str, Any]) -> None:
    if os.name != "nt" and hasattr(os, "chown"):
        os.chown(path, spec["uid"], spec["gid"], follow_symlinks=spec["type"] != "symlink")
    if spec["type"] != "symlink":
        os.chmod(path, spec["mode"])
    _set_xattrs(path, spec["xattrs"])


def _install_entry(root: Path, contract: dict[str, Any], item: dict[str, Any]) -> None:
    source = source_path(contract, item["source"], REPOSITORY)
    destination = rooted(root, item["target"])
    atomic_bytes(destination, source.read_bytes(), item["mode"])
    _set_metadata(destination, target_spec(item))


def _install_symlink(root: Path, item: dict[str, Any]) -> None:
    destination = rooted(root, item["target"])
    current = destination.lstat() if destination.exists() or destination.is_symlink() else None
    if current is None:
        atomic_link(destination, item["link_target"])
    elif not stat.S_ISLNK(current.st_mode) or os.readlink(destination) != item["link_target"]:
        raise MutationError(f"enabled setup symlink has an unexpected destination: {item['target']}")
    _set_metadata(destination, {"type": "symlink", "uid": item["uid"], "gid": item["gid"], "xattrs": item["xattrs"]})
    candidate = virtual_symlink_target(root, destination, item["link_target"])
    if not candidate.is_file():
        raise MutationError(f"enabled setup symlink does not resolve inside the root: {item['target']}")


def _validate_output(root: Path, before: Inventory, after: Inventory, contract: dict[str, Any], parent: dict[str, Any]) -> list[str]:
    for item in contract["entries"]:
        entry = after.get(item["target"])
        if entry is None or entry["type"] != "file" or entry["sha256"] != item["sha256"]:
            raise MutationError(f"setup payload output is not exact: {item['target']}")
        check_spec(entry, target_spec(item), item["target"])
    for item in contract["symlinks"]:
        entry = after.get(item["target"])
        if item["postimage"] == "absent":
            if entry is not None:
                raise MutationError(f"disabled setup unit is enabled: {item['target']}")
        else:
            if entry is None or entry["type"] != "symlink" or entry["mode"] != item["mode"] or entry["target"] != item["link_target"]:
                raise MutationError(f"enabled setup path is not exact: {item['target']}")
    for item in contract["preserved_paths"]:
        expected = item["preimage"]
        if expected["kind"] == "absent":
            if item["target"] in after:
                raise MutationError(f"setup preserved path was unexpectedly created: {item['target']}")
        elif after.get(item["target"]) != before.get(item["target"]):
            raise MutationError(f"setup preserved path changed: {item['target']}")
    if any(marker in after for marker in contract["stale_runtime_markers"]):
        raise MutationError("stale setup runtime material remains")
    allowed = {item["target"] for item in contract["entries"]} | {item["target"] for item in contract["symlinks"]} | set(contract["stale_runtime_markers"])
    changed = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
    for path in changed:
        if path not in allowed:
            raise MutationError(f"unauthorized setup-layer mutation: {path}")
    return changed


def _provenance(board: str, source: object, contract_digest: str, parent: dict[str, Any], source_inputs: list[dict[str, Any]], before: Inventory, after: Inventory, changed: list[str], tool_digest: str) -> dict[str, Any]:
    source_identity = canonical_source_identity(source)
    parent_identity = json.loads(json.dumps(parent, sort_keys=True))
    return {"proof_schema": "octessera.image-setup-mutation-provenance.v1", "schema_version": 1, "board_profile": board, "source_identity": source_identity, "parent": {"identity": parent_identity, "digest": digest_object(parent_identity)}, "setup_layer": {"contract_digest": contract_digest, "source_inputs": source_inputs}, "inventories": {"pre": inventory_digest(before), "post": inventory_digest(after)}, "changed_paths": changed, "finalizer": {"source_identity": source_identity, "tool_identity": SETUP_TOOL_IDENTITY, "tool_code_digest": tool_digest}}


def mutate_setup(root: Path, board_profile: str, source_identity: object, *, contract_path: Path | None = None, mutation_hook: Callable[[str], None] | None = None, tool_digest: str = "") -> SetupMutationResult:
    root = _root_is_real(root)
    contract, contract_digest = load_contract(contract_path or (Path(__file__).resolve().parents[2] / "resources" / "image-mutations" / f"{board_profile}-setup.json"))
    if contract["board_profile"] != board_profile:
        raise MutationError("setup contract does not match the requested board")
    try:
        before = build_inventory(root)
        source_inputs = validate_sources(contract, Path(__file__).resolve().parents[2])
        parent = _validate_parent(root, before, contract)
    except (InventoryError, SetupContractError, UnicodeError) as exc:
        raise MutationError(str(exc)) from exc
    mutable = [item["target"] for item in contract["entries"]] + [item["target"] for item in contract["symlinks"]] + contract["stale_runtime_markers"]
    mutable += [path.rsplit("/", 1)[0] for path in mutable if "/" in path]
    snapshot = MutableSnapshot.capture(root, before, tuple(dict.fromkeys(mutable)), tuple())
    try:
        if mutation_hook:
            mutation_hook("validated")
        _remove_markers(root, contract["stale_runtime_markers"])
        if mutation_hook:
            mutation_hook("stale-markers-removed")
        for item in contract["entries"]:
            _install_entry(root, contract, item)
            if mutation_hook:
                mutation_hook(f"installed:{item['target']}")
        for item in contract["symlinks"]:
            if item["postimage"] == "required":
                _install_symlink(root, item)
        after = build_inventory(root)
        changed = _validate_output(root, before, after, contract, parent)
        provenance = _provenance(board_profile, source_identity, contract_digest, parent, list(source_inputs.values()), before, after, changed, tool_digest or setup_tool_code_digest())
        return SetupMutationResult(board_profile, contract_digest, list(source_inputs.values()), inventory_digest(before), inventory_digest(after), changed, parent, provenance)
    except Exception as exc:
        try:
            snapshot.restore()
        except Exception as rollback_error:
            raise MutationError(f"setup mutation failed and rollback failed: {rollback_error}") from exc
        raise exc if isinstance(exc, MutationError) else MutationError(str(exc)) from exc
    finally:
        snapshot.close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Apply the strict mounted-root setup portal mutation")
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--board", required=True)
    parser.add_argument("--source-identity", required=True)
    parser.add_argument("--contract", type=Path)
    args = parser.parse_args()
    result = mutate_setup(args.root, args.board, args.source_identity, contract_path=args.contract)
    print(json.dumps(result.provenance, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (MutationError, SetupContractError, InventoryError) as exc:
        print(f"setup mutation rejected: {exc}")
        raise SystemExit(2) from exc
