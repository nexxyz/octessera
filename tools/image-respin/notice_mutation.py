from __future__ import annotations

import hashlib
import json
import os
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Callable, NoReturn

try:
    from .inventory import Inventory, build_inventory, inventory_digest, remove_path
    from .runtime_contract import MutationError
except ImportError:
    from inventory import Inventory, build_inventory, inventory_digest, remove_path
    from runtime_contract import MutationError

try:
    from stage_notices import NoticeStageError, load_manifest, stage_notices
except ImportError:
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "legal"))
    from stage_notices import NoticeStageError, load_manifest, stage_notices


NOTICE_MANIFEST = "resources/legal/notice-bundle.json"
NOTICE_STAGER = "tools/legal/stage_notices.py"
NOTICE_TARGET = "usr/share/doc/octessera"
NOTICE_PARENT = "usr/share/doc"
NOTICE_STAGE_PREFIX = ".octessera-notice-stage-"
NOTICE_STAGE_PATTERNS = (f"{NOTICE_PARENT}/{NOTICE_STAGE_PREFIX}*", f"{NOTICE_PARENT}/{NOTICE_STAGE_PREFIX}*/*")
NOTICE_TOOL_IDENTITY = "octessera-image-respin-notice-mutation/1"
NOTICE_TOOL_SCHEMA = "octessera-image-respin-notice-tool-code/v1"
NOTICE_RECORD_KEYS = {"contract", "manifest", "stager", "notice_tool", "preimage", "output", "changed_paths"}


@dataclass(frozen=True)
class NoticeMutationResult:
    record: dict[str, Any]
    changed_paths: list[str]


def _fail(message: str) -> NoReturn:
    raise MutationError(message)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")


