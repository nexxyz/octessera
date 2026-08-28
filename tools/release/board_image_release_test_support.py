from __future__ import annotations

import hashlib
import json
import shutil
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "image-respin"))
sys.path.insert(0, str(ROOT / "tools" / "device-update"))
sys.path.insert(0, str(ROOT / "tools" / "legal"))

from disk_layout import DiskLayout, PartitionIdentity  # type: ignore[import-not-found]
from disk_packaging import compression_identity, file_digest, package_derived  # type: ignore[import-not-found]
from inventory import build_inventory, inventory_digest  # type: ignore[import-not-found]
from post_proof_record import _bundle_identity  # type: ignore[import-not-found]
from package_portable_zip import package_portable_zip  # type: ignore[import-not-found]
from provenance import build_provenance, digest_object  # type: ignore[import-not-found]
from record_hashing import canonical_bytes  # type: ignore[import-not-found]
from record_paths import identity  # type: ignore[import-not-found]
from record_tool_contract import tool_identity  # type: ignore[import-not-found]
from setup_contract import contract_for_board, load_contract  # type: ignore[import-not-found]
from setup_mutation import SETUP_TOOL_IDENTITY  # type: ignore[import-not-found]
from setup_provenance import setup_tool_code_model  # type: ignore[import-not-found]
from setup_workflow_record import SETUP_PROOF_TOOLS  # type: ignore[import-not-found]
from test_workflow_records import (  # type: ignore[import-not-found]
    notice_record,
    requested_setup,
    write_json as _write_json,
    write_orange_proof,
    write_respin_provenance,
)
from trust_manifest import load_manifest, parent_context_for_board  # type: ignore[import-not-found]

from tools.release.board_image_release import MANIFEST, ORANGE, RPI, RESPIN_FEATURE_COMMANDS


VERSION = "0.7.6"
SOURCE_SHA = "a" * 40
MANIFEST_PATH = ROOT / MANIFEST


def write_json(path: Path, value: dict[str, Any]) -> None:
    _write_json(path, value)


