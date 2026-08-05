from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

try:
    from .inventory import Inventory, build_inventory, inventory_digest
    from .runtime_contract import MutationError, check_spec
    from .runtime_contract import rooted
    from .setup_contract import load_contract, target_spec
    from .setup_mutation import _validate_owned_directory, _validate_prerequisites
except ImportError:
    from inventory import Inventory, build_inventory, inventory_digest
    from runtime_contract import MutationError, check_spec
    from runtime_contract import rooted
    from setup_contract import load_contract, target_spec
    from setup_mutation import _validate_owned_directory, _validate_prerequisites


def _check_postimage(root: Path, inventory: Inventory, contract: dict[str, Any]) -> list[str]:
    for item in contract["directories"]:
        entry = inventory.get(item["target"])
        if entry is None or entry.get("type") != "directory":
            raise MutationError(f"setup proof directory is not exact: {item['target']}")
        check_spec(entry, target_spec(item), item["target"])
        _validate_owned_directory(inventory, contract, item["target"], item["target"])
    for item in contract["entries"]:
        entry = inventory.get(item["target"])
        if entry is None or entry.get("type") != "file" or entry.get("sha256") != item["sha256"]:
            raise MutationError(f"setup proof payload is not exact: {item['target']}")
        check_spec(entry, target_spec(item), item["target"])
    for item in contract["symlinks"]:
        entry = inventory.get(item["target"])
        if item["postimage"] == "absent":
            if entry is not None:
                raise MutationError(f"setup proof found a disabled unit link: {item['target']}")
        elif entry is None or entry.get("type") != "symlink" or entry.get("mode") != item["mode"] or entry.get("target") != item["link_target"]:
            raise MutationError(f"setup proof enabled link is not exact: {item['target']}")
    for item in contract["preserved_paths"]:
        if item["postimage"] == "absent" and item["target"] in inventory:
            raise MutationError(f"setup proof found an absent-only path: {item['target']}")
        if item["postimage"] == "preserve" and item["preimage"]["kind"] == "exact":
            entry = inventory.get(item["target"])
            if entry is None:
                raise MutationError(f"setup proof lost a preserved path: {item['target']}")
            expected = item["preimage"]
            check_spec(entry, {key: expected[key] for key in ("type", "mode", "uid", "gid", "symlink", "xattrs", "capability")}, item["target"])
            if expected["type"] == "file" and entry.get("sha256") != expected["sha256"]:
                raise MutationError(f"setup proof changed a preserved path: {item['target']}")
    if any(path in inventory for path in contract["stale_runtime_markers"]):
        raise MutationError("setup proof found stale runtime material")
    for item in contract["directories"]:
        rooted(root, item["target"])
    for item in contract["entries"]:
        rooted(root, item["target"])
    return sorted([item["target"] for item in contract["directories"]] + [item["target"] for item in contract["entries"]])


def prove_setup_root(root: Path, board_profile: str, *, contract_path: Path | None = None) -> dict[str, Any]:
    contract, contract_digest = load_contract(contract_path or (Path(__file__).resolve().parents[2] / "resources" / "image-mutations" / f"{board_profile}-setup.json"))
    if contract["board_profile"] != board_profile:
        raise MutationError("setup proof contract does not match the board")
    inventory = build_inventory(Path(root))
    prerequisites = _validate_prerequisites(Path(root), inventory, contract)
    paths = _check_postimage(Path(root), inventory, contract)
    return {"proof": "setup-layer-mounted", "schema_version": 1, "board_profile": board_profile, "contract_sha256": contract_digest, "inventory_sha256": inventory_digest(inventory), "prerequisites": prerequisites, "verified_paths": paths}


def main() -> int:
    parser = argparse.ArgumentParser(description="Prove an installed setup layer on a mounted root")
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--board", required=True)
    args = parser.parse_args()
    result = prove_setup_root(args.root, args.board)
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
