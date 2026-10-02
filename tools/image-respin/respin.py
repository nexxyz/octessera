"""Respin the reviewed Orange parent image with a new runtime bundle.

The parent image is identified by resources/image-parents/<board>-current.json.
Only the runtime release, its links, the update state, and the four runtime
keys in build-metadata.env change; the partition table, pre-partition bytes,
and every file under /boot must stay byte-identical.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import lzma
import os
import re
import shutil
import tempfile
from pathlib import Path

try:
    from .disk_layout import assert_no_drift
    from .disk_mount import mounted_runtime, require_linux_root
except ImportError:
    from disk_layout import assert_no_drift
    from disk_mount import mounted_runtime, require_linux_root

BOARD = "orange-pi-zero-2w"
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
RELEASES = "opt/octessera/releases"
CURRENT_LINK = "opt/octessera/current"
BINARY_LINK = "usr/local/bin/octessera-pi"
STATE = "opt/octessera/update-state.json"
BUILD_METADATA = "etc/octessera/build-metadata.env"
BUNDLE_MODES = {"octessera-pi": 0o555, "octessera-runtime.json": 0o444, "SHA256SUMS": 0o444}
METADATA_HASH_KEYS = {
    "OCTESSERA_RUNTIME_BINARY_SHA256": "octessera-pi",
    "OCTESSERA_RUNTIME_METADATA_SHA256": "octessera-runtime.json",
    "OCTESSERA_RUNTIME_MANIFEST_SHA256": "SHA256SUMS",
}


class RespinError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def update_manifest(version: str) -> dict[str, object]:
    return {
        "schema_version": 2,
        "updater_protocol": 2,
        "candidate_health_protocol": 1,
        "tag": f"v{version}",
        "version": version,
        "board_profile": BOARD,
        "arch": "aarch64-unknown-linux-gnu",
        "binary": "octessera-pi",
        "platforms": [BOARD, "linux-aarch64-device"],
        "updater_supported": True,
        "distribution": "runtime-updater",
    }


def boot_hashes(root: Path) -> dict[str, str]:
    boot = root / "boot"
    hashes: dict[str, str] = {}
    for path in sorted(boot.rglob("*")):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            hashes[relative] = "link:" + os.readlink(path)
        elif path.is_file():
            hashes[relative] = sha256_file(path)
        else:
            hashes[relative] = "dir"
    if not hashes:
        raise RespinError("parent image has no /boot content")
    return hashes


def write_json(path: Path, value: object, mode: int) -> None:
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    os.chmod(path, mode)
    os.chown(path, 0, 0)


def replace_link(link: Path, target: str) -> None:
    temporary = link.with_name(f".{link.name}.respin")
    temporary.unlink(missing_ok=True)
    os.symlink(target, temporary)
    os.lchown(temporary, 0, 0)
    os.replace(temporary, link)


def install_runtime(root: Path, bundle: Path, version: str) -> str:
    state_path = root / STATE
    state = json.loads(state_path.read_text(encoding="utf-8"))
    prior = state.get("current")
    if state.get("phase") != "committed" or not isinstance(prior, str) or not VERSION_RE.fullmatch(prior):
        raise RespinError("parent update state is not a committed release")
    releases = root / RELEASES
    if not (releases / prior).is_dir():
        raise RespinError(f"parent release {prior} is missing")
    if version != prior and (releases / version).exists():
        raise RespinError(f"release {version} already exists in the parent image")

    staged = releases / f".respin-{version}"
    shutil.rmtree(staged, ignore_errors=True)
    staged.mkdir(mode=0o555)
    os.chown(staged, 0, 0)
    for name, mode in BUNDLE_MODES.items():
        shutil.copyfile(bundle / name, staged / name)
        os.chmod(staged / name, mode)
        os.chown(staged / name, 0, 0)
    write_json(staged / "update-manifest.json", update_manifest(version), 0o444)
    os.chmod(staged, 0o555)
    shutil.rmtree(releases / prior)
    os.replace(staged, releases / version)

    replace_link(root / CURRENT_LINK, f"/opt/octessera/releases/{version}")
    replace_link(root / BINARY_LINK, "/opt/octessera/current/octessera-pi")

    state.update({"current": version, "previous": None, "release": update_manifest(version)})
    write_json(state_path, state, 0o644)

    metadata_path = root / BUILD_METADATA
    replacements = {"OCTESSERA_RUNTIME_VERSION": version}
    replacements.update({key: sha256_file(releases / version / name) for key, name in METADATA_HASH_KEYS.items()})
    lines = metadata_path.read_text(encoding="utf-8").splitlines(keepends=True)
    seen: set[str] = set()
    for index, line in enumerate(lines):
        key = line.split("=", 1)[0]
        if key in replacements:
            lines[index] = f"{key}={replacements[key]}\n"
            seen.add(key)
    if seen != set(replacements):
        raise RespinError(f"build metadata is missing runtime keys: {sorted(set(replacements) - seen)}")
    metadata_path.write_text("".join(lines), encoding="utf-8")
    os.chmod(metadata_path, 0o644)
    return prior


def respin(parent_record: Path, parent_image: Path, bundle: Path, version: str, output: Path) -> Path:
    require_linux_root()
    if not VERSION_RE.fullmatch(version):
        raise RespinError("version must be strict semver")
    record = json.loads(parent_record.read_text(encoding="utf-8"))
    if record.get("board_profile") != BOARD:
        raise RespinError("parent record is not for the Orange board")
    if parent_image.name != record["image"]["name"] or sha256_file(parent_image) != record["image"]["sha256"]:
        raise RespinError("parent image does not match the reviewed parent record")
    expected_name = f"octessera-{version}-{BOARD}-derived-runtime-respin.img.xz"
    if output.name != expected_name or output.exists():
        raise RespinError(f"output must be a new {expected_name}")
    if sorted(path.name for path in bundle.iterdir()) != sorted(BUNDLE_MODES):
        raise RespinError("runtime bundle must contain exactly the release files")

    work = Path(tempfile.mkdtemp(prefix="octessera-respin-"))
    try:
        image = work / "parent.img"
        with lzma.open(parent_image, "rb") as source, image.open("wb") as destination:
            shutil.copyfileobj(source, destination, 1024 * 1024)
        with mounted_runtime(image, BOARD) as mounted:
            assert mounted.root_mount is not None
            boot_before = boot_hashes(mounted.root_mount)
            prior = install_runtime(mounted.root_mount, bundle, version)
            if boot_hashes(mounted.root_mount) != boot_before:
                raise RespinError("runtime respin changed /boot")
        assert mounted.pre_layout is not None and mounted.post_layout is not None
        assert_no_drift(mounted.pre_layout, mounted.post_layout)
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_name(f".{output.name}.tmp")
        with image.open("rb") as source, lzma.open(temporary, "wb", format=lzma.FORMAT_XZ, preset=9) as destination:
            shutil.copyfileobj(source, destination, 1024 * 1024)
        os.replace(temporary, output)
        print(f"respun {parent_image.name} runtime {prior} -> {version}: {output}")
        return output
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--parent-record", type=Path, required=True)
    parser.add_argument("--parent-image", type=Path, required=True)
    parser.add_argument("--runtime-bundle", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    respin(args.parent_record, args.parent_image, args.runtime_bundle, args.version, args.output.absolute())
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RespinError as exc:
        print(f"runtime respin rejected: {exc}")
        raise SystemExit(2) from exc
