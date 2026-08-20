from __future__ import annotations

import shutil
from pathlib import Path

from test_orange_image_proof_support import (
    REPOSITORY,
    RELEASE,
    MODULE_RELATIVE,
    replace_option,
    root_args,
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
