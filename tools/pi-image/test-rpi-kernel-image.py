#!/usr/bin/env python3
from __future__ import annotations

import gzip
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Callable

HERE = Path(__file__).resolve().parent
KERNEL = HERE.parent / "pi-kernel"
sys.path.insert(0, str(KERNEL))
sys.path.insert(0, str(HERE))


def _load(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


CONTRACT = _load(KERNEL / "rpi_kernel_contract.py", "image_test_contract")
PACKAGE_TESTS = _load(KERNEL / "test-rpi-kernel.py", "image_test_package")
HOOK_MASK = _load(
    HERE / "stage3-octessera-kernel/files/root/usr/local/lib/octessera/raspi_firmware_hook_mask.py",
    "raspi_firmware_hook_mask",
)
INSTALLER = _load(HERE / "install-rpi-kernel.py", "image_test_installer")
STAGE_INSTALLER = _load(
    HERE / "stage3-octessera-kernel/files/root/usr/local/lib/octessera/install-rpi-kernel.py",
    "image_test_stage_installer",
)


def _expect(label: str, operation: Callable[[], Any]) -> None:
    try:
        operation()
    except (INSTALLER.ImageInstallError, STAGE_INSTALLER.ImageInstallError):
        return
    raise AssertionError(f"image fixture was accepted: {label}")


def _write(path: Path, value: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())


def _cpio_record(name: str, payload: bytes, mode: int) -> bytes:
    fields = (1, mode, 0, 0, 1, 0, len(payload), 0, 0, 0, 0, len(name) + 1, 0)
    header = b"070701" + b"".join(f"{value:08x}".encode() for value in fields)
    named = header + name.encode() + b"\0"
    return named + b"\0" * (-len(named) % 4) + payload + b"\0" * (-len(payload) % 4)


def _make_initramfs(payload: bytes) -> bytes:
    archive = _cpio_record("init", payload, 0o100755) + _cpio_record("TRAILER!!!", b"", 0)
    return gzip.compress(archive, mtime=0)


def _hook_metadata(path: Path) -> tuple[bytes, int, int, int]:
    metadata = path.stat()
    return path.read_bytes(), metadata.st_uid, metadata.st_gid, metadata.st_mode & 0o7777


def _make_hooks(root: Path) -> list[Path]:
    paths = [root / relative for relative in HOOK_MASK.RASPI_FIRMWARE_HOOKS]
    for index, path in enumerate(paths):
        _write(path, f"hook-{index}".encode())
        os.chmod(path, 0o751 - index * 0o100)
    return paths


def _expect_hook_rejection(work: Path, label: str, mutate: Callable[[Path], None]) -> None:
    root = work / f"hook-mask-{label}"
    paths = _make_hooks(root)
    mutate(paths[1])
    try:
        with STAGE_INSTALLER.temporarily_mask_raspi_firmware_hooks(root):
            raise AssertionError(f"invalid Raspberry firmware hook accepted: {label}")
    except ValueError as error:
        assert str(paths[1]) in str(error)
    assert paths[0].stat().st_mode & 0o7777 == 0o751


def _test_transactional_hook_mask(work: Path) -> None:
    root = work / "hook-mask"
    paths = _make_hooks(root)
    original = [_hook_metadata(path) for path in paths]
    with STAGE_INSTALLER.temporarily_mask_raspi_firmware_hooks(root):
        for path, state in zip(paths, original):
            assert _hook_metadata(path) == (state[0], state[1], state[2], state[3] & ~0o111)
    assert [_hook_metadata(path) for path in paths] == original

    original_run = STAGE_INSTALLER._run_in_root

    def fail_dpkg(rootfs: Path, command: list[str]) -> None:
        assert command[:2] == ["dpkg", "-i"]
        for path in paths:
            assert path.stat().st_mode & 0o111 == 0
        paths[0].write_bytes(b"dpkg changed this hook")
        os.chmod(paths[0], 0o600)
        paths[1].unlink()
        raise RuntimeError("synthetic dpkg failure")

    STAGE_INSTALLER._run_in_root = fail_dpkg
    try:
        try:
            STAGE_INSTALLER._install_package(root, work / "package.deb")
        except RuntimeError as error:
            assert str(error) == "synthetic dpkg failure"
        else:
            raise AssertionError("synthetic dpkg failure was not raised")
    finally:
        STAGE_INSTALLER._run_in_root = original_run
    assert [_hook_metadata(path) for path in paths] == original

    _expect_hook_rejection(work, "missing", lambda path: path.unlink())
    _expect_hook_rejection(work, "non-executable", lambda path: os.chmod(path, 0o644))

    def make_symlink(path: Path) -> None:
        path.unlink()
        path.symlink_to(path.parent / "other-hook")

    _expect_hook_rejection(work, "symlink", make_symlink)

    def make_directory(path: Path) -> None:
        path.unlink()
        path.mkdir()

    _expect_hook_rejection(work, "directory", make_directory)

    failure_root = work / "hook-mask-dual-failure"
    failure_paths = _make_hooks(failure_root)

    def fail_dpkg_with_broken_hooks(rootfs: Path, command: list[str]) -> None:
        assert command[:2] == ["dpkg", "-i"]
        failure_paths[0].unlink()
        failure_paths[0].mkdir()
        failure_paths[1].unlink()
        failure_paths[1].symlink_to(failure_paths[0])
        raise RuntimeError("synthetic dpkg failure")

    original_run = STAGE_INSTALLER._run_in_root
    STAGE_INSTALLER._run_in_root = fail_dpkg_with_broken_hooks
    try:
        try:
            STAGE_INSTALLER._install_package(failure_root, work / "package.deb")
        except ValueError as error:
            message = str(error)
            assert "synthetic dpkg failure" in message
            assert str(failure_paths[0]) in message
            assert str(failure_paths[1]) in message
            assert isinstance(error.__cause__, RuntimeError)
        else:
            raise AssertionError("dual hook-mask failure was not raised")
    finally:
        STAGE_INSTALLER._run_in_root = original_run
    assert failure_paths[0].is_dir()
    assert failure_paths[1].is_symlink()


def _assert_selectors(config: Path, contract: Any) -> None:
    lines = [line.strip() for line in config.read_text(encoding="utf-8").splitlines()]
    for expected in (
        "kernel=octessera/kernel8.img",
        "device_tree=octessera/bcm2710-rpi-zero-2-w.dtb",
        f"initramfs octessera/initrd.img-{contract.kernel_release} followkernel",
        "overlay_prefix=octessera/overlays/",
    ):
        assert lines.count(expected) == 1, expected
    assert not any(line.startswith("auto_initramfs=") for line in lines)
    assert len([line for line in lines if line.startswith("kernel=")]) == 1


def _main() -> int:
    contract = CONTRACT.load_contract(HERE.parents[1])
    image_contract = STAGE_INSTALLER.image_contract()
    assert image_contract.package_filename == contract.package_filename
    assert image_contract.kernel_release == contract.kernel_release
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-image-test-") as temporary:
        work = Path(temporary)
        (work / "package.deb").write_bytes(b"package")
        _test_transactional_hook_mask(work)
        package = PACKAGE_TESTS._make_package(work, contract, "image", compressed_kernel=True)
        PACKAGE_TESTS.VALIDATOR.validate_package(package, contract)

        image = work / "root"
        _write(image / "boot/firmware/config.txt", "kernel=kernel8.img\nauto_initramfs=1\n")
        _write(image / "boot/firmware/kernel8.img", b"stock-kernel")
        stock_initrd_path = image / f"boot/initrd.img-{contract.kernel_release}"
        stock_initrd_original = _make_initramfs(b"stock-initramfs")
        stock_initrd_regenerated = _make_initramfs(b"regenerated-stock-initramfs")
        _write(stock_initrd_path, stock_initrd_original)
        image_hooks = _make_hooks(image)
        image_hook_original = [_hook_metadata(path) for path in image_hooks]
        original_install = STAGE_INSTALLER._install_package
        STAGE_INSTALLER._install_package = lambda rootfs, value: value
        try:
            STAGE_INSTALLER.install_image(image, package, contract)
        finally:
            STAGE_INSTALLER._install_package = original_install
        boot = image / "boot/firmware"
        original_root_command = STAGE_INSTALLER._run_in_root

        def fake_root_command(rootfs: Path, command: list[str]) -> None:
            if command[0] == "update-initramfs":
                _write(rootfs / f"boot/initrd.img-{contract.kernel_release}", stock_initrd_regenerated)

        STAGE_INSTALLER._run_in_root = fake_root_command
        try:
            STAGE_INSTALLER._finalize(image, package, contract)
        finally:
            STAGE_INSTALLER._run_in_root = original_root_command
        assert (boot / f"octessera/initrd.img-{contract.kernel_release}").read_bytes() == stock_initrd_regenerated
        assert (boot / "octessera/kernel8.img").read_bytes() == PACKAGE_TESTS._arm64_image()
        assert [_hook_metadata(path) for path in image_hooks] == image_hook_original
        _assert_selectors(boot / "config.txt", contract)
        stock_entries = json.loads((boot / "octessera/recovery-stock/manifest.json").read_text(encoding="utf-8"))
        stock_initrd_entry = next(entry for entry in stock_entries if entry["path"] == f"boot/initrd.img-{contract.kernel_release}")
        assert (image / stock_initrd_entry["recovery_path"]).read_bytes() == stock_initrd_original

        final_root = work / "actual-finalizer-root"
        final_boot = final_root / "boot/firmware"
        hardware_block = "# --- octessera additions ---\n" + (HERE / "stage4-octessera/files/boot/config.txt.append").read_text(encoding="utf-8")
        _write(
            final_boot / "config.txt",
            "kernel=old.img\nkernel=conflict.img\nauto_initramfs=1\ndtoverlay=i2s-dac-no20\n\n" + hardware_block,
        )
        final_hooks = _make_hooks(final_root)
        final_hook_original = [_hook_metadata(path) for path in final_hooks]
        final_artifacts = final_root / "var/lib/octessera/rpi-kernel"
        final_artifacts.mkdir(parents=True)
        shutil.copy2(package, final_artifacts / package.name)
        final_lib = final_root / "usr/local/lib/octessera"
        final_lib.mkdir(parents=True)
        finalizer = HERE / "stage3-octessera-kernel/files/root/usr/local/sbin/octessera-finalize-rpi-kernel"
        final_sbin = final_root / "usr/local/sbin"
        final_sbin.mkdir(parents=True)
        shutil.copy2(finalizer, final_sbin / "octessera-finalize-rpi-kernel")
        for helper in ("install-rpi-kernel.py", "rpi_kernel_image.py", "raspi_firmware_hook_mask.py"):
            shutil.copy2(finalizer.parent.parent / "lib/octessera" / helper, final_lib / helper)
        fake_bin = work / "fake-bin"
        fake_bin.mkdir()
        fake_chroot = fake_bin / "chroot"
        fake_script = (
            "#!/bin/sh\nset -eu\n"
            "root=\"$1\"\n"
            "case \"$2\" in\n"
            "  depmod) ;;\n"
            "  update-initramfs)"
            "    for hook in \"$root/etc/initramfs/post-update.d/z50-raspi-firmware\" \"$root/etc/kernel/postinst.d/z50-raspi-firmware\"; do"
            "      if [ -x \"$hook\" ]; then printf '%s\\n' '# raspi-firmware regenerated config' > \"$root/boot/firmware/config.txt\"; fi"
            "    done\n"
            f"    mkdir -p \"$root/boot\"; printf '%s\\n' fixture > \"$root/boot/initrd.img-{contract.kernel_release}\" ;;\n"
            "  *) exit 1 ;;\n"
            "esac\n"
        )
        _write(fake_chroot, fake_script)
        os.chmod(fake_chroot, 0o755)
        final_environment = os.environ.copy()
        final_environment["OCTESSERA_KERNEL_ROOTFS"] = str(final_root)
        final_environment["PATH"] = str(fake_bin) + os.pathsep + final_environment["PATH"]
        try:
            subprocess.run(["bash", str(finalizer)], env=final_environment, check=True, capture_output=True, text=True)
        except subprocess.CalledProcessError as error:
            raise AssertionError(error.stderr) from error
        assert [_hook_metadata(path) for path in final_hooks] == final_hook_original
        assert hardware_block in (final_boot / "config.txt").read_text(encoding="utf-8")
        _assert_selectors(final_boot / "config.txt", contract)
        assert (final_boot / f"octessera/initrd.img-{contract.kernel_release}").is_file()

        _expect("missing package", lambda: STAGE_INSTALLER.install_image(image, work / "missing.deb", contract))
    print("Raspberry kernel image synthetic tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