def _sha(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _runtime_bundle(work: Path, board: str) -> Path:
    bundle = work / f"{board}-runtime"
    bundle.mkdir()
    binary = bundle / "octessera-pi"
    binary.write_bytes(f"runtime-{board}".encode())
    digest = _sha(binary.read_bytes())
    write_json(
        bundle / "octessera-runtime.json",
        {
            "artifact_kind": "production-runtime",
            "binary_sha256": digest,
            "name": "octessera-pi",
            "profile": board,
            "runtime_ready": True,
            "version": VERSION,
        },
    )
    (bundle / "SHA256SUMS").write_text(f"{digest}  octessera-pi\n", encoding="utf-8")
    return bundle


def _setup_prerequisites(contract: dict[str, Any]) -> dict[str, Any]:
    return {
        "packages_sha256": "1" * 64,
        "accounts": {
            **{f"user:{item['user']}": "user" for item in contract["prerequisites"]["accounts"]},
            **{f"group:{item['group']}": "group" for item in contract["prerequisites"]["accounts"]},
        },
        "passwd_sha256": "2" * 64,
        "group_sha256": "3" * 64,
        "executables": {
            path: {
                "path": path,
                "type": "file",
                "uid": 0,
                "gid": 0,
                "mode": 493,
                "symlink": False,
                "target": None,
                "sha256": "0" * 64,
                "xattrs": {},
                "capability": None,
            }
            for path in contract["prerequisites"]["executables"]
        },
        "services": {path: "service" for path in contract["prerequisites"]["services"]},
    }


def _setup_proof(board: str, contract: dict[str, Any], contract_digest: str) -> dict[str, Any]:
    paths = sorted(
        [item["target"] for item in contract["directories"]]
        + [item["target"] for item in contract["entries"]]
        + [item["target"] for item in contract["symlinks"] if item["postimage"] == "absent"]
    )
    return {
        "proof": "setup-layer-mounted",
        "schema_version": 1,
        "board_profile": board,
        "contract_sha256": contract_digest,
        "inventory_sha256": "6" * 64,
        "prerequisites": _setup_prerequisites(contract),
        "verified_paths": paths,
    }


def _rpi_provenance(work: Path, bundle: Path, artifact: Path) -> tuple[Path, Path]:
    board = RPI
    contract_path = contract_for_board(board)
    contract, contract_digest = load_contract(contract_path)
    context = parent_context_for_board(load_manifest(MANIFEST_PATH), board)
    manifest_digest = _sha(MANIFEST_PATH.read_bytes())
    parent_identity = {
        "board_profile": board,
        "prior_version": "0.7.5",
        "prior_release_entries": {"octessera-pi": "b" * 64, "update-manifest.json": "c" * 64},
        "prior_release_digest": "d" * 64,
        "prior_state_preimage_sha256": "e" * 64,
        "prior_build_metadata_preimage_sha256": None,
        "current_target": "releases/0.7.5",
        "parent_context": context,
        "parent_context_sha256": digest_object(context),
    }
    bundle_digest = inventory_digest(build_inventory(bundle))
    runtime_contract_digest = identity(ROOT / "resources/image-mutations" / f"{board}.json", ROOT)["sha256"]
    notice = notice_record()
    runtime = build_provenance(
        board_profile=board,
        version=VERSION,
        source_identity=SOURCE_SHA,
        parent_identity=parent_identity,
        payload_digest=bundle_digest,
        mutation_contract_digest=runtime_contract_digest,
        pre_inventory_digest="0" * 64,
        post_inventory_digest="1" * 64,
        changed_paths=notice["changed_paths"],
        notice=notice,
    )
    setup_parent = {
        "board_profile": board,
        "preimage_source": contract["preimage_source"],
        "prerequisites": _setup_prerequisites(contract),
        "preimage_digest": "4" * 64,
    }
    setup_paths = sorted(
        [item["target"] for item in contract["directories"]]
        + [item["target"] for item in contract["entries"]]
        + [item["target"] for item in contract["symlinks"] if item["postimage"] == "absent"]
    )
    setup_mutation = {
        "proof_schema": "octessera.image-setup-mutation-provenance.v1",
        "schema_version": 1,
        "board_profile": board,
        "source_identity": SOURCE_SHA,
        "parent": {"identity": setup_parent, "digest": digest_object(setup_parent)},
        "setup_layer": {
            "contract_digest": contract_digest,
            "source_inputs": [identity(ROOT / item["path"], ROOT) for item in contract["source_inputs"]],
        },
        "inventories": {"pre": "2" * 64, "post": "6" * 64},
        "changed_paths": setup_paths,
        "finalizer": {
            "source_identity": SOURCE_SHA,
            "tool_identity": SETUP_TOOL_IDENTITY,
            "tool_code_digest": setup_tool_code_model(ROOT / "tools/image-respin")["digest"],
        },
    }
    proof = _setup_proof(board, contract, contract_digest)
    image = work / "rpi.img"
    image.write_bytes(b"abcdefgh")
    layout = DiskLayout(
        board,
        8,
        "dos",
        "disk",
        0,
        7,
        1,
        (
            PartitionIdentity(1, "", 1, 2, "type", "p1", "vfat", "f1", "boot"),
            PartitionIdentity(2, "", 3, 2, "type", "p2", "ext4", "f2", "root"),
        ),
        _sha(b"a"),
        _sha(b"bc"),
    )
    image_digest, image_size = file_digest(image)
    artifact_digest, artifact_size = file_digest(artifact)
    provenance = {
        "proof_schema": "octessera.image-derived-setup-respin-provenance.v1",
        "schema_version": 1,
        "board_profile": board,
        "version": VERSION,
        "source_identity": SOURCE_SHA,
        "parent": {
            "context": context,
            "trust_manifest_sha256": manifest_digest,
            "digest": digest_object({"context": context, "trust_manifest_sha256": manifest_digest}),
        },
        "runtime_mutation": {"digest": digest_object(runtime), "provenance": runtime},
        "setup_mutation": {"digest": digest_object(setup_mutation), "provenance": setup_mutation},
        "setup_proof": proof,
        "disk_invariants": {"pre": layout.as_dict(), "post": layout.as_dict(), "digest": digest_object({"pre": layout.as_dict(), "post": layout.as_dict()})},
        "derived_image": {"sha256": image_digest, "size": image_size},
        "packaged_artifact": {"sha256": artifact_digest, "size": artifact_size, "path": artifact.name},
        "finalizer": {
            "tool_identity": "octessera-image-respin-runtime-mutation/1",
            "compression_identity": compression_identity(board),
            "setup_tool_code": setup_tool_code_model(ROOT / "tools/image-respin"),
        },
    }
    provenance_path = work / f"{artifact.name}.provenance.json"
    write_json(provenance_path, provenance)
    proof_path = work / "setup-layer-proof.json"
    write_json(proof_path, proof)
    return provenance_path, proof_path


def _orange_provenance(work: Path, bundle: Path, artifact: Path) -> tuple[Path, Path, Path]:
    context = parent_context_for_board(load_manifest(MANIFEST_PATH), ORANGE)
    provenance_path = work / f"{artifact.name}.provenance.json"
    write_respin_provenance(provenance_path, ORANGE, VERSION, context, bundle, artifact)
    provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
    contract_path = contract_for_board(ORANGE)
    contract, contract_digest = load_contract(contract_path)
    proof = _setup_proof(ORANGE, contract, contract_digest)
    setup_mutation = {
        "proof_schema": "octessera.image-setup-mutation-provenance.v1",
        "schema_version": 1,
        "board_profile": ORANGE,
        "source_identity": SOURCE_SHA,
        "parent": {},
        "setup_layer": {},
        "inventories": {},
        "changed_paths": [],
        "finalizer": {},
    }
    provenance.update(
        {
            "proof_schema": "octessera.image-derived-setup-respin-provenance.v2",
            "derivation_kind": "setup-portal",
            "setup_mutation": {"digest": digest_object(setup_mutation), "provenance": setup_mutation},
            "setup_proof": proof,
            "finalizer": {
                "tool_identity": "octessera-image-respin-runtime-mutation/1",
                "compression_identity": compression_identity(ORANGE),
                "setup_tool_code": setup_tool_code_model(ROOT / "tools/image-respin"),
            },
        }
    )
    provenance["runtime_mutation"]["provenance"]["source_identity"] = SOURCE_SHA
    write_json(provenance_path, provenance)
    proof_path = work / "setup-layer-proof.json"
    write_json(proof_path, proof)
    image_proof = work / "orange-image-proof.json"
    write_orange_proof(image_proof, provenance_path, artifact, context, "setup-portal")
    return provenance_path, proof_path, image_proof


def _file_record(path: Path, recorded_path: str) -> dict[str, Any]:
    payload = path.read_bytes()
    return {"path": recorded_path, "sha256": _sha(payload), "size": len(payload)}


def _write_setup_record(board: str, handoff: Path, runtime: Path, artifact: Path, provenance: Path, proof: Path, production: Path, requested: Path) -> None:
    manifest = load_manifest(MANIFEST_PATH)
    context = parent_context_for_board(manifest, board)
    bundle = _bundle_identity(runtime, ROOT)
    bundle["path"] = "runtime-bundle"
    bundle["entries"] = [{**entry, "path": f"runtime-bundle/{Path(entry['path']).name}"} for entry in bundle["entries"]]
    bundle["sha256"] = _sha(canonical_bytes(bundle["entries"]))
    parent = next(item for item in manifest["image_parents"] if item["board"] == board)
    assets = {item["name"]: item for item in manifest["assets"]}
    companions = [{"path": f"parent-assets/{name}", "sha256": assets[name]["sha256"], "size": assets[name]["size"]} for name in sorted((parent["asset"], *parent["proof_companion_assets"]))]
    source = json.loads(requested.read_text(encoding="utf-8"))["source"]
    requested_name = "requested-build.json"
    artifact_name = artifact.name
    provenance_name = provenance.name
    production_name = production.name
    record = {
        "schema": "octessera.image-respin-setup-post-proof/v1",
        "schema_version": 1,
        "record_kind": "setup-post-proof",
        "result": {"status": "success", "setup_proof_succeeded": True},
        "source": source,
        "requested_build": _file_record(requested, f"respin-output/{requested_name}"),
        "parent": {"context": context, "trust_manifest": identity(MANIFEST_PATH, ROOT)},
        "runtime_bundle": bundle,
        "derived_artifact": _file_record(artifact, f"respin-output/{artifact_name}"),
        "setup_provenance": _file_record(provenance, f"respin-output/{provenance_name}"),
        "setup_proof": _file_record(proof, f"respin-output/{proof.name}"),
        "production_proofs": {"raspberry-sanitized" if board == RPI else "orange-image": _file_record(production, f"respin-output/{production_name}")},
        "proof_tools": [identity(ROOT / path, ROOT) for path in SETUP_PROOF_TOOLS[board]],
        "companions": companions,
        "workflow": identity(ROOT / ".github/workflows/respin-board-image.yml", ROOT),
        "tool": tool_identity(ROOT / "tools/image-respin/setup_workflow_record.py", ROOT, "tools/image-respin/setup_workflow_record.py", "octessera-image-respin-setup-post-proof"),
    }
    write_json(handoff / "setup-post-proof.json", record)


def _make_handoff(work: Path, board: str) -> tuple[Path, Path]:
    handoff = work / f"octessera-{board}-image-release-assets"
    handoff.mkdir()
    runtime = _runtime_bundle(work, board)
    requested = handoff / "requested-build.json"
    requested_value = requested_setup(board)
    requested_value["source"]["feature_command"] = RESPIN_FEATURE_COMMANDS[board]
    write_json(requested, requested_value)
    artifact_name = f"octessera-{VERSION}-{board}-derived-setup-respin{'.zip' if board == RPI else '.img.xz'}"
    raw_image = work / f"{board}.img"
    raw_image.write_bytes(b"abcdefgh" if board == RPI else b"orange-image")
    artifact = work / artifact_name
    package_derived(raw_image, artifact, board, VERSION, "setup")
    provenance_path = work / f"{artifact.name}.provenance.json"
    setup_proof = work / "setup-layer-proof.json"
    if board == RPI:
        provenance_path, setup_proof = _rpi_provenance(work, runtime, artifact)
        production = work / "raspberry-sanitized-image-proof.txt"
        production.write_text("Pi image sanitation check passed (boot layer: trusted-parent-finalization)\n", encoding="utf-8")
    else:
        provenance_path, setup_proof, production = _orange_provenance(work, runtime, artifact)
    for path in (artifact, provenance_path, setup_proof, production):
        shutil.copyfile(path, handoff / path.name)
    _write_setup_record(board, handoff, runtime, handoff / artifact.name, handoff / provenance_path.name, handoff / setup_proof.name, handoff / production.name, requested)
    names = (artifact.name, provenance_path.name, requested.name, "setup-post-proof.json", setup_proof.name, production.name)
    checksum = handoff / f"SHA256SUMS-{board}.txt"
    checksum.write_text("".join(f"{_sha((handoff / name).read_bytes())}  {name}\n" for name in names), encoding="utf-8")
    return handoff, runtime


def fixture(work: Path) -> tuple[Path, Path, Path, Path, Path]:
    gathered = work / "gathered"
    gathered.mkdir()
    rpi_handoff, rpi_runtime = _make_handoff(work, RPI)
    orange_handoff, orange_runtime = _make_handoff(work, ORANGE)
    shutil.move(rpi_handoff, gathered / rpi_handoff.name)
    shutil.move(orange_handoff, gathered / orange_handoff.name)
    release = work / "release"
    evidence = work / "evidence"
    release.mkdir()
    evidence.mkdir()
    for relative in ("raspberry/image", "raspberry/device", "raspberry/kernel", "raspberry/runtime", "orange/image", "orange/device", "orange/kernel", "orange/runtime"):
        (evidence / relative).mkdir(parents=True)
    return gathered, rpi_runtime, orange_runtime, release, evidence


def _write_device_zip(path: Path, runtime: Path, board: str, version: str, updater: bool = False) -> None:
    if board == RPI:
        names = ("octessera-pi", "octessera-device-release.json", "LICENSE", "NOTICE")
        manifest = {"updater_protocol": 2, "board_profile": board, "version": version}
    elif updater:
        names = ("octessera-pi", "octessera-device-release.json", "LICENSE", "NOTICE")
        manifest = {"updater_supported": True, "distribution": "runtime-updater", "updater_protocol": 2, "board_profile": board, "version": version}
    else:
        names = ("octessera-pi", "octessera-runtime.json", "SHA256SUMS", "octessera-device-release.json", "LICENSE", "NOTICE")
        manifest = {"updater_supported": False, "candidate_health_protocol": 1, "distribution": "standalone-manual"}
    payloads = {"octessera-pi": (runtime / "octessera-pi").read_bytes(), "octessera-device-release.json": json.dumps(manifest).encode(), "LICENSE": (ROOT / "LICENSE").read_bytes(), "NOTICE": (ROOT / "NOTICE").read_bytes()}
    if not updater and board == ORANGE:
        payloads["octessera-runtime.json"] = (runtime / "octessera-runtime.json").read_bytes()
        payloads["SHA256SUMS"] = (runtime / "SHA256SUMS").read_bytes()
    with path.open("wb") as output:
        import zipfile

        with zipfile.ZipFile(output, "w") as archive:
            for name in names:
                info = zipfile.ZipInfo(name)
                mode = 0o755 if name == "octessera-pi" else 0o644
                info.external_attr = (0o100000 | mode) << 16
                archive.writestr(info, payloads[name])


def _write_single_checksum(directory: Path, checksum_name: str, name: str) -> None:
    (directory / checksum_name).write_text(f"{_sha((directory / name).read_bytes())}  {name}\n", encoding="utf-8")


def full_fixture(work: Path) -> tuple[Path, Path, Path, Path, Path]:
    gathered, rpi_runtime, orange_runtime, release, evidence = fixture(work)
    prefix = f"octessera-{VERSION}"
    windows = gathered / "octessera-windows-release-assets"
    windows.mkdir()
    executable = work / "windows.exe"
    executable.write_bytes(b"windows")
    installer = windows / f"{prefix}-windows-installer.exe"
    installer.write_bytes(b"installer")
    package_portable_zip(ROOT, executable, windows / f"{prefix}-windows-portable.zip")
    (windows / "SHA256SUMS-windows.txt").write_text("".join(f"{_sha((windows / name).read_bytes())}  {name}\n" for name in sorted(path.name for path in windows.iterdir())), encoding="utf-8")
    ubuntu = gathered / "octessera-ubuntu-release-assets"
    ubuntu.mkdir()
    for name in (f"{prefix}-ubuntu-amd64.deb", f"{prefix}-ubuntu-x86_64.AppImage"):
        (ubuntu / name).write_bytes(name.encode())
    (ubuntu / "SHA256SUMS-ubuntu.txt").write_text("".join(f"{_sha((ubuntu / name).read_bytes())}  {name}\n" for name in sorted(path.name for path in ubuntu.iterdir())), encoding="utf-8")
    rpi_device = gathered / "octessera-raspberry-device-release-assets"
    rpi_device.mkdir()
    rpi_zip = rpi_device / f"{prefix}-{RPI}-device-aarch64.zip"
    _write_device_zip(rpi_zip, rpi_runtime, RPI, VERSION)
    _write_single_checksum(rpi_device, f"SHA256SUMS-{RPI}-device.txt", rpi_zip.name)
    orange_device = gathered / "octessera-orange-device-release-assets"
    orange_device.mkdir()
    orange_zip = orange_device / f"{prefix}-{ORANGE}-standalone-manual-aarch64.zip"
    _write_device_zip(orange_zip, orange_runtime, ORANGE, VERSION)
    _write_single_checksum(orange_device, f"SHA256SUMS-{ORANGE}-device.txt", orange_zip.name)
    orange_updater = orange_device / f"{prefix}-{ORANGE}-runtime-updater-aarch64.zip"
    _write_device_zip(orange_updater, orange_runtime, ORANGE, VERSION, updater=True)
    _write_single_checksum(orange_device, f"SHA256SUMS-{ORANGE}-runtime-updater.txt", orange_updater.name)
    return gathered, rpi_runtime, orange_runtime, release, evidence


def refresh_checksum(handoff: Path, board: str) -> None:
    artifact = next(path for path in handoff.iterdir() if "derived-setup-respin" in path.name and not path.name.endswith(".json"))
    names = (artifact.name, f"{artifact.name}.provenance.json", "requested-build.json", "setup-post-proof.json", "setup-layer-proof.json", "raspberry-sanitized-image-proof.txt" if board == RPI else "orange-image-proof.json")
    (handoff / f"SHA256SUMS-{board}.txt").write_text("".join(f"{_sha((handoff / name).read_bytes())}  {name}\n" for name in names), encoding="utf-8")


__all__ = ["ROOT", "RPI", "ORANGE", "SOURCE_SHA", "VERSION", "fixture", "full_fixture", "refresh_checksum", "write_json"]
