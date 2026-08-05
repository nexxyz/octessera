from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
from pathlib import Path
from typing import Any

try:
    from .setup_contract_schema import SetupContractSchemaError, validate_setup_contract
except ImportError:
    from setup_contract_schema import SetupContractSchemaError, validate_setup_contract


class SetupContractError(ValueError):
    pass


CONTRACTS = Path(__file__).resolve().parents[2] / "resources" / "image-mutations"
BOARDS = {"raspberry-pi-zero-2w", "orange-pi-zero-2w"}


def contract_for_board(board: str) -> Path:
    if board not in BOARDS:
        raise SetupContractError(f"unsupported board profile: {board}")
    return CONTRACTS / f"{board}-setup.json"


def load_contract(path: Path) -> tuple[dict[str, Any], str]:
    try:
        raw = Path(path).read_bytes()
        value = json.loads(raw.decode("utf-8"))
        validate_setup_contract(value)
    except (OSError, UnicodeError, ValueError, SetupContractSchemaError) as exc:
        raise SetupContractError(f"setup contract is invalid: {path}: {exc}") from exc
    return value, hashlib.sha256(raw).hexdigest()


def source_path(contract: dict[str, Any], source: str, root: Path) -> Path:
    base = Path(root).resolve(strict=True)
    path = (base / contract["source_root"] / source).resolve(strict=True)
    try:
        path.relative_to((base / contract["source_root"]).resolve(strict=True))
    except ValueError as exc:
        raise SetupContractError(f"setup source escapes its approved root: {source}") from exc
    if path.is_symlink() or not path.is_file():
        raise SetupContractError(f"setup source is not a regular file: {source}")
    return path


def validate_sources(contract: dict[str, Any], root: Path) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    for item in contract["source_inputs"]:
        path = (Path(root) / item["path"]).resolve(strict=True)
        if path.is_symlink() or not path.is_file():
            raise SetupContractError(f"setup source input is not a regular file: {item['path']}")
        raw = path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        if digest != item["sha256"] or len(raw) != item["size"]:
            raise SetupContractError(f"setup source input changed: {item['path']}")
        records[item["path"]] = {"path": item["path"], "sha256": digest, "size": len(raw)}
    for entry in contract["entries"]:
        source = source_path(contract, entry["source"], root)
        raw = source.read_bytes()
        if hashlib.sha256(raw).hexdigest() != entry["sha256"]:
            raise SetupContractError(f"setup payload changed: {entry['source']}")
    return records


def setup_source_paths(contract: dict[str, Any]) -> list[str]:
    paths = {item["path"] for item in contract["source_inputs"]}
    paths.update(f'{contract["source_root"]}/{item["source"]}' for item in contract["entries"])
    return sorted(paths)


def _repository_source(repository: Path, relative: str) -> Path:
    candidate = repository / relative
    if Path(relative).is_absolute() or "\\" in relative:
        raise SetupContractError(f"setup contract source path is not project-relative: {relative}")
    try:
        metadata = candidate.lstat()
    except (OSError, ValueError) as exc:
        raise SetupContractError(f"setup contract source is missing: {relative}") from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise SetupContractError(f"setup contract source is not a regular file: {relative}")
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(repository)
    except (OSError, ValueError) as exc:
        raise SetupContractError(f"setup contract source escapes the repository: {relative}") from exc
    return resolved


def _git_result(repository: Path, command: list[str], relative: str) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(["git", "-C", str(repository), *command, "--", relative], check=False, capture_output=True, text=True)
    except OSError as exc:
        raise SetupContractError(f"cannot inspect Git setup source visibility: {relative}") from exc


def _is_tracked(repository: Path, relative: str) -> bool:
    result = _git_result(repository, ["ls-files", "--error-unmatch"], relative)
    return result.returncode == 0 and result.stdout.strip().splitlines() == [relative]


def _is_ignored(repository: Path, relative: str) -> bool:
    result = _git_result(repository, ["check-ignore", "--no-index", "--quiet"], relative)
    if result.returncode not in (0, 1):
        raise SetupContractError(f"cannot inspect Git ignore rules: {relative}")
    return result.returncode == 0


def validate_tracked_sources(contract: dict[str, Any], root: Path, *, strict: bool | None = None) -> None:
    repository = Path(root).resolve()
    strict_mode = os.environ.get("CI", "").lower() == "true" if strict is None else strict
    for relative in setup_source_paths(contract):
        _repository_source(repository, relative)
        if _is_ignored(repository, relative):
            raise SetupContractError(f"setup contract source is ignored: {relative}")
        if _is_tracked(repository, relative):
            continue
        if strict_mode:
            raise SetupContractError(f"setup contract source is not Git-tracked: {relative}")


def target_spec(item: dict[str, Any]) -> dict[str, Any]:
    return {key: item.get(key, False) if key == "symlink" else item[key] for key in ("type", "mode", "uid", "gid", "symlink", "xattrs", "capability")}


def setup_targets(contract: dict[str, Any]) -> set[str]:
    return {item["target"] for item in contract["entries"]} | {item["target"] for item in contract["symlinks"]} | {item["target"] for item in contract["preserved_paths"]}


__all__ = ["BOARDS", "SetupContractError", "contract_for_board", "load_contract", "setup_source_paths", "setup_targets", "source_path", "target_spec", "validate_sources", "validate_tracked_sources"]
