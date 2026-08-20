from __future__ import annotations

import re
from typing import Any, cast


class RecordError(ValueError):
    pass


SHA_RE = re.compile(r"^[0-9a-f]{64}$")
SOURCE_RE = re.compile(r"^[0-9a-f]{40}$")
VERSION_RE = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
DOCKER_ID_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
DOCKER_DIGEST_RE = re.compile(r"^[^\s/@]+(?:/[^\s/@]+)*@sha256:[0-9a-f]{64}$")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RecordError(message)


def require_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    require(isinstance(value, dict) and set(value) == keys, f"{label} keys are not exact")
    return cast(dict[str, Any], value)


def verify_docker_id(value: str, label: str) -> None:
    require(DOCKER_ID_RE.fullmatch(value) is not None, f"{label} is not a Docker image ID")


def verify_docker_digests(values: Any, label: str, *, required: bool) -> None:
    require(isinstance(values, list), f"{label} digests are not an array")
    require(
        (bool(values) or not required)
        and all(isinstance(value, str) and DOCKER_DIGEST_RE.fullmatch(value) is not None for value in values),
        f"{label} digests are invalid",
    )


def verify_source(source_sha: str, version: str, board: str, boards: set[str]) -> None:
    require(SOURCE_RE.fullmatch(source_sha) is not None, "source SHA is invalid")
    require(VERSION_RE.fullmatch(version) is not None, "version is not strict semver")
    require(board in boards, f"unsupported board: {board}")
