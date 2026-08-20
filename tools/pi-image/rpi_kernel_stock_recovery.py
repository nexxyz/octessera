#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path
from typing import Callable

from rpi_kernel_image_mount import ImageProofError
from rpi_kernel_boot_proof import hash_matches, resolve_regular_file, verify_initramfs, run_lsinitramfs


def verify_stock_recovery(root: Path, boot: Path, run_listing: Callable[[Path], str] = run_lsinitramfs) -> list[dict[str, str]]:
    recovery_root = boot / "octessera/recovery-stock"
    if not recovery_root.is_dir() or recovery_root.is_symlink():
        raise ImageProofError("stock recovery directory is missing or unsafe")
    recovery_root = recovery_root.resolve(strict=True)
    manifest = resolve_regular_file(root, recovery_root / "manifest.json", "stock recovery manifest")
    try:
        entries = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ImageProofError(f"invalid stock recovery manifest: {manifest}") from error
    if not isinstance(entries, list) or not entries:
        raise ImageProofError("stock recovery manifest is empty")
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"path", "recovery_path", "sha256"} or not all(isinstance(entry[key], str) for key in ("path", "recovery_path", "sha256")):
            raise ImageProofError("stock recovery manifest entry changed")
        if len(entry["sha256"]) != 64 or any(character not in "0123456789abcdef" for character in entry["sha256"]):
            raise ImageProofError("stock recovery manifest hash changed")
        retained = resolve_regular_file(root, root / entry["path"], "retained stock file")
        recovery = resolve_regular_file(root, root / entry["recovery_path"], "stock recovery file")
        try:
            recovery.relative_to(recovery_root)
        except ValueError as error:
            raise ImageProofError(f"stock recovery file is outside the recovery directory: {recovery}") from error
        hash_matches(recovery, entry["sha256"], "stock recovery file")
        is_initramfs = Path(entry["path"]).parent == Path("boot") and Path(entry["path"]).name.startswith("initrd.img-")
        if is_initramfs:
            verify_initramfs(retained, run_listing)
            verify_initramfs(recovery, run_listing)
        else:
            hash_matches(retained, entry["sha256"], "retained stock file")
    return entries
