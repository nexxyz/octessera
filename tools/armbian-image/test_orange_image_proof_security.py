from __future__ import annotations

import os
import shutil
from pathlib import Path
from typing import Callable

from test_orange_image_proof_support import REPOSITORY, root_args, run_proof, verifier_args, write


def run_security_proof(work: Path, root: Path, image: Path, dtb: Path, evidence: Path, provenance: Path) -> None:
    args = verifier_args(root, image, dtb, evidence, provenance)
    validator = REPOSITORY / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"

    def reject_terminal_fixture(name: str, mutate: Callable[[Path], object]) -> None:
        negative = work / f"negative-terminal-{name}"
        shutil.copytree(root, negative, symlinks=True)
        mutate(negative)
        run_proof(root_args(args, negative), False)

    reject_terminal_fixture("stale-welcome", lambda path: write(path / "etc/profile.d/octessera-welcome.sh", b"stale\n"))
    reject_terminal_fixture(
        "stale-device-config-validator",
        lambda path: write(path / "usr/local/lib/octessera/device_config.py", bytes([validator.read_bytes()[0] ^ 1]) + validator.read_bytes()[1:]),
    )
    reject_terminal_fixture("device-config-validator-size", lambda path: write(path / "usr/local/lib/octessera/device_config.py", validator.read_bytes()[:-1]))
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
    reject_terminal_fixture("wrong-build-metadata-mode", lambda path: path.joinpath("etc/octessera/build-metadata.env").chmod(0o600))

    def wrong_build_metadata_owner(path: Path) -> None:
        os.chown(path / "etc/octessera/build-metadata.env", 1000, 1000)  # type: ignore[attr-defined]

    reject_terminal_fixture("wrong-build-metadata-owner", wrong_build_metadata_owner)
    reject_terminal_fixture("missing-group", lambda path: path.joinpath("etc/group").write_text(path.joinpath("etc/group").read_text().replace("octessera:x:1000:\n", "")))
    reject_terminal_fixture("duplicate-group", lambda path: path.joinpath("etc/group").write_text(path.joinpath("etc/group").read_text() + "octessera:x:1000:\n"))
    reject_terminal_fixture("wrong-group-gid", lambda path: path.joinpath("etc/group").write_text(path.joinpath("etc/group").read_text().replace("octessera:x:1000:", "octessera:x:1001:")))
    reject_terminal_fixture("pam-override", lambda path: write(path / "etc/pam.d/10-octessera", b"override\n"))
    reject_terminal_fixture("motd-override", lambda path: write(path / "etc/update-motd.d/10-octessera", b"override\n"))

    def reject_notice_fixture(name: str, mutate: Callable[[Path], object]) -> None:
        negative = work / f"negative-notice-{name}"
        shutil.copytree(root, negative, symlinks=True)
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
