from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import stat
import urllib.error
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import urlsplit, urlunsplit


REPOSITORY = "nexxyz/octessera"
RECORD_RELATIVE = "resources/image-parents/orange-pi-zero-2w-current.json"
SCHEMA = "octessera.image-current-parent/v1"
BOARD = "orange-pi-zero-2w"
IMAGE_SUFFIX = ".img.xz"
IMAGE_CHECKSUM_SUFFIX = ".sha256"
CHECKSUM_NAME = "SHA256SUMS-orange-pi-zero-2w.txt"
FIXED_COMPANIONS = (
    "octessera-orange-image-proof.json",
    "octessera-orange-kernel-evidence.env",
    "octessera-orange-kernel-provenance.txt",
)
KERNEL_COMPANION_PATTERNS = (
    re.compile(r"^linux-dtb-current-sunxi64_[A-Za-z0-9.+:~-]+_arm64\.deb$"),
    re.compile(r"^linux-image-current-sunxi64_[A-Za-z0-9.+:~-]+_arm64\.deb$"),
)
VERSION_RE = re.compile(r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SHA256_PREFIX = "sha256:"


class CurrentParentError(ValueError):
    pass


class _NoAuthorizationRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request: urllib.request.Request, *args: Any, **kwargs: Any) -> urllib.request.Request | None:
        redirected = super().redirect_request(request, *args, **kwargs)
        if redirected is None:
            return None
        for name in ("Accept", "X-GitHub-Api-Version"):
            value = request.get_header(name)
            if value is not None:
                redirected.add_header(name, value)
        for headers in (redirected.headers, redirected.unredirected_hdrs):
            for name in list(headers):
                if name.lower() == "authorization":
                    del headers[name]
        return redirected


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise CurrentParentError(message)


def _safe_url(url: str) -> str:
    parts = urlsplit(url)
    return urlunsplit((parts.scheme, parts.netloc, parts.path, "", ""))


def _network_error(error: BaseException, url: str) -> str:
    if isinstance(error, urllib.error.HTTPError):
        return f"HTTP {error.code} from {_safe_url(str(error.url or url))}"
    return f"{type(error).__name__} from {_safe_url(url)}"


def _sha256(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
                size += len(chunk)
    except OSError as error:
        raise CurrentParentError(f"cannot read current parent file: {path}") from error
    return digest.hexdigest(), size


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        _require(key not in value, f"current parent record contains duplicate key: {key}")
        value[key] = item
    return value


def _json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicate_keys)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CurrentParentError(f"current parent record is unreadable: {path}") from error
    _require(isinstance(value, dict), "current parent record is not an object")
    return value


def _safe_filename(name: Any, label: str) -> str:
    _require(isinstance(name, str), f"{label} is not a filename")
    relative = PurePosixPath(name)
    _require(
        name == relative.as_posix()
        and not relative.is_absolute()
        and len(relative.parts) == 1
        and name not in {"", ".", ".."}
        and "\\" not in name,
        f"{label} is not a safe exact filename",
    )
    return name


def _digest(value: Any, label: str, prefixed: bool = False) -> str:
    expected_length = 64 + (len(SHA256_PREFIX) if prefixed else 0)
    _require(isinstance(value, str) and len(value) == expected_length, f"{label} is not a SHA-256 digest")
    if prefixed:
        _require(value.startswith(SHA256_PREFIX), f"{label} is not a SHA-256 digest")
        value = value[len(SHA256_PREFIX) :]
    _require(all(character in "0123456789abcdef" for character in value), f"{label} is not a SHA-256 digest")
    return value


def _positive_int(value: Any, label: str) -> int:
    _require(type(value) is int and value > 0, f"{label} is not a positive integer")
    return value


def _version(value: Any, label: str) -> str:
    _require(isinstance(value, str) and VERSION_RE.fullmatch(value) is not None, f"{label} is not a semantic version")
    return value


