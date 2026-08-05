#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import os
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


def _expect_rejected(operation: Callable[[], None], label: str) -> None:
    try:
        operation()
    except (PROOF.ImageProofError, ValueError):
        return
    raise AssertionError(f"initramfs proof accepted {label}")


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
        runtime_path.parent.mkdir(parents=True)
        runtime_path.write_bytes(runtime)
        os.chmod(runtime_path, 0o755)
        (root / "opt/octessera/current").symlink_to("/opt/octessera/releases/1.2.3")
        (root / "usr/local/bin").mkdir(parents=True)
        (root / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
        initramfs = root / "initramfs.img"
        initramfs.write_bytes(FIXTURES.make_splash_initramfs(script, runtime))
        PROOF._verify_selected_initramfs_entries(initramfs, contract, root)

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

        entry = "scripts/init-premount/octessera-boot-splash"
        for label, record in (
            ("symlink", f"lrwxrwxrwx 1 root root 6 Jan 1 1970 {entry} -> target"),
            ("hardlink", f"-rwxr-xr-x 2 root root 6 Jan 1 1970 {entry}"),
            ("device", f"crw-rw-rw- 1 root root 0 Jan 1 1970 {entry}"),
            ("oversized", f"-rwxr-xr-x 1 root root 67108865 Jan 1 1970 {entry}"),
        ):
            _expect_rejected(
                lambda record=record: PROOF.extract_regular_files(initramfs, [entry], lambda _: record),
                f"{label} initramfs entry",
            )
        _expect_rejected(
            lambda: PROOF.extract_regular_files(initramfs, [entry], lambda _: f"{record}\n{record}"),
            "duplicate initramfs entry",
        )
    print("Raspberry initramfs rootfs-byte binding tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
