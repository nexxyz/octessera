from __future__ import annotations

import hashlib
import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable


class DiskLayoutError(ValueError):
    pass


CommandRunner = Callable[[list[str]], subprocess.CompletedProcess[str]]


@dataclass(frozen=True)
class PartitionIdentity:
    index: int
    node: str
    start: int
    size: int
    partition_type: str
    partition_uuid: str | None
    filesystem_type: str
    filesystem_uuid: str | None
    filesystem_label: str | None

    def as_dict(self) -> dict[str, Any]:
        return {"index": self.index, "start": self.start, "size": self.size, "partition_type": self.partition_type, "partition_uuid": self.partition_uuid, "filesystem_type": self.filesystem_type, "filesystem_uuid": self.filesystem_uuid, "filesystem_label": self.filesystem_label}


@dataclass(frozen=True)
class DiskLayout:
    board_profile: str
    image_size: int
    table_label: str
    disk_id: str | None
    first_lba: int
    last_lba: int
    sector_size: int
    partitions: tuple[PartitionIdentity, ...]
    raw_prepartition_sha256: str
    raw_boot_partition_sha256: str | None

    def as_dict(self) -> dict[str, Any]:
        return {"board_profile": self.board_profile, "image_size": self.image_size, "table_label": self.table_label, "disk_id": self.disk_id, "first_lba": self.first_lba, "last_lba": self.last_lba, "sector_size": self.sector_size, "partitions": [partition.as_dict() for partition in self.partitions], "raw_prepartition_sha256": self.raw_prepartition_sha256, "raw_boot_partition_sha256": self.raw_boot_partition_sha256}


def _run(command: list[str], runner: CommandRunner | None) -> subprocess.CompletedProcess[str]:
    try:
        result = runner(command) if runner is not None else subprocess.run(command, check=True, capture_output=True, text=True)
    except (OSError, subprocess.CalledProcessError) as exc:
        raise DiskLayoutError(f"disk command failed: {' '.join(command)}") from exc
    if result.returncode != 0:
        raise DiskLayoutError(f"disk command failed: {' '.join(command)}")
    return result


def _parse_export(output: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in output.splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key] = value.strip('"')
    return values


def _range_sha256(image: Path, start: int, length: int | None = None) -> str:
    digest = hashlib.sha256()
    remaining = length
    try:
        with image.open("rb") as handle:
            handle.seek(start)
            while remaining is None or remaining > 0:
                chunk = handle.read(1024 * 1024 if remaining is None else min(1024 * 1024, remaining))
                if not chunk:
                    break
                digest.update(chunk)
                if remaining is not None:
                    remaining -= len(chunk)
    except OSError as exc:
        raise DiskLayoutError(f"cannot hash disk image region: {image}") from exc
    if remaining not in (None, 0):
        raise DiskLayoutError(f"disk image region is truncated: {image}")
    return digest.hexdigest()


def _partition_node(loop_device: str, index: int, raw_node: object) -> str:
    if isinstance(raw_node, str) and raw_node:
        return raw_node
    return f"{loop_device}p{index}"


def capture_layout(image: Path, board_profile: str, loop_device: str, runner: CommandRunner | None = None) -> DiskLayout:
    image = Path(image)
    if board_profile not in {"raspberry-pi-zero-2w", "orange-pi-zero-2w"}:
        raise DiskLayoutError(f"unsupported board profile: {board_profile}")
    try:
        image_size = image.stat().st_size
    except OSError as exc:
        raise DiskLayoutError(f"disk image is unavailable: {image}") from exc
    try:
        document = json.loads(_run(["sfdisk", "--json", loop_device], runner).stdout)
        table = document["partitiontable"]
        raw_partitions = table["partitions"]
        table_label = str(table["label"])
        disk_id = table.get("id") or table.get("uuid")
        disk_id = str(disk_id) if disk_id is not None else None
        first_lba = int(table.get("firstlba", 0))
        last_lba = int(table.get("lastlba", 0))
        sector_size = int(table.get("sectorsize", table.get("sector-size", 512)))
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        raise DiskLayoutError("sfdisk returned invalid partition geometry") from exc
    if sector_size <= 0 or not isinstance(raw_partitions, list):
        raise DiskLayoutError("sfdisk returned invalid partition geometry")
    expected_count = 2 if board_profile == "raspberry-pi-zero-2w" else 1
    if len(raw_partitions) != expected_count:
        raise DiskLayoutError(f"{board_profile} requires exactly {expected_count} partitions")
    partitions: list[PartitionIdentity] = []
    for index, raw in enumerate(raw_partitions, 1):
        if not isinstance(raw, dict):
            raise DiskLayoutError("sfdisk returned a malformed partition")
        try:
            start, size = int(raw["start"]), int(raw["size"])
        except (KeyError, TypeError, ValueError) as exc:
            raise DiskLayoutError("partition start or size is invalid") from exc
        if start < 0 or size <= 0 or (partitions and start < partitions[-1].start + partitions[-1].size):
            raise DiskLayoutError("partition geometry overlaps or is invalid")
        node = _partition_node(loop_device, index, raw.get("node"))
        values = _parse_export(_run(["blkid", "-o", "export", node], runner).stdout)
        filesystem_type = values.get("TYPE", "").lower()
        if not filesystem_type:
            raise DiskLayoutError(f"partition filesystem type is missing: {node}")
        partitions.append(PartitionIdentity(index, node, start, size, str(raw.get("type", "")), raw.get("uuid"), filesystem_type, values.get("UUID"), values.get("LABEL")))
    types = [partition.filesystem_type for partition in partitions]
    if board_profile == "orange-pi-zero-2w" and types != ["ext4"]:
        raise DiskLayoutError("Orange parent must contain one ext4 root partition")
    if board_profile == "raspberry-pi-zero-2w" and types != ["vfat", "ext4"]:
        raise DiskLayoutError("Raspberry parent must contain p1 vfat and p2 ext4")
    first = partitions[0]
    raw_prepartition = _range_sha256(image, 0, first.start * sector_size)
    raw_boot = _range_sha256(image, first.start * sector_size, first.size * sector_size) if board_profile == "raspberry-pi-zero-2w" else None
    return DiskLayout(board_profile, image_size, table_label, disk_id, first_lba, last_lba, sector_size, tuple(partitions), raw_prepartition, raw_boot)


def assert_no_drift(before: DiskLayout, after: DiskLayout) -> None:
    if before.as_dict() != after.as_dict():
        raise DiskLayoutError("disk geometry, filesystem identity, or untouched raw region drifted")