def _expiry(value: Any, label: str, require_live: bool = False) -> dt.datetime:
    _require(isinstance(value, str), f"{label} is not an ISO-8601 timestamp")
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise CurrentParentError(f"{label} is not an ISO-8601 timestamp") from error
    _require(parsed.tzinfo is not None and parsed.utcoffset() == dt.timedelta(0), f"{label} is not UTC")
    if require_live:
        _require(parsed > dt.datetime.now(dt.timezone.utc), f"{label} is expired")
    return parsed


def _validate_entries(entries: Any, image_name: str) -> None:
    _require(isinstance(entries, list), "current parent artifact entries are not an array")
    names: list[str] = []
    for index, name in enumerate(entries):
        checked = _safe_filename(name, f"current parent artifact entry {index}")
        _require(checked not in names, "current parent artifact entries are not unique")
        names.append(checked)
    image_checksum = image_name + IMAGE_CHECKSUM_SUFFIX
    kernel_entries = [
        name
        for name in names
        if any(pattern.fullmatch(name) is not None for pattern in KERNEL_COMPANION_PATTERNS)
    ]
    _require(len(kernel_entries) == len(KERNEL_COMPANION_PATTERNS), "current parent kernel companions are not exact")
    required = {image_name, image_checksum, CHECKSUM_NAME, *FIXED_COMPANIONS, *kernel_entries}
    _require(set(names) == required and len(names) == len(required), "current parent artifact entries are not exact")


def load_record(repository_root: Path, path: Path | None = None) -> tuple[dict[str, Any], str]:
    root = Path(repository_root).resolve(strict=True)
    expected = root / RECORD_RELATIVE
    candidate = expected if path is None else Path(path)
    _require(candidate.resolve(strict=True) == expected, "current parent record path is not canonical")
    _require(candidate.is_file() and not candidate.is_symlink(), "current parent record is missing or symlinked")
    record = _json(candidate)
    _require(set(record) == {"schema", "board_profile", "version", "constructor", "artifact", "image"}, "current parent record keys are not exact")
    _require(record["schema"] == SCHEMA and record["board_profile"] == BOARD, "current parent record identity changed")
    version = _version(record["version"], "current parent version")

    constructor = record["constructor"]
    _require(isinstance(constructor, dict) and set(constructor) == {"run_id", "source_sha"}, "current parent constructor identity is not exact")
    _positive_int(constructor["run_id"], "current parent constructor run")
    _require(isinstance(constructor["source_sha"], str) and COMMIT_RE.fullmatch(constructor["source_sha"]) is not None, "current parent constructor source is not a commit SHA")

    artifact = record["artifact"]
    _require(isinstance(artifact, dict) and set(artifact) == {"id", "name", "size", "digest", "expires_at", "entries"}, "current parent artifact identity is not exact")
    _positive_int(artifact["id"], "current parent artifact ID")
    _require(isinstance(artifact["name"], str) and artifact["name"].strip(), "current parent artifact name is invalid")
    _positive_int(artifact["size"], "current parent artifact size")
    _digest(artifact["digest"], "current parent artifact digest", prefixed=True)
    _expiry(artifact["expires_at"], "current parent artifact expiry")

    image = record["image"]
    _require(isinstance(image, dict) and set(image) == {"name", "size", "sha256"}, "current parent image identity is not exact")
    image_name = _safe_filename(image["name"], "current parent image name")
    _require(image_name == f"octessera-{version}-{BOARD}{IMAGE_SUFFIX}", "current parent image name is not exact")
    _positive_int(image["size"], "current parent image size")
    _digest(image["sha256"], "current parent image digest")
    _validate_entries(artifact["entries"], image_name)
    digest, _ = _sha256(candidate)
    return record, digest


def parent_context(repository_root: Path, path: Path | None = None) -> dict[str, Any]:
    root = Path(repository_root).resolve(strict=True)
    candidate = root / RECORD_RELATIVE if path is None else Path(path)
    record, digest = load_record(root, candidate)
    return {
        "schema": SCHEMA,
        "repository": REPOSITORY,
        "board_profile": record["board_profile"],
        "version": record["version"],
        "constructor": dict(record["constructor"]),
        "artifact": dict(record["artifact"]),
        "image": dict(record["image"]),
        "record": {"path": RECORD_RELATIVE, "sha256": digest, "size": candidate.stat().st_size},
    }


