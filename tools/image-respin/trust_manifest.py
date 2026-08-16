import hashlib
import json
import re
from pathlib import Path
from typing import Any


SCHEMA = "octessera.image-parent-trust/v1"
REPOSITORY = "nexxyz/octessera"
RELEASE_TAG = "v0.7.5"
RELEASE_URL = "https://github.com/nexxyz/octessera/releases/tag/v0.7.5"
PUBLISHED_AT = "2026-08-02T14:27:16Z"
SOURCE_COMMIT = "4eec2b7edf6619fa22c709d4a589237a5748de78"
ASSET_COUNT = 27
LIVE_RESPIN_WITHDRAWN_ASSET_NAMES = frozenset(
    {
        "octessera-0.7.5-macos-unsigned.dmg",
        "SHA256SUMS-macos.txt",
    }
)
LIVE_RESPIN_ASSET_COUNT = ASSET_COUNT - len(LIVE_RESPIN_WITHDRAWN_ASSET_NAMES)

REQUIRED_BOARD_NAMES = ("orange-pi-zero-2w", "raspberry-pi-zero-2w")
BOARD_ARTIFACTS = {
    "orange-pi-zero-2w": {
        "asset": "octessera-0.7.5-orange-pi-zero-2w.img.xz",
        "proof_companion_assets": (
            "octessera-0.7.5-orange-pi-zero-2w.img.xz.sha256",
            "linux-image-current-sunxi64_26.8.0-trunk.417_arm64.deb",
            "linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64.deb",
            "octessera-orange-kernel-evidence.env",
            "octessera-orange-kernel-provenance.txt",
            "octessera-orange-image-provenance.txt",
            "SHA256SUMS-orange-pi-zero-2w.txt",
        ),
    },
    "raspberry-pi-zero-2w": {
        "asset": "octessera-0.7.5-raspberry-pi-zero-2w.img.zip",
        "proof_companion_assets": (
            "octessera-0.7.5-raspberry-pi-zero-2w.rpi-imager-manifest",
            "SHA256SUMS-pi.txt",
            "linux-image-6.12.93-octessera-rpi-v8-0.7.5_6.12.93-octessera0.7.5-1_arm64.deb",
            "octessera-0.7.5-raspberry-pi-zero-2w-kernel-SHA256SUMS",
            "octessera-0.7.5-raspberry-pi-zero-2w-kernel-inventory.json",
            "octessera-0.7.5-raspberry-pi-zero-2w-kernel-provenance.json",
        ),
    },
}

OTHER_RELEASE_ASSET_NAMES = (
    "octessera-0.7.5-macos-unsigned.dmg",
    "octessera-0.7.5-orange-pi-zero-2w-standalone-manual-aarch64.zip",
    "octessera-0.7.5-raspberry-pi-zero-2w-device-aarch64.zip",
    "octessera-0.7.5-ubuntu-amd64.deb",
    "octessera-0.7.5-ubuntu-x86_64.AppImage",
    "octessera-0.7.5-windows-installer.exe",
    "octessera-0.7.5-windows-portable.exe",
    "SHA256SUMS-macos.txt",
    "SHA256SUMS-orange-pi-zero-2w-device.txt",
    "SHA256SUMS-raspberry-pi-zero-2w-device.txt",
    "SHA256SUMS-ubuntu.txt",
    "SHA256SUMS-windows.txt",
)

