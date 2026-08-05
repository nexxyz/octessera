#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools/armbian-image"
sys.path.insert(0, str(ROOT / "tools/image-respin"))

from orange_trusted_parent_proof import _load_manifest, _protected_state, artifact_identity, digest_object, verify_trusted_roots
from disk_packaging import compression_identity
from provenance import tool_code_model
from setup_provenance import setup_tool_code_model


VERIFY_SPEC = importlib.util.spec_from_file_location("orange_image_verifier", TOOLS / "verify-orange-image.py")
assert VERIFY_SPEC is not None and VERIFY_SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(VERIFY_SPEC)
VERIFY_SPEC.loader.exec_module(VERIFY)


CONTRACT_PATH = ROOT / "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json"
CONTRACT = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
BOARD = "orange-pi-zero-2w"
RELEASE = "6.18.38-current-sunxi64"


def write(path: Path, value: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make_root(path: Path) -> None:
    write(path / "etc/os-release", "ID=armbian\n")
    write(path / "boot/Image", b"trusted-kernel")
    write(path / "boot/uInitrd", b"trusted-initramfs")
    write(path / "boot/initrd.img-6.18.38-current-sunxi64", b"trusted-initramfs")
    write(path / "boot/config-6.18.38-current-sunxi64", b"trusted-config\n")
    write(path / "boot/armbianEnv.txt", "verbosity=1\n")
    write(path / "usr/lib/linux-image-6.18.38-current-sunxi64/Image", b"trusted-kernel")
    write(path / f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb", b"\xd0\x0d\xfe\xedtrusted-dtb")
    write(path / f"usr/lib/modules/{RELEASE}/modules.dep", b"trusted-modules\n")
    write(path / "etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash", "#!/bin/sh\n")
    write(path / "etc/udev/rules.d/70-octessera-orange-runtime.rules", "KERNEL==\"i2c-2\", GROUP=\"octessera-runtime\", MODE=\"0660\"\n")
    write(path / "etc/systemd/system/octessera-orange-boot-splash.service", "[Service]\nExecStart=/usr/local/sbin/octessera-orange-oled-logo boot\n")
    write(path / "etc/systemd/system/octessera-orange-oled-shutdown.service", "[Service]\nExecStart=/usr/local/sbin/octessera-orange-oled-logo shutdown\n")
    write(path / "etc/systemd/system/octessera.service", "[Service]\nUser=octessera-runtime\n")
    write(path / "usr/lib/systemd/system-sleep/octessera-orange-oled", "#!/bin/sh\n")
    write(path / "usr/local/sbin/octessera-orange-oled-logo", "#!/bin/sh\n")
    write(path / "usr/share/octessera/oled/octessera-mark.svg", "<svg/>\n")
    write(path / "usr/share/octessera/oled/octessera-wordmark.svg", "<svg/>\n")
    (path / "lib").symlink_to("usr/lib")
    boot_link = path / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service"
    boot_link.parent.mkdir(parents=True, exist_ok=True)
    boot_link.symlink_to("../octessera-orange-boot-splash.service")
    runtime_link = path / "etc/systemd/system/multi-user.target.wants/octessera.service"
    runtime_link.parent.mkdir(parents=True, exist_ok=True)
    runtime_link.symlink_to("../octessera.service")


def make_manifest(path: Path, parent_image: Path) -> dict:
    asset = {"name": parent_image.name, "node_id": "RA_fixture", "size": parent_image.stat().st_size, "sha256": digest(parent_image), "content_type": "application/x-xz", "artifact_class": "trusted-production-parent"}
    document = {"schema": "octessera.image-parent-trust/v1", "release": CONTRACT["parent_release"], "image_parents": [{"board": BOARD, "artifact_class": "trusted-production-parent", "asset": parent_image.name}], "assets": [asset]}
    path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    return asset


def fixture_contract(asset: dict) -> dict:
    contract = copy.deepcopy(CONTRACT)
    contract["parent_asset"].update({key: asset[key] for key in ("name", "size", "sha256")})
    return contract


def make_provenance(path: Path, derived: Path, manifest: Path, contract: dict, kind: str) -> None:
    parent_image = next(manifest.parent.glob("*.img.xz"))
    asset = {"name": parent_image.name, "node_id": "RA_fixture", "size": parent_image.stat().st_size, "sha256": digest(parent_image)}
    context = {"schema": "octessera.image-parent-trust/v1", "repository": "nexxyz/octessera", "tag": "v0.7.5", "source_commit": contract["parent_release"]["source_commit"], "asset": asset}
    parent_identity = {"board_profile": BOARD, "prior_version": "0.7.5", "prior_release_entries": {"octessera-pi": "a" * 64, "octessera-runtime.json": "b" * 64, "SHA256SUMS": "c" * 64}, "prior_release_digest": "d" * 64, "prior_state_preimage_sha256": None, "prior_build_metadata_preimage_sha256": "e" * 64, "current_target": "/opt/octessera/releases/0.7.6", "parent_context": context, "parent_context_sha256": digest_object(context)}
    runtime_tool = tool_code_model(ROOT / "tools/image-respin")
    runtime = {"proof_schema": "octessera.image-mutation-provenance.v1", "schema_version": 1, "board_profile": BOARD, "version": "0.7.6", "source_identity": "fixture", "parent": {"identity": parent_identity, "digest": digest_object(parent_identity)}, "payload": {"digest": "a" * 64}, "mutation_contract": {"digest": digest(ROOT / "resources/image-mutations/orange-pi-zero-2w.json")}, "finalizer": {"source_identity": "fixture", "tool_identity": "octessera-image-respin-runtime-mutation/1", "tool_code_schema": runtime_tool["schema"], "tool_code_version": runtime_tool["version"], "tool_code_digest": runtime_tool["digest"], "tool_code_files": runtime_tool["files"]}, "inventories": {"pre": "b" * 64, "post": "c" * 64}, "parent_inventory_digest": "b" * 64, "post_inventory_digest": "c" * 64, "changed_paths": ["opt/octessera/runtime.txt"]}
    layout = {"board_profile": BOARD, "image_size": 100, "table_label": "gpt", "disk_id": None, "first_lba": 0, "last_lba": 99, "sector_size": 512, "partitions": [{"index": 1, "start": 2048, "size": 90000, "partition_type": "83", "partition_uuid": None, "filesystem_type": "ext4", "filesystem_uuid": None, "filesystem_label": None}], "raw_prepartition_sha256": "d" * 64, "raw_boot_partition_sha256": None}
    state = _protected_state(derived, contract)
    boot_inventory = {"digest": state["digest"], "count": state["count"]}
    boot_integrity = {"pre": boot_inventory, "post": boot_inventory, "selected_kernel": "boot/Image", "selected_initramfs": "boot/uInitrd", "selected_dtb": f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb", "selectors": {"format": "armbianEnv.txt", "kernel": "Image", "initramfs": "uInitrd", "dtb": "sun50i-h618-orangepi-zero2w.dtb"}, "protected_scopes": contract["protected_scopes"], "protected_paths": contract["protected_paths"], "expected_absent_paths": contract["expected_absent_paths"], "changed_paths": []}
    finalizer = {"tool_identity": "octessera-image-respin-runtime-mutation/1", "compression_identity": compression_identity(BOARD), "runtime_tool_code_schema": runtime_tool["schema"], "runtime_tool_code_version": runtime_tool["version"], "runtime_tool_code_digest": runtime_tool["digest"], "runtime_tool_code_files": runtime_tool["files"]} if kind == "runtime-only" else {"tool_identity": "octessera-image-respin-runtime-mutation/1", "compression_identity": compression_identity(BOARD), "setup_tool_code": setup_tool_code_model(ROOT / "tools/image-respin")}
    document = {"proof_schema": "octessera.image-derived-setup-respin-provenance.v2" if kind == "setup-portal" else "octessera.image-derived-respin-provenance.v2", "schema_version": 2, "proof_mode": contract["proof_mode"], "derivation_kind": kind, "board_profile": BOARD, "version": "0.7.6", "source_identity": "fixture", "boot_mutation": False, "phase5_claim": False, "policy": contract["respin_provenance"]["policy"], "parent": {"context": context, "asset": asset, "trust_manifest_sha256": digest(manifest), "digest": digest_object({"context": context, "trust_manifest_sha256": digest(manifest)})}, "runtime_mutation": {"digest": digest_object(runtime), "provenance": runtime}, "boot_integrity": boot_integrity, "disk_invariants": {"pre": layout, "post": layout, "digest": digest_object({"pre": layout, "post": layout})}, "derived_image": {"sha256": artifact_identity(derived)[0], "size": artifact_identity(derived)[1]}, "packaged_artifact": {"sha256": artifact_identity(derived)[0], "size": artifact_identity(derived)[1], "path": derived.name}, "finalizer": finalizer}
    if kind == "setup-portal":
        setup_proof = {"proof": "setup-layer-mounted", "schema_version": 1, "board_profile": BOARD, "contract_sha256": digest(ROOT / "resources/image-mutations/orange-pi-zero-2w-setup.json"), "inventory_sha256": "f" * 64, "prerequisites": {}, "verified_paths": []}
        setup = {"proof_schema": "octessera.image-setup-mutation-provenance.v1", "schema_version": 1, "board_profile": BOARD, "source_identity": "fixture", "parent": {}, "setup_layer": {}, "inventories": {}, "changed_paths": [], "finalizer": {}}
        document["setup_mutation"] = {"digest": digest_object(setup), "provenance": setup}
        document["setup_proof"] = setup_proof
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def verify_roots(parent: Path, derived: Path, parent_image: Path, manifest: Path, provenance: Path, contract: dict, kind: str, setup_proof: Path | None = None) -> dict:
    asset = {"name": parent_image.name, "node_id": "RA_fixture", "size": parent_image.stat().st_size, "sha256": digest(parent_image)}
    layout = {"board_profile": BOARD, "image_size": 100, "table_label": "gpt", "disk_id": None, "first_lba": 0, "last_lba": 99, "sector_size": 512, "partitions": [{"index": 1, "start": 2048, "size": 90000, "partition_type": "83", "partition_uuid": None, "filesystem_type": "ext4", "filesystem_uuid": None, "filesystem_label": None}], "raw_prepartition_sha256": "d" * 64, "raw_boot_partition_sha256": None}
    identity = artifact_identity(derived)
    return verify_trusted_roots(parent, derived, asset, digest(manifest), contract, provenance, kind, setup_proof, ROOT, identity, identity, derived.name, layout, layout, manifest, CONTRACT_PATH)


def main() -> None:
    canonical_asset, _ = _load_manifest(ROOT / "resources/image-parents/v0.7.5-trust-manifest.json", CONTRACT, ROOT)
    assert canonical_asset["sha256"] == CONTRACT["parent_asset"]["sha256"]
    assert canonical_asset["size"] == CONTRACT["parent_asset"]["size"]
    with tempfile.TemporaryDirectory(prefix="octessera-orange-trusted-proof-") as temporary:
        work = Path(temporary)
        parent = work / "parent-root"
        derived = work / "derived-root"
        make_root(parent)
        shutil.copytree(parent, derived, symlinks=True)
        write(derived / "opt/octessera/runtime.txt", "runtime-only mutation\n")
        parent_image = work / "octessera-0.7.5-orange-pi-zero-2w.img.xz"
        write(parent_image, b"synthetic trusted parent")
        manifest = work / "trust-manifest.json"
        asset = make_manifest(manifest, parent_image)
        contract = fixture_contract(asset)
        runtime_proof = work / "runtime-provenance.json"
        make_provenance(runtime_proof, derived, manifest, contract, "runtime-only")
        verify_roots(parent, derived, parent_image, manifest, runtime_proof, contract, "runtime-only")
        cli_reject = subprocess.run([sys.executable, str(TOOLS / "verify-orange-image.py"), "--image", str(parent_image), "--boot-proof-mode", "trusted-v0.7.5-boot-neutral", "--parent-root", str(parent)], capture_output=True, text=True)
        assert cli_reject.returncode != 0
        trusted_base = ["--image", str(parent_image), "--boot-proof-mode", "trusted-v0.7.5-boot-neutral", "--boot-neutral-contract", str(CONTRACT_PATH), "--parent-image", str(parent_image), "--trust-manifest", str(ROOT / "resources/image-parents/v0.7.5-trust-manifest.json"), "--respin-provenance", str(runtime_proof), "--derivation-kind", "runtime-only"]
        for option, value in (("--manifest", str(ROOT / "tools/kernel-patches/orange-midi-interface-manifest.json")), ("--construction-contract", str(ROOT / "resources/image-construction/boot-layers/orange-pi-zero-2w.json")), ("--linux-image", str(work / "linux.deb")), ("--linux-dtb", str(work / "dtb.deb")), ("--evidence", str(work / "evidence.env")), ("--provenance", str(work / "kernel.env"))):
            rejected = VERIFY.parse_args([*trusted_base, option, value])
            try:
                VERIFY._trusted(rejected, derived, artifact_identity(derived), artifact_identity(derived), derived.name, ROOT, {})
            except Exception:
                pass
            else:
                raise AssertionError(f"trusted mode accepted constructor argument: {option}")
        setup_provenance = work / "setup-provenance.json"
        make_provenance(setup_provenance, derived, manifest, contract, "setup-portal")
        setup_proof = work / "setup-proof.json"
        setup_proof.write_text(json.dumps(json.loads(setup_provenance.read_text())["setup_proof"]) + "\n", encoding="utf-8")
        verify_roots(parent, derived, parent_image, manifest, setup_provenance, contract, "setup-portal", setup_proof)
        bad_provenance = json.loads(runtime_proof.read_text())
        bad_provenance["boot_integrity"]["expected_absent_paths"] = []
        runtime_proof.write_text(json.dumps(bad_provenance) + "\n", encoding="utf-8")
        try:
            verify_roots(parent, derived, parent_image, manifest, runtime_proof, contract, "runtime-only")
        except Exception:
            pass
        else:
            raise AssertionError("tampered expected-absent policy was accepted")
        make_provenance(runtime_proof, derived, manifest, contract, "runtime-only")
        for relative, value in (("boot/unexpected", b"unknown"), (f"usr/lib/modules/{RELEASE}/modules.dep", b"tampered"), ("etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash", b"tampered"), ("usr/lib/systemd/system-sleep/octessera-orange-oled", b"tampered"), ("etc/udev/rules.d/70-octessera-orange-runtime.rules", b"tampered"), ("usr/local/sbin/octessera-orange-oled-handoff.py", b"unexpected")):
            tampered = work / "tampered"
            shutil.rmtree(tampered, ignore_errors=True)
            shutil.copytree(derived, tampered, symlinks=True)
            write(tampered / relative, value)
            try:
                verify_roots(parent, tampered, parent_image, manifest, runtime_proof, contract, "runtime-only")
            except Exception:
                pass
            else:
                raise AssertionError(f"protected mutation was accepted: {relative}")
        tampered = work / "tampered-lib-target"
        shutil.copytree(derived, tampered, symlinks=True)
        (tampered / "lib").unlink()
        (tampered / "lib").symlink_to("usr")
        try:
            verify_roots(parent, tampered, parent_image, manifest, runtime_proof, contract, "runtime-only")
        except Exception:
            pass
        else:
            raise AssertionError("protected lib symlink target mutation was accepted")
    print("Orange trusted parent lower-level proof and CLI rejection fixtures passed")


if __name__ == "__main__":
    main()
