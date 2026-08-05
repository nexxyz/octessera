from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

try:
    from .provenance import TOOL_CODE_EXTERNAL_FILES
except ImportError:
    from provenance import TOOL_CODE_EXTERNAL_FILES


SETUP_TOOL_CODE_SCHEMA = "octessera.image-setup-finalizer-tool-code/v1"
SETUP_TOOL_CODE_FILES = ("inventory.py", "provenance.py", "runtime_contract_schema.py", "runtime_contract.py", "runtime_payload.py", "runtime_transaction.py", "runtime_mutation.py", "disk_layout.py", "disk_mount.py", "disk_packaging.py", "disk_provenance.py", "setup_contract_schema.py", "setup_contract.py", "setup_provenance.py", "setup_mutation.py", "setup_proof.py", "disk_setup_respin.py", "boot_neutral.py")


def _canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")


def setup_tool_code_model(directory: Path | None = None) -> dict[str, Any]:
    base = Path(directory) if directory is not None else Path(__file__).resolve().parent
    files = []
    repository = base.parents[1]
    for name in (*SETUP_TOOL_CODE_FILES, *TOOL_CODE_EXTERNAL_FILES):
        path = base / name if name in SETUP_TOOL_CODE_FILES else repository / name
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"setup tool-code file is not a regular file: {name}")
        raw = path.read_bytes()
        files.append({"path": name, "sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)})
    body = {"schema": SETUP_TOOL_CODE_SCHEMA, "version": 1, "files": files}
    return {**body, "digest": hashlib.sha256(_canonical(body)).hexdigest()}


def setup_tool_code_digest(directory: Path | None = None) -> str:
    return str(setup_tool_code_model(directory)["digest"])


__all__ = ["SETUP_TOOL_CODE_FILES", "SETUP_TOOL_CODE_SCHEMA", "setup_tool_code_digest", "setup_tool_code_model"]
