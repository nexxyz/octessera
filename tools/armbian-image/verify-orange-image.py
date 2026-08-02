#!/usr/bin/env python3
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import lzma
import re
import struct
import subprocess
import sys
import tempfile
from fnmatch import fnmatchcase
from pathlib import Path
from typing import Any, cast

from orange_boot_selection import BootSelectionError, parse_boot_selectors, safe_resolve
from orange_image_mount import ImageMountError, mounted_image
from verify_runtime_account import (
    read_kv_records,
    reject_unsupported_updater,
    require_owner_mode,
    require_runtime_service,
    require_runtime_udev_rule,
    runtime_account,
)


class ImageProofError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ImageProofError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_kv(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ImageProofError(f"cannot read provenance file: {path}") from error
    for line in lines:
        key, separator, value = line.partition("=")
        require(bool(separator and key and key not in values), f"malformed or duplicate provenance field: {line}")
        values[key] = value
    return values


def dpkg_fields(path: Path, *fields: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for field in fields:
        try:
            result = subprocess.run(
                ["dpkg-deb", "-f", str(path), field], check=True, capture_output=True, text=True
            )
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise ImageProofError(f"cannot read Debian package identity: {path}") from error
        values[field] = result.stdout.rstrip("\n")
    return values


def extract_package(path: Path, destination: Path) -> None:
    try:
        subprocess.run(["dpkg-deb", "-x", str(path), str(destination)], check=True, capture_output=True, text=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageProofError(f"cannot extract exact Debian package: {path}") from error


def file_hash_from_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def decompress_module(path: Path) -> bytes:
    try:
        if path.name.endswith(".ko"):
            return path.read_bytes()
        if path.name.endswith(".ko.gz"):
            return gzip.decompress(path.read_bytes())
        if path.name.endswith(".ko.xz"):
            return lzma.decompress(path.read_bytes())
        if path.name.endswith(".ko.bz2"):
            import bz2

            return bz2.decompress(path.read_bytes())
        if path.name.endswith(".ko.zst"):
            command = ["zstd", "-q", "-dc", str(path)]
        elif path.name.endswith(".ko.lz4"):
            command = ["lz4", "-q", "-dc", str(path)]
        else:
            raise ImageProofError(f"unsupported usb_f_midi compression: {path.name}")
        return subprocess.run(command, check=True, capture_output=True).stdout
    except (OSError, FileNotFoundError, subprocess.CalledProcessError) as error:
        raise ImageProofError(f"cannot decompress usb_f_midi module: {path}") from error


def module_facts(path: Path, release: str) -> dict[str, str]:
    compressed = path.read_bytes()
    decompressed = decompress_module(path)
    require(bool(decompressed), f"usb_f_midi module is empty: {path}")
    require(decompressed[:4] == b"\x7fELF", f"usb_f_midi is not an ELF module: {path}")
    require(decompressed[4:5] == b"\x02", f"usb_f_midi is not ELF64: {path}")
    require(struct.unpack_from("<H", decompressed, 18)[0] == 183, f"usb_f_midi is not AArch64: {path}")
    strings = [match.decode("ascii") for match in re.findall(rb"[ -~]{4,}", decompressed)]
    vermagic = [value.removeprefix("vermagic=") for value in strings if value.startswith("vermagic=")]
    require(len(vermagic) == 1, f"usb_f_midi vermagic marker is not unique: {path}")
    require(vermagic[0] == release or vermagic[0].startswith(f"{release} "), "usb_f_midi vermagic does not match ABI")
    markers = ("interface_string", "f_midi_opts_attr_interface_string", "midi_interface_string")
    for marker in markers:
        require(marker in strings, f"usb_f_midi marker is missing: {marker}")
    return {
        "compressed_sha256": file_hash_from_bytes(compressed),
        "decompressed_sha256": file_hash_from_bytes(decompressed),
        "vermagic": vermagic[0],
        "interface_string": markers[0],
        "interface_options": markers[1],
        "interface_runtime": markers[2],
    }


def initramfs_content(path: Path) -> bytes:
    raw = path.read_bytes()
    if raw[:4] == b"\x27\x05\x19\x56" and len(raw) >= 64:
        size = struct.unpack_from(">I", raw, 12)[0]
        raw = raw[64 : 64 + size]
    for decoder in (gzip.decompress, lzma.decompress):
        try:
            return decoder(raw)
        except (OSError, EOFError, lzma.LZMAError):
            continue
    return raw


def package_suffix(path: Path, canonical: str, label: str) -> str:
    if path.name == canonical:
        return "canonical"
    prefix = canonical.removesuffix(".deb") + "__"
    require(bool(path.name.startswith(prefix) and path.name.endswith(".deb")), f"{label} is not a native package handoff: {path.name}")
    suffix = path.name[len(prefix) : -4]
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9+._-]*", suffix) is not None, f"invalid native package suffix: {suffix}")
    return suffix


def verify_package_chain(
    image_package: Path,
    dtb_package: Path,
    evidence: dict[str, str],
    provenance: dict[str, str],
    manifest: dict[str, Any],
    work: Path,
) -> dict[str, Any]:
    armbian = manifest["build_frameworks"]["armbian"]
    orange = manifest["kernels"]["orange"]
    canonical_image, canonical_dtb = armbian["packages"]
    image_suffix = package_suffix(image_package, canonical_image, "linux-image package")
    require(package_suffix(dtb_package, canonical_dtb, "linux-dtb package") == image_suffix, "native package suffixes differ")
    require(
        image_package.name == canonical_image or fnmatchcase(image_package.name, armbian["native_package_patterns"][0]),
        "linux-image package name is not manifest-approved",
    )
    require(
        dtb_package.name == canonical_dtb or fnmatchcase(dtb_package.name, armbian["native_package_patterns"][1]),
        "linux-dtb package name is not manifest-approved",
    )
    require(sha256_file(image_package) == evidence["image_package_sha256"], "linux-image package hash does not match evidence")
    require(sha256_file(dtb_package) == evidence["dtb_package_sha256"], "linux-dtb package hash does not match evidence")
    require(evidence["image_package_native_basename"] == provenance.get("image_package_native"), "linux-image native package evidence is not bound")
    require(evidence["dtb_package_native_basename"] == provenance.get("dtb_package_native"), "linux-dtb native package evidence is not bound")
    require(evidence["artifact_suffix"] == image_suffix or image_suffix == "canonical", "native package suffix evidence is not bound")
    require(provenance.get("image_package") == canonical_image and provenance.get("dtb_package") == canonical_dtb, "kernel provenance canonical package identity changed")
    if image_package.name.startswith(canonical_image.removesuffix(".deb") + "__"):
        require(provenance.get("image_package_native") == image_package.name, "kernel provenance native image package identity changed")
    else:
        require(fnmatchcase(provenance.get("image_package_native", ""), armbian["native_package_patterns"][0]), "kernel provenance native image package identity is missing")
    if dtb_package.name.startswith(canonical_dtb.removesuffix(".deb") + "__"):
        require(provenance.get("dtb_package_native") == dtb_package.name, "kernel provenance native DTB package identity changed")
    else:
        require(fnmatchcase(provenance.get("dtb_package_native", ""), armbian["native_package_patterns"][1]), "kernel provenance native DTB package identity is missing")
    require(provenance.get("image_package_sha256") == evidence["image_package_sha256"], "kernel provenance image hash is not bound")
    require(provenance.get("dtb_package_sha256") == evidence["dtb_package_sha256"], "kernel provenance DTB hash is not bound")
    require(provenance.get("evidence_sha256") == evidence.get("_sha256"), "kernel provenance evidence hash is not bound")
    require(provenance.get("kernel_source_repository") == orange["repository"], "kernel provenance repository does not match manifest")
    require(provenance.get("kernel_source_commit") == orange["commit"], "kernel provenance source commit changed")
    release = armbian["kernel_release"]
    require(provenance.get("kernel_release") == release, "kernel provenance ABI changed")

    image_identity = dpkg_fields(
        image_package,
        "Package",
        "Version",
        "Architecture",
        "Source",
        "Armbian-Kernel-Version",
        "Armbian-Kernel-Version-Family",
    )
    dtb_identity = dpkg_fields(dtb_package, "Package", "Version", "Architecture")
    expected_architecture = canonical_image.rsplit("_", 1)[1].removesuffix(".deb")
    expected_name = canonical_image.split("_", 1)[0]
    expected_dtb_name = canonical_dtb.split("_", 1)[0]
    for key, expected in {
        "Package": expected_name,
        "Version": armbian["package_revision"],
        "Architecture": expected_architecture,
        "Source": "linux-6.18.38",
        "Armbian-Kernel-Version": release.split("-", 1)[0],
        "Armbian-Kernel-Version-Family": release,
    }.items():
        require(image_identity[key] == expected, f"linux-image dpkg identity changed: {key}")
    for key, expected in {"Package": expected_dtb_name, "Version": armbian["package_revision"], "Architecture": expected_architecture}.items():
        require(dtb_identity[key] == expected, f"linux-dtb dpkg identity changed: {key}")

    image_root = work / "package-image"
    dtb_root = work / "package-dtb"
    extract_package(image_package, image_root)
    extract_package(dtb_package, dtb_root)
    config = image_root / f"boot/config-{release}"
    require(config.is_file() and not config.is_symlink(), "exact package kernel config is missing")
    config_hash = sha256_file(config)
    require(config_hash == evidence["final_config_sha256"], "package kernel config hash does not match evidence")
    require(evidence["packaged_config_expected_sha256"] == armbian["packaged_config_sha256"], "packaged config expectation changed")
    required_dtb = armbian["required_dtb"]
    image_dtb = image_root / f"usr/lib/linux-image-{release}/allwinner/{required_dtb}"
    dtb_payload = dtb_root / f"boot/dtb-{release}/allwinner/{required_dtb}"
    require(image_dtb.is_file() and dtb_payload.is_file(), "exact package Zero 2W DTB is missing")
    require(image_dtb.read_bytes() == dtb_payload.read_bytes(), "linux-image and linux-dtb DTB bytes differ")
    require(image_dtb.read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "exact package Zero 2W DTB has invalid magic")
    require(file_hash_from_bytes(image_dtb.read_bytes()) == evidence["image_dtb_sha256"], "image package DTB hash does not match evidence")
    require(file_hash_from_bytes(dtb_payload.read_bytes()) == evidence["dtb_package_dtb_sha256"], "DTB package hash does not match evidence")

    kernel_candidates = sorted(
        path
        for path in (
            image_root / "boot/Image",
            image_root / f"boot/vmlinuz-{release}",
            image_root / f"usr/lib/linux-image-{release}/Image",
            image_root / f"usr/lib/linux-image-{release}/boot/Image",
            image_root / f"usr/lib/linux-image-{release}/vmlinuz",
        )
        if path.is_file() and not path.is_symlink()
    )
    require(len(kernel_candidates) == 1, "exact package must contain one canonical boot kernel")
    module_root = image_root / f"lib/modules/{release}"
    modules = sorted(path for path in module_root.rglob("usb_f_midi.ko*") if path.is_file())
    require(len(modules) == 1, "exact package must contain one usb_f_midi module")
    facts = module_facts(modules[0], release)
    require(facts["compressed_sha256"] == evidence["module_compressed_sha256"], "package usb_f_midi compressed hash does not match evidence")
    require(facts["decompressed_sha256"] == evidence["module_decompressed_sha256"], "package usb_f_midi decompressed hash does not match evidence")
    require(facts["vermagic"] == evidence["module_vermagic"], "package usb_f_midi vermagic does not match evidence")
    require(facts["interface_string"] == evidence["module_interface_string_marker"], "package usb_f_midi interface marker does not match evidence")
    require(facts["interface_options"] == evidence["module_interface_options_marker"], "package usb_f_midi options marker does not match evidence")
    require(facts["interface_runtime"] == evidence["module_interface_runtime_marker"], "package usb_f_midi runtime marker does not match evidence")
    require(str(modules[0].relative_to(image_root)) == evidence["module_relative_path"], "package usb_f_midi path does not match evidence")
    return {
        "release": release,
        "config_hash": config_hash,
        "kernel": kernel_candidates[0].read_bytes(),
        "dtb": dtb_payload.read_bytes(),
        "module": facts,
        "module_relative_path": evidence["module_relative_path"],
        "image_identity": image_identity,
        "dtb_identity": dtb_identity,
    }


def verify_symlink(path: Path, root: Path, release: str, label: str) -> Path:
    require(path.is_symlink(), f"selected {label} must be a symlink")
    target = safe_resolve(root, path, label)
    require(release in str(target), f"selected {label} symlink is not ABI-specific: {path}")
    return target


def verify_boot(root: Path, package: dict[str, Any]) -> dict[str, str]:
    release = package["release"]
    selected = parse_boot_selectors(root, release)
    kernel_link = root / "boot/Image"
    initrd_link = root / "boot/uInitrd"
    if kernel_link.exists() or kernel_link.is_symlink():
        verify_symlink(kernel_link, root, release, "kernel")
    if initrd_link.exists() or initrd_link.is_symlink():
        verify_symlink(initrd_link, root, release, "initramfs")
    kernel = selected["linux"]
    initrd = selected["initrd"]
    dtb = selected["fdt"]
    require(kernel.is_file() and kernel.stat().st_size > 0, "selected boot kernel is missing or empty")
    require(kernel.read_bytes() == package["kernel"], "selected boot kernel differs from exact package kernel")
    require(initrd.is_file() and initrd.stat().st_size > 0, "selected boot initramfs is missing or empty")
    require(dtb.is_file() and dtb.read_bytes() == package["dtb"], "selected boot DTB differs from the exact package DTB")
    require(dtb.read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "selected boot DTB has invalid magic")
    config = root / f"boot/config-{release}"
    require(config.is_file() and not config.is_symlink(), "selected boot kernel config is missing")
    require(sha256_file(config) == package["config_hash"], "selected boot kernel config differs from exact package config")
    module_root = root / f"lib/modules/{release}"
    require((module_root / "modules.dep").is_file(), "selected kernel modules.dep is missing")
    for module_name in ("snd-seq.ko", "snd-seq-midi.ko", "snd-rawmidi.ko", "snd-usb-audio.ko"):
        require(
            len([path for path in module_root.rglob(f"{module_name}*") if path.is_file()]) == 1,
            f"selected kernel is missing exactly one {module_name} module",
        )
    module_candidates = sorted(path for path in module_root.rglob("usb_f_midi.ko*") if path.is_file())
    require(len(module_candidates) == 1, "selected kernel must contain one usb_f_midi module")
    facts = module_facts(module_candidates[0], release)
    require(facts == package["module"], "selected usb_f_midi module differs from exact package evidence")
    require(str(module_candidates[0].relative_to(root)) == package["module_relative_path"], "selected usb_f_midi module path differs from exact package")
    raw_initrd = initramfs_content(initrd)
    for marker in (release.encode(), b"usb_f_midi", b"snd_seq", b"snd_rawmidi", b"snd_usb_audio"):
        require(marker in raw_initrd, f"selected initramfs omits ABI/module marker: {marker.decode(errors='replace')}")
    return {"selected_kernel": str(kernel.relative_to(root)), "selected_initramfs": str(initrd.relative_to(root)), "selected_dtb": str(dtb.relative_to(root))}


def verify_dpkg_status(root: Path, package: dict[str, Any]) -> None:
    status_path = root / "var/lib/dpkg/status"
    if not status_path.is_file():
        return
    records = status_path.read_text(encoding="utf-8").split("\n\n")
    fields: dict[str, dict[str, str]] = {}
    for record in records:
        values: dict[str, str] = {}
        for line in record.splitlines():
            key, separator, value = line.partition(": ")
            if separator:
                values[key] = value
        if values.get("Package"):
            fields[values["Package"]] = values
    for identity in (package["image_identity"], package["dtb_identity"]):
        installed = fields.get(identity["Package"])
        if installed is None:
            raise ImageProofError(f"dpkg status omits installed kernel package: {identity['Package']}")
        require(installed.get("Status") == "install ok installed", f"dpkg status does not mark package installed: {identity['Package']}")
        for key in ("Version", "Architecture"):
            require(installed.get(key) == identity[key], f"dpkg status identity changed: {identity['Package']} {key}")


def metadata_values(root: Path) -> dict[str, str]:
    return read_kv(root / "etc/octessera/build-metadata.env")


def verify_runtime(root: Path, mode: str) -> dict[str, str]:
    metadata = metadata_values(root)
    require(metadata.get("OCTESSERA_IMAGE_MODE") == mode, "final image mode does not match the requested proof mode")
    require(metadata.get("OCTESSERA_RUNTIME_ENABLED_DEFAULT") == ("true" if mode == "production" else "false"), "final image runtime mode is not explicit")
    contract_path = root / "etc/octessera/image-contract.json"
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    require(contract == {"schema_version": 1, "image_kind": mode, "runtime_enabled_default": mode == "production"}, "final image contract is not exact")
    if mode == "diagnostic":
        for path in (root / "usr/local/bin/octessera-pi", root / "etc/systemd/system/octessera.service", root / "opt/octessera/current", root / "opt/octessera/releases"):
            require(not path.exists() and not path.is_symlink(), f"diagnostic image contains production runtime path: {path.relative_to(root)}")
        return {"runtime_service_mode": "disabled"}

    version = metadata.get("OCTESSERA_RUNTIME_VERSION", "")
    binary_hash = metadata.get("OCTESSERA_RUNTIME_BINARY_SHA256", "")
    metadata_hash = metadata.get("OCTESSERA_RUNTIME_METADATA_SHA256", "")
    sums_hash = metadata.get("OCTESSERA_RUNTIME_MANIFEST_SHA256", "")
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,63}", version) is not None, "production runtime version is invalid")
    require(all(re.fullmatch(r"[0-9a-f]{64}", value or "") for value in (binary_hash, metadata_hash, sums_hash)), "production runtime hashes are invalid")
    release_root = root / f"opt/octessera/releases/{version}"
    binary = release_root / "octessera-pi"
    runtime_metadata_path = release_root / "octessera-runtime.json"
    sums = release_root / "SHA256SUMS"
    require(release_root.is_dir() and not release_root.is_symlink(), "production runtime release directory is missing")
    require(binary.is_file() and runtime_metadata_path.is_file() and sums.is_file(), "production runtime bundle is incomplete")
    require(sha256_file(binary) == binary_hash, "production runtime binary hash mismatch")
    require(sha256_file(runtime_metadata_path) == metadata_hash, "production runtime metadata hash mismatch")
    require(sha256_file(sums) == sums_hash, "production runtime checksum manifest hash mismatch")
    runtime_metadata = json.loads(runtime_metadata_path.read_text(encoding="utf-8"))
    require(set(runtime_metadata) == {"artifact_kind", "binary_sha256", "name", "profile", "runtime_ready", "version"}, "production runtime metadata keys changed")
    require(runtime_metadata == {"artifact_kind": "production-runtime", "binary_sha256": binary_hash, "name": "octessera-pi", "profile": "orange-pi-zero-2w", "runtime_ready": True, "version": version}, "production runtime metadata is not hash-bound")
    require(sums.read_text(encoding="utf-8") == f"{binary_hash}  octessera-pi\n", "production runtime checksum manifest is not exact")
    require(binary.read_bytes()[:7] == b"\x7fELF\x02\x01\x01" and struct.unpack_from("<H", binary.read_bytes(), 18)[0] == 183, "production runtime is not ELF64 AArch64")
    runtime_uid, runtime_gid = runtime_account(root, require)
    require_owner_mode(binary, 0, 0, 0o555, require)
    require_owner_mode(release_root, 0, 0, 0o555, require)
    require_owner_mode(runtime_metadata_path, 0, 0, 0o444, require)
    require_owner_mode(sums, 0, 0, 0o444, require)
    require_owner_mode(root / "var/lib/octessera/presets", runtime_uid, runtime_gid, 0o755, require)
    require_owner_mode(root / "var/lib/octessera/samples", runtime_uid, runtime_gid, 0o755, require)
    require_runtime_udev_rule(root, require)
    require((root / "opt/octessera/current").is_symlink() and (root / "opt/octessera/current").readlink().as_posix() == f"/opt/octessera/releases/{version}", "production current runtime symlink is wrong")
    require((root / "usr/local/bin/octessera-pi").is_symlink() and (root / "usr/local/bin/octessera-pi").readlink().as_posix() == "/opt/octessera/current/octessera-pi", "production executable symlink is wrong")
    require_runtime_service(root, require)
    reject_unsupported_updater(root, require)
    return {"runtime_binary_sha256": binary_hash, "runtime_metadata_sha256": metadata_hash, "runtime_service_mode": "enabled"}


def expected_artifact(image_name: str, image_hash: str, package: dict[str, Any], boot: dict[str, str], runtime: dict[str, str], evidence: dict[str, str], provenance: dict[str, str], image_compression: str) -> dict[str, str]:
    values = {
        "schema": "1",
        "image_name": image_name,
        "image_sha256": image_hash,
        "image_compression": image_compression,
        "linux_image_package": provenance["image_package"],
        "linux_image_package_sha256": evidence["image_package_sha256"],
        "linux_dtb_package": provenance["dtb_package"],
        "linux_dtb_package_sha256": evidence["dtb_package_sha256"],
        "kernel_evidence_sha256": evidence["_sha256"],
        "kernel_provenance_sha256": provenance["_sha256"],
        "kernel_release": package["release"],
        "selected_kernel": boot["selected_kernel"],
        "selected_initramfs": boot["selected_initramfs"],
        "selected_dtb": boot["selected_dtb"],
        "runtime_service_mode": runtime["runtime_service_mode"],
    }
    values.update({key: value for key, value in runtime.items() if key != "runtime_service_mode"})
    return values


def verify_artifact(path: Path, expected: dict[str, str]) -> None:
    actual = read_kv(path)
    require(set(actual) == set(expected), "Orange image provenance fields changed")
    for key, value in expected.items():
        if key == "image_name":
            continue
        require(actual[key] == value, f"Orange image provenance is not bound: {key}")


def prove_root(root: Path, args: argparse.Namespace, image_hash: str, image_name: str, compression: str) -> dict[str, str]:
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    evidence = read_kv(args.evidence)
    provenance = read_kv(args.provenance)
    evidence["_sha256"] = sha256_file(args.evidence)
    provenance["_sha256"] = sha256_file(args.provenance)
    required_evidence = {"image_package_native_basename", "dtb_package_native_basename", "artifact_suffix", "image_package_sha256", "dtb_package_sha256", "image_dtb_sha256", "dtb_package_dtb_sha256", "dtb_byte_equal", "packaged_config_expected_sha256", "final_config_sha256", "module_relative_path", "module_compressed_sha256", "module_decompressed_sha256", "module_vermagic", "module_interface_string_marker", "module_interface_options_marker", "module_interface_runtime_marker"}
    require(set(evidence) - {"_sha256"} == required_evidence, "Orange kernel evidence fields changed")
    for key in ("image_package", "dtb_package", "image_package_native", "dtb_package_native", "image_package_sha256", "dtb_package_sha256", "evidence_sha256", "kernel_source_repository", "kernel_source_commit", "kernel_release"):
        require(key in provenance, f"Orange kernel provenance omits required field: {key}")
    require(evidence.get("dtb_byte_equal") == "true", "kernel evidence does not prove equal package DTB bytes")
    with tempfile.TemporaryDirectory(prefix="octessera-orange-package-proof-") as temporary:
        package = verify_package_chain(args.linux_image, args.linux_dtb, evidence, provenance, manifest, Path(temporary))
    boot = verify_boot(root, package)
    verify_dpkg_status(root, package)
    runtime = verify_runtime(root, args.mode)
    result = expected_artifact(image_name, image_hash, package, boot, runtime, evidence, provenance, compression)
    if args.image_provenance:
        verify_artifact(args.image_provenance, result)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text("\n".join(f"{key}={value}" for key, value in result.items()) + "\n", encoding="utf-8")
    return result


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prove the exact Orange kernel packages in a final Armbian image.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--image", type=Path)
    source.add_argument("--root", type=Path)
    parser.add_argument("--image-sha256", help="Synthetic root proof image SHA-256; only valid with --root")
    parser.add_argument("--linux-image", type=Path, required=True)
    parser.add_argument("--linux-dtb", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path(__file__).resolve().parents[1] / "kernel-patches/orange-midi-interface-manifest.json")
    parser.add_argument("--image-provenance", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mode", choices=("diagnostic", "production"), default="production")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    try:
        require(args.manifest.is_file(), f"missing Orange kernel manifest: {args.manifest}")
        if args.root:
            root_source = cast(Path, args.root)
            image_sha256 = cast(str, args.image_sha256)
            require(image_sha256 is not None and re.fullmatch(r"[0-9a-fA-F]{64}", image_sha256) is not None, "--root requires a 64-character --image-sha256")
            image_hash = image_sha256.lower()
            image_name = root_source.name
            compression = "root-fixture"
            with mounted_image(root_source) as root:
                result = prove_root(root, args, image_hash, image_name, compression)
        else:
            require(args.image_sha256 is None, "--image-sha256 is only valid with --root")
            image_source = cast(Path, args.image)
            require(image_source is not None and image_source.is_file(), "final Orange image is missing")
            require(image_source.suffix == ".img" or image_source.suffixes[-2:] == [".img", ".xz"], "Orange image proof accepts only .img or .img.xz")
            image_hash = sha256_file(image_source)
            compression = "xz" if image_source.suffix == ".xz" else "none"
            with mounted_image(image_source) as root:
                result = prove_root(root, args, image_hash, image_source.name, compression)
        print(json.dumps(result, indent=2, sort_keys=True))
        print("Orange final image proof passed")
        return 0
    except (BootSelectionError, ImageMountError, ImageProofError, OSError, json.JSONDecodeError, struct.error) as error:
        print(f"Orange final image proof failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
