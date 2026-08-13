from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
import orange_image_mount
from orange_boot_contract import verify_runtime
from stage_notices import stage_notices  # type: ignore[import-not-found]
TOOLS = Path(__file__).resolve().parent
REPOSITORY = TOOLS.parents[1]
CONSTRUCTION = json.loads((REPOSITORY / "resources/image-construction/boot-layers/orange-pi-zero-2w.json").read_text())
VERIFY_SPEC = importlib.util.spec_from_file_location("orange_image_verifier", TOOLS / "verify-orange-image.py")
assert VERIFY_SPEC is not None and VERIFY_SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(VERIFY_SPEC)
VERIFY_SPEC.loader.exec_module(VERIFY)

RELEASE = "6.18.38-current-sunxi64"
REVISION = "26.8.0-trunk.417"
IMAGE_NAME = "linux-image-current-sunxi64"
DTB_NAME = "linux-dtb-current-sunxi64"
CANONICAL_IMAGE = f"{IMAGE_NAME}_{REVISION}_arm64.deb"
CANONICAL_DTB = f"{DTB_NAME}_{REVISION}_arm64.deb"
NATIVE_IMAGE = f"{IMAGE_NAME}_{REVISION}_arm64__fixture.deb"
NATIVE_DTB = f"{DTB_NAME}_{REVISION}_arm64__fixture.deb"
DTB_RELATIVE = f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb"
MODULE_RELATIVE = f"lib/modules/{RELEASE}/kernel/drivers/usb/gadget/function/usb_f_midi.ko"
BUILTIN_CONFIG_LINES = ("CONFIG_SPI_SUN6I=y", "CONFIG_SPI_SPIDEV=y", "CONFIG_PINCTRL_SUNXI=y")

def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path: Path, content: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content if isinstance(content, bytes) else content.encode())


def make_uboot_initramfs(payload: bytes, declared_size: int | None = None) -> bytes:
    header = bytearray(64)
    struct.pack_into(">I", header, 0, 0x27051956)
    struct.pack_into(">I", header, 12, len(payload) if declared_size is None else declared_size)
    return bytes(header) + payload