EXPECTED_ASSET_NAMES = frozenset(
    name
    for board in BOARD_ARTIFACTS.values()
    for name in (board["asset"], *board["proof_companion_assets"])
) | frozenset(OTHER_RELEASE_ASSET_NAMES)
ASSET_NAME_RE = re.compile(r"^[^/\\]+$")
NODE_ID_RE = re.compile(r"^RA_[A-Za-z0-9_-]+$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
CONTENT_TYPE_RE = re.compile(
    r"^[A-Za-z0-9!#$&^_.+-]+/[A-Za-z0-9!#$&^_.+-]+"
    r"(?:;\s*[A-Za-z0-9!#$&^_.+-]+=[A-Za-z0-9!#$&^_.+-]+)*$"
)


class ManifestError(ValueError):
    pass


def _duplicate_json_key_rejected(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    document: dict[str, Any] = {}
    for key, value in pairs:
        if key in document:
            raise ManifestError(f"duplicate JSON key: {key}")
        document[key] = value
    return document


def parse_json_text(text: str, source: str) -> Any:
    try:
        return json.loads(text, object_pairs_hook=_duplicate_json_key_rejected)
    except ManifestError:
        raise
    except json.JSONDecodeError as error:
        raise ManifestError(f"invalid JSON in {source}: {error.msg}") from error


def load_json_file(path: Path) -> Any:
    try:
        return parse_json_text(path.read_text(encoding="utf-8"), str(path))
    except OSError as error:
        raise ManifestError(f"cannot read {path}: {error}") from error


def _require_object(value: Any, path: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ManifestError(f"{path} must be an object")
    return value


def _require_list(value: Any, path: str) -> list[Any]:
    if not isinstance(value, list):
        raise ManifestError(f"{path} must be an array")
    return value


def _require_string(value: Any, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise ManifestError(f"{path} must be a non-empty string")
    return value


def _require_exact_keys(value: dict[str, Any], expected: set[str], path: str) -> None:
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        detail = []
        if missing:
            detail.append(f"missing {missing}")
        if extra:
            detail.append(f"extra {extra}")
        raise ManifestError(f"{path} has invalid keys: {', '.join(detail)}")


def _require_exact_value(value: Any, expected: Any, path: str) -> None:
    if value != expected or type(value) is not type(expected):
        raise ManifestError(f"{path} is not {expected!r}")


def _expected_asset_class(name: str) -> str:
    for board in BOARD_ARTIFACTS.values():
        if name == board["asset"]:
            return "trusted-production-parent"
        if name in board["proof_companion_assets"]:
            return "proof-companion"
    if name in OTHER_RELEASE_ASSET_NAMES:
        return "other-release-asset"
    raise ManifestError(f"asset is not an exact v0.7.5 anchor: {name}")


def _validate_release_identity(release: dict[str, Any]) -> None:
    _require_exact_keys(
        release,
        {
            "repository",
            "tag",
            "url",
            "published_at",
            "source_commit",
            "asset_count",
            "is_draft",
            "is_prerelease",
        },
        "release",
    )
    _require_exact_value(release["repository"], REPOSITORY, "release.repository")
    _require_exact_value(release["tag"], RELEASE_TAG, "release.tag")
    _require_exact_value(release["url"], RELEASE_URL, "release.url")
    _require_exact_value(release["published_at"], PUBLISHED_AT, "release.published_at")
    source_commit = _require_string(release["source_commit"], "release.source_commit")
    if not COMMIT_RE.fullmatch(source_commit) or source_commit != SOURCE_COMMIT:
        raise ManifestError("release.source_commit is not the v0.7.5 source commit")
    asset_count = release["asset_count"]
    if type(asset_count) is not int or asset_count != ASSET_COUNT:
        raise ManifestError("release.asset_count is not 27")
    _require_exact_value(release["is_draft"], False, "release.is_draft")
    _require_exact_value(release["is_prerelease"], False, "release.is_prerelease")


def _validate_image_parents(parents: list[Any], assets: dict[str, dict[str, Any]]) -> None:
    if len(parents) != len(REQUIRED_BOARD_NAMES):
        raise ManifestError("image_parents must contain exactly the two supported boards")
    seen_boards: set[str] = set()
    referenced_names: set[str] = set()
    for index, raw_parent in enumerate(parents):
        parent = _require_object(raw_parent, f"image_parents[{index}]")
        _require_exact_keys(
            parent,
            {"board", "artifact_class", "asset", "proof_companion_assets"},
            f"image_parents[{index}]",
        )
        board_name = _require_string(parent["board"], f"image_parents[{index}].board")
        if board_name not in BOARD_ARTIFACTS or board_name in seen_boards:
            raise ManifestError(f"unsupported or duplicate board: {board_name}")
        seen_boards.add(board_name)
        _require_exact_value(
            parent["artifact_class"],
            "trusted-production-parent",
            f"image_parents[{index}].artifact_class",
        )
        expected = BOARD_ARTIFACTS[board_name]
        _require_exact_value(parent["asset"], expected["asset"], f"image_parents[{index}].asset")
        proof_assets = _require_list(
            parent["proof_companion_assets"],
            f"image_parents[{index}].proof_companion_assets",
        )
        proof_names = []
        for proof_index, proof_asset in enumerate(proof_assets):
            proof_names.append(
                _require_string(
                    proof_asset,
                    f"image_parents[{index}].proof_companion_assets[{proof_index}]",
                )
            )
        if tuple(proof_names) != expected["proof_companion_assets"]:
            raise ManifestError(f"proof companions for {board_name} are not exact")
        for name in (expected["asset"], *expected["proof_companion_assets"]):
            if name in referenced_names:
                raise ManifestError(f"asset referenced by more than one board: {name}")
            referenced_names.add(name)
            if name not in assets:
                raise ManifestError(f"referenced asset is missing: {name}")

    if set(seen_boards) != set(REQUIRED_BOARD_NAMES):
        raise ManifestError("image_parents board set is not exact")


def _validate_asset_record(raw_asset: Any, index: int) -> tuple[str, str, dict[str, Any]]:
    asset = _require_object(raw_asset, f"assets[{index}]")
    _require_exact_keys(
        asset,
        {"name", "node_id", "size", "sha256", "content_type", "artifact_class"},
        f"assets[{index}]",
    )
    name = _require_string(asset["name"], f"assets[{index}].name")
    if not ASSET_NAME_RE.fullmatch(name) or name in {".", ".."}:
        raise ManifestError(f"assets[{index}].name is not a safe exact filename")
    if name not in EXPECTED_ASSET_NAMES:
        raise ManifestError(f"extra asset name: {name}")
    node_id = _require_string(asset["node_id"], f"assets[{index}].node_id")
    if not NODE_ID_RE.fullmatch(node_id):
        raise ManifestError(f"malformed GitHub node ID: {node_id}")
    size = asset["size"]
    if type(size) is not int or size < 0:
        raise ManifestError(f"assets[{index}].size must be a non-negative integer")
    sha256 = _require_string(asset["sha256"], f"assets[{index}].sha256")
    if not SHA256_RE.fullmatch(sha256):
        raise ManifestError(f"malformed SHA-256 digest for {name}")
    content_type = _require_string(asset["content_type"], f"assets[{index}].content_type")
    if not CONTENT_TYPE_RE.fullmatch(content_type):
        raise ManifestError(f"malformed content type for {name}")
    artifact_class = _require_string(asset["artifact_class"], f"assets[{index}].artifact_class")
    if artifact_class != _expected_asset_class(name):
        raise ManifestError(f"unsupported or incorrect artifact class for {name}")
    return name, node_id, asset


def validate_manifest_document(document: Any) -> dict[str, Any]:
    manifest = _require_object(document, "manifest")
    _require_exact_keys(manifest, {"schema", "release", "image_parents", "assets"}, "manifest")
    _require_exact_value(manifest["schema"], SCHEMA, "schema")
    release = _require_object(manifest["release"], "release")
    _validate_release_identity(release)
    raw_assets = _require_list(manifest["assets"], "assets")
    if len(raw_assets) != ASSET_COUNT:
        raise ManifestError(f"assets must contain exactly {ASSET_COUNT} records")
    assets: dict[str, dict[str, Any]] = {}
    node_ids: set[str] = set()
    for index, raw_asset in enumerate(raw_assets):
        name, node_id, asset = _validate_asset_record(raw_asset, index)
        if name in assets:
            raise ManifestError(f"duplicate asset name: {name}")
        if node_id in node_ids:
            raise ManifestError(f"duplicate GitHub node ID: {node_id}")
        assets[name] = asset
        node_ids.add(node_id)
    if set(assets) != EXPECTED_ASSET_NAMES:
        missing = sorted(EXPECTED_ASSET_NAMES - set(assets))
        extra = sorted(set(assets) - EXPECTED_ASSET_NAMES)
        raise ManifestError(f"asset set is not exact; missing={missing}, extra={extra}")
    _validate_image_parents(_require_list(manifest["image_parents"], "image_parents"), assets)
    return manifest


def load_manifest(path: Path) -> dict[str, Any]:
    return validate_manifest_document(load_json_file(path))


def parent_context_for_board(manifest: dict[str, Any], board: str) -> dict[str, Any]:
    checked = validate_manifest_document(manifest)
    if board not in REQUIRED_BOARD_NAMES:
        raise ManifestError(f"unsupported board profile: {board}")
    parents = [parent for parent in checked["image_parents"] if parent["board"] == board]
    if len(parents) != 1:
        raise ManifestError(f"trusted parent is not unique for {board}")
    asset_name = parents[0]["asset"]
    assets = {asset["name"]: asset for asset in checked["assets"]}
    asset = assets[asset_name]
    return {
        "schema": checked["schema"],
        "repository": checked["release"]["repository"],
        "tag": checked["release"]["tag"],
        "source_commit": checked["release"]["source_commit"],
        "asset": {key: asset[key] for key in ("name", "node_id", "size", "sha256")},
    }


def _release_alias(document: dict[str, Any], aliases: tuple[str, ...], label: str) -> Any:
    present = [key for key in aliases if key in document]
    if not present:
        raise ManifestError(f"release JSON is missing {label}")
    values = [document[key] for key in present]
    if any(value != values[0] for value in values[1:]):
        raise ManifestError(f"release JSON has conflicting {label} fields")
    return values[0]


def _release_asset_node_id(asset: dict[str, Any], index: int) -> Any:
    if "node_id" in asset:
        return asset["node_id"]
    if "id" in asset:
        return asset["id"]
    raise ManifestError(f"release JSON asset {index} is missing its GitHub node ID")


def _validate_exact_release_document(
    document: Any,
    manifest: dict[str, Any],
    expected_names: frozenset[str],
    expected_count: int,
    label: str,
) -> None:
    checked_manifest = validate_manifest_document(manifest)
    release = _require_object(document, label)
    tag = _release_alias(release, ("tag_name", "tagName"), "tag")
    if tag != RELEASE_TAG:
        raise ManifestError(f"{label} tag is not v0.7.5")
    if "html_url" in release:
        release_url = release["html_url"]
    elif "url" in release:
        release_url = release["url"]
    else:
        raise ManifestError(f"{label} is missing its release URL")
    if release_url != RELEASE_URL:
        raise ManifestError(f"{label} URL is not the exact v0.7.5 release URL")
    published_at = _release_alias(release, ("published_at", "publishedAt"), "published time")
    if published_at != PUBLISHED_AT:
        raise ManifestError(f"{label} published time is not the exact v0.7.5 time")
    draft = _release_alias(release, ("draft", "is_draft", "isDraft"), "draft state")
    prerelease = _release_alias(
        release, ("prerelease", "is_prerelease", "isPrerelease"), "prerelease state"
    )
    _require_exact_value(draft, False, f"{label} draft state")
    _require_exact_value(prerelease, False, f"{label} prerelease state")
    if "repository" in release:
        repository = release["repository"]
        if isinstance(repository, dict):
            repository = repository.get("full_name")
        if repository != REPOSITORY:
            raise ManifestError(f"{label} repository is not nexxyz/octessera")
    for source_key in ("source_commit", "sourceCommit"):
        if source_key in release and release[source_key] != SOURCE_COMMIT:
            raise ManifestError(f"{label} source commit is not the v0.7.5 source commit")
    raw_assets = _require_list(release.get("assets"), f"{label} assets")
    if len(raw_assets) != expected_count:
        raise ManifestError(f"{label} asset count is not {expected_count}")
    manifest_assets = {asset["name"]: asset for asset in checked_manifest["assets"]}
    diagnostic_prefix = "" if label == "release JSON" else "live respin "
    seen_names: set[str] = set()
    for index, raw_asset in enumerate(raw_assets):
        asset = _require_object(raw_asset, f"{label} assets[{index}]")
        name = _require_string(asset.get("name"), f"{label} assets[{index}].name")
        if name in seen_names:
            raise ManifestError(f"{label} has duplicate asset: {name}")
        seen_names.add(name)
        expected = manifest_assets.get(name)
        if expected is None or name not in expected_names:
            raise ManifestError(f"{label} has an extra asset: {name}")
        node_id = _release_asset_node_id(asset, index)
        if node_id != expected["node_id"]:
            raise ManifestError(f"{diagnostic_prefix}node ID anchor mismatch for {name}")
        if asset.get("size") != expected["size"] or type(asset.get("size")) is not int:
            raise ManifestError(f"{diagnostic_prefix}size anchor mismatch for {name}")
        content_type = asset.get("content_type", asset.get("contentType"))
        if content_type != expected["content_type"]:
            raise ManifestError(f"{diagnostic_prefix}content type anchor mismatch for {name}")
        if asset.get("digest") != f"sha256:{expected['sha256']}":
            raise ManifestError(f"{diagnostic_prefix}SHA-256 anchor mismatch for {name}")
    if seen_names != expected_names:
        missing = sorted(expected_names - seen_names)
        raise ManifestError(f"{label} is missing assets: {missing}")


def validate_release_document(document: Any, manifest: dict[str, Any]) -> None:
    _validate_exact_release_document(
        document, manifest, EXPECTED_ASSET_NAMES, ASSET_COUNT, "release JSON"
    )


def validate_live_respin_release_document(document: Any, manifest: dict[str, Any]) -> None:
    _validate_exact_release_document(
        document,
        manifest,
        EXPECTED_ASSET_NAMES - LIVE_RESPIN_WITHDRAWN_ASSET_NAMES,
        LIVE_RESPIN_ASSET_COUNT,
        "live respin release JSON",
    )


def _board_asset_names(manifest: dict[str, Any], boards: tuple[str, ...]) -> tuple[str, ...]:
    names: list[str] = []
    for parent in manifest["image_parents"]:
        if parent["board"] in boards:
            names.extend((parent["asset"], *parent["proof_companion_assets"]))
    return tuple(names)


def validate_downloaded_directory(
    directory: Path, manifest: dict[str, Any], boards: tuple[str, ...] | None = None
) -> None:
    checked_manifest = validate_manifest_document(manifest)
    selected_boards = REQUIRED_BOARD_NAMES if boards is None else boards
    if not selected_boards or len(set(selected_boards)) != len(selected_boards):
        raise ManifestError("downloaded directory board selection must be non-empty and unique")
    if any(board not in REQUIRED_BOARD_NAMES for board in selected_boards):
        raise ManifestError("downloaded directory board selection is unsupported")
    if not directory.is_dir() or directory.is_symlink():
        raise ManifestError(f"downloaded directory does not exist as a real directory: {directory}")
    expected_names = set(_board_asset_names(checked_manifest, selected_boards))
    entries = list(directory.iterdir())
    actual_names = {entry.name for entry in entries}
    if actual_names != expected_names:
        missing = sorted(expected_names - actual_names)
        extra = sorted(actual_names - expected_names)
        raise ManifestError(f"downloaded board asset set is not exact; missing={missing}, extra={extra}")
    manifest_assets = {asset["name"]: asset for asset in checked_manifest["assets"]}
    for entry in entries:
        if entry.is_symlink() or not entry.is_file():
            raise ManifestError(f"downloaded board asset is not a regular file: {entry.name}")
        expected = manifest_assets[entry.name]
        digest = hashlib.sha256()
        byte_count = 0
        try:
            with entry.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    byte_count += len(chunk)
                    digest.update(chunk)
        except OSError as error:
            raise ManifestError(f"cannot read downloaded asset {entry}: {error}") from error
        if byte_count != expected["size"]:
            raise ManifestError(f"downloaded size mismatch for {entry.name}")
        if digest.hexdigest() != expected["sha256"]:
            raise ManifestError(f"downloaded SHA-256 mismatch for {entry.name}")