def validate_run_metadata(metadata: Any, record: dict[str, Any]) -> None:
    _require(isinstance(metadata, dict), "constructor run metadata is not an object")
    constructor = record["constructor"]
    _require(metadata.get("id") == constructor["run_id"], "constructor run ID does not match current parent")
    _require(metadata.get("head_sha") == constructor["source_sha"], "constructor run source does not match current parent")
    _require(metadata.get("head_branch") == "main" and metadata.get("status") == "completed" and metadata.get("conclusion") == "success", "constructor run is not the exact successful main run")


def validate_artifact_metadata(metadata: Any, record: dict[str, Any]) -> None:
    _require(isinstance(metadata, dict), "current parent artifact metadata is not an object")
    expected = record["artifact"]
    _require(metadata.get("id") == expected["id"], "current parent artifact ID does not match")
    _require(metadata.get("name") == expected["name"], "current parent artifact name does not match")
    _require(metadata.get("size_in_bytes") == expected["size"], "current parent artifact size does not match")
    _require(metadata.get("digest") == expected["digest"], "current parent artifact digest does not match")
    _require(metadata.get("expired") is False and metadata.get("expires_at") == expected["expires_at"], "current parent artifact is expired or has a different expiry")
    _expiry(metadata.get("expires_at"), "current parent artifact expiry", require_live=True)
    run = metadata.get("workflow_run")
    _require(isinstance(run, dict) and run.get("id") == record["constructor"]["run_id"] and run.get("head_sha") == record["constructor"]["source_sha"], "current parent artifact workflow run does not match")


def _validate_checksum_file(directory: Path, record: dict[str, Any]) -> None:
    checksum = directory / CHECKSUM_NAME
    try:
        lines = checksum.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as error:
        raise CurrentParentError("current parent checksum companion is unreadable") from error
    expected_names = set(record["artifact"]["entries"]) - {record["image"]["name"], checksum.name}
    actual_names: list[str] = []
    for line in lines:
        parts = line.split("  ")
        _require(len(parts) == 2, "current parent checksum companion is malformed")
        digest, name = parts
        _digest(digest, "current parent checksum entry")
        _safe_filename(name, "current parent checksum entry")
        _require(name not in actual_names and name in expected_names, "current parent checksum entries are not exact")
        actual_names.append(name)
        actual_digest, _ = _sha256(directory / name)
        _require(actual_digest == digest, f"current parent checksum mismatch: {name}")
    _require(set(actual_names) == expected_names and len(actual_names) == len(expected_names), "current parent checksum entries are not exact")


def validate_downloaded_directory(directory: Path, record: dict[str, Any]) -> None:
    directory = Path(directory)
    _require(directory.is_dir() and not directory.is_symlink(), "current parent asset directory is not a real directory")
    entries = sorted(directory.iterdir(), key=lambda item: item.name)
    _require([entry.name for entry in entries] == sorted(record["artifact"]["entries"]), "current parent asset entries are not exact")
    for entry in entries:
        _require(entry.is_file() and not entry.is_symlink() and stat.S_ISREG(entry.stat().st_mode), f"current parent asset is not a regular file: {entry.name}")
    image = directory / record["image"]["name"]
    digest, size = _sha256(image)
    _require((digest, size) == (record["image"]["sha256"], record["image"]["size"]), "current parent image hash or size does not match")
    image_checksum = directory / f"{image.name}.sha256"
    try:
        image_checksum_text = image_checksum.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise CurrentParentError("current parent image checksum companion is unreadable") from error
    _require(image_checksum_text == f"{record['image']['sha256']}  {image.name}\n", "current parent image checksum companion is not exact")
    _validate_checksum_file(directory, record)


def _safe_archive_member(info: zipfile.ZipInfo, expected: set[str]) -> None:
    name = info.filename
    _safe_filename(name, "current parent archive entry")
    _require(name in expected, f"current parent archive contains an extra entry: {name}")
    file_type = (info.external_attr >> 16) & 0o170000
    _require(not info.is_dir() and file_type in {0, stat.S_IFREG}, f"current parent archive entry is not a regular file: {name}")


