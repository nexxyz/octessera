#!/usr/bin/env python3
"""Generate and verify the pinned attribution inventory for repository samples."""

from __future__ import annotations

import argparse
import csv
import hashlib
import re
import subprocess
from collections import Counter
from pathlib import Path
from typing import NoReturn
from urllib.parse import quote


COMMIT = "dbfd6ec52d4ed53b60bdbea5fc6adf295127c027"
REPOSITORY = "stargatedaw/stargate-sample-pack"
RAW_ROOT = f"https://raw.githubusercontent.com/{REPOSITORY}/"
LICENSE = "CC0-1.0"
MEDIA_SUFFIXES = frozenset({".aif", ".aiff", ".flac", ".mp3", ".ogg", ".wav"})
EXPECTED_MEDIA_COUNT = 320
HEADER = (
    "path",
    "size",
    "sha256",
    "upstream_repository",
    "upstream_commit",
    "upstream_path",
    "source_url",
    "license",
    "license_url",
)
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


def media_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file() and path.suffix.lower() in MEDIA_SUFFIXES
    )


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_source_url(upstream_path: str) -> str:
    return f"{RAW_ROOT}{COMMIT}/{quote(upstream_path, safe='/')}"


def expected_license_url() -> str:
    return f"{RAW_ROOT}{COMMIT}/LICENSE"


def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def read_rows(path: Path) -> list[dict[str, str]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read inventory {path}: {error}")
    if not lines:
        fail(f"empty inventory: {path}")
    try:
        records = list(csv.reader(lines, delimiter="\t", strict=True))
    except csv.Error as error:
        fail(f"malformed TSV {path}: {error}")
    if tuple(records[0]) != HEADER:
        fail(f"unexpected inventory header: {records[0]!r}")
    rows = []
    for line_number, record in enumerate(records[1:], start=2):
        if len(record) != len(HEADER) or not all(record):
            fail(f"malformed inventory row at line {line_number}")
        rows.append(dict(zip(HEADER, record)))
    return rows


def validate_row(row: dict[str, str], sample_root: Path) -> None:
    path = row["path"]
    if (
        not path
        or "\\" in path
        or path.startswith("/")
        or "//" in path
        or any(part in {"", ".", ".."} for part in path.split("/"))
        or any(ord(char) < 32 for char in path)
    ):
        fail(f"malformed sample path: {path!r}")
    if not re.fullmatch(r"[0-9]+", row["size"]):
        fail(f"malformed sample size for {path}")
    if not re.fullmatch(r"[0-9a-f]{64}", row["sha256"]):
        fail(f"malformed sample hash for {path}")
    upstream_path = row["upstream_path"]
    if (
        not upstream_path
        or "\\" in upstream_path
        or upstream_path.startswith("/")
        or "//" in upstream_path
        or any(part in {"", ".", ".."} for part in upstream_path.split("/"))
    ):
        fail(f"malformed upstream path for {path}")
    if row["upstream_repository"] != REPOSITORY:
        fail(f"unexpected upstream repository for {path}")
    if row["upstream_commit"] != COMMIT:
        fail(f"unexpected upstream commit for {path}")
    if row["license"] != LICENSE:
        fail(f"unexpected license for {path}")
    if row["source_url"] != expected_source_url(upstream_path):
        fail(f"source URL is not the pinned upstream path for {path}")
    if row["license_url"] != expected_license_url():
        fail(f"license URL is not the pinned upstream license for {path}")
    if any(
        token in row["source_url"] or token in row["license_url"]
        for token in ("/main/", "/master/", "/heads/")
    ):
        fail(f"mutable URL in inventory row for {path}")
    sample = sample_root / Path(*path.split("/"))
    if not sample.is_file() or sample.is_symlink():
        fail(f"missing or symlinked sample: {path}")
    if sample.stat().st_size != int(row["size"]):
        fail(f"stale size for {path}")
    if digest(sample) != row["sha256"]:
        fail(f"stale hash for {path}")


def verify(sample_root: Path, inventory: Path) -> None:
    rows = read_rows(inventory)
    if len(rows) != EXPECTED_MEDIA_COUNT:
        fail(f"expected {EXPECTED_MEDIA_COUNT} inventory rows, found {len(rows)}")
    paths = [row["path"] for row in rows]
    upstream_paths = [row["upstream_path"] for row in rows]
    duplicate_paths = [path for path, count in Counter(paths).items() if count > 1]
    duplicate_upstream = [path for path, count in Counter(upstream_paths).items() if count > 1]
    if duplicate_paths:
        fail(f"duplicate inventory paths: {duplicate_paths}")
    if duplicate_upstream:
        fail(f"duplicate upstream paths: {duplicate_upstream}")
    for row in rows:
        validate_row(row, sample_root)
    actual = {path.relative_to(sample_root).as_posix() for path in media_files(sample_root)}
    expected = set(paths)
    missing = sorted(actual - expected)
    extra = sorted(expected - actual)
    if missing or extra:
        fail(f"inventory mismatch; missing={missing!r} extra={extra!r}")
    print(f"verified {len(rows)} sample attribution rows")


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
