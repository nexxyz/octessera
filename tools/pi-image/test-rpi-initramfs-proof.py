#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import os
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[2]


def _load(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


PROOF = _load(ROOT / "tools/pi-image/verify-rpi-kernel-image.py", "rpi_initramfs_proof_test")
FIXTURES = _load(ROOT / "tools/pi-image/rpi_initramfs_fixture.py", "rpi_initramfs_fixture_test")
HELPER = _load(ROOT / "tools/pi-image/rpi_initramfs_proof.py", "rpi_initramfs_proof_helper_test")


def _expect_rejected(operation: Callable[[], None], label: str) -> None:
    try:
        operation()
    except (PROOF.ImageProofError, ValueError):
        return
    raise AssertionError(f"initramfs proof accepted {label}")


def _listing_line(name: str, mode: str = "-rwxr-xr-x", links: int = 1, size: int = 1, target: str | None = None) -> str:
    entry = name if target is None else f"{name} -> {target}"
    return f"{mode} {links} root root {size} Jan 1 1970 {entry}"


def _canonical_listing(script: bytes, runtime: bytes) -> str:
    lines = [
        _listing_line("scripts/init-premount/octessera-boot-splash", size=len(script)),
        _listing_line("usr/local/bin/octessera-pi", size=len(runtime)),
        _listing_line("bin", mode="lrwxrwxrwx", size=7, target="usr/bin"),
        _listing_line("usr/bin/sh", mode="lrwxrwxrwx", size=4, target="dash"),
    ]
    lines.extend(_listing_line(name, size=len(payload)) for name, payload, _, _ in FIXTURES.canonical_command_records()[2:])
    lines.extend(
        (
            _listing_line("lib/modules/fixture/kernel/drivers/spi/spi-bcm2835.ko"),
            _listing_line("lib/modules/fixture/kernel/drivers/spi/spidev.ko"),
        )
    )
    return "\n".join(lines)


def _replace_command_record(
    records: tuple[tuple[str, bytes, int, int], ...],
    name: str,
    payload: bytes | None = None,
    mode: int | None = None,
    links: int | None = None,
) -> tuple[tuple[str, bytes, int, int], ...]:
    return tuple(
        (record[0], payload if payload is not None else record[1], mode if mode is not None else record[2], links if links is not None else record[3])
        if record[0] == name
        else record
        for record in records
    )


def _remove_path(path: Path) -> None:
    if path.is_symlink() or path.is_file():
        path.unlink()
    elif path.is_dir():
        shutil.rmtree(path)


def main() -> int:
    script = (ROOT / "tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash").read_bytes()
    runtime = b"current-runtime-bundle\n"
    contract = PROOF._load_boot_layer_contract()
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-initramfs-proof-test-") as temporary:
        root = Path(temporary) / "root"
        script_path = root / "etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
        runtime_path = root / "opt/octessera/releases/1.2.3/octessera-pi"
        script_path.parent.mkdir(parents=True)
        script_path.write_bytes(script)
        os.chmod(script_path, 0o755)
        runtime_path.parent.mkdir(parents=True)
        runtime_path.write_bytes(runtime)
        os.chmod(runtime_path, 0o755)
        (root / "opt/octessera/current").symlink_to("/opt/octessera/releases/1.2.3")
        (root / "usr/local/bin").mkdir(parents=True)
        (root / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
        initramfs = root / "initramfs.img"
        initramfs.write_bytes(FIXTURES.make_splash_initramfs(script, runtime))
        PROOF._verify_selected_initramfs_entries(initramfs, contract, root)

        canonical_listing = _canonical_listing(script, runtime)
        records = PROOF.parse_initramfs_listing(canonical_listing)
        HELPER.validate_command_records(records, contract["selected_initramfs"])
        canonical_records = FIXTURES.canonical_command_records()
        runtime_link = root / "usr/local/bin/octessera-pi"
        current_link = root / "opt/octessera/current"
        original_current_target = os.readlink(current_link)

        def expect_runtime_variant(label: str, target: str, prepare: Callable[[Path], None] | None = None) -> None:
            release = root / "opt/octessera/releases/9.8.7"
            _remove_path(release)
            if prepare is not None:
                prepare(release)
            current_link.unlink()
            current_link.symlink_to(target)
            try:
                _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), label)
            finally:
                current_link.unlink()
                current_link.symlink_to(original_current_target)
                _remove_path(release)

        expect_runtime_variant("releases/latest runtime link", "/opt/octessera/releases/latest")
        expect_runtime_variant("traversing runtime release link", "../outside")

        def make_loop(release: Path) -> None:
            release.mkdir(parents=True)
            (release / "octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")

        expect_runtime_variant("looped resolved runtime", "/opt/octessera/releases/9.8.7", make_loop)

        outside = root.parent / "outside-runtime"
        outside.write_bytes(runtime)

        def make_outside(release: Path) -> None:
            release.mkdir(parents=True)
            (release / "octessera-pi").symlink_to(outside)

        expect_runtime_variant("outside-root resolved runtime", "/opt/octessera/releases/9.8.7", make_outside)
        outside.unlink()

        def make_non_regular(release: Path) -> None:
            (release / "octessera-pi").mkdir(parents=True)

        expect_runtime_variant("non-regular resolved runtime", "/opt/octessera/releases/9.8.7", make_non_regular)

        def make_non_executable(release: Path) -> None:
            release.mkdir(parents=True)
            binary = release / "octessera-pi"
            binary.write_bytes(runtime)
            os.chmod(binary, 0o644)

        expect_runtime_variant("non-executable resolved runtime", "/opt/octessera/releases/9.8.7", make_non_executable)

        def make_hardlink(release: Path) -> None:
            release.mkdir(parents=True)
            peer = root / "opt/octessera/hardlink-peer"
            peer.write_bytes(runtime)
            os.link(peer, release / "octessera-pi")

        expect_runtime_variant("hardlinked resolved runtime", "/opt/octessera/releases/9.8.7", make_hardlink)
        runtime_link.unlink()
        runtime_link.symlink_to("/tmp/outside-runtime")
        _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), "unsafe runtime link")
        runtime_link.unlink()
        runtime_link.symlink_to("/opt/octessera/current/octessera-pi")
        trailing_listing = canonical_listing + "\n" + "\n".join(
            _listing_line(f"usr/lib/fixture-trailing/{index:04d}") for index in range(8192)
        )
        assert "usr/bin/sh" in {record["name"] for record in PROOF.parse_initramfs_listing(trailing_listing)}

        for executable in contract["selected_initramfs"]["required_regular_executables"]:
            initramfs.write_bytes(
                FIXTURES.make_splash_initramfs(
                    script,
                    runtime,
                    command_records=tuple(record for record in canonical_records if record[0] != executable),
                )
            )
            _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), f"missing {executable}")

        for symlink, target in (("bin", "wrong"), ("usr/bin/sh", "../sh")):
            initramfs.write_bytes(
                FIXTURES.make_splash_initramfs(
                    script,
                    runtime,
                    command_records=_replace_command_record(canonical_records, symlink, target.encode(), 0o120777),
                )
            )
            _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), f"wrong {symlink} target")

        for label, payload, mode, links in (
            ("zero command payload", b"", 0o100755, 1),
            ("non-executable command", b"fixture", 0o100644, 1),
            ("non-regular command", b"", 0o040755, 1),
            ("device command", b"", 0o020666, 1),
            ("hardlink command", b"fixture", 0o100755, 2),
        ):
            initramfs.write_bytes(
                FIXTURES.make_splash_initramfs(
                    script,
                    runtime,
                    command_records=_replace_command_record(canonical_records, "usr/bin/sleep", payload, mode, links),
                )
            )
            _expect_rejected(lambda label=label: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), label)

        altered = FIXTURES.make_splash_initramfs(
            script,
            runtime,
            command_records=_replace_command_record(canonical_records, "bin", b"wrong", 0o120777),
        )
        initramfs.write_bytes(altered)
        original_listing = PROOF._run_lsinitramfs
        PROOF._run_lsinitramfs = lambda _: canonical_listing
        try:
            _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), "extraction-backed symlink mutation")
        finally:
            PROOF._run_lsinitramfs = original_listing

        initramfs.write_bytes(
            FIXTURES.make_splash_initramfs(
                script,
                runtime,
                command_records=_replace_command_record(canonical_records, "usr/bin/sleep", b"", 0o100755),
            )
        )
        PROOF._run_lsinitramfs = lambda _: canonical_listing
        try:
            _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), "extraction-backed zero payload")
        finally:
            PROOF._run_lsinitramfs = original_listing

        for label, replacement in (
            ("wrong symlink", ("bin", "lrwxrwxrwx", 1, 7, "wrong")),
            ("absolute symlink", ("bin", "lrwxrwxrwx", 1, 8, "/usr/bin")),
            ("escaping symlink", ("bin", "lrwxrwxrwx", 1, 10, "../usr/bin")),
            ("cyclic symlink", ("bin", "lrwxrwxrwx", 1, 3, "bin")),
        ):
            name, mode, links, size, target = replacement
            altered_listing = canonical_listing.replace(
                _listing_line(name, mode="lrwxrwxrwx", size=7, target="usr/bin"),
                _listing_line(name, mode=mode, links=links, size=size, target=target),
            )
            _expect_rejected(
                lambda altered_listing=altered_listing: HELPER.validate_command_records(HELPER.parse_initramfs_listing(altered_listing), contract["selected_initramfs"]),
                f"{label} command target",
            )

        for label, altered_listing in (
            (
                "legacy regular bin command",
                canonical_listing + "\n" + _listing_line("bin/sleep"),
            ),
            (
                "missing command target",
                canonical_listing.replace(_listing_line("usr/bin/dash", size=7) + "\n", ""),
            ),
            (
                "non-executable command target",
                canonical_listing.replace(_listing_line("usr/bin/sleep", size=7), _listing_line("usr/bin/sleep", mode="-rw-r--r--", size=7)),
            ),
            (
                "non-regular command target",
                canonical_listing.replace(_listing_line("usr/bin/sleep", size=7), _listing_line("usr/bin/sleep", mode="drwxr-xr-x", size=0)),
            ),
            (
                "device command target",
                canonical_listing.replace(_listing_line("usr/bin/sleep", size=7), _listing_line("usr/bin/sleep", mode="crw-rw-rw-", size=0)),
            ),
            (
                "hardlink command target",
                canonical_listing.replace(_listing_line("usr/bin/sleep", size=7), _listing_line("usr/bin/sleep", links=2, size=7)),
            ),
            (
                "oversized command target",
                canonical_listing.replace(_listing_line("usr/bin/sleep", size=7), _listing_line("usr/bin/sleep", size=67108865)),
            ),
        ):
            _expect_rejected(
                lambda altered_listing=altered_listing: HELPER.validate_command_records(HELPER.parse_initramfs_listing(altered_listing), contract["selected_initramfs"]),
                label,
            )

        duplicate_listing = canonical_listing + "\n" + _listing_line("usr/bin/sleep", size=7)
        _expect_rejected(
            lambda: PROOF.parse_initramfs_listing(duplicate_listing),
            "duplicate command target",
        )

        initramfs.write_bytes(
            FIXTURES.make_splash_initramfs(
                script,
                runtime,
                "etc/initramfs-tools/scripts/init-premount/octessera-boot-splash",
            )
        )
        _expect_rejected(
            lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root),
            "obsolete initramfs-tools archive path",
        )

        stale_script = script.replace(b"sleep 3", b"sleep 2", 1)
        initramfs.write_bytes(FIXTURES.make_splash_initramfs(stale_script, runtime))
        _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), "stale initramfs script")

        stale_runtime = b"stale-runtime-bundle\n"
        initramfs.write_bytes(FIXTURES.make_splash_initramfs(script, stale_runtime))
        _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), "stale initramfs binary")

        initramfs.write_bytes(
            FIXTURES.make_splash_initramfs(
                script,
                runtime,
                runtime_record=(b"/opt/octessera/current/octessera-pi", 0o120777, 1),
            )
        )
        _expect_rejected(lambda: PROOF._verify_selected_initramfs_entries(initramfs, contract, root), "archive runtime symlink")

        entry = "scripts/init-premount/octessera-boot-splash"
        for label, record in (
            ("symlink", f"lrwxrwxrwx 1 root root 6 Jan 1 1970 {entry} -> target"),
            ("hardlink", f"-rwxr-xr-x 2 root root 6 Jan 1 1970 {entry}"),
            ("device", f"crw-rw-rw- 1 root root 0 Jan 1 1970 {entry}"),
            ("oversized", f"-rwxr-xr-x 1 root root 67108865 Jan 1 1970 {entry}"),
        ):
            _expect_rejected(
                lambda record=record: PROOF.extract_regular_files(initramfs, [entry], lambda _: record, (), contract["selected_initramfs"]),
                f"{label} initramfs entry",
            )
        _expect_rejected(
            lambda: PROOF.extract_regular_files(initramfs, [entry], lambda _: f"{record}\n{record}", (), contract["selected_initramfs"]),
            "duplicate initramfs entry",
        )
    print("Raspberry initramfs rootfs-byte binding tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
