from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path


EXACT_VERSION = re.compile(r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$")
EXACT_TAG = re.compile(r"^v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$")
REQUIRED_WORKSPACE_PACKAGES = (
    Path("apps/desktop/package.json"),
    Path("packages/device-contracts/package.json"),
)


class VersionConsistencyError(ValueError):
    def __init__(self, issues: list[str]) -> None:
        self.issues = issues
        super().__init__("; ".join(issues))


def _display_path(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix()


def _load_json(root: Path, path: Path, issues: list[str]) -> dict[str, object] | None:
    try:
        with path.open(encoding="utf-8") as handle:
            document = json.load(handle)
    except FileNotFoundError:
        issues.append(f"Missing manifest: {_display_path(root, path)}")
        return None
    except (OSError, json.JSONDecodeError) as error:
        issues.append(f"Malformed JSON {_display_path(root, path)}: {error}")
        return None
    if not isinstance(document, dict):
        issues.append(f"Malformed JSON {_display_path(root, path)}: expected an object")
        return None
    return document


def _load_toml(root: Path, path: Path, issues: list[str]) -> dict[str, object] | None:
    try:
        with path.open("rb") as handle:
            document = tomllib.load(handle)
    except FileNotFoundError:
        issues.append(f"Missing manifest: {_display_path(root, path)}")
        return None
    except (OSError, tomllib.TOMLDecodeError) as error:
        issues.append(f"Malformed TOML {_display_path(root, path)}: {error}")
        return None
    return document


def _cargo_package_version(root: Path, path: Path, issues: list[str]) -> object:
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        issues.append(f"Missing manifest: {_display_path(root, path)}")
        return None
    except OSError as error:
        issues.append(f"Malformed TOML {_display_path(root, path)}: {error}")
        return None

    package_match = re.search(r"(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)", text)
    if package_match is None:
        issues.append(f"Missing [package] table: {_display_path(root, path)}")
        return None
    package_text = package_match.group(1)
    version_match = re.search(r"(?m)^\s*version\s*=\s*(.*)$", package_text)
    if version_match is None:
        return None
    raw_value = version_match.group(1).strip()
    quoted_match = re.fullmatch(r"(['\"])(.*?)\1", raw_value)
    return quoted_match.group(2) if quoted_match is not None else raw_value


def _record_version(
    root: Path,
    path: Path,
    value: object,
    versions: list[tuple[str, str]],
    issues: list[str],
) -> None:
    display_path = _display_path(root, path)
    if value is None:
        issues.append(f"Missing version field: {display_path}")
    elif not isinstance(value, str) or EXACT_VERSION.fullmatch(value) is None:
        issues.append(f"Malformed version at {display_path}: {value!r}; expected X.Y.Z")
    else:
        versions.append((display_path, value))


def _workspace_patterns(root: Path, issues: list[str]) -> list[str]:
    path = root / "pnpm-workspace.yaml"
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        issues.append(f"Missing workspace manifest: {_display_path(root, path)}")
        return []
    except OSError as error:
        issues.append(f"Malformed workspace manifest {_display_path(root, path)}: {error}")
        return []

    patterns = [
        match.group(2)
        for line in text.splitlines()
        if (match := re.fullmatch(r"\s*-\s*(['\"])(.+)\1\s*", line))
    ]
    if not patterns:
        issues.append(f"Malformed workspace manifest {_display_path(root, path)}: no package globs")
    return patterns


def _check_package_versions(root: Path, versions: list[tuple[str, str]], issues: list[str]) -> None:
    root_package = root / "package.json"
    document = _load_json(root, root_package, issues)
    if document is not None:
        _record_version(root, root_package, document.get("version"), versions, issues)

    discovered: set[Path] = set()
    for pattern in _workspace_patterns(root, issues):
        for entry in root.glob(pattern):
            manifest = entry / "package.json" if entry.is_dir() else entry
            if manifest.is_file() and manifest != root_package:
                discovered.add(manifest)

    for required in REQUIRED_WORKSPACE_PACKAGES:
        manifest = root / required
        if not manifest.is_file():
            issues.append(f"Missing manifest: {required.as_posix()}")
        elif manifest not in discovered:
            issues.append(f"Workspace package manifest is not covered by pnpm-workspace.yaml: {required.as_posix()}")
            discovered.add(manifest)

    for manifest in sorted(discovered):
        document = _load_json(root, manifest, issues)
        if document is not None:
            _record_version(root, manifest, document.get("version"), versions, issues)


def _check_cargo_versions(root: Path, versions: list[tuple[str, str]], issues: list[str]) -> None:
    workspace_path = root / "Cargo.toml"
    workspace_document = _load_toml(root, workspace_path, issues)
    if workspace_document is None:
        return

    workspace = workspace_document.get("workspace")
    if not isinstance(workspace, dict):
        issues.append("Missing [workspace] table: Cargo.toml")
        return

    workspace_package = workspace.get("package")
    if isinstance(workspace_package, dict) and "version" in workspace_package:
        _record_version(root, workspace_path, workspace_package["version"], versions, issues)

    members = workspace.get("members")
    if not isinstance(members, list) or not members:
        issues.append("Missing workspace.members list: Cargo.toml")
        return

    for member in members:
        if not isinstance(member, str) or any(character in member for character in "*?["):
            issues.append(f"Malformed workspace member in Cargo.toml: {member!r}")
            continue
        member_path = root / member
        manifest_path = member_path if member_path.name == "Cargo.toml" else member_path / "Cargo.toml"
        value = _cargo_package_version(root, manifest_path, issues)
        _record_version(root, manifest_path, value, versions, issues)


def _check_tauri_version(root: Path, versions: list[tuple[str, str]], issues: list[str]) -> None:
    path = root / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
    document = _load_json(root, path, issues)
    if document is not None:
        _record_version(root, path, document.get("version"), versions, issues)


def check_repository(root: Path, tag: str | None = None) -> str:
    root = root.resolve()
    versions: list[tuple[str, str]] = []
    issues: list[str] = []
    _check_cargo_versions(root, versions, issues)
    _check_package_versions(root, versions, issues)
    _check_tauri_version(root, versions, issues)

    if tag is not None and EXACT_TAG.fullmatch(tag) is None:
        issues.append(f"Malformed release tag: {tag!r}; expected vX.Y.Z")

    if versions:
        expected_path, expected = versions[0]
        mismatches = [(path, value) for path, value in versions[1:] if value != expected]
        if mismatches:
            values = ", ".join(f"{path}={value!r}" for path, value in versions)
            issues.append(f"Version mismatch (expected {expected_path}={expected!r}): {values}")
        if tag is not None and EXACT_TAG.fullmatch(tag) is not None and tag.removeprefix("v") != expected:
            issues.append(f"Release tag {tag!r} does not match application version {expected!r}")
    elif not issues:
        issues.append("No application version fields were found")

    if issues:
        raise VersionConsistencyError(issues)
    return versions[0][1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check Octessera release version consistency.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--tag", help="Optional exact release tag in vX.Y.Z form")
    args = parser.parse_args(argv)
    try:
        print(check_repository(args.root, args.tag))
    except VersionConsistencyError as error:
        print("Release version consistency check failed:", file=sys.stderr)
        for issue in error.issues:
            print(f"- {issue}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
