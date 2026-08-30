#!/usr/bin/env python3
from __future__ import annotations

import argparse
import stat
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from tools.samples.sample_library import (  # noqa: E402
    read_manifest,
    verify_manifest,
    verify_media_tree,
    verify_metadata_tree,
)


def _account_identity(root: Path) -> tuple[int, int]:
    passwd_rows = [line.split(":") for line in (root / "etc/passwd").read_text(encoding="utf-8").splitlines() if line]
    pi_rows = [row for row in passwd_rows if row[0] == "pi"]
    if len(pi_rows) != 1 or len(pi_rows[0]) != 7:
        raise ValueError("Raspberry pi account is not exact")
    uid, gid = int(pi_rows[0][2]), int(pi_rows[0][3])
    group_rows = [line.split(":") for line in (root / "etc/group").read_text(encoding="utf-8").splitlines() if line]
    if sum(1 for row in group_rows if row[0] == "pi" and len(row) >= 3 and row[2] == str(gid)) != 1:
        raise ValueError("Raspberry pi group is not exact")
    return uid, gid


def _verify_tree_ownership(root: Path, uid: int, gid: int, label: str) -> None:
    if not root.is_dir() or root.is_symlink():
        raise ValueError(f"{label} root is not a real directory")
    paths = [root, *root.rglob("*")]
    for path in paths:
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise ValueError(f"{label} tree contains a symlink: {path.relative_to(root)}")
        if stat.S_ISDIR(metadata.st_mode):
            expected_mode = 0o755
        elif stat.S_ISREG(metadata.st_mode):
            expected_mode = 0o644
        else:
            raise ValueError(f"{label} tree contains a special entry: {path.relative_to(root)}")
        actual_mode = stat.S_IMODE(metadata.st_mode)
        if (metadata.st_uid, metadata.st_gid, actual_mode) != (uid, gid, expected_mode):
            raise ValueError(f"{label} ownership or mode is unsafe: {path.relative_to(root)}")


def verify(root: Path, repository_root: Path) -> None:
    records = read_manifest(repository_root / "samples/MANIFEST.tsv")
    pi_uid, pi_gid = _account_identity(root)
    media_root = root / "home/pi/samples"
    metadata_root = root / "usr/share/octessera/samples"
    verify_media_tree(media_root, records, ("sd-card",))
    verify_metadata_tree(metadata_root, repository_root / "samples")
    verify_manifest(metadata_root / "MANIFEST.tsv", records)
    if (metadata_root / "files").exists() or (metadata_root / "files").is_symlink():
        raise ValueError("Raspberry sample metadata tree contains packaged media")
    _verify_tree_ownership(media_root, pi_uid, pi_gid, "Raspberry sample media")
    _verify_tree_ownership(metadata_root, 0, 0, "Raspberry sample metadata")
    mountpoint = media_root / "sd-card"
    if any(mountpoint.iterdir()):
        raise ValueError("Raspberry SD-card mountpoint is not empty")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--repository-root", type=Path, default=ROOT)
    arguments = parser.parse_args()
    verify(arguments.root.resolve(), arguments.repository_root.resolve())
    print("verified Raspberry rootfs sample tree: 320 media files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
