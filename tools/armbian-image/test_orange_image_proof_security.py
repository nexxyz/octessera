from __future__ import annotations

import os
from pathlib import Path
from typing import Callable

from test_orange_image_proof_support import REPOSITORY, copy_fixture_root, root_args, run_proof, verifier_args, write


def run_security_proof(work: Path, root: Path, image: Path, dtb: Path, evidence: Path, manifest: Path) -> None:
    args = verifier_args(root, image, dtb, evidence, manifest=manifest)
    validator = REPOSITORY / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"

    def reject_terminal_fixture(name: str, mutate: Callable[[Path], object]) -> None:
        negative = work / f"negative-terminal-{name}"
        copy_fixture_root(root, negative)
        mutate(negative)
        run_proof(root_args(args, negative), False)

    reject_terminal_fixture("stale-welcome", lambda path: write(path / "etc/profile.d/octessera-welcome.sh", b"stale\n"))
    reject_terminal_fixture(
        "stale-device-config-validator",
        lambda path: write(path / "usr/local/lib/octessera/device_config.py", bytes([validator.read_bytes()[0] ^ 1]) + validator.read_bytes()[1:]),
    )
    reject_terminal_fixture("device-config-validator-size", lambda path: write(path / "usr/local/lib/octessera/device_config.py", validator.read_bytes()[:-1]))
    reject_terminal_fixture("wrong-build-metadata-mode", lambda path: path.joinpath("etc/octessera/build-metadata.env").chmod(0o600))

    def wrong_build_metadata_owner(path: Path) -> None:
        os.chown(path / "etc/octessera/build-metadata.env", 1000, 1000)  # type: ignore[attr-defined]

    reject_terminal_fixture("wrong-build-metadata-owner", wrong_build_metadata_owner)
    reject_terminal_fixture("unrestricted-sudoers", lambda path: write(path / "etc/sudoers", b"octessera ALL=(ALL) NOPASSWD: ALL\n"))
    reject_terminal_fixture("unrestricted-sudoers-dropin", lambda path: write(path / "etc/sudoers.d/negative", b"octessera ALL=(ALL) NOPASSWD: ALL\n"))

    def reject_notice_fixture(name: str, mutate: Callable[[Path], object]) -> None:
        negative = work / f"negative-notice-{name}"
        copy_fixture_root(root, negative)
        mutate(negative)
        run_proof(root_args(args, negative), False)

    reject_notice_fixture("stale", lambda path: write(path / "usr/share/doc/octessera/LICENSE", b"stale\n"))
    reject_notice_fixture("missing", lambda path: (path / "usr/share/doc/octessera/LICENSE").unlink())
    reject_notice_fixture("extra", lambda path: write(path / "usr/share/doc/octessera/extra.txt", b"extra\n"))
    reject_notice_fixture("mode", lambda path: path.joinpath("usr/share/doc/octessera/LICENSE").chmod(0o600))

    def symlink_notice(path: Path) -> None:
        (path / "usr/share/doc/octessera/LICENSE").unlink()
        (path / "usr/share/doc/octessera/LICENSE").symlink_to("/dev/null")

    reject_notice_fixture("symlink", symlink_notice)