def validate_archive(archive_path: Path, record: dict[str, Any], output: Path) -> None:
    expected = set(record["artifact"]["entries"])
    output = Path(output)
    _require(not output.exists() and not output.is_symlink(), "current parent output directory already exists")
    try:
        with zipfile.ZipFile(archive_path, "r") as archive:
            infos = archive.infolist()
            names = [info.filename for info in infos]
            _require(len(names) == len(set(names)), "current parent archive contains duplicate entries")
            for info in infos:
                _safe_archive_member(info, expected)
            _require(set(names) == expected and len(names) == len(expected), "current parent archive entries are not exact")
            _require(archive.testzip() is None, "current parent archive has a CRC failure")
            output.mkdir(parents=True)
            for info in infos:
                destination = output / info.filename
                with archive.open(info, "r") as source, destination.open("xb") as target:
                    shutil.copyfileobj(source, target, 1024 * 1024)
        validate_downloaded_directory(output, record)
    except (OSError, RuntimeError, zipfile.BadZipFile, zipfile.LargeZipFile) as error:
        shutil.rmtree(output, ignore_errors=True)
        if isinstance(error, CurrentParentError):
            raise
        raise CurrentParentError("current parent archive is unreadable") from error
    except CurrentParentError:
        shutil.rmtree(output, ignore_errors=True)
        raise


def _github_json(url: str, token: str) -> Any:
    request = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json", "Authorization": f"Bearer {token}", "X-GitHub-Api-Version": "2022-11-28"})
    try:
        with urllib.request.urlopen(request) as response:
            return json.load(response)
    except (OSError, urllib.error.HTTPError, urllib.error.URLError) as error:
        raise CurrentParentError(f"GitHub metadata request failed: {_network_error(error, url)}") from error
    except json.JSONDecodeError as error:
        raise CurrentParentError(f"GitHub metadata response is invalid: {_safe_url(url)}") from error


def _download(url: str, token: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"Accept": "application/octet-stream", "Authorization": f"Bearer {token}", "X-GitHub-Api-Version": "2022-11-28"})
    try:
        with urllib.request.build_opener(_NoAuthorizationRedirectHandler).open(request) as response, destination.open("wb") as target:
            shutil.copyfileobj(response, target, 1024 * 1024)
    except (OSError, urllib.error.HTTPError, urllib.error.URLError) as error:
        raise CurrentParentError(f"current parent artifact download failed: {_network_error(error, url)}") from error


def acquire(repository_root: Path, repository: str, record_path: Path, output: Path, token: str) -> None:
    _require(repository == REPOSITORY, "current parent repository is not canonical")
    root = Path(repository_root).resolve(strict=True)
    record, _ = load_record(root, record_path)
    output = Path(output)
    _require(not output.exists() and not output.is_symlink(), "current parent output directory already exists")
    _require(bool(token.strip()), "GitHub token is required for current parent acquisition")
    api_root = f"https://api.github.com/repos/{repository}"
    validate_run_metadata(_github_json(f"{api_root}/actions/runs/{record['constructor']['run_id']}", token), record)
    validate_artifact_metadata(_github_json(f"{api_root}/actions/artifacts/{record['artifact']['id']}", token), record)
    temporary = Path(output.parent) / f".current-parent-{os.getpid()}.zip"
    try:
        _require(not temporary.exists() and not temporary.is_symlink(), "current parent temporary archive already exists")
        _download(f"{api_root}/actions/artifacts/{record['artifact']['id']}/zip", token, temporary)
        digest, size = _sha256(temporary)
        _require(size == record["artifact"]["size"] and f"sha256:{digest}" == record["artifact"]["digest"], "downloaded current parent archive identity does not match")
        validate_archive(temporary, record, output)
    finally:
        temporary.unlink(missing_ok=True)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Acquire the exact reviewed current Orange parent artifact.")
    parser.add_argument("--repository", required=True)
    parser.add_argument("--record", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        acquire(Path(__file__).resolve().parents[2], args.repository, args.record, args.output, os.environ.get("GH_TOKEN", ""))
        return 0
    except (CurrentParentError, OSError) as error:
        print(f"current parent rejected: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv[1:]))
