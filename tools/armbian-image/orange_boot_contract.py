from __future__ import annotations

import bz2
import fnmatch
import gzip
import hashlib
import json
import lzma
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from orange_boot_selection import parse_boot_selectors, safe_resolve
from orange_phase5_proof import verify_selected_initramfs
from verify_runtime_account import (
    require_orange_boot_service,
    require_orange_shutdown_service,
    require_orange_suspend_service,
    require_owner_mode,
    require_production_updater,
    require_runtime_service,
    require_runtime_udev_rule,
    runtime_account,
)

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "legal"))
from stage_notices import NoticeStageError, stage_notices  # type: ignore[import-not-found]


class BootContractError(ValueError):
    pass


SOURCE_BOUND_PROOF_SOURCES = {
    "tools/armbian-image/verify-orange-image.py",
    "tools/armbian-image/orange_boot_contract.py",
    "tools/armbian-image/orange_boot_inventory.py",
    "tools/armbian-image/orange_boot_selection.py",
    "tools/armbian-image/orange_image_mount.py",
    "tools/armbian-image/orange_initramfs.py",
    "tools/armbian-image/orange_phase5_proof.py",
    "tools/armbian-image/orange_trusted_parent_proof.py",
    "tools/armbian-image/verify_runtime_account.py",
    "tools/kernel-patches/orange-midi-interface-manifest.json",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise BootContractError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_kv(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        require(bool(separator and key and key not in values), f"malformed or duplicate provenance field: {line}")
        values[key] = value
    return values


def _file_hash(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _dpkg_fields(path: Path, *fields: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for field in fields:
        try:
            result = subprocess.run(["dpkg-deb", "-f", str(path), field], check=True, capture_output=True, text=True)
        except (FileNotFoundError, subprocess.CalledProcessError) as error:
            raise BootContractError(f"cannot read Debian package identity: {path}") from error
        values[field] = result.stdout.rstrip("\n")
    return values


def _extract_package(path: Path, destination: Path) -> None:
    try:
        subprocess.run(["dpkg-deb", "-x", str(path), str(destination)], check=True, capture_output=True, text=True)
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise BootContractError(f"cannot extract exact Debian package: {path}") from error


def _decompress_module(path: Path) -> bytes:
    try:
        if path.name.endswith(".ko"):
            return path.read_bytes()
        if path.name.endswith(".ko.gz"):
            return gzip.decompress(path.read_bytes())
        if path.name.endswith(".ko.xz"):
            return lzma.decompress(path.read_bytes())
        if path.name.endswith(".ko.bz2"):
            return bz2.decompress(path.read_bytes())
        if path.name.endswith(".ko.zst"):
            command = ["zstd", "-q", "-dc", str(path)]
        elif path.name.endswith(".ko.lz4"):
            command = ["lz4", "-q", "-dc", str(path)]
        else:
            raise BootContractError(f"unsupported usb_f_midi compression: {path.name}")
        return subprocess.run(command, check=True, capture_output=True).stdout
    except (FileNotFoundError, OSError, subprocess.CalledProcessError) as error:
        raise BootContractError(f"cannot decompress usb_f_midi module: {path}") from error


def _module_facts(path: Path, release: str) -> dict[str, str]:
    compressed = path.read_bytes()
    decompressed = _decompress_module(path)
    require(bool(decompressed), f"usb_f_midi module is empty: {path}")
    require(decompressed[:4] == b"\x7fELF" and decompressed[4:5] == b"\x02", f"usb_f_midi is not ELF64: {path}")
    require(struct.unpack_from("<H", decompressed, 18)[0] == 183, f"usb_f_midi is not AArch64: {path}")
    strings = [match.decode("ascii") for match in re.findall(rb"[ -~]{4,}", decompressed)]
    vermagic = [value.removeprefix("vermagic=") for value in strings if value.startswith("vermagic=")]
    require(len(vermagic) == 1 and (vermagic[0] == release or vermagic[0].startswith(f"{release} ")), "usb_f_midi vermagic does not match ABI")
    markers = ("interface_string", "f_midi_opts_attr_interface_string", "midi_interface_string")
    for marker in markers:
        require(marker in strings, f"usb_f_midi marker is missing: {marker}")
    return {"compressed_sha256": _file_hash(compressed), "decompressed_sha256": _file_hash(decompressed), "vermagic": vermagic[0], "interface_string": markers[0], "interface_options": markers[1], "interface_runtime": markers[2]}


def _package_suffix(path: Path, canonical: str, label: str) -> str:
    if path.name == canonical:
        return "canonical"
    prefix = canonical.removesuffix(".deb") + "__"
    require(path.name.startswith(prefix) and path.name.endswith(".deb"), f"{label} is not a native package handoff: {path.name}")
    suffix = path.name[len(prefix) : -4]
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9+._-]*", suffix) is not None, f"invalid native package suffix: {suffix}")
    return suffix


def verify_package_chain(image_package: Path, dtb_package: Path, evidence: dict[str, str], provenance: dict[str, str], manifest: dict[str, Any], work: Path) -> dict[str, Any]:
    armbian = manifest["build_frameworks"]["armbian"]
    orange = manifest["kernels"]["orange"]
    canonical_image, canonical_dtb = armbian["packages"]
    image_suffix = _package_suffix(image_package, canonical_image, "linux-image package")
    require(_package_suffix(dtb_package, canonical_dtb, "linux-dtb package") == image_suffix, "native package suffixes differ")
    require(image_package.name == canonical_image or fnmatch.fnmatchcase(image_package.name, armbian["native_package_patterns"][0]), "linux-image package name is not manifest-approved")
    require(dtb_package.name == canonical_dtb or fnmatch.fnmatchcase(dtb_package.name, armbian["native_package_patterns"][1]), "linux-dtb package name is not manifest-approved")
    require(sha256_file(image_package) == evidence["image_package_sha256"], "linux-image package hash does not match evidence")
    require(sha256_file(dtb_package) == evidence["dtb_package_sha256"], "linux-dtb package hash does not match evidence")
    require(evidence["image_package_native_basename"] == provenance.get("image_package_native") and evidence["dtb_package_native_basename"] == provenance.get("dtb_package_native"), "native package evidence is not bound")
    require(evidence["artifact_suffix"] == image_suffix or image_suffix == "canonical", "native package suffix evidence is not bound")
    require(provenance.get("image_package") == canonical_image and provenance.get("dtb_package") == canonical_dtb, "kernel provenance canonical package identity changed")
    for package, canonical, pattern, key in ((image_package, canonical_image, armbian["native_package_patterns"][0], "image_package_native"), (dtb_package, canonical_dtb, armbian["native_package_patterns"][1], "dtb_package_native")):
        if package.name.startswith(canonical.removesuffix(".deb") + "__"):
            require(provenance.get(key) == package.name, "kernel provenance native package identity changed")
        else:
            require(fnmatch.fnmatchcase(provenance.get(key, ""), pattern), "kernel provenance native package identity is missing")
    require(provenance.get("image_package_sha256") == evidence["image_package_sha256"] and provenance.get("dtb_package_sha256") == evidence["dtb_package_sha256"], "kernel provenance package hash is not bound")
    require(provenance.get("evidence_sha256") == evidence.get("_sha256"), "kernel provenance evidence hash is not bound")
    require(provenance.get("kernel_source_repository") == orange["repository"] and provenance.get("kernel_source_commit") == orange["commit"], "kernel provenance source changed")
    release = armbian["kernel_release"]
    require(provenance.get("kernel_release") == release, "kernel provenance ABI changed")
    image_identity = _dpkg_fields(image_package, "Package", "Version", "Architecture", "Source", "Armbian-Kernel-Version", "Armbian-Kernel-Version-Family")
    dtb_identity = _dpkg_fields(dtb_package, "Package", "Version", "Architecture")
    expected_architecture = canonical_image.rsplit("_", 1)[1].removesuffix(".deb")
    for key, expected in {"Package": canonical_image.split("_", 1)[0], "Version": armbian["package_revision"], "Architecture": expected_architecture, "Source": "linux-6.18.38", "Armbian-Kernel-Version": release.split("-", 1)[0], "Armbian-Kernel-Version-Family": release}.items():
        require(image_identity[key] == expected, f"linux-image dpkg identity changed: {key}")
    for key, expected in {"Package": canonical_dtb.split("_", 1)[0], "Version": armbian["package_revision"], "Architecture": expected_architecture}.items():
        require(dtb_identity[key] == expected, f"linux-dtb dpkg identity changed: {key}")
    image_root, dtb_root = work / "package-image", work / "package-dtb"
    _extract_package(image_package, image_root)
    _extract_package(dtb_package, dtb_root)
    config = image_root / f"boot/config-{release}"
    require(config.is_file() and not config.is_symlink(), "exact package kernel config is missing")
    config_hash = sha256_file(config)
    require(config_hash == evidence["final_config_sha256"] and evidence["packaged_config_expected_sha256"] == armbian["packaged_config_sha256"], "package kernel config evidence changed")
    required_dtb = armbian["required_dtb"]
    image_dtb = image_root / f"usr/lib/linux-image-{release}/allwinner/{required_dtb}"
    dtb_payload = dtb_root / f"boot/dtb-{release}/allwinner/{required_dtb}"
    require(image_dtb.is_file() and dtb_payload.is_file() and image_dtb.read_bytes() == dtb_payload.read_bytes() and image_dtb.read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "exact package Zero 2W DTB is invalid")
    require(_file_hash(image_dtb.read_bytes()) == evidence["image_dtb_sha256"] and _file_hash(dtb_payload.read_bytes()) == evidence["dtb_package_dtb_sha256"], "package DTB hash does not match evidence")
    kernel_candidates = [path for path in (image_root / "boot/Image", image_root / f"boot/vmlinuz-{release}", image_root / f"usr/lib/linux-image-{release}/Image", image_root / f"usr/lib/linux-image-{release}/boot/Image", image_root / f"usr/lib/linux-image-{release}/vmlinuz") if path.is_file() and not path.is_symlink()]
    require(len(kernel_candidates) == 1, "exact package must contain one canonical boot kernel")
    modules = sorted(path for path in (image_root / f"lib/modules/{release}").rglob("usb_f_midi.ko*") if path.is_file())
    require(len(modules) == 1, "exact package must contain one usb_f_midi module")
    facts = _module_facts(modules[0], release)
    for key in ("compressed_sha256", "decompressed_sha256", "vermagic"):
        require(facts[key] == evidence[f"module_{key}"], f"package usb_f_midi {key} does not match evidence")
    require(facts["interface_string"] == evidence["module_interface_string_marker"], "package usb_f_midi interface marker does not match evidence")
    require(facts["interface_options"] == evidence["module_interface_options_marker"], "package usb_f_midi options marker does not match evidence")
    require(facts["interface_runtime"] == evidence["module_interface_runtime_marker"], "package usb_f_midi runtime marker does not match evidence")
    require(str(modules[0].relative_to(image_root)) == evidence["module_relative_path"], "package usb_f_midi path does not match evidence")
    return {"release": release, "config_hash": config_hash, "kernel": kernel_candidates[0].read_bytes(), "dtb": dtb_payload.read_bytes(), "module": facts, "module_relative_path": evidence["module_relative_path"], "image_identity": image_identity, "dtb_identity": dtb_identity}


def _verify_symlink(path: Path, root: Path, release: str, label: str) -> None:
    require(path.is_symlink(), f"selected {label} must be a symlink")
    target = safe_resolve(root, path, label)
    require(release in str(target), f"selected {label} symlink is not ABI-specific: {path}")


def _verify_terminal_identity(root: Path, construction: dict[str, Any]) -> None:
    records = [line.split(":") for line in (root / "etc/passwd").read_text(encoding="utf-8").splitlines() if line]
    interactive = [record for record in records if record and record[0] == "octessera"]
    require(len(interactive) == 1 and len(interactive[0]) == 7, "Orange interactive account is missing, duplicated, or malformed")
    account = interactive[0]
    require(account[2].isdigit() and account[3].isdigit() and account[5] == "/home/octessera" and account[6] == "/bin/bash", "Orange interactive account home or shell is not exact")
    groups = [line.split(":") for line in (root / "etc/group").read_text(encoding="utf-8").splitlines() if line]
    primary_groups = [group for group in groups if group and group[0] == "octessera"]
    require(len(primary_groups) == 1 and len(primary_groups[0]) == 4 and primary_groups[0][2].isdigit() and int(primary_groups[0][2]) == int(account[3]), "Orange interactive group is missing, duplicated, or has the wrong GID")
    home = root / account[5].lstrip("/")
    require(home.is_dir() and not home.is_symlink(), "Orange interactive account home is missing or symlinked")
    hush = root / construction["terminal_invariants"]["hushlogin_path"]
    require(hush.is_file() and not hush.is_symlink(), "Orange hushlogin is missing or symlinked")
    metadata = hush.lstat()
    require(metadata.st_uid == int(account[2]) and metadata.st_gid == int(primary_groups[0][2]) and metadata.st_mode & 0o777 == construction["terminal_invariants"]["hushlogin_mode"] and metadata.st_size == 0, "Orange hushlogin ownership, mode, or content is not exact")
    for directory in (root / "etc/pam.d", root / "etc/update-motd.d"):
        if directory.is_dir() and not directory.is_symlink():
            for path in directory.rglob("*"):
                if any("octessera" in part.lower() for part in path.relative_to(directory).parts):
                    require(False, f"Orange repository PAM or update-motd override remains: {path}")


def _verify_notice_bundle(root: Path, repository_root: Path, construction: dict[str, Any]) -> None:
    try:
        stage_notices(repository_root, root, check=True)
    except (OSError, NoticeStageError) as error:
        raise BootContractError(f"Orange installed legal notice bundle is not exact: {error}") from error
    for relative in construction["notice_bundle"]["parent_sentinels"]:
        path = root / relative
        require(path.is_file() and not path.is_symlink() and path.stat().st_size > 0, f"Orange parent legal sentinel is missing or empty: {relative}")


def _verify_device_apply_lane(root: Path, repository_root: Path, construction: dict[str, Any]) -> None:
    assets = (
        ("userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket", "etc/systemd/system/octessera-device-apply-reboot.socket", 0o644),
        ("userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service", "etc/systemd/system/octessera-device-apply-reboot@.service", 0o644),
        ("userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot", "usr/local/sbin/octessera-device-apply-reboot", 0o755),
    )
    exact_inputs = {item["path"]: item for item in construction["exact_inputs"]}
    for source_relative, installed_relative, mode in assets:
        source = repository_root / source_relative
        installed = root / installed_relative
        expected = exact_inputs.get(source_relative)
        if expected is None:
            raise BootContractError(f"Orange device apply source identity is missing: {source_relative}")
        require(source.is_file() and not source.is_symlink(), f"Orange device apply source is missing or symlinked: {source_relative}")
        require(installed.is_file() and not installed.is_symlink(), f"Orange installed device apply asset is missing or symlinked: {installed_relative}")
        require(sha256_file(source) == expected["sha256"] and source.stat().st_size == expected["size"], f"Orange device apply source identity changed: {source_relative}")
        require(installed.read_bytes() == source.read_bytes(), f"Orange installed device apply asset differs from its canonical source: {installed_relative}")
        require_owner_mode(installed, 0, 0, mode, require)
    device_apply_script = (repository_root / assets[2][0]).read_text(encoding="utf-8")
    for line in (
        'SYSTEMCTL_PATH = "/usr/bin/systemctl"',
        'REBOOT_REQUEST = b"reboot\\n"',
        'ACCEPTED = b"accepted\\n"',
        'REJECTED = b"rejected\\n"',
        'subprocess.run([SYSTEMCTL_PATH, command], check=True)',
        'output_stream.write(ACCEPTED)',
        'output_stream.write(REJECTED)',
    ):
        require(line in device_apply_script, f"Orange device apply script contract is missing: {line}")
    socket_link = root / "etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket"
    require(socket_link.is_symlink() and socket_link.readlink().as_posix() in {"../octessera-device-apply-reboot.socket", "/etc/systemd/system/octessera-device-apply-reboot.socket"}, "Orange device apply socket is not enabled by the exact symlink")


def _verify_oled_assets(root: Path, repository_root: Path, construction: dict[str, Any]) -> None:
    exact_inputs = {item["path"]: item for item in construction["exact_inputs"]}
    for source_relative, installed_relative in (
        ("userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565", "usr/share/octessera/oled/octessera-pi-booting.rgb565"),
        ("userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565", "usr/share/octessera/oled/octessera-pi-shutdown.rgb565"),
    ):
        source = repository_root / source_relative
        installed = root / installed_relative
        expected = exact_inputs.get(source_relative)
        require(expected is not None, f"Orange OLED asset source identity is missing: {source_relative}")
        if expected is None:
            raise BootContractError(f"Orange OLED asset source identity is missing: {source_relative}")
        require(source.is_file() and not source.is_symlink(), f"Orange OLED asset source is missing or symlinked: {source_relative}")
        require(installed.is_file() and not installed.is_symlink(), f"Orange installed OLED asset is missing or symlinked: {installed_relative}")
        require(sha256_file(source) == expected["sha256"] and source.stat().st_size == expected["size"], f"Orange OLED asset source identity changed: {source_relative}")
        require(installed.read_bytes() == source.read_bytes(), f"Orange installed OLED asset differs from its canonical source: {installed_relative}")
        require_owner_mode(installed, 0, 0, 0o644, require)


def verify_boot(root: Path, package: dict[str, Any], construction: dict[str, Any], repository_root: Path) -> dict[str, Any]:
    _verify_notice_bundle(root, repository_root, construction)
    release = package["release"]
    selected = parse_boot_selectors(root, release)
    for path, label in ((root / "boot/Image", "kernel"), (root / "boot/uInitrd", "initramfs")):
        if path.exists() or path.is_symlink():
            _verify_symlink(path, root, release, label)
    for name, expected in (("linux", package["kernel"]), ("fdt", package["dtb"])):
        path = selected[name]
        require(path.is_file() and path.stat().st_size > 0 and path.read_bytes() == expected, f"selected boot {name} differs from exact package")
    require(selected["fdt"].read_bytes()[:4] == b"\xd0\x0d\xfe\xed", "selected boot DTB has invalid magic")
    initrd = selected["initrd"]
    require(initrd.is_file() and initrd.stat().st_size > 0, "selected boot initramfs is missing or empty")
    config = root / f"boot/config-{release}"
    require(config.is_file() and sha256_file(config) == package["config_hash"], "selected boot kernel config differs from exact package")
    config_lines = config.read_text(encoding="utf-8").splitlines()
    for line in construction["required_builtin_kernel_config_lines"]:
        require(config_lines.count(line) == 1, f"selected boot kernel config must contain exactly one: {line}")
    module_root = root / f"lib/modules/{release}"
    require((module_root / "modules.dep").is_file(), "selected kernel modules.dep is missing")
    for module_name in ("snd-seq.ko", "snd-seq-midi.ko", "snd-rawmidi.ko", "snd-usb-audio.ko"):
        require(len([path for path in module_root.rglob(f"{module_name}*") if path.is_file()]) == 1, f"selected kernel is missing exactly one {module_name} module")
    module_candidates = sorted(path for path in module_root.rglob("usb_f_midi.ko*") if path.is_file())
    require(len(module_candidates) == 1 and _module_facts(module_candidates[0], release) == package["module"], "selected usb_f_midi module differs from exact package evidence")
    require_orange_boot_service(root, require)
    require_orange_shutdown_service(root, require)
    require_orange_suspend_service(root, require)
    _verify_device_apply_lane(root, repository_root, construction)
    _verify_oled_assets(root, repository_root, construction)
    welcome = root / construction["terminal_invariants"]["welcome_path"]
    require(welcome.is_file() and not welcome.is_symlink(), "canonical Orange welcome file is missing or symlinked")
    require_owner_mode(welcome, 0, 0, 0o644, require)
    welcome_inputs = [item for item in construction["exact_inputs"] if item["path"] == "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh"]
    require(len(welcome_inputs) == 1, "canonical Orange welcome input is not unique")
    welcome_source = repository_root / welcome_inputs[0]["path"]
    require(welcome_source.is_file() and not welcome_source.is_symlink() and sha256_file(welcome_source) == welcome_inputs[0]["sha256"] and welcome_source.stat().st_size == welcome_inputs[0]["size"], "canonical Orange welcome input changed")
    require(welcome.read_bytes() == welcome_source.read_bytes() and sha256_file(welcome) == welcome_inputs[0]["sha256"], "installed Orange welcome differs from canonical input")
    require(next((item for item in construction["managed_outputs"] if item["path"] == construction["terminal_invariants"]["welcome_path"]), None) == {"path": "etc/profile.d/octessera-welcome.sh", "mode": 420, "uid": 0, "gid": 0}, "Orange welcome managed output changed")
    default_source = repository_root / "config/generated/pi/default.json"
    default = root / "usr/share/octessera/defaults/pi-default.json"
    default_input = next((item for item in construction["exact_inputs"] if item["path"] == "config/generated/pi/default.json"), None)
    require(default_input == {"path": "config/generated/pi/default.json", "sha256": sha256_file(default_source), "size": default_source.stat().st_size, "mode": 420}, "Orange default input identity changed")
    require(default.is_file() and not default.is_symlink(), "Orange default config is missing or symlinked")
    require_owner_mode(default, 0, 0, 0o644, require)
    require(default.read_bytes() == default_source.read_bytes(), "Orange default config differs from canonical input")
    require(next((item for item in construction["managed_outputs"] if item["path"] == "usr/share/octessera/defaults/pi-default.json"), None) == {"path": "usr/share/octessera/defaults/pi-default.json", "mode": 420, "uid": 0, "gid": 0}, "Orange default managed output changed")
    validator_input = [item for item in construction["exact_inputs"] if item["path"] == "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"]
    require(len(validator_input) == 1, "canonical Orange device config validator input is not unique")
    validator_source = repository_root / validator_input[0]["path"]
    validator = root / "usr/local/lib/octessera/device_config.py"
    require(validator_source.is_file() and not validator_source.is_symlink(), "canonical Orange device config validator source is missing or symlinked")
    validator_source_hash = sha256_file(validator_source)
    validator_source_size = validator_source.stat().st_size
    require(validator_input[0]["sha256"] == validator_source_hash and validator_input[0]["size"] == validator_source_size, "canonical Orange device config validator input changed")
    require(validator.is_file() and not validator.is_symlink(), "Orange device config validator is missing or symlinked")
    require_owner_mode(validator, 0, 0, 0o644, require)
    require(sha256_file(validator) == validator_source_hash and validator.stat().st_size == validator_source_size, "installed Orange device config validator is not byte-identical to the canonical source")
    require(next((item for item in construction["managed_outputs"] if item["path"] == "usr/local/lib/octessera/device_config.py"), None) == {"path": "usr/local/lib/octessera/device_config.py", "mode": 420, "uid": 0, "gid": 0}, "Orange validator managed output changed")
    require(next((item for item in construction["managed_outputs"] if item["path"] == construction["terminal_invariants"]["hushlogin_path"]), None) == {"path": "home/octessera/.hushlogin", "mode": 420, "owner": "octessera", "group": "octessera", "content": "empty"}, "Orange hushlogin managed output changed")
    _verify_terminal_identity(root, construction)
    uart = construction["uart_invariants"]
    for boot_config in (root / "boot/armbianEnv.txt", root / "boot/extlinux/extlinux.conf"):
        if boot_config.is_file() and re.search(r"(^|[ \t])console=ttyS0(?:,|[ \t]|$)", boot_config.read_text(encoding="utf-8"), re.MULTILINE):
            require(False, f"Orange boot configuration still selects {uart['forbidden_console_token']}")
    mask = root / uart["serial_getty_mask"]
    require(mask.is_symlink() and mask.readlink().as_posix() == "/dev/null", "Orange UART serial-getty mask is not /dev/null")
    for output in (
        root / "usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts",
        root / "boot/overlay-user/octessera-h618-input-routing.dtbo",
    ):
        require(output.is_file() and not output.is_symlink(), f"Orange UART overlay output is missing or symlinked: {output}")
        require_owner_mode(output, 0, 0, 0o644, require)
    verify_selected_initramfs(root, initrd, construction, require)
    return {"selected_kernel": str(selected["linux"].relative_to(root)), "selected_initramfs": str(initrd.relative_to(root)), "selected_dtb": str(selected["fdt"].relative_to(root)), "device_config_validator": {"path": validator_input[0]["path"], "sha256": validator_source_hash, "size": validator_source_size}}


def verify_dpkg_status(root: Path, package: dict[str, Any]) -> None:
    status_path = root / "var/lib/dpkg/status"
    if not status_path.is_file():
        return
    records = status_path.read_text(encoding="utf-8").split("\n\n")
    fields = {values["Package"]: values for values in ({key: value for key, _, value in (line.partition(": ") for line in record.splitlines()) if _} for record in records) if values.get("Package")}
    for identity in (package["image_identity"], package["dtb_identity"]):
        installed = fields.get(identity["Package"])
        require(installed is not None and installed.get("Status") == "install ok installed", f"dpkg status omits installed kernel package: {identity['Package']}")
        assert installed is not None
        for key in ("Version", "Architecture"):
            require(installed[key] == identity[key], f"dpkg status identity changed: {identity['Package']} {key}")


def verify_runtime(root: Path, mode: str, construction: dict[str, Any], repository_root: Path) -> dict[str, str]:
    metadata_path = root / "etc/octessera/build-metadata.env"
    require_owner_mode(metadata_path, 0, 0, 0o644, require)
    metadata = read_kv(metadata_path)
    require(metadata.get("OCTESSERA_IMAGE_MODE") == mode and metadata.get("OCTESSERA_RUNTIME_ENABLED_DEFAULT") == ("true" if mode == "production" else "false"), "final image runtime mode is not explicit")
    contract = json.loads((root / "etc/octessera/image-contract.json").read_text(encoding="utf-8"))
    require(contract == {"schema_version": 1, "image_kind": mode, "runtime_enabled_default": mode == "production"}, "final image contract is not exact")
    if mode == "diagnostic":
        runtime_uid, runtime_gid = runtime_account(root, require)
        require_owner_mode(root / "var/lib/octessera/samples", runtime_uid, runtime_gid, 0o755, require)
        for path in (root / "usr/local/bin/octessera-pi", root / "etc/systemd/system/octessera.service", root / "opt/octessera/current", root / "opt/octessera/releases"):
            require(not path.exists() and not path.is_symlink(), f"diagnostic image contains production runtime path: {path.relative_to(root)}")
        return {"runtime_service_mode": "disabled"}
    version, binary_hash, metadata_hash, sums_hash = (metadata.get(key, "") for key in ("OCTESSERA_RUNTIME_VERSION", "OCTESSERA_RUNTIME_BINARY_SHA256", "OCTESSERA_RUNTIME_METADATA_SHA256", "OCTESSERA_RUNTIME_MANIFEST_SHA256"))
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,63}", version) is not None, "production runtime version is invalid")
    require(all(re.fullmatch(r"[0-9a-f]{64}", value or "") for value in (binary_hash, metadata_hash, sums_hash)), "production runtime hashes are invalid")
    release_root = root / f"opt/octessera/releases/{version}"
    binary, runtime_metadata_path, sums = release_root / "octessera-pi", release_root / "octessera-runtime.json", release_root / "SHA256SUMS"
    require(release_root.is_dir() and binary.is_file() and runtime_metadata_path.is_file() and sums.is_file() and (release_root / "update-manifest.json").is_file(), "production runtime bundle is incomplete")
    require(sha256_file(binary) == binary_hash and sha256_file(runtime_metadata_path) == metadata_hash and sha256_file(sums) == sums_hash, "production runtime hash mismatch")
    require(sums.read_text(encoding="utf-8") == f"{binary_hash}  octessera-pi\n", "production runtime checksum manifest is not exact")
    binary_bytes = binary.read_bytes()
    require(binary_bytes[:7] == b"\x7fELF\x02\x01\x01" and struct.unpack_from("<H", binary_bytes, 18)[0] == 183, "production runtime is not ELF64 AArch64")
    runtime_metadata = json.loads(runtime_metadata_path.read_text(encoding="utf-8"))
    require(runtime_metadata == {"artifact_kind": "production-runtime", "binary_sha256": binary_hash, "name": "octessera-pi", "profile": "orange-pi-zero-2w", "runtime_ready": True, "version": version}, "production runtime metadata is not hash-bound")
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
    require_production_updater(root, construction, repository_root, version, require)
    return {"runtime_binary_sha256": binary_hash, "runtime_metadata_sha256": metadata_hash, "runtime_service_mode": "enabled"}


def validate_construction_contract(root: Path, contract: dict[str, Any]) -> str:
    require(contract.get("proof_mode") == "phase5-constructor", "Orange construction proof mode is not phase5-constructor")
    require(contract.get("constructor_required") is True and contract.get("trusted_parent_finalization") == "forbidden" and contract.get("mutation_authority") == "none", "Orange construction authority is invalid")
    require(contract.get("board_profile") == "orange-pi-zero-2w", "Orange construction board is invalid")
    require(contract.get("required_builtin_kernel_config_lines") == ["CONFIG_SPI_SUN6I=y", "CONFIG_SPI_SPIDEV=y", "CONFIG_PINCTRL_SUNXI=y"], "Orange built-in kernel config contract changed")
    selected_initramfs = contract.get("selected_initramfs")
    require(isinstance(selected_initramfs, dict) and selected_initramfs.get("required_python_modules") == ["fcntl", "math", "_json", "_posixsubprocess", "select", "_struct", "zlib"], "Orange Python module contract changed")
    require(contract.get("notice_bundle") == {"manifest": "resources/legal/notice-bundle.json", "stager": "tools/legal/stage_notices.py", "installed_root": "usr/share/doc/octessera", "installed_outputs": "manifest-files", "proof": "tools/armbian-image/orange_boot_contract.py", "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"]}, "Orange legal notice contract is not exact")
    require(contract.get("terminal_invariants") == {"welcome_path": "etc/profile.d/octessera-welcome.sh", "hushlogin_path": "home/octessera/.hushlogin", "hushlogin_mode": 420, "hushlogin_empty": True, "forbidden_pam_update_motd_overrides": True}, "Orange terminal invariants changed")
    require(contract.get("uart_invariants") == {"overlay_name": "octessera-h618-input-routing", "forbidden_console_token": "console=ttyS0", "serial_getty_mask": "etc/systemd/system/serial-getty@ttyS0.service", "uart0_status": "disabled", "stdout_path": ""}, "Orange UART invariants changed")
    exact_inputs = contract.get("exact_inputs", [])
    require(isinstance(exact_inputs, list) and bool(exact_inputs), "Orange construction source inputs are empty")
    exact_input_paths: set[str] = set()
    validator_inputs = [item for item in exact_inputs if item.get("path") == "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"]
    require(len(validator_inputs) == 1, "Orange device config validator source identity is not unique")
    for item in exact_inputs:
        require(set(item) == {"path", "sha256", "size", "mode"}, "Orange construction source input changed")
        require(isinstance(item["path"], str) and not Path(item["path"]).is_absolute() and ".." not in Path(item["path"]).parts and item["path"] not in exact_input_paths, "Orange construction source input path is unsafe or duplicated")
        exact_input_paths.add(item["path"])
        source = root / item["path"]
        require(source.is_file() and not source.is_symlink() and sha256_file(source) == item["sha256"] and source.stat().st_size == item["size"], f"Orange construction source input changed: {source}")
    require(SOURCE_BOUND_PROOF_SOURCES <= exact_input_paths, "Orange verifier source identities are incomplete")
    return sha256_file(root / "resources/image-construction/boot-layers/orange-pi-zero-2w.json")


def load_construction_contract(path: Path, repository_root: Path) -> tuple[Path, dict[str, Any], str]:
    expected = repository_root / "resources/image-construction/boot-layers/orange-pi-zero-2w.json"
    require(path.name == expected.name and path.resolve(strict=True) == expected.resolve(strict=True), "Orange construction contract path is not canonical")
    require(path.is_file() and not path.is_symlink(), "Orange construction contract is missing or symlinked")
    try:
        contract = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BootContractError("Orange construction contract is unreadable") from error
    contract_hash = validate_construction_contract(repository_root, contract)
    return expected, contract, contract_hash


def verify_artifact(path: Path, expected: dict[str, Any]) -> None:
    actual = json.loads(path.read_text(encoding="utf-8"))
    require(actual == expected, "Orange structured proof changed")


def constructor_proof(root: Path, args: Any, image_hash: str, image_name: str, compression: str, repository_root: Path) -> dict[str, Any]:
    contract_path, contract, contract_hash = load_construction_contract(args.construction_contract, repository_root)
    require(args.manifest is not None, "--manifest is required for phase5-constructor")
    manifest_path = args.manifest
    if manifest_path is None:
        raise BootContractError("--manifest is required for phase5-constructor")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    evidence = read_kv(args.evidence)
    provenance = read_kv(args.provenance)
    evidence["_sha256"], provenance["_sha256"] = sha256_file(args.evidence), sha256_file(args.provenance)
    required = {"image_package_native_basename", "dtb_package_native_basename", "artifact_suffix", "image_package_sha256", "dtb_package_sha256", "image_dtb_sha256", "dtb_package_dtb_sha256", "dtb_byte_equal", "packaged_config_expected_sha256", "final_config_sha256", "module_relative_path", "module_compressed_sha256", "module_decompressed_sha256", "module_vermagic", "module_interface_string_marker", "module_interface_options_marker", "module_interface_runtime_marker"}
    require(set(evidence) - {"_sha256"} == required and evidence.get("dtb_byte_equal") == "true", "Orange kernel evidence fields changed")
    for key in ("image_package", "dtb_package", "image_package_native", "dtb_package_native", "image_package_sha256", "dtb_package_sha256", "evidence_sha256", "kernel_source_repository", "kernel_source_commit", "kernel_release"):
        require(key in provenance, f"Orange kernel provenance omits required field: {key}")
    with tempfile.TemporaryDirectory(prefix="octessera-orange-package-proof-") as temporary:
        package = verify_package_chain(args.linux_image, args.linux_dtb, evidence, provenance, manifest, Path(temporary))
    boot = verify_boot(root, package, contract, repository_root)
    verify_dpkg_status(root, package)
    runtime = verify_runtime(root, args.mode, contract, repository_root)
    return {"schema": "octessera.image-proof/v2", "schema_version": 2, "proof_mode": "phase5-constructor", "phase5_claim": True, "boot_state": "phase5-v1", "artifact": {"name": image_name, "sha256": image_hash, "compression": compression}, "board_profile": "orange-pi-zero-2w", "runtime": runtime, "kernel": {"release": package["release"], "linux_image_package": provenance["image_package"], "linux_dtb_package": provenance["dtb_package"], "evidence_sha256": evidence["_sha256"], "provenance_sha256": provenance["_sha256"]}, "device_config_validator": boot["device_config_validator"], "contract": {"path": str(contract_path.relative_to(repository_root)), "sha256": contract_hash}}