def _identity(path: Path, root: Path) -> dict[str, Any]:
    try:
        relative = path.resolve(strict=True).relative_to(root.resolve(strict=True)).as_posix()
        metadata = path.lstat()
    except (OSError, ValueError) as exc:
        _fail(f"notice identity path is unavailable: {path}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        _fail(f"notice identity path is not a regular single-link file: {path}")
    raw = path.read_bytes()
    return {"path": relative, "sha256": _sha256(raw), "size": len(raw)}


def _safe_source(root: Path, relative: str) -> Path:
    if not relative or relative.startswith("/") or "\\" in relative or ".." in PurePosixPath(relative).parts:
        _fail(f"notice source path is unsafe: {relative}")
    path = root / relative
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
        resolved.relative_to(root.resolve(strict=True))
    except (OSError, ValueError) as exc:
        _fail(f"notice source path is unavailable: {relative}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        _fail(f"notice source path is not a regular single-link file: {relative}")
    return path


def _manifest_sources(repository_root: Path) -> tuple[dict[str, Any], dict[str, Any], list[tuple[Path, Path, bytes]]]:
    manifest_path = repository_root / NOTICE_MANIFEST
    try:
        manifest = load_manifest(manifest_path)
    except (OSError, NoticeStageError) as exc:
        raise MutationError(f"notice manifest is invalid: {exc}") from exc
    sources: list[tuple[Path, Path, bytes]] = []
    for item in manifest["files"]:
        source = _safe_source(repository_root, item["source"])
        raw = source.read_bytes()
        if _sha256(raw) != item["sha256"] or len(raw) != item["size"]:
            _fail(f"notice source identity changed: {item['source']}")
        destination = PurePosixPath(item["destination"])
        sources.append((source, Path(*destination.parts), raw))
    contract_identity = _identity(manifest_path, repository_root)
    manifest_identity = {"schema": manifest["schema"], "schema_version": manifest["schema_version"], "destination_root": manifest["destination_root"], "file_count": len(sources), "sha256": contract_identity["sha256"]}
    return manifest, {"contract": contract_identity, "manifest": manifest_identity}, sources


def _tool_model(repository_root: Path) -> dict[str, Any]:
    files = [_identity(repository_root / "tools/image-respin/notice_mutation.py", repository_root), _identity(repository_root / NOTICE_STAGER, repository_root)]
    body = {"schema": NOTICE_TOOL_SCHEMA, "version": 1, "files": files}
    return {"identity": NOTICE_TOOL_IDENTITY, "code_schema": body["schema"], "code_version": body["version"], "code_digest": _sha256(_canonical(body)), "code_files": files}


def _real_parent(root: Path) -> Path:
    parent = root
    for part in NOTICE_PARENT.split("/"):
        parent /= part
        try:
            metadata = parent.lstat()
        except OSError as exc:
            raise MutationError(f"notice parent is unavailable: {parent}") from exc
        if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
            _fail(f"notice parent must be a real directory: {parent}")
    return parent


def _absent(path: Path, label: str) -> None:
    try:
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as exc:
        raise MutationError(f"cannot inspect {label}: {path}") from exc
    _fail(f"{label} must be absent: {path}")


def _set_root_metadata(path: Path, mode: int) -> None:
    os.chmod(path, mode)
    if hasattr(os, "chown") and os.name != "nt":
        os.chown(path, 0, 0)


def _verify_root_metadata(path: Path, mode: int, label: str) -> None:
    if os.name != "nt":
        metadata = path.lstat()
        if metadata.st_uid != 0 or metadata.st_gid != 0:
            _fail(f"{label} is not root-owned: {path}")
        if stat.S_IMODE(metadata.st_mode) != mode:
            _fail(f"{label} mode is not {mode:o}: {path}")


def _expected_paths(sources: list[tuple[Path, Path, bytes]]) -> tuple[set[str], list[str]]:
    directories = {NOTICE_TARGET}
    files: set[str] = set()
    for _, destination, _ in sources:
        relative = f"{NOTICE_TARGET}/{destination.as_posix()}"
        files.add(relative)
        parent = PurePosixPath(relative).parent
        while str(parent) != NOTICE_TARGET:
            directories.add(str(parent))
            parent = parent.parent
    return directories | files, sorted(directories | files)


def _canonical_output_inventory(sources: list[tuple[Path, Path, bytes]]) -> Inventory:
    expected, _ = _expected_paths(sources)
    hashes = {f"{NOTICE_TARGET}/{destination.as_posix()}": _sha256(data) for _, destination, data in sources}
    inventory: Inventory = {}
    for relative in sorted(expected):
        local = "." if relative == NOTICE_TARGET else relative[len(NOTICE_TARGET) + 1 :]
        is_directory = relative not in hashes
        inventory[local] = {"path": local, "type": "directory" if is_directory else "file", "uid": 0, "gid": 0, "mode": 0o755 if is_directory else 0o644, "symlink": False, "target": None, "sha256": None if is_directory else hashes[relative], "xattrs": {}, "capability": None}
    return inventory


def _validate_tree(tree: Path, sources: list[tuple[Path, Path, bytes]]) -> tuple[Inventory, list[str]]:
    inventory = build_inventory(tree)
    expected, changed_paths = _expected_paths(sources)
    expected_relative = {"." if relative == NOTICE_TARGET else relative[len(NOTICE_TARGET) + 1 :] for relative in expected}
    if set(inventory) != expected_relative:
        _fail(f"notice output paths are not exact: missing={sorted(expected_relative - set(inventory))} extra={sorted(set(inventory) - expected_relative)}")
    for relative in sorted(expected):
        path = tree if relative == NOTICE_TARGET else tree / relative[len(NOTICE_TARGET) + 1 :]
        entry = inventory["." if relative == NOTICE_TARGET else relative[len(NOTICE_TARGET) + 1 :]]
        expected_mode = 0o755 if entry["type"] == "directory" else 0o644
        if entry["type"] not in {"directory", "file"} or entry["symlink"] or entry["xattrs"] or entry["capability"] is not None:
            _fail(f"notice output has invalid type or metadata: {relative}")
        _verify_root_metadata(path, expected_mode, "notice output")
        if entry["type"] == "file" and path.lstat().st_nlink != 1:
            _fail(f"notice output is hard-linked: {relative}")
    expected_hashes = {f"{NOTICE_TARGET}/{destination.as_posix()}": _sha256(data) for _, destination, data in sources}
    for relative, expected_hash in expected_hashes.items():
        entry = inventory[relative[len(NOTICE_TARGET) + 1 :]]
        if entry["sha256"] != expected_hash:
            _fail(f"notice output content is not canonical: {relative}")
    if os.name != "nt" and inventory_digest(inventory) != inventory_digest(_canonical_output_inventory(sources)):
        _fail("notice output inventory is not canonical")
    return inventory, changed_paths


def _set_stage_tree_metadata(tree: Path) -> None:
    inventory = build_inventory(tree)
    for relative, entry in sorted(inventory.items(), key=lambda item: (item[0].count("/"), item[0])):
        if entry["type"] == "directory":
            _set_root_metadata(tree if relative == "." else tree / relative, 0o755)


def _assert_preimage(root: Path, before: Inventory, stage: Path, allowed_prefixes: tuple[str, ...] = ()) -> None:
    stage_relative = stage.relative_to(root).as_posix()
    allowed = (stage_relative, *allowed_prefixes)
    current = build_inventory(root)
    filtered = {path: entry for path, entry in current.items() if not any(path == prefix or path.startswith(prefix + "/") for prefix in allowed)}
    if filtered != before:
        _fail("root changed outside the private notice stage before commit")


def _fsync_directory(path: Path) -> None:
    if os.name == "nt":
        return
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _record(repository_root: Path, manifest: dict[str, Any], identities: dict[str, Any], sources: list[tuple[Path, Path, bytes]], output: Inventory, changed_paths: list[str]) -> dict[str, Any]:
    return {"contract": identities["contract"], "manifest": identities["manifest"], "stager": _identity(repository_root / NOTICE_STAGER, repository_root), "notice_tool": _tool_model(repository_root), "preimage": {"path": NOTICE_TARGET, "status": "absent"}, "output": {"inventory_sha256": inventory_digest(output), "inventory_count": len(output)}, "changed_paths": changed_paths}


def install_notices(root: Path, before: Inventory, repository_root: Path, mutation_hook: Callable[[str], None] | None = None, allowed_prefixes: tuple[str, ...] = ()) -> NoticeMutationResult:
    root = Path(root).resolve(strict=True)
    repository_root = Path(repository_root).resolve(strict=True)
    parent = _real_parent(root)
    target = root / NOTICE_TARGET
    _absent(target, "notice target")
    for relative in before:
        if any(relative == pattern[:-1] or relative.startswith(pattern[:-1]) for pattern in NOTICE_STAGE_PATTERNS):
            _fail(f"stale private notice stage exists: {relative}")
    manifest, identities, sources = _manifest_sources(repository_root)
    stage: Path | None = None
    published = False
    try:
        stage = Path(tempfile.mkdtemp(prefix=NOTICE_STAGE_PREFIX, dir=parent))
        tree = stage / "octessera"
        stage_notices(repository_root, stage)
        tree = stage / NOTICE_TARGET
        _set_stage_tree_metadata(tree)
        _validate_tree(tree, sources)
        if mutation_hook:
            mutation_hook("notice-staged")
        _assert_preimage(root, before, stage, allowed_prefixes)
        _absent(target, "notice target")
        os.replace(tree, target)
        published = True
        _fsync_directory(parent)
        if mutation_hook:
            mutation_hook("notice-published")
        output, changed_paths = _validate_tree(target, sources)
        return NoticeMutationResult(_record(repository_root, manifest, identities, sources, output, changed_paths), changed_paths)
    except (OSError, NoticeStageError) as exc:
        raise MutationError(str(exc)) from exc
    except Exception:
        if published:
            remove_path(target)
        raise
    finally:
        if stage is not None:
            remove_path(stage)


def validate_notice_record(record: Any, repository_root: Path) -> None:
    if not isinstance(record, dict) or set(record) != NOTICE_RECORD_KEYS:
        _fail("notice provenance keys are not exact")
    manifest, identities, sources = _manifest_sources(Path(repository_root).resolve(strict=True))
    if record["contract"] != identities["contract"] or record["manifest"] != identities["manifest"]:
        _fail("notice contract identity changed")
    if record["stager"] != _identity(Path(repository_root) / NOTICE_STAGER, Path(repository_root)):
        _fail("notice stager identity changed")
    if record["notice_tool"] != _tool_model(Path(repository_root)):
        _fail("notice tool identity changed")
    if record["preimage"] != {"path": NOTICE_TARGET, "status": "absent"}:
        _fail("notice target preimage changed")
    expected, changed_paths = _expected_paths(sources)
    output = record["output"]
    if not isinstance(output, dict) or set(output) != {"inventory_sha256", "inventory_count"} or not isinstance(output["inventory_sha256"], str) or len(output["inventory_sha256"]) != 64 or any(character not in "0123456789abcdef" for character in output["inventory_sha256"]) or output["inventory_count"] != len(expected):
        _fail("notice output inventory identity changed")
    if os.name != "nt" and output["inventory_sha256"] != inventory_digest(_canonical_output_inventory(sources)):
        _fail("notice output inventory is not canonical")
    if record["changed_paths"] != changed_paths or set(record["changed_paths"]) != expected:
        _fail("notice changed paths are not the exact manifest-derived set")


def verify_mounted_notice_tree(derived_root: Path, record: Any) -> Inventory:
    repository_root = Path(__file__).resolve().parents[2]
    validate_notice_record(record, repository_root)
    _, _, sources = _manifest_sources(repository_root)
    _real_parent(Path(derived_root).resolve(strict=True))
    target = Path(derived_root).resolve(strict=True) / NOTICE_TARGET
    inventory, changed_paths = _validate_tree(target, sources)
    if inventory_digest(inventory) != record["output"]["inventory_sha256"] or len(inventory) != record["output"]["inventory_count"]:
        _fail("mounted notice inventory does not match provenance")
    if changed_paths != record["changed_paths"]:
        _fail("mounted notice changed paths do not match provenance")
    return inventory


__all__ = ["NOTICE_MANIFEST", "NOTICE_PARENT", "NOTICE_STAGE_PATTERNS", "NOTICE_TARGET", "NOTICE_TOOL_IDENTITY", "NoticeMutationResult", "install_notices", "validate_notice_record", "verify_mounted_notice_tree"]
