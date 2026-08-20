from __future__ import annotations

from pathlib import Path
from typing import Any

try:
    from .record_paths import identity
    from .record_validation import require, require_keys
except ImportError:
    from record_paths import identity
    from record_validation import require, require_keys


def tool_identity(module_path: Path, root: Path, relative_path: str, name: str) -> dict[str, Any]:
    record = identity(module_path, root)
    return {"name": name, "version": 1, "path": relative_path, "sha256": record["sha256"], "size": record["size"]}


def verify_tool(record: Any, module_path: Path, root: Path, relative_path: str, name: str) -> None:
    tool = require_keys(record, {"name", "version", "path", "sha256", "size"}, "tool")
    require(tool["name"] == name and tool["version"] == 1 and tool["path"] == relative_path, "tool identity is not exact")
    require(tool["sha256"] == identity(module_path, root)["sha256"] and tool["size"] == identity(module_path, root)["size"], "record tool changed")
