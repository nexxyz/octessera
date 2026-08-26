from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

from test_orange_image_proof_support import (
    REPOSITORY,
    RELEASE,
    MODULE_RELATIVE,
    copy_fixture_root,
    replace_option,
    root_args,
    run_proof_failure,
    sha256,
    run_proof,
    verifier_args,
    without_option,
    write,
)


def run_boot_proof(work: Path, root: Path, image: Path, dtb: Path, evidence: Path, provenance: Path) -> None:
    args = verifier_args(root, image, dtb, evidence, provenance)
    socket_link = root / "etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket"
    original_socket_target = socket_link.readlink()
    for target, expected in (
        ("/etc/systemd/system/octessera-device-apply-reboot.socket", True),
        ("/etc/systemd/system/../system/octessera-device-apply-reboot.socket", False),
        ("/tmp/octessera-device-apply-reboot.socket", False),
    ):
        socket_link.unlink()
        socket_link.symlink_to(target)
        run_proof(args, expected)
    socket_link.unlink()
    socket_link.symlink_to(original_socket_target)

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
    wrong_contract_args = replace_option(args, "--construction-contract", wrong_contract)
    run_proof(wrong_contract_args, False)

    for name, contents in (
        ("empty-fdt", "fdtfile=\n"),
        ("duplicate-fdt", "fdtfile=sun50i-h618-orangepi-zero2w.dtb\nfdtfile=sun50i-h618-orangepi-zero2w.dtb\n"),
    ):
        negative = work / name
        copy_fixture_root(root, negative)
        write(negative / "boot/armbianEnv.txt", contents)
        run_proof(root_args(args, negative), False)
    for name, mutate in (
        ("config", lambda path: path.write_bytes(path.read_bytes() + b"CONFIG_BAD=y\n")),
        ("module", lambda path: path.write_bytes(path.read_bytes() + b"tampered")),
        ("status", lambda path: path.write_text(path.read_text().replace("install ok installed", "deinstall ok config-files", 1))),
    ):
        negative = work / f"negative-{name}"
        copy_fixture_root(root, negative)
        target = negative / (f"boot/config-{RELEASE}" if name == "config" else MODULE_RELATIVE if name == "module" else "var/lib/dpkg/status")
        mutate(target)
        run_proof(root_args(args, negative), False)
    for name, mutate, reason in (
        ("missing-hdmi-rsyslog", lambda path: path.unlink(), "installed HDMI rsyslog drop-in is missing or symlinked"),
        ("wrong-hdmi-rsyslog", lambda path: write(path, b"wrong\n"), "installed HDMI rsyslog drop-in differs from its canonical source"),
    ):
        negative = work / name
        copy_fixture_root(root, negative)
        mutate(negative / "etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf")
        run_proof_failure(root_args(args, negative), reason)

    audio_dtbo = root / "boot/overlay-user/octessera-ahub0-pcm5102.dtbo"
    for name, mutate, reason in (
        ("missing-audio-dtbo", lambda path: path.unlink(), "installed Orange audio DTBO is missing or symlinked"),
        ("wrong-audio-dtbo", lambda path: shutil.copyfile(root / "boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo", path), "Orange audio DTBO topology or overlay composition proof failed"),
    ):
        negative = work / name
        copy_fixture_root(root, negative)
        negative_audio_dtbo = negative / audio_dtbo.relative_to(root)
        mutate(negative_audio_dtbo)
        if name == "wrong-audio-dtbo":
            metadata = negative / "etc/octessera/build-metadata.env"
            lines = [line if not line.startswith("OCTESSERA_AHUB0_PCM5102_DTBO_SHA256=") else f"OCTESSERA_AHUB0_PCM5102_DTBO_SHA256={sha256(negative_audio_dtbo)}" for line in metadata.read_text().splitlines()]
            write(metadata, "\n".join(lines) + "\n")
        run_proof_failure(root_args(args, negative), reason)

    negative = work / "audio-peripheral-mutation"
    copy_fixture_root(root, negative)
    mutated_dts = work / "mutated-audio-peripheral.dts"
    write(mutated_dts, "/dts-v1/;\n/plugin/;\n\n&spi0 {\n\tstatus = \"disabled\";\n};\n")
    mutated_dtbo = work / "mutated-audio-peripheral.dtbo"
    subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(mutated_dtbo), str(mutated_dts)], check=True, capture_output=True)
    negative_audio_dtbo = negative / audio_dtbo.relative_to(root)
    shutil.copyfile(mutated_dtbo, negative_audio_dtbo)
    metadata = negative / "etc/octessera/build-metadata.env"
    lines = [line if not line.startswith("OCTESSERA_AHUB0_PCM5102_DTBO_SHA256=") else f"OCTESSERA_AHUB0_PCM5102_DTBO_SHA256={sha256(negative_audio_dtbo)}" for line in metadata.read_text().splitlines()]
    write(metadata, "\n".join(lines) + "\n")
    run_proof_failure(root_args(args, negative), "Merged Orange-proof tree changed /spi@5010000/status")

    negative = work / "audio-pi0-pin-claim"
    copy_fixture_root(root, negative)
    pi0_dts = work / "audio-pi0-pin-claim.dts"
    canonical_audio_dts = (REPOSITORY / "userpatches/overlay/usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts").read_text()
    write(pi0_dts, canonical_audio_dts.replace('pins = "PI1", "PI2";', 'pins = "PI0", "PI2";', 1))
    pi0_dtbo = work / "audio-pi0-pin-claim.dtbo"
    subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(pi0_dtbo), str(pi0_dts)], check=True, capture_output=True)
    negative_audio_dtbo = negative / audio_dtbo.relative_to(root)
    shutil.copyfile(pi0_dtbo, negative_audio_dtbo)
    metadata = negative / "etc/octessera/build-metadata.env"
    lines = [line if not line.startswith("OCTESSERA_AHUB0_PCM5102_DTBO_SHA256=") else f"OCTESSERA_AHUB0_PCM5102_DTBO_SHA256={sha256(negative_audio_dtbo)}" for line in metadata.read_text().splitlines()]
    write(metadata, "\n".join(lines) + "\n")
    run_proof_failure(root_args(args, negative), "Unexpected pins at")

    negative = work / "audio-i2c1-mutation"
    copy_fixture_root(root, negative)
    i2c1_dts = work / "audio-i2c1-mutation.dts"
    audio_root, audio_close = canonical_audio_dts.rsplit("\n};", 1)
    write(i2c1_dts, audio_root + "\n\tfragment@2 {\n\t\ttarget = <&i2c1>;\n\t\t__overlay__ {\n\t\t\taudio-overlay-marker = \"forbidden\";\n\t\t};\n\t};\n};" + audio_close)
    i2c1_dtbo = work / "audio-i2c1-mutation.dtbo"
    subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(i2c1_dtbo), str(i2c1_dts)], check=True, capture_output=True)
    negative_audio_dtbo = negative / audio_dtbo.relative_to(root)
    shutil.copyfile(i2c1_dtbo, negative_audio_dtbo)
    metadata = negative / "etc/octessera/build-metadata.env"
    lines = [line if not line.startswith("OCTESSERA_AHUB0_PCM5102_DTBO_SHA256=") else f"OCTESSERA_AHUB0_PCM5102_DTBO_SHA256={sha256(negative_audio_dtbo)}" for line in metadata.read_text().splitlines()]
    write(metadata, "\n".join(lines) + "\n")
    run_proof_failure(root_args(args, negative), "Merged Orange-proof tree changed properties at /i2c@5002400")

    negative = work / "rogue-stock-i2c1-overlay"
    copy_fixture_root(root, negative)
    rogue_dts = work / "rogue-stock-i2c1-pi.dts"
    write(rogue_dts, "/dts-v1/;\n/plugin/;\n\n&i2c1 {\n\tstatus = \"okay\";\n\trogue-stock-marker = \"not-from-package\";\n};\n")
    rogue_dtbo = work / "rogue-stock-i2c1-pi.dtbo"
    subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(rogue_dtbo), str(rogue_dts)], check=True, capture_output=True)
    stock_path = negative / f"boot/dtb-{RELEASE}/allwinner/overlay/sun50i-h616-i2c1-pi.dtbo"
    shutil.copyfile(rogue_dtbo, stock_path)
    rogue_merged = work / "rogue-stock-i2c1-pi-merged.dtb"
    subprocess.run(["fdtoverlay", "-i", str(negative / f"boot/dtb-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb"), "-o", str(rogue_merged), str(rogue_dtbo)], check=True, capture_output=True)
    subprocess.run(["fdtget", "-t", "s", str(rogue_merged), "/i2c@5002400", "status"], check=True, capture_output=True, text=True)
    run_proof_failure(root_args(args, negative), "installed stock i2c1-pi DTBO differs from the supplied linux-dtb package")

    for name, environment, reason in (
        ("missing-audio-token", "verbosity=1\nconsole=display\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing\noverlays=i2c1-pi\n", "Orange Armbian user_overlays assignment is not exact"),
        ("duplicate-audio-token", "verbosity=1\nconsole=display\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102 octessera-ahub0-pcm5102\noverlays=i2c1-pi\n", "Orange Armbian user_overlays assignment is not exact"),
        ("extra-user-overlay-token", "verbosity=1\nconsole=display\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102 extra\noverlays=i2c1-pi\n", "Orange Armbian user_overlays assignment is not exact"),
        ("extra-overlay-token", "verbosity=1\nconsole=display\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102\noverlays=i2c1-pi spidev1_0\n", "Orange Armbian overlays assignment is not exact"),
    ):
        negative = work / name
        copy_fixture_root(root, negative)
        write(negative / "boot/armbianEnv.txt", environment)
        run_proof_failure(root_args(args, negative), reason)

    negative = work / "masked-tty1-getty"
    copy_fixture_root(root, negative)
    (negative / "etc/systemd/system/getty@tty1.service").symlink_to("/dev/null")
    run_proof(root_args(args, negative), False)
