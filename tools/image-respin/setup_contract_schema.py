from __future__ import annotations

import re
from typing import Any


class SetupContractSchemaError(ValueError):
    pass


BOARDS = {"raspberry-pi-zero-2w", "orange-pi-zero-2w"}
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
FILE_SPEC_KEYS = {"type", "mode", "uid", "gid", "symlink", "xattrs", "capability"}
SETUP_UI_DIRECTORY = "usr/local/share/octessera-setup-ui"
SETUP_UI_FILES = {"app.js", "index.html", "styles.css", "README.md", "octessera-mark.svg", "octessera-wordmark.svg"}
RASPBERRY_PARENT_SUDOERS_TARGET = "etc/sudoers.d/010_pi-nopasswd"
RASPBERRY_PARENT_SUDOERS_SHA256 = "aa7549b5a2544e53652d7c844af396ca05044e41b05f56372162dc8b0cf3f089"


def _keys(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise SetupContractSchemaError(f"{label} keys are not exact")
    return value


def _path(value: Any, label: str) -> None:
    if not isinstance(value, str) or not value or value.startswith("/") or "\\" in value:
        raise SetupContractSchemaError(f"{label} is not a safe relative path")
    parts = value.split("/")
    if any(not part or part in {".", ".."} for part in parts):
        raise SetupContractSchemaError(f"{label} contains traversal")


def _digest(value: Any, label: str) -> None:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        raise SetupContractSchemaError(f"{label} is not a SHA-256 digest")


def _metadata(value: Any, label: str, *, allow_type: set[str] | None = None) -> dict[str, Any]:
    result = _keys(value, FILE_SPEC_KEYS, label)
    if result["type"] not in (allow_type or {"file", "directory", "symlink"}):
        raise SetupContractSchemaError(f"{label}.type is invalid")
    if result["symlink"] != (result["type"] == "symlink"):
        raise SetupContractSchemaError(f"{label}.symlink is inconsistent")
    for key in ("mode", "uid", "gid"):
        if isinstance(result[key], bool) or not isinstance(result[key], int) or result[key] < 0:
            raise SetupContractSchemaError(f"{label}.{key} is invalid")
    if not isinstance(result["xattrs"], dict) or any(not isinstance(key, str) or not key or not isinstance(value, str) or len(value) % 2 or re.fullmatch(r"[0-9a-f]*", value) is None for key, value in result["xattrs"].items()):
        raise SetupContractSchemaError(f"{label}.xattrs is invalid")
    capability = result["capability"]
    if capability is not None and (not isinstance(capability, str) or len(capability) % 2 or re.fullmatch(r"[0-9a-f]+", capability) is None or result["xattrs"].get("security.capability") != capability):
        raise SetupContractSchemaError(f"{label}.capability is inconsistent")
    if capability is None and "security.capability" in result["xattrs"]:
        raise SetupContractSchemaError(f"{label}.capability is missing")
    return result


def _preimage(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SetupContractSchemaError(f"{label} is invalid")
    kind = value.get("kind")
    if kind == "absent":
        if set(value) != {"kind"}:
            raise SetupContractSchemaError(f"{label} absent rule is not exact")
    elif kind == "unresolved":
        if set(value) != {"kind", "reason"} or not isinstance(value["reason"], str) or not value["reason"].strip():
            raise SetupContractSchemaError(f"{label} unresolved rule is not exact")
    elif kind == "exact":
        if value.get("type") == "symlink":
            expected = {"kind", "type", "mode", "uid", "gid", "symlink", "xattrs", "capability", "link_target"}
            if set(value) != expected or not isinstance(value["link_target"], str) or "\\" in value["link_target"]:
                raise SetupContractSchemaError(f"{label} symlink rule is not exact")
            _metadata({"type": "symlink", "mode": value["mode"], "uid": value["uid"], "gid": value["gid"], "symlink": value["symlink"], "xattrs": value["xattrs"], "capability": value["capability"]}, label, allow_type={"symlink"})
            return value
        _metadata({key: value[key] for key in FILE_SPEC_KEYS}, label, allow_type={"file", "directory"})
        expected = FILE_SPEC_KEYS | {"kind"} | ({"sha256"} if value["type"] == "file" else set())
        if set(value) != expected:
            raise SetupContractSchemaError(f"{label} exact rule is not exact")
        if value["type"] == "file":
            _digest(value["sha256"], f"{label}.sha256")
    else:
        raise SetupContractSchemaError(f"{label}.kind is unsupported")
    return value


def _source_inputs(value: Any) -> None:
    if not isinstance(value, list) or not value:
        raise SetupContractSchemaError("source_inputs is invalid")
    paths: set[str] = set()
    for index, item in enumerate(value):
        record = _keys(item, {"path", "sha256", "size"}, f"source_inputs[{index}]")
        _path(record["path"], f"source_inputs[{index}].path")
        _digest(record["sha256"], f"source_inputs[{index}].sha256")
        if isinstance(record["size"], bool) or not isinstance(record["size"], int) or record["size"] < 0:
            raise SetupContractSchemaError(f"source_inputs[{index}].size is invalid")
        if record["path"] in paths:
            raise SetupContractSchemaError("source_inputs contains duplicates")
        paths.add(record["path"])


def _entries(value: Any, label: str) -> None:
    if not isinstance(value, list) or not value:
        raise SetupContractSchemaError(f"{label} is invalid")
    targets: set[str] = set()
    for index, item in enumerate(value):
        expected = {"classification", "source", "target", "type", "mode", "uid", "gid", "symlink", "xattrs", "capability", "sha256", "preimage"}
        entry = _keys(item, expected, f"{label}[{index}]")
        if not isinstance(entry["classification"], str) or not entry["classification"]:
            raise SetupContractSchemaError(f"{label}[{index}] classification is invalid")
        if not isinstance(entry["source"], str) or not entry["source"]:
            raise SetupContractSchemaError(f"{label}[{index}] source is invalid")
        _path(entry["source"], f"{label}[{index}].source")
        _path(entry["target"], f"{label}[{index}].target")
        _metadata({key: entry[key] for key in FILE_SPEC_KEYS}, f"{label}[{index}]")
        _digest(entry["sha256"], f"{label}[{index}].sha256")
        _preimage(entry["preimage"], f"{label}[{index}].preimage")
        if entry["target"] in targets:
            raise SetupContractSchemaError(f"{label} contains duplicate targets")
        targets.add(entry["target"])


def _validate_ui_preimages(value: list[dict[str, Any]], board: str) -> None:
    prefix = SETUP_UI_DIRECTORY + "/"
    ui_entries = [entry for entry in value if entry["target"].startswith(prefix)]
    if {entry["target"][len(prefix):] for entry in ui_entries} != SETUP_UI_FILES:
        raise SetupContractSchemaError("setup UI entries are not the exact six-file set")
    for entry in ui_entries:
        if entry["type"] != "file" or entry["mode"] != 420 or entry["uid"] != 0 or entry["gid"] != 0 or entry["symlink"] or entry["xattrs"] or entry["capability"] is not None:
            raise SetupContractSchemaError(f"setup UI output metadata is not exact: {entry['target']}")
        preimage = entry["preimage"]
        if board == "raspberry-pi-zero-2w":
            if preimage != {"kind": "absent"}:
                raise SetupContractSchemaError(f"Raspberry setup UI preimage is not absent: {entry['target']}")
        elif preimage != {"kind": "exact", "type": "file", "mode": 420, "uid": 1001, "gid": 1001, "symlink": False, "xattrs": {}, "capability": None, "sha256": preimage.get("sha256")}:
            raise SetupContractSchemaError(f"Orange setup UI preimage ownership is not exact: {entry['target']}")


def _directories(value: Any, board: str) -> None:
    if not isinstance(value, list) or len(value) != 1:
        raise SetupContractSchemaError("directories is not the exact singleton section")
    directory = _keys(value[0], {"target", "type", "mode", "uid", "gid", "symlink", "xattrs", "capability", "preimage", "postimage"}, "directories[0]")
    _path(directory["target"], "directories[0].target")
    if directory["target"] != SETUP_UI_DIRECTORY:
        raise SetupContractSchemaError("directories contains an unexpected path")
    _metadata({key: directory[key] for key in FILE_SPEC_KEYS}, "directories[0]", allow_type={"directory"})
    if directory["type"] != "directory" or directory["mode"] != 493 or directory["uid"] != 0 or directory["gid"] != 0 or directory["symlink"] or directory["xattrs"] or directory["capability"] is not None:
        raise SetupContractSchemaError("directories[0] output metadata is not exact")
    preimage = directory["preimage"]
    _preimage(preimage, "directories[0].preimage")
    if board == "raspberry-pi-zero-2w":
        if preimage != {"kind": "absent"}:
            raise SetupContractSchemaError("Raspberry setup directory preimage is not exact")
    elif preimage != {"kind": "exact", "type": "directory", "mode": 493, "uid": 1001, "gid": 1001, "symlink": False, "xattrs": {}, "capability": None}:
        raise SetupContractSchemaError("Orange setup directory preimage is not exact")
    if directory["postimage"] != "required":
        raise SetupContractSchemaError("directories[0].postimage is invalid")


def _symlinks(value: Any, board: str) -> None:
    if not isinstance(value, list) or not value:
        raise SetupContractSchemaError("symlinks is invalid")
    targets: set[str] = set()
    for index, item in enumerate(value):
        if isinstance(item, dict) and item.get("type") == "absent":
            entry = _keys(item, {"classification", "target", "type", "preimage", "postimage"}, f"symlinks[{index}]")
            _path(entry["target"], f"symlinks[{index}].target")
            _preimage(entry["preimage"], f"symlinks[{index}].preimage")
            if entry["postimage"] != "absent":
                raise SetupContractSchemaError(f"symlinks[{index}] absent postimage is invalid")
            if entry["target"] in targets:
                raise SetupContractSchemaError("symlinks contains duplicate targets")
            targets.add(entry["target"])
            continue
        entry = _keys(item, {"classification", "target", "type", "mode", "uid", "gid", "symlink", "xattrs", "capability", "link_target", "preimage", "postimage"}, f"symlinks[{index}]")
        _path(entry["target"], f"symlinks[{index}].target")
        _metadata({"type": entry["type"], "mode": entry["mode"], "uid": entry["uid"], "gid": entry["gid"], "symlink": entry["symlink"], "xattrs": entry["xattrs"], "capability": entry["capability"]}, f"symlinks[{index}]", allow_type={"symlink", "absent"})
        if entry["type"] == "symlink" and (not entry["symlink"] or not entry["link_target"]):
            raise SetupContractSchemaError(f"symlinks[{index}] target is invalid")
        _preimage(entry["preimage"], f"symlinks[{index}].preimage")
        if entry["postimage"] not in {"required", "preserve", "absent"}:
            raise SetupContractSchemaError(f"symlinks[{index}].postimage is invalid")
        if entry["target"] in targets:
            raise SetupContractSchemaError("symlinks contains duplicate targets")
        targets.add(entry["target"])
    parent_sudoers = [item for item in value if item["target"] == RASPBERRY_PARENT_SUDOERS_TARGET]
    if board == "raspberry-pi-zero-2w":
        if len(parent_sudoers) != 1:
            raise SetupContractSchemaError("Raspberry setup contract must remove the parent sudoers grant")
        item = parent_sudoers[0]
        expected_preimage = {"kind": "exact", "type": "file", "mode": 288, "uid": 0, "gid": 0, "symlink": False, "xattrs": {}, "capability": None, "sha256": RASPBERRY_PARENT_SUDOERS_SHA256}
        if item != {"classification": "parent-sudoers-removed", "target": RASPBERRY_PARENT_SUDOERS_TARGET, "type": "absent", "preimage": expected_preimage, "postimage": "absent"}:
            raise SetupContractSchemaError("Raspberry parent sudoers removal is not exact")
    elif parent_sudoers:
        raise SetupContractSchemaError("Orange setup contract must not remove the Raspberry parent sudoers grant")


def validate_setup_contract(contract: Any) -> None:
    top = _keys(contract, {"schema_version", "contract_kind", "board_profile", "source_root", "preimage_source", "source_inputs", "directories", "entries", "symlinks", "preserved_paths", "stale_runtime_markers", "prerequisites", "recipe"}, "setup contract")
    if top["schema_version"] != 1 or top["contract_kind"] != "setup-layer" or top["board_profile"] not in BOARDS:
        raise SetupContractSchemaError("setup contract identity is invalid")
    _path(top["source_root"], "source_root")
    if top["source_root"] not in {"userpatches/overlay", "tools/pi-image/stage4-octessera/files/root"}:
        raise SetupContractSchemaError("source_root is not an approved setup asset root")
    preimage_source = _keys(top["preimage_source"], {"kind", "commit", "proof"}, "preimage_source")
    if preimage_source["kind"] not in {"pinned-commit-staging", "release-absence-proof"} or not isinstance(preimage_source["commit"], str) or re.fullmatch(r"[0-9a-f]{40}", preimage_source["commit"]) is None or not isinstance(preimage_source["proof"], str) or not preimage_source["proof"].strip():
        raise SetupContractSchemaError("preimage_source is invalid")
    expected_kind = "release-absence-proof" if top["board_profile"] == "raspberry-pi-zero-2w" else "pinned-commit-staging"
    if preimage_source["kind"] != expected_kind:
        raise SetupContractSchemaError("preimage_source kind is not exact for the board")
    _source_inputs(top["source_inputs"])
    _directories(top["directories"], top["board_profile"])
    _entries(top["entries"], "entries")
    _validate_ui_preimages(top["entries"], top["board_profile"])
    _symlinks(top["symlinks"], top["board_profile"])
    if not isinstance(top["preserved_paths"], list) or not top["preserved_paths"]:
        raise SetupContractSchemaError("preserved_paths is invalid")
    for index, item in enumerate(top["preserved_paths"]):
        value = _keys(item, {"classification", "target", "preimage", "postimage"}, f"preserved_paths[{index}]")
        _path(value["target"], f"preserved_paths[{index}].target")
        _preimage(value["preimage"], f"preserved_paths[{index}].preimage")
        if value["postimage"] not in {"preserve", "absent"}:
            raise SetupContractSchemaError("preserved path postimage is invalid")
    if not isinstance(top["stale_runtime_markers"], list) or not top["stale_runtime_markers"] or len(set(top["stale_runtime_markers"])) != len(top["stale_runtime_markers"]):
        raise SetupContractSchemaError("stale_runtime_markers is invalid")
    for marker in top["stale_runtime_markers"]:
        _path(marker, "stale runtime marker")
    prerequisites = _keys(top["prerequisites"], {"packages", "executables", "accounts", "services"}, "prerequisites")
    if not isinstance(prerequisites["packages"], list) or not prerequisites["packages"] or any(not isinstance(item, str) or not item for item in prerequisites["packages"]):
        raise SetupContractSchemaError("package prerequisites are invalid")
    if not isinstance(prerequisites["executables"], list) or any((not isinstance(item, str) or not item) for item in prerequisites["executables"]):
        raise SetupContractSchemaError("executable prerequisites are invalid")
    for path in prerequisites["executables"]:
        _path(path, "executable prerequisite")
    if not isinstance(prerequisites["services"], list) or not prerequisites["services"] or any(not isinstance(item, str) or not item.endswith(".service") for item in prerequisites["services"]):
        raise SetupContractSchemaError("service prerequisites are invalid")
    if not isinstance(prerequisites["accounts"], list) or not prerequisites["accounts"]:
        raise SetupContractSchemaError("account prerequisites are invalid")
    for index, account in enumerate(prerequisites["accounts"]):
        value = _keys(account, {"user", "group", "home", "shell"}, f"account prerequisite[{index}]")
        if any(not isinstance(value[key], str) or not value[key] for key in value):
            raise SetupContractSchemaError("account prerequisite is invalid")
    recipe = _keys(top["recipe"], {"account_mutation", "package_mutation", "network_mutation", "boot_mutation", "firmware_mutation", "enabled_units", "disabled_units", "proof"}, "recipe")
    for key in ("account_mutation", "package_mutation", "network_mutation", "boot_mutation", "firmware_mutation"):
        if recipe[key] is not False:
            raise SetupContractSchemaError(f"recipe {key} must be false")
    if not isinstance(recipe["enabled_units"], list) or not isinstance(recipe["disabled_units"], list) or not isinstance(recipe["proof"], str) or not recipe["proof"]:
        raise SetupContractSchemaError("recipe identity is invalid")
