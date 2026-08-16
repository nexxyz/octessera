#!/usr/bin/env python3
"""Generate and verify the pinned attribution inventory for repository samples."""

from __future__ import annotations

import argparse
import csv
import hashlib
import subprocess
import sys
from pathlib import Path
from typing import NoReturn
from urllib.parse import quote

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.samples.sample_library import (  # noqa: E402
    EXPECTED_MEDIA_COUNT,
    INVENTORY_HEADER as HEADER,
    LICENSE,
    MEDIA_SUFFIXES,
    RAW_ROOT,
    UPSTREAM_COMMIT,
    UPSTREAM_REPOSITORY,
    media_files,
    verify_library,
)

COMMIT = UPSTREAM_COMMIT
REPOSITORY = UPSTREAM_REPOSITORY
ROOT = Path(__file__).resolve().parents[2]
SAMPLE_ROOT = ROOT / "samples"
INVENTORY = SAMPLE_ROOT / "ATTRIBUTIONS.tsv"

DUPLICATE_UPSTREAM_PATHS = {
    "Drum/kick/BDrumNew_hit_v2_rr1_Sum.wav":
        "stargate-sample-pack/sgossner/VCSL/Membranophones/Bass Drum 1/BDrumNew_hit_v2_rr1_Sum.wav",
    "Drum/kick/BDrumNew_hit_v2_rr2_Sum.wav":
        "stargate-sample-pack/sgossner/VCSL/Membranophones/Bass Drum 1/BDrumNew_hit_v2_rr2_Sum.wav",
    "Drum/other/Conga/Conga_HitN_v2_rr1_Sum.wav":
        "stargate-sample-pack/sgossner/VCSL/Membranophones/Conga/Conga_HitN_v2_rr1_Sum.wav",
    "Drum/other/Conga/Conga_HitN_v2_rr2_Sum.wav":
        "stargate-sample-pack/sgossner/VCSL/Membranophones/Conga/Conga_HitN_v2_rr2_Sum.wav",
    "Drum/percussion/BDrumNewhit_v2_rr1_Sum.wav":
        "stargate-sample-pack/sgossner/VCSO-2-CE/Percussion/BDrumNewhit_v2_rr1_Sum.wav",
    "Drum/percussion/BDrumNewhit_v2_rr2_Sum.wav":
        "stargate-sample-pack/sgossner/VCSO-2-CE/Percussion/BDrumNewhit_v2_rr2_Sum.wav",
    "Drum/percussion/Conga-HitN_v2_rr1_Sum.wav":
        "stargate-sample-pack/sgossner/VCSO-2-CE/Percussion/Conga-HitN_v2_rr1_Sum.wav",
    "Drum/percussion/Conga-HitN_v2_rr2_Sum.wav":
        "stargate-sample-pack/sgossner/VCSO-2-CE/Percussion/Conga-HitN_v2_rr2_Sum.wav",
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_source_url(upstream_path: str) -> str:
    return f"{RAW_ROOT}{COMMIT}/{quote(upstream_path, safe='/')}"


def expected_license_url() -> str:
    return f"{RAW_ROOT}{COMMIT}/LICENSE"


def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def verify(sample_root: Path, inventory: Path) -> None:
    records = verify_library(sample_root, inventory)
    print(f"verified {len(records)} sample attribution rows")


def upstream_commit(root: Path) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        fail(f"cannot read upstream commit from {root}: {error}")
    return result.stdout.strip()


def generate(sample_root: Path, inventory: Path, upstream_root: Path) -> None:
    if upstream_commit(upstream_root) != COMMIT:
        fail(f"upstream checkout is not pinned to {COMMIT}")
    upstream_by_hash: dict[str, list[Path]] = {}
    for path in media_files(upstream_root):
        upstream_by_hash.setdefault(digest(path), []).append(path)
    rows = []
    for sample in media_files(sample_root):
        relative = sample.relative_to(sample_root).as_posix()
        candidates = upstream_by_hash.get(digest(sample), [])
        if not candidates:
            fail(f"no upstream byte match for {relative}")
        if len(candidates) > 1:
            selected_name = DUPLICATE_UPSTREAM_PATHS.get(relative)
            selected = [
                path
                for path in candidates
                if path.relative_to(upstream_root).as_posix() == selected_name
            ]
            if len(selected) != 1:
                fail(f"ambiguous upstream byte match for {relative}")
            upstream = selected[0]
        else:
            upstream = candidates[0]
        upstream_path = upstream.relative_to(upstream_root).as_posix()
        rows.append(
            {
                "path": relative,
                "size": str(sample.stat().st_size),
                "sha256": digest(sample),
                "upstream_repository": REPOSITORY,
                "upstream_commit": COMMIT,
                "upstream_path": upstream_path,
                "source_url": expected_source_url(upstream_path),
                "license": LICENSE,
                "license_url": expected_license_url(),
            }
        )
    if len(rows) != EXPECTED_MEDIA_COUNT:
        fail(f"expected {EXPECTED_MEDIA_COUNT} media files, found {len(rows)}")
    inventory.parent.mkdir(parents=True, exist_ok=True)
    with inventory.open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(
            output,
            fieldnames=HEADER,
            delimiter="\t",
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(rows)
    verify(sample_root, inventory)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--upstream-root", type=Path)
    parser.add_argument("--sample-root", type=Path, default=SAMPLE_ROOT)
    parser.add_argument("--inventory", type=Path, default=INVENTORY)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.generate:
        upstream_root = args.upstream_root
        if upstream_root is None:
            fail("--upstream-root is required with --generate")
        generate(args.sample_root, args.inventory, upstream_root)
    else:
        verify(args.sample_root, args.inventory)


if __name__ == "__main__":
    main()
