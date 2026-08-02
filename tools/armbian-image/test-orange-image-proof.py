from __future__ import annotations

import hashlib
import json
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import orange_image_mount

TOOLS = Path(__file__).resolve().parent


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


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path: Path, content: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content if isinstance(content, bytes) else content.encode())


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
    config = b"# CONFIG_RT_GROUP_SCHED is not set\nCONFIG_SND_SEQUENCER=m\n"
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
    (final_root / "boot").mkdir(exist_ok=True)
    (final_root / "boot/Image").symlink_to(f"../usr/lib/linux-image-{RELEASE}/Image")
    write(final_root / f"boot/initrd.img-{RELEASE}", f"initramfs {RELEASE} usb_f_midi snd_seq snd_rawmidi snd_usb_audio")
    (final_root / "boot/uInitrd").symlink_to(f"initrd.img-{RELEASE}")
    write(final_root / "boot/armbianEnv.txt", f"fdtfile=dtb-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb\n")
    write(final_root / "etc/os-release", "ID=armbian\n")
    write(final_root / "etc/octessera/build-metadata.env", "OCTESSERA_IMAGE_MODE=diagnostic\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\n")
    write(final_root / "etc/octessera/image-contract.json", '{"schema_version": 1, "image_kind": "diagnostic", "runtime_enabled_default": false}\n')
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
        "--mode",
        mode,
    ]


def run_proof(args: list[str], expected: bool) -> None:
    result = subprocess.run(args, capture_output=True, text=True)
    if (result.returncode == 0) != expected:
        raise AssertionError(result.stdout + result.stderr)


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
    if shutil.which("dpkg-deb") is None:
        print("Orange image proof fixture skipped: dpkg-deb is unavailable")
        return
    with tempfile.TemporaryDirectory(prefix="octessera-orange-proof-fixture-") as temporary:
        work = Path(temporary)
        root, image, dtb, evidence, provenance = make_fixture(work)
        args = verifier_args(root, image, dtb, evidence, provenance)
        run_proof(args, True)
        artifact = work / "image-provenance.txt"
        run_proof([*args, "--output", str(artifact)], True)
        tampered_artifact = work / "tampered-image-provenance.txt"
        tampered_artifact.write_text(artifact.read_text().replace("image_sha256=" + "a" * 64, "image_sha256=" + "b" * 64))
        run_proof([*args, "--image-provenance", str(tampered_artifact)], False)
        canonical_image = work / CANONICAL_IMAGE
        canonical_dtb = work / CANONICAL_DTB
        shutil.copy2(image, canonical_image)
        shutil.copy2(dtb, canonical_dtb)
        run_proof(verifier_args(root, canonical_image, canonical_dtb, evidence, provenance), True)
        for name, mutate in (
            ("config", lambda path: path.write_bytes(path.read_bytes() + b"CONFIG_BAD=y\n")),
            ("module", lambda path: path.write_bytes(path.read_bytes() + b"tampered")),
            ("status", lambda path: path.write_text(path.read_text().replace("install ok installed", "deinstall ok config-files", 1))),
        ):
            negative = work / f"negative-{name}"
            shutil.copytree(root, negative, symlinks=True)
            target = negative / (f"boot/config-{RELEASE}" if name == "config" else MODULE_RELATIVE if name == "module" else "var/lib/dpkg/status")
            mutate(target)
            run_proof([*args[: args.index(str(root))], str(negative), *args[args.index(str(root)) + 1 :]], False)
        production = work / "production"
        shutil.copytree(root, production, symlinks=True)
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
            "octessera-runtime:x:990:\naudio:x:29:octessera-runtime\ni2c:x:998:octessera-runtime\n"
            "spi:x:997:octessera-runtime\ngpio:x:996:octessera-runtime\n",
        )
        (production / "var/lib/octessera/presets").mkdir(parents=True)
        (production / "var/lib/octessera/samples").mkdir(parents=True)
        write(production / "etc/systemd/system/octessera.service", "[Service]\nUser=octessera-runtime\nGroup=octessera-runtime\nEnvironment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w\nEnvironment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets\nEnvironment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples\nEnvironment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json\nNoNewPrivileges=yes\nProtectSystem=strict\nReadWritePaths=/var/lib/octessera /run/octessera\nPrivateTmp=yes\nProtectHome=yes\nRuntimeDirectory=octessera\nLimitRTPRIO=70\nLimitMEMLOCK=infinity\nExecStart=/usr/local/bin/octessera-pi\n")
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