def make_cpio_initramfs(work: Path, source_root: Path, stale: bool = False, extension: str | None = None) -> bytes:
    source = work / ("stale-initramfs-source" if stale else "initramfs-source")
    write(source / "init", b"#!/bin/sh\n")
    if not stale:
        installed_matches = CONSTRUCTION["selected_initramfs"]["installed_output_matches"]
        for item in installed_matches:
            target = source / item["initramfs_path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_root / item["installed_path"], target)
        for tool in CONSTRUCTION["selected_initramfs"]["required_tools"]:
            write(source / tool, b"synthetic-tool\n")
        write(source / "usr/bin/python3", b"synthetic-python\n")
        for relative in CONSTRUCTION["selected_initramfs"]["python_files"]:
            write(source / f"usr/lib/python3.13/{relative}", b"synthetic-python-closure\n")
        if extension is not None:
            write(source / extension, (source_root / extension).read_bytes())
    return subprocess.run(
        ["cpio", "--quiet", "-o", "-H", "newc"],
        cwd=source,
        input=subprocess.run(["find", ".", "-print"], cwd=source, capture_output=True, check=True).stdout,
        capture_output=True,
        check=True,
    ).stdout


def make_fixture(work: Path) -> tuple[Path, Path, Path, Path, Path]:
    image_root = work / "image-package"
    dtb_root = work / "dtb-package"
    image_root.mkdir()
    dtb_root.mkdir()
    write(
        image_root / "DEBIAN/control",
        f"Package: {IMAGE_NAME}\nVersion: {REVISION}\nSource: linux-6.18.38\nArchitecture: arm64\n"
        f"Armbian-Kernel-Version: 6.18.38\nArmbian-Kernel-Version-Family: {RELEASE}\n",
    )
    write(dtb_root / "DEBIAN/control", f"Package: {DTB_NAME}\nVersion: {REVISION}\nArchitecture: arm64\n")
    kernel = b"synthetic-orange-kernel-" + RELEASE.encode()
    config = b"# CONFIG_RT_GROUP_SCHED is not set\n" + b"\n".join(line.encode() for line in BUILTIN_CONFIG_LINES) + b"\nCONFIG_SND_SEQUENCER=m\n"
    dtb = b"\xd0\x0d\xfe\xedsynthetic-zero2w-dtb"
    module = b"\x7fELF" + b"\x02\x01\x01" + bytes(11) + struct.pack("<H", 183) + b"vermagic=" + RELEASE.encode() + b" SMP\ninterface_string\ninterface_string\nf_midi_opts_attr_interface_string\nmidi_interface_string\n"
    write(image_root / f"usr/lib/linux-image-{RELEASE}/Image", kernel)
    write(image_root / f"boot/config-{RELEASE}", config)
    write(image_root / DTB_RELATIVE, dtb)
    write(image_root / MODULE_RELATIVE, module)
    write(image_root / f"lib/modules/{RELEASE}/modules.dep", b"kernel/drivers/usb/gadget/function/usb_f_midi.ko:\n")
    for module_name in ("snd-seq.ko", "snd-seq-midi.ko", "snd-rawmidi.ko", "snd-usb-audio.ko"):
        write(image_root / f"lib/modules/{RELEASE}/kernel/sound/{module_name}", b"synthetic-module")
    write(dtb_root / f"boot/dtb-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb", dtb)
    packages = work / "packages"
    packages.mkdir()
    subprocess.run(["dpkg-deb", "--build", str(image_root), str(packages / NATIVE_IMAGE)], check=True, capture_output=True)
    subprocess.run(["dpkg-deb", "--build", str(dtb_root), str(packages / NATIVE_DTB)], check=True, capture_output=True)
    final_root = work / "final-root"
    final_root.mkdir()
    subprocess.run(["dpkg-deb", "-x", str(packages / NATIVE_IMAGE), str(final_root)], check=True, capture_output=True)
    subprocess.run(["dpkg-deb", "-x", str(packages / NATIVE_DTB), str(final_root)], check=True, capture_output=True)
    stage_notices(REPOSITORY, final_root)
    write(final_root / "usr/share/common-licenses/GPL-3", b"fixture GPL license\n")
    write(final_root / "usr/share/doc/base-files/copyright", b"fixture base-files copyright\n")
    (final_root / "boot").mkdir(exist_ok=True)
    (final_root / "boot/Image").symlink_to(f"../usr/lib/linux-image-{RELEASE}/Image")
    phase5_outputs = {
        "etc/profile.d/octessera-welcome.sh": "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh",
        "etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash": "userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash",
        "usr/local/sbin/octessera-orange-oled-logo": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo",
        "usr/local/sbin/octessera-orange-oled-handoff.py": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py",
        "usr/local/sbin/octessera-orange-oled-lifecycle.py": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py",
        "usr/local/sbin/octessera-orange-oled-suspend": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend",
        "usr/share/octessera/oled/octessera-mark.svg": "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-mark.svg",
        "usr/share/octessera/oled/octessera-wordmark.svg": "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-wordmark.svg",
    }
    for installed_path, source_path in phase5_outputs.items():
        write(final_root / installed_path, (REPOSITORY / source_path).read_bytes())
    write(final_root / "etc/systemd/system/octessera-orange-boot-splash.service", (REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service").read_bytes())
    write(final_root / "etc/systemd/system/octessera-orange-oled-shutdown.service", (REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service").read_bytes())
    write(final_root / "etc/systemd/system/octessera-orange-oled-suspend.service", (REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service").read_bytes())
    (final_root / "etc/systemd/system/sysinit.target.wants").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service").symlink_to("../octessera-orange-boot-splash.service")
    (final_root / "etc/systemd/system/sleep.target.requires").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service").symlink_to("../octessera-orange-oled-suspend.service")
    initramfs = make_cpio_initramfs(work, final_root)
    compressed_initramfs = subprocess.run(["zstd", "-q", "-c"], input=initramfs, capture_output=True, check=True).stdout
    write(final_root / f"boot/initrd.img-{RELEASE}", make_uboot_initramfs(compressed_initramfs))
    (final_root / "boot/uInitrd").symlink_to(f"initrd.img-{RELEASE}")
    write(final_root / "boot/armbianEnv.txt", "verbosity=1\n")
    write(final_root / "boot/overlay-user/octessera-h618-input-routing.dtbo", b"synthetic-input-routing-dtbo")
    write(final_root / "usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts", b"synthetic-input-routing-dts")
    (final_root / "etc/systemd/system").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/serial-getty@ttyS0.service").symlink_to("/dev/null")
    write(final_root / "etc/os-release", "ID=armbian\n")
    write(final_root / "etc/octessera/build-metadata.env", "OCTESSERA_IMAGE_MODE=diagnostic\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\n")
    write(final_root / "etc/octessera/image-contract.json", '{"schema_version": 1, "image_kind": "diagnostic", "runtime_enabled_default": false}\n')
    write(final_root / "etc/passwd", "octessera:x:1000:1000:Octessera:/home/octessera:/bin/bash\noctessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin\n")
    (final_root / "home/octessera").mkdir(parents=True, exist_ok=True)
    write(final_root / "home/octessera/.hushlogin", b"")
    os.chown(final_root / "home/octessera/.hushlogin", 1000, 1000)  # type: ignore[attr-defined]
    write(final_root / "etc/pam.d/20-vendor-login", b"vendor\n")
    write(final_root / "etc/update-motd.d/20-vendor-status", b"vendor\n")
    write(final_root / "etc/shadow", "octessera:!:1:0:99999:7:::\noctessera-runtime:!:1:0:99999:7:::\n")
    write(final_root / "etc/group", "octessera:x:1000:\noctessera-runtime:x:990:\naudio:x:29:octessera-runtime\ni2c:x:998:octessera-runtime\nspi:x:997:octessera-runtime\ngpio:x:996:octessera-runtime\n")
    write(
        final_root / "var/lib/dpkg/status",
        f"Package: {IMAGE_NAME}\nStatus: install ok installed\nVersion: {REVISION}\nArchitecture: arm64\n\n"
        f"Package: {DTB_NAME}\nStatus: install ok installed\nVersion: {REVISION}\nArchitecture: arm64\n",
    )
    evidence_path = work / "evidence.env"
    evidence_values = {
        "image_package_native_basename": NATIVE_IMAGE,
        "dtb_package_native_basename": NATIVE_DTB,
        "artifact_suffix": "fixture",
        "image_package_sha256": sha256(packages / NATIVE_IMAGE),
        "dtb_package_sha256": sha256(packages / NATIVE_DTB),
        "image_dtb_sha256": hashlib.sha256(dtb).hexdigest(),
        "dtb_package_dtb_sha256": hashlib.sha256(dtb).hexdigest(),
        "dtb_byte_equal": "true",
        "packaged_config_expected_sha256": "fddbc3ff39e27b7e0aeb80b97496b93f5fca91b8fd166f2937f6924dc034c352",
        "final_config_sha256": hashlib.sha256(config).hexdigest(),
        "module_relative_path": MODULE_RELATIVE,
        "module_compressed_sha256": hashlib.sha256(module).hexdigest(),
        "module_decompressed_sha256": hashlib.sha256(module).hexdigest(),
        "module_vermagic": f"{RELEASE} SMP",
        "module_interface_string_marker": "interface_string",
        "module_interface_options_marker": "f_midi_opts_attr_interface_string",
        "module_interface_runtime_marker": "midi_interface_string",
    }
    evidence_path.write_text("\n".join(f"{key}={value}" for key, value in evidence_values.items()) + "\n")
    provenance_path = work / "kernel-provenance.txt"
    provenance_values = {
        "schema": "1",
        "image_package": CANONICAL_IMAGE,
        "image_package_native": NATIVE_IMAGE,
        "image_package_sha256": evidence_values["image_package_sha256"],
        "dtb_package": CANONICAL_DTB,
        "dtb_package_native": NATIVE_DTB,
        "dtb_package_sha256": evidence_values["dtb_package_sha256"],
        "evidence_sha256": sha256(evidence_path),
        "kernel_source_repository": "https://github.com/torvalds/linux.git",
        "kernel_source_commit": "e46dc0adfe39724bcf52cea47b8f9c9aed86a394",
        "kernel_release": RELEASE,
    }
    provenance_path.write_text("\n".join(f"{key}={value}" for key, value in provenance_values.items()) + "\n")
    return final_root, packages / NATIVE_IMAGE, packages / NATIVE_DTB, evidence_path, provenance_path


def verifier_args(root: Path, image: Path, dtb: Path, evidence: Path, provenance: Path, mode: str = "diagnostic", privileged: bool = False) -> list[str]:
    command = [sys.executable, str(TOOLS / "verify-orange-image.py")]
    if privileged:
        command = ["sudo", "-n", *command]
    return [
        *command,
        "--root",
        str(root),
        "--image-sha256",
        "a" * 64,
        "--linux-image",
        str(image),
        "--linux-dtb",
        str(dtb),
        "--evidence",
        str(evidence),
        "--provenance",
        str(provenance),
        "--manifest",
        str(REPOSITORY / "tools/kernel-patches/orange-midi-interface-manifest.json"),
        "--construction-contract",
        str(REPOSITORY / "resources/image-construction/boot-layers/orange-pi-zero-2w.json"),
        "--boot-proof-mode",
        "phase5-constructor",
        "--mode",
        mode,
    ]


def run_proof(args: list[str], expected: bool) -> None:
    result = subprocess.run(args, capture_output=True, text=True)
    if (result.returncode == 0) != expected:
        raise AssertionError(result.stdout + result.stderr)


def root_args(args: list[str], root: Path) -> list[str]:
    result = list(args)
    result[result.index("--root") + 1] = str(root)
    return result


def without_option(args: list[str], option: str) -> list[str]:
    index = args.index(option)
    return [*args[:index], *args[index + 2 :]]


def replace_option(args: list[str], option: str, value: Path) -> list[str]:
    result = list(args)
    result[result.index(option) + 1] = str(value)
    return result


def make_missing_builtin_fixture(work: Path, root: Path, image: Path, evidence: Path, provenance: Path) -> tuple[Path, Path, Path, Path]:
    negative_root = work / "negative-missing-builtin"
    shutil.copytree(root, negative_root, symlinks=True)
    config_path = negative_root / f"boot/config-{RELEASE}"
    config = config_path.read_bytes().replace(b"CONFIG_SPI_SPIDEV=y\n", b"", 1)
    write(config_path, config)
    image_root = work / "negative-image-root"
    subprocess.run(["dpkg-deb", "-R", str(image), str(image_root)], check=True, capture_output=True)
    write(image_root / f"boot/config-{RELEASE}", config)
    negative_image = work / "negative-packages" / NATIVE_IMAGE
    negative_image.parent.mkdir()
    subprocess.run(["dpkg-deb", "--build", str(image_root), str(negative_image)], check=True, capture_output=True)
    negative_evidence = work / "negative-evidence.env"
    evidence_lines = []
    for line in evidence.read_text().splitlines():
        key, _, value = line.partition("=")
        value = {"image_package_sha256": sha256(negative_image), "final_config_sha256": hashlib.sha256(config).hexdigest()}.get(key, value)
        evidence_lines.append(f"{key}={value}")
    negative_evidence.write_text("\n".join(evidence_lines) + "\n")
    negative_provenance = work / "negative-provenance.txt"
    provenance_lines = []
    for line in provenance.read_text().splitlines():
        key, _, value = line.partition("=")
        value = {"image_package_sha256": sha256(negative_image), "evidence_sha256": sha256(negative_evidence)}.get(key, value)
        provenance_lines.append(f"{key}={value}")
    negative_provenance.write_text("\n".join(provenance_lines) + "\n")
    return negative_root, negative_image, negative_evidence, negative_provenance


def main() -> None:
    original_run = orange_image_mount._run

    def fake_lsblk_run(command: list[str], **_: object) -> subprocess.CompletedProcess[str]:
        if "--bytes" not in command:
            raise AssertionError("lsblk partition geometry must use bytes")
        payload = {
            "blockdevices": [
                {
                    "name": "/dev/loop0",
                    "type": "loop",
                    "children": [
                        {"name": "/dev/loop0p1", "type": "part", "start": 2048, "size": 536870912}
                    ],
                }
            ]
        }
        return subprocess.CompletedProcess(command, 0, json.dumps(payload), "")

    try:
        orange_image_mount._run = fake_lsblk_run
        assert orange_image_mount._lsblk("/dev/loop0") == ["/dev/loop0p1"]
    finally:
        orange_image_mount._run = original_run
    missing_tools = [tool for tool in ("cpio", "dpkg-deb", "zstd") if shutil.which(tool) is None]
    if missing_tools:
        print(f"Orange image proof fixture skipped: missing {', '.join(missing_tools)}")
        return
    with tempfile.TemporaryDirectory(prefix="octessera-orange-proof-fixture-") as temporary:
        work = Path(temporary)
        root, image, dtb, evidence, provenance = make_fixture(work)
        args = verifier_args(root, image, dtb, evidence, provenance)
        run_proof(args, True)
        extension = "usr/lib/python3.13/lib-dynload/_json.cpython-313-aarch64-linux-gnu.so"; write(root / extension, b"synthetic-python-extension")
        write(root / f"boot/initrd.img-{RELEASE}", make_uboot_initramfs(subprocess.run(["zstd", "-q", "-c"], input=make_cpio_initramfs(work, root, extension=extension), capture_output=True, check=True).stdout))
        run_proof(args, True)
        negative_extension = work / "negative-extension-mismatch"; shutil.copytree(root, negative_extension, symlinks=True); write(negative_extension / extension, b"mismatched-python-extension")
        run_proof(root_args(args, negative_extension), False)
        negative_root, negative_image, negative_evidence, negative_provenance = make_missing_builtin_fixture(work, root, image, evidence, provenance)
        negative_args = args
        for option, value in (("--root", negative_root), ("--linux-image", negative_image), ("--evidence", negative_evidence), ("--provenance", negative_provenance)):
            negative_args = replace_option(negative_args, option, value)
        negative_result = subprocess.run(negative_args, capture_output=True, text=True)
        if negative_result.returncode == 0 or "CONFIG_SPI_SPIDEV=y" not in negative_result.stderr:
            raise AssertionError(negative_result.stdout + negative_result.stderr)

        def reject_terminal_fixture(name: str, mutate: object) -> None:
            negative = work / f"negative-terminal-{name}"
            shutil.copytree(root, negative, symlinks=True)
            mutate(negative)  # type: ignore[operator]
            run_proof(root_args(args, negative), False)

        reject_terminal_fixture("stale-welcome", lambda path: write(path / "etc/profile.d/octessera-welcome.sh", b"stale\n"))
        reject_terminal_fixture("missing-hushlogin", lambda path: (path / "home/octessera/.hushlogin").unlink())
        reject_terminal_fixture("nonempty-hushlogin", lambda path: write(path / "home/octessera/.hushlogin", b"x"))

        def wrong_hush_owner(path: Path) -> None:
            os.chown(path / "home/octessera/.hushlogin", 0, 0)  # type: ignore[attr-defined]

        reject_terminal_fixture("wrong-hush-owner", wrong_hush_owner)
        reject_terminal_fixture("wrong-hush-mode", lambda path: path.joinpath("home/octessera/.hushlogin").chmod(0o600))

        def symlink_hush(path: Path) -> None:
            (path / "home/octessera/.hushlogin").unlink()
            (path / "home/octessera/.hushlogin").symlink_to("/dev/null")

        reject_terminal_fixture("symlink-hushlogin", symlink_hush)
        reject_terminal_fixture("duplicate-account", lambda path: path.joinpath("etc/passwd").write_text(path.joinpath("etc/passwd").read_text() + "octessera:x:1001:1001:Duplicate:/home/octessera:/bin/bash\n"))
        reject_terminal_fixture("wrong-home", lambda path: path.joinpath("etc/passwd").write_text(path.joinpath("etc/passwd").read_text().replace("/home/octessera:/bin/bash", "/srv/octessera:/bin/bash")))
        reject_terminal_fixture("wrong-shell", lambda path: path.joinpath("etc/passwd").write_text(path.joinpath("etc/passwd").read_text().replace("/home/octessera:/bin/bash", "/home/octessera:/bin/sh")))
        reject_terminal_fixture("python-parent-symlink", lambda path: ((path / "usr/lib/python3.13").rename(path / "usr/lib/python-runtime-target"), (path / "usr/lib/python3.13").symlink_to("python-runtime-target", target_is_directory=True)))
        reject_terminal_fixture("wrong-build-metadata-mode", lambda path: path.joinpath("etc/octessera/build-metadata.env").chmod(0o600))
        def wrong_build_metadata_owner(path: Path) -> None:
            os.chown(path / "etc/octessera/build-metadata.env", 1000, 1000)  # type: ignore[attr-defined]

        reject_terminal_fixture("wrong-build-metadata-owner", wrong_build_metadata_owner)
        reject_terminal_fixture("missing-group", lambda path: path.joinpath("etc/group").write_text(path.joinpath("etc/group").read_text().replace("octessera:x:1000:\n", "")))
        reject_terminal_fixture("duplicate-group", lambda path: path.joinpath("etc/group").write_text(path.joinpath("etc/group").read_text() + "octessera:x:1000:\n"))
        reject_terminal_fixture("wrong-group-gid", lambda path: path.joinpath("etc/group").write_text(path.joinpath("etc/group").read_text().replace("octessera:x:1000:", "octessera:x:1001:")))
        reject_terminal_fixture("pam-override", lambda path: write(path / "etc/pam.d/10-octessera", b"override\n"))
        reject_terminal_fixture("motd-override", lambda path: write(path / "etc/update-motd.d/10-octessera", b"override\n"))

        def reject_notice_fixture(name: str, mutate: object) -> None:
            negative = work / f"negative-notice-{name}"
            shutil.copytree(root, negative, symlinks=True)
            mutate(negative)  # type: ignore[operator]
            run_proof(root_args(args, negative), False)

        reject_notice_fixture("stale", lambda path: write(path / "usr/share/doc/octessera/LICENSE", b"stale\n"))
        reject_notice_fixture("missing", lambda path: (path / "usr/share/doc/octessera/LICENSE").unlink())
        reject_notice_fixture("extra", lambda path: write(path / "usr/share/doc/octessera/extra.txt", b"extra\n"))
        reject_notice_fixture("mode", lambda path: path.joinpath("usr/share/doc/octessera/LICENSE").chmod(0o600))

        def symlink_notice(path: Path) -> None:
            (path / "usr/share/doc/octessera/LICENSE").unlink()
            (path / "usr/share/doc/octessera/LICENSE").symlink_to("/dev/null")

        reject_notice_fixture("symlink", symlink_notice)
        run_proof(without_option(args, "--manifest"), False)
        for opposite_option, opposite_value in (
            ("--trust-manifest", str(REPOSITORY / "resources/image-parents/v0.7.5-trust-manifest.json")),
            ("--boot-neutral-contract", str(REPOSITORY / "resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.7.5.json")),
            ("--parent-image", str(work / "parent.img.xz")),
            ("--respin-provenance", str(work / "respin.json")),
            ("--derivation-kind", "runtime-only"),
            ("--setup-proof", str(work / "setup-proof.json")),
        ):
            run_proof([*args, opposite_option, opposite_value], False)
        run_proof(without_option(args, "--construction-contract"), False)
        wrong_contract = work / "wrong-construction.json"
        shutil.copyfile(REPOSITORY / "resources/image-construction/boot-layers/orange-pi-zero-2w.json", wrong_contract)
        wrong_contract_args = list(args)
        wrong_contract_args[wrong_contract_args.index("--construction-contract") + 1] = str(wrong_contract)
        run_proof(wrong_contract_args, False)
        artifact = work / "image-provenance.txt"
        run_proof([*args, "--output", str(artifact)], True)
        proof_document = json.loads(artifact.read_text())
        assert proof_document["schema"] == "octessera.image-proof/v2"
        assert proof_document["schema_version"] == 2
        assert proof_document["proof_mode"] == "phase5-constructor"
        tampered_artifact = work / "tampered-image-provenance.txt"
        tampered = json.loads(artifact.read_text())
        tampered["artifact"]["sha256"] = "b" * 64
        tampered_artifact.write_text(json.dumps(tampered) + "\n")
        run_proof([*args, "--image-provenance", str(tampered_artifact)], False)
        canonical_image = work / CANONICAL_IMAGE
        canonical_dtb = work / CANONICAL_DTB
        shutil.copy2(image, canonical_image)
        shutil.copy2(dtb, canonical_dtb)
        run_proof(verifier_args(root, canonical_image, canonical_dtb, evidence, provenance), True)
        for name, contents in (
            ("truncated-uboot-header", b"\x27\x05\x19\x56\x00"),
            ("oversized-uboot-payload", make_uboot_initramfs(b"raw", declared_size=4)),
            ("truncated-zstd", make_uboot_initramfs(b"\x28\xb5\x2f\xfd")),
            ("damaged-zstd-magic", b"\x28\xb5\x2f\xfe"),
            ("corrupt-gzip", b"\x1f\x8bcorrupt"),
            ("corrupt-xz", b"\xfd7zXZ\x00corrupt"),
        ):
            negative = work / f"negative-{name}"
            shutil.copytree(root, negative, symlinks=True)
            write(negative / f"boot/initrd.img-{RELEASE}", contents)
            run_proof(root_args(args, negative), False)
        stale = work / "stale-v0.7.5-parent"
        shutil.copytree(root, stale, symlinks=True)
        stale_payload = make_cpio_initramfs(work, stale, True)
        stale_compressed = subprocess.run(["zstd", "-q", "-c"], input=stale_payload, capture_output=True, check=True).stdout
        write(stale / f"boot/initrd.img-{RELEASE}", make_uboot_initramfs(stale_compressed))
        run_proof(root_args(args, stale), False)
        verify_runtime(stale, "diagnostic")
        for name, contents in (
            ("empty-fdt", "fdtfile=\n"),
            ("duplicate-fdt", "fdtfile=sun50i-h618-orangepi-zero2w.dtb\nfdtfile=sun50i-h618-orangepi-zero2w.dtb\n"),
        ):
            negative = work / name
            shutil.copytree(root, negative, symlinks=True)
            write(negative / "boot/armbianEnv.txt", contents)
            run_proof(root_args(args, negative), False)
        for name, mutate in (
            ("config", lambda path: path.write_bytes(path.read_bytes() + b"CONFIG_BAD=y\n")),
            ("module", lambda path: path.write_bytes(path.read_bytes() + b"tampered")),
            ("status", lambda path: path.write_text(path.read_text().replace("install ok installed", "deinstall ok config-files", 1))),
        ):
            negative = work / f"negative-{name}"
            shutil.copytree(root, negative, symlinks=True)
            target = negative / (f"boot/config-{RELEASE}" if name == "config" else MODULE_RELATIVE if name == "module" else "var/lib/dpkg/status")
            mutate(target)
            run_proof(root_args(args, negative), False)
        production = work / "production"
        shutil.copytree(root, production, symlinks=True)
        os.chown(production / "home/octessera/.hushlogin", 1000, 1000)  # type: ignore[attr-defined]
        binary = b"\x7fELF\x02\x01\x01" + bytes(11) + struct.pack("<H", 183) + bytes(64)
        version = "0.5.0"
        release_dir = production / f"opt/octessera/releases/{version}"
        release_dir.mkdir(parents=True)
        write(release_dir / "octessera-pi", binary)
        binary_hash = sha256(release_dir / "octessera-pi")
        write(release_dir / "SHA256SUMS", f"{binary_hash}  octessera-pi\n")
        runtime_metadata = {"artifact_kind": "production-runtime", "binary_sha256": binary_hash, "name": "octessera-pi", "profile": "orange-pi-zero-2w", "runtime_ready": True, "version": version}
        write(release_dir / "octessera-runtime.json", json.dumps(runtime_metadata, sort_keys=True, indent=2) + "\n")
        (production / "opt/octessera/current").symlink_to(f"/opt/octessera/releases/{version}")
        (production / "usr/local/bin").mkdir(parents=True)
        (production / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
        write(production / "etc/octessera/image-contract.json", '{"schema_version": 1, "image_kind": "production", "runtime_enabled_default": true}\n')
        write(
            production / "etc/passwd",
            "octessera:x:1000:1000:Octessera:/home/octessera:/bin/bash\n"
            "octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin\n",
        )
        write(production / "etc/shadow", "octessera:*:1:0:99999:7:::\noctessera-runtime:!:1:0:99999:7:::\n")
        write(
            production / "etc/group",
            "octessera:x:1000:\noctessera-runtime:x:990:\naudio:x:29:octessera-runtime\ni2c:x:998:octessera-runtime\n"
            "spi:x:997:octessera-runtime\ngpio:x:996:octessera-runtime\n",
        )
        (production / "var/lib/octessera/presets").mkdir(parents=True)
        (production / "var/lib/octessera/samples").mkdir(parents=True)
        write(production / "etc/systemd/system/octessera.service", "[Unit]\nStartLimitIntervalSec=30s\nStartLimitBurst=3\n[Service]\nUser=octessera-runtime\nGroup=octessera-runtime\nEnvironment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w\nEnvironment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets\nEnvironment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples\nEnvironment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json\nEnvironment=OCTESSERA_OLED_BOOT_HANDOFF=v1\nNoNewPrivileges=yes\nProtectSystem=strict\nReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot\nPrivateTmp=yes\nProtectHome=yes\nRuntimeDirectory=octessera\nLimitRTPRIO=70\nLimitMEMLOCK=infinity\nExecStart=/usr/local/bin/octessera-pi\nRestart=on-failure\nRestartSec=5s\n")
        write(production / "etc/udev/rules.d/70-octessera-orange-runtime.rules", "KERNEL==\"i2c-2\", GROUP=\"octessera-runtime\", MODE=\"0660\"\nKERNEL==\"spidev1.0\", GROUP=\"octessera-runtime\", MODE=\"0660\"\nKERNEL==\"gpiochip1\", GROUP=\"octessera-runtime\", MODE=\"0660\"\n")
        write(production / "etc/udev/rules.d/10-wifi-disable-powermanagement.rules", 'KERNEL=="wlan*", ACTION=="add", RUN+="/sbin/iw dev %k set power_save off"\n')
        (production / "etc/udev/rules.d/09-disabled.rules").symlink_to("/dev/null")
        (production / "etc/systemd/system/multi-user.target.wants").mkdir(parents=True)
        (production / "etc/systemd/system/multi-user.target.wants/octessera.service").symlink_to("/etc/systemd/system/octessera.service")
        runtime_metadata_hash = sha256(release_dir / "octessera-runtime.json")
        sums_hash = sha256(release_dir / "SHA256SUMS")
        write(production / "etc/octessera/build-metadata.env", f"OCTESSERA_IMAGE_MODE=production\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=true\nOCTESSERA_RUNTIME_VERSION={version}\nOCTESSERA_RUNTIME_BINARY_SHA256={binary_hash}\nOCTESSERA_RUNTIME_METADATA_SHA256={runtime_metadata_hash}\nOCTESSERA_RUNTIME_MANIFEST_SHA256={sums_hash}\n")
        can_privilege = shutil.which("sudo") is not None and subprocess.run(["sudo", "-n", "true"], check=False, capture_output=True).returncode == 0
        if can_privilege:
            try:
                subprocess.run(["sudo", "-n", "chown", "-R", "root:root", str(release_dir)], check=True)
                subprocess.run(["sudo", "-n", "chmod", "0555", str(release_dir), str(release_dir / "octessera-pi")], check=True)
                subprocess.run(["sudo", "-n", "chmod", "0444", str(release_dir / "octessera-runtime.json"), str(release_dir / "SHA256SUMS")], check=True)
                subprocess.run(["sudo", "-n", "chown", "-R", "990:990", str(production / "var/lib/octessera")], check=True)
                subprocess.run(["sudo", "-n", "chown", "0:0", str(production / "etc/udev/rules.d/70-octessera-orange-runtime.rules")], check=True)
                subprocess.run(["sudo", "-n", "chmod", "0644", str(production / "etc/udev/rules.d/70-octessera-orange-runtime.rules")], check=True)
                run_proof(verifier_args(production, image, dtb, evidence, provenance, "production", True), True)
                enabled = production / "etc/systemd/system/multi-user.target.wants/octessera.service"
                enabled.unlink()
                run_proof(verifier_args(production, image, dtb, evidence, provenance, "production", True), False)
            finally:
                owner = work.stat()
                subprocess.run(["sudo", "-n", "chown", "-R", f"{owner.st_uid}:{owner.st_gid}", str(work)], check=False)
    print("Orange final image proof synthetic fixtures passed")


if __name__ == "__main__":
    main()
