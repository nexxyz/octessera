"""Shared Cargo inventory policy, workspace, and source checks."""

from __future__ import annotations

import hashlib
import json
import os
import re
import tarfile
import tomllib
from pathlib import Path
from typing import Any


REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
WORKSPACE_PACKAGE_NAMES = {
    "platform-capabilities-build",
    "platform-core",
    "playback-runtime",
    "realtime-engine",
    "rodio-engine-source",
    "octessera-hal",
    "octessera-pi",
    "octessera-desktop",
}
POLICY_PATH = "licenses/cargo/reviewed-dependency-policy.json"
REFERENCE_PATHS = {
    "MPL-2.0": "licenses/cargo/reference/MPL-2.0.txt",
    "Apache-2.0": "licenses/cargo/reference/Apache-2.0.txt",
    "MIT": "licenses/cargo/reference/MIT.txt",
    "BSD-3-Clause": "licenses/cargo/reference/BSD-3-Clause.txt",
    "Zlib": "licenses/cargo/reference/Zlib.txt",
    "LLVM-exception": "licenses/cargo/reference/LLVM-exception.txt",
}
REFERENCE_SHA256 = {
    "MPL-2.0": "66a3107d5ad6a058aab753eaac2047ccb2ed0e39465dd0fe5844da3e300d5172",
    "Apache-2.0": "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
    "MIT": "b05785f9f18e6716bab63424b11454513b9943a222595b70411009202fc592b5",
    "BSD-3-Clause": "5a93d5831e1297ab10fe643e1a631e83be392896da14ee2951285a79012df69d",
    "Zlib": "bfb1112d49db5b1daecdfef24bd7e2f3ea0bafb33aa67aa0ab51e2bf8407c03d",
    "LLVM-exception": "e34c58338bd89d43e709e226610d8f32b3e3c47f4ad9a99a8dc1d4ac7842488e",
}
PRESERVED_CARGO_PATHS = {
    POLICY_PATH,
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def policy_key(package: dict[str, Any]) -> str:
    return "|".join(
        (
            package["name"],
            package["version"],
            package["registry_source"],
            package["checksum"],
            package["license"],
        )
    )


def workspace_license_records(root: Path, metadata: dict[str, Any]) -> list[dict[str, Any]]:
    members = [package for package in metadata["packages"] if package["id"] in metadata["workspace_members"]]
    names = {package["name"] for package in members}
    if names != WORKSPACE_PACKAGE_NAMES:
        raise RuntimeError(f"unexpected first-party workspace package set: {sorted(names)}")
    root_license = (root / "LICENSE").resolve()
    if not root_license.is_file() or root_license.is_symlink():
        raise RuntimeError("root LICENSE is missing or symlinked")
    digest = sha256(root_license.read_bytes())
    records = []
    for package in sorted(members, key=lambda item: item["name"]):
        raw_path = package.get("license_file")
        if not raw_path or Path(str(raw_path).replace("\\", "/")).is_absolute():
            raise RuntimeError(f"workspace package has no relative license_file: {package['name']}")
        resolved = (Path(package["manifest_path"]).parent / str(raw_path).replace("\\", "/")).resolve()
        if resolved != root_license or not resolved.is_file() or resolved.is_symlink():
            raise RuntimeError(f"workspace license_file does not resolve to root LICENSE: {package['name']}")
        records.append(
            {
                "name": package["name"],
                "version": package["version"],
                "manifest_license_file": str(raw_path).replace("\\", "/"),
                "resolved_license_file": "LICENSE",
                "license_sha256": digest,
            }
        )
    return records


def manifest_data(package_root: Path) -> dict[str, Any]:
    return tomllib.loads((package_root / "Cargo.toml").read_text(encoding="utf-8"))["package"]


def relative_declared_license(package_root: Path, package_manifest: dict[str, Any]) -> str | None:
    raw = package_manifest.get("license-file")
    if raw is None:
        return None
    path = Path(str(raw).replace("\\", "/"))
    if path.is_absolute() or ".." in path.parts:
        raise RuntimeError(f"invalid declared license-file path: {raw}")
    resolved = (package_root / path).resolve()
    if not resolved.is_file() or resolved.is_symlink():
        raise RuntimeError(f"declared license-file is missing: {raw}")
    return path.as_posix()


def license_source_files(package_root: Path, package_manifest: dict[str, Any], predicate: Any) -> tuple[str | None, list[Path], str]:
    declared = relative_declared_license(package_root, package_manifest)
    if declared is not None:
        return declared, [package_root / declared], "declared-license-file"
    files = [
        path
        for path in package_root.iterdir()
        if not path.is_symlink() and path.is_file() and predicate(path.name)
    ]
    return None, sorted(files, key=lambda path: path.name.casefold()), "recognized-top-level-file" if files else "manifest-license-no-file"


def registry_archive(package_name: str, version: str, checksum: str) -> Path | None:
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    filename = f"{package_name}-{version}.crate"
    for cache in sorted((cargo_home / "registry/cache").glob("*")):
        archive = cache / filename
        if archive.is_file() and sha256(archive.read_bytes()) == checksum:
            return archive
    return None


def archive_checksum_payload(archive: Path, package_name: str, checksum: str) -> bytes:
    files = {}
    with tarfile.open(archive, "r:*") as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            prefix, separator, relative = member.name.partition("/")
            if not separator or prefix != archive.stem:
                raise RuntimeError(f"unexpected crate archive member: {member.name}")
            stream = source.extractfile(member)
            if stream is None:
                raise RuntimeError(f"cannot read crate archive member: {member.name}")
            content = stream.read()
            files[relative] = sha256(content)
    payload = {"package": checksum, "files": dict(sorted(files.items()))}
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")


def verify_cargo_checksum(package_root: Path, package_name: str, version: str, checksum: str) -> dict[str, Any]:
    sidecar = package_root / ".cargo-checksum.json"
    if not sidecar.is_file() or sidecar.is_symlink():
        archive = registry_archive(package_name, version, checksum)
        if archive is None:
            return {"status": "not-present-in-registry-src", "sha256": None}
        sidecar_bytes = archive_checksum_payload(archive, package_name, checksum)
        payload = json.loads(sidecar_bytes)
        for raw_path, expected in payload["files"].items():
            source = package_root / Path(raw_path)
            if not source.is_file() or source.is_symlink() or sha256(source.read_bytes()) != expected:
                raise RuntimeError(f"registry source differs from crate archive: {package_name} {version}/{raw_path}")
        return {"status": "verified-from-crate-archive", "sha256": sha256(sidecar_bytes)}
    payload = json.loads(sidecar.read_text(encoding="utf-8"))
    if payload.get("package") != checksum or not isinstance(payload.get("files"), dict):
        raise RuntimeError(f"invalid .cargo-checksum.json package identity: {package_root}")
    for raw_path, expected in payload["files"].items():
        path = Path(raw_path.replace("\\", "/"))
        if path.is_absolute() or ".." in path.parts:
            raise RuntimeError(f"unsafe .cargo-checksum.json path: {raw_path}")
        source = package_root / Path(raw_path.replace("\\", "/"))
        if not source.is_file() or source.is_symlink() or sha256(source.read_bytes()) != expected:
            raise RuntimeError(f".cargo-checksum.json file mismatch: {package_root}/{raw_path}")
    return {"status": "verified", "sha256": sha256(sidecar.read_bytes())}


def load_policy(root: Path) -> dict[str, Any]:
    path = root / POLICY_PATH
    policy = json.loads(path.read_text(encoding="utf-8"))
    if policy.get("schema_version") != 1 or policy.get("generated") is not False:
        raise RuntimeError("reviewed dependency policy is not a hand-reviewed schema-1 file")
    if policy.get("scope") != "cargo-lock-overinclusive":
        raise RuntimeError("reviewed dependency policy has the wrong scope")
    if not isinstance(policy.get("records"), dict):
        raise RuntimeError("reviewed dependency policy records must be an object")
    if not no_absolute_paths(policy):
        raise RuntimeError("reviewed dependency policy contains an absolute path")
    return policy


def reference_text_bytes(root: Path, license_id: str) -> bytes:
    path = root / REFERENCE_PATHS[license_id]
    if path.is_file() and not path.is_symlink():
        content = path.read_bytes()
        expected = REFERENCE_SHA256.get(license_id)
        if expected is not None and sha256(content) != expected:
            raise RuntimeError(f"reviewed SPDX reference changed: {license_id}")
        return content
    if license_id == "Apache-2.0":
        content = (root / "third_party/cpal-0.15.3/LICENSE").read_bytes()
        if sha256(content) != REFERENCE_SHA256[license_id]:
            raise RuntimeError("vendored Apache reference changed")
        return content
    raise RuntimeError(f"reviewed SPDX reference is missing: {license_id}")


def reference_paths_for(package: dict[str, Any]) -> list[str]:
    effective = package.get("effective_license")
    if effective in REFERENCE_PATHS:
        return [REFERENCE_PATHS[effective]]
    paths = []
    for identifier in package.get("license_identifiers", []):
        if identifier == "Apache-2.0 WITH LLVM-exception":
            paths.extend((REFERENCE_PATHS["Apache-2.0"], REFERENCE_PATHS["LLVM-exception"]))
        elif identifier in REFERENCE_PATHS:
            paths.append(REFERENCE_PATHS[identifier])
    return sorted(set(paths))


def validate_policy(root: Path, packages: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return validate_policy_data(root, packages, load_policy(root))


def validate_policy_data(root: Path, packages: list[dict[str, Any]], policy: dict[str, Any]) -> dict[str, dict[str, Any]]:
    candidates = {policy_key(package): package for package in packages if package["license_class"] != "permissive"}
    records = policy["records"]
    if set(records) != set(candidates):
        missing = sorted(set(candidates) - set(records))
        extra = sorted(set(records) - set(candidates))
        raise RuntimeError(f"reviewed dependency policy mismatch; missing={missing}, extra={extra}")
    for key, decision in records.items():
        package = candidates[key]
        for field in ("package", "version", "source", "checksum", "manifest_license"):
            expected = (
                package["name"]
                if field == "package"
                else package["license"]
                if field == "manifest_license"
                else package["registry_source"]
                if field == "source"
                else package[field]
            )
            if decision.get(field) != expected:
                raise RuntimeError(f"reviewed dependency policy field mismatch: {key} {field}")
        reference = decision.get("reference_text")
        if reference not in REFERENCE_PATHS.values():
            raise RuntimeError(f"reviewed dependency policy reference is missing: {key}")
        reference_text_bytes(root, next(license_id for license_id, path in REFERENCE_PATHS.items() if path == reference))
        if package["license"] == "MPL-2.0":
            expected_fields = {
                "decision": "allow-unmodified-mpl-2.0",
                "effective_license": "MPL-2.0",
                "license_class": "file-level-copyleft",
                "review_status": "reviewed-with-source-obligation",
                "source_status": "unmodified-registry-source",
                "source_required": True,
                "reference_text": REFERENCE_PATHS["MPL-2.0"],
            }
        elif package["name"] == "r-efi":
            expected_fields = {
                "decision": "select-license-alternative",
                "selected_license": "Apache-2.0",
                "effective_license": "Apache-2.0",
                "license_class": "reviewed-license-alternative",
                "review_status": "reviewed-alternative",
                "source_status": "unmodified-registry-source",
                "source_required": False,
                "reference_text": REFERENCE_PATHS["Apache-2.0"],
            }
        else:
            raise RuntimeError(f"unreviewed nonpermissive dependency: {key}")
        for field, expected in expected_fields.items():
            if decision.get(field) != expected:
                raise RuntimeError(f"reviewed dependency policy decision mismatch: {key} {field}")
    return records


def no_absolute_paths(value: Any) -> bool:
    if isinstance(value, str):
        return not bool(re.match(r"^(?:[A-Za-z]:[\\/]|\\\\|/)", value))
    if isinstance(value, dict):
        return all(no_absolute_paths(child) for child in value.values())
    if isinstance(value, list):
        return all(no_absolute_paths(child) for child in value)
    return True
