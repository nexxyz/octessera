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
from types import SimpleNamespace
from typing import Any, Callable

HERE = Path(__file__).resolve().parent
KERNEL = HERE.parent / "pi-kernel"
sys.path.insert(0, str(KERNEL))
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "legal"))

from rpi_initramfs_fixture import make_splash_initramfs
from stage_notices import stage_notices  # type: ignore[import-not-found]


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
PROOF = _load(HERE / "verify-rpi-kernel-image.py", "image_test_proof")
STAGE_INSTALLER = _load(
    HERE / "stage3-octessera-kernel/files/root/usr/local/lib/octessera/install-rpi-kernel.py",
    "image_test_stage_installer",
)


def _expect(label: str, operation: Callable[[], Any]) -> None:
    try:
        operation()
    except (INSTALLER.ImageInstallError, STAGE_INSTALLER.ImageInstallError, PROOF.ImageProofError):
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


def _main() -> int:
    root = HERE.parents[1]
    contract = CONTRACT.load_contract(root)
    assert STAGE_INSTALLER.image_contract().package_filename == contract.package_filename
    assert STAGE_INSTALLER.image_contract().package_name == contract.package_name
    assert STAGE_INSTALLER.image_contract().package_version == contract.package_version
    assert STAGE_INSTALLER.image_contract().kernel_release == contract.kernel_release
    assert STAGE_INSTALLER.image_contract().required_modules == contract.required_modules
    original_run = PROOF.subprocess.run

    def fake_lsblk(command: list[str], **kwargs: Any) -> Any:
        if command[0] == "lsblk":
            assert command[command.index("--output") + 1] == "NAME,TYPE"
            return SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": [{"name": "/dev/loop0p2", "type": "part"}, {"name": "/dev/loop0p1", "type": "part"}]}]}))
        return original_run(command, **kwargs)
    PROOF.subprocess.run = fake_lsblk
    try:
        assert PROOF._expected_partitions("/dev/loop0") == ("/dev/loop0p1", "/dev/loop0p2")
    finally:
        PROOF.subprocess.run = original_run
    for children in (
        [{"name": "/dev/loop0p3", "type": "part"}, {"name": "/dev/loop0p2", "type": "part"}],
        [{"name": "/dev/loop0p1", "type": "part"}, {"name": "/dev/loop0p2", "type": "part"}, {"name": "/dev/loop0p3", "type": "part"}],
    ):
        PROOF.subprocess.run = lambda command, **_: SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": children}]}))
        try:
            try:
                PROOF._expected_partitions("/dev/loop0")
            except PROOF.ImageProofError:
                pass
            else:
                raise AssertionError("invalid fixed partition layout was accepted")
        finally:
            PROOF.subprocess.run = original_run
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-mount-test-") as temporary:
        image_path = Path(temporary) / "image.img"
        image_path.write_bytes(b"image")
        mount_commands: list[list[str]] = []

        def fake_mount_run(command: list[str], **_: Any) -> Any:
            mount_commands.append(command)
            if command[0] == "losetup" and "--show" in command:
                return SimpleNamespace(stdout="/dev/loop0\n")
            if command[0] == "lsblk":
                partitions = [{"name": "/dev/loop0p1", "type": "part"}, {"name": "/dev/loop0p2", "type": "part"}]
                return SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": partitions}]}))
            return SimpleNamespace(stdout="")
        PROOF.subprocess.run = fake_mount_run
        try:
            with PROOF._mounted_image(image_path):
                pass
        finally:
            PROOF.subprocess.run = original_run
        assert any(command[:5] == ["mount", "-t", "vfat", "-o", "ro"] for command in mount_commands)
        assert any(command[:5] == ["mount", "-t", "ext4", "-o", "ro,noload"] for command in mount_commands)
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-image-test-") as temporary:
        work = Path(temporary)
        (work / "package.deb").write_bytes(b"package")
        _test_transactional_hook_mask(work)
        package = PACKAGE_TESTS._make_package(work, contract, "image", compressed_kernel=True)
        checksum = work / "SHA256SUMS"
        checksum.write_text(f"{STAGE_INSTALLER.sha256_file(package)}  {package.name}\n", encoding="utf-8")
        validator = _load(KERNEL / "validate-rpi-kernel-package.py", "image_test_validator")
        inventory = validator.validate_package(package, contract)
        provenance = dict(inventory)
        provenance["build"] = PACKAGE_TESTS._build_provenance(root, contract, inventory)
        provenance_path = work / "image-provenance.json"
        provenance_path.write_text(json.dumps(provenance), encoding="utf-8")
        image = work / "root"
        stage_notices(HERE.parents[1], image)
        _write(image / "var/lib/dpkg/status", "Package: fixture\nStatus: install ok installed\nVersion: 1\nDescription: fixture\n")
        _write(image / "usr/share/common-licenses/GPL-3", "fixture GPL license\n")
        _write(image / "usr/share/doc/base-files/copyright", "fixture base-files copyright\n")
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
            STAGE_INSTALLER.install_image(image, package, checksum, provenance_path, contract, finalize=False)
        finally:
            STAGE_INSTALLER._install_package = original_install
        boot = image / "boot/firmware"
        _write(boot / "octessera/overlays/i2s-dac-no20.dtbo", b"i2s")
        subprocess.run(["dpkg-deb", "-x", str(package), str(image)], check=True, capture_output=True)
        _write(image / f"lib/modules/{contract.kernel_release}/modules.dep", "fixture-module-dependencies\n")
        runtime_bytes = b"constructor-runtime-binary\n"
        _write(image / "opt/octessera/releases/1.2.3/octessera-pi", runtime_bytes)
        os.chmod(image / "opt/octessera/releases/1.2.3/octessera-pi", 0o755)
        (image / "opt/octessera/current").symlink_to("/opt/octessera/releases/1.2.3")
        (image / "usr/local/bin").mkdir(parents=True, exist_ok=True)
        (image / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
        for path in (
            image / "etc/initramfs-tools/hooks/octessera-boot-splash",
            image / "etc/initramfs-tools/scripts/init-premount/octessera-boot-splash",
            image / "etc/systemd/system/octessera-boot-splash.service",
            image / "etc/systemd/system/octessera.service",
            image / "etc/profile.d/octessera-welcome.sh",
            image / "usr/local/sbin/octessera-usb-gadget",
        ):
            source = HERE / "stage4-octessera/files/root" / path.relative_to(image)
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, path)
            os.chmod(path, 0o755 if "initramfs" in str(path) or path.name == "octessera-usb-gadget" else 0o644)
        default_config = image / "home/pi/presets/default.json"
        default_config.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(HERE.parents[1] / "config/generated/pi/default.json", default_config)
        os.chmod(default_config, 0o644)
        os.chown(default_config, 1000, 1000)  # type: ignore[attr-defined]
        validator_path = image / "usr/local/lib/octessera/device_config.py"
        validator_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(HERE / "stage4-octessera/files/root/usr/local/lib/octessera/device_config.py", validator_path)
        os.chmod(validator_path, 0o644)
        os.chown(validator_path, 0, 0)  # type: ignore[attr-defined]
        hushlogin = image / "home/pi/.hushlogin"
        hushlogin.parent.mkdir(parents=True, exist_ok=True)
        hushlogin.write_bytes(b"")
        os.chmod(hushlogin, 0o644)
        os.chown(hushlogin, 1000, 1000)  # type: ignore[attr-defined]
        (image / "etc/systemd/system/sysinit.target.wants").mkdir(parents=True, exist_ok=True)
        (image / "etc/systemd/system/multi-user.target.wants").mkdir(parents=True, exist_ok=True)
        (image / "etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service").symlink_to("../octessera-boot-splash.service")
        (image / "etc/systemd/system/multi-user.target.wants/octessera.service").symlink_to("../octessera.service")
        original_root_command = STAGE_INSTALLER._run_in_root
        def fake_root_command(rootfs: Path, command: list[str]) -> None:
            if command[0] == "update-initramfs":
                _write(rootfs / f"boot/initrd.img-{contract.kernel_release}", stock_initrd_regenerated)
        STAGE_INSTALLER._run_in_root = fake_root_command
        try:
            evidence = STAGE_INSTALLER._finalize(image, package, checksum, provenance_path, contract)
        finally:
            STAGE_INSTALLER._run_in_root = original_root_command
        assert evidence["initramfs"]["path"] == f"octessera/initrd.img-{contract.kernel_release}"
        assert [_hook_metadata(path) for path in image_hooks] == image_hook_original
        STAGE_INSTALLER.verify_selectors(boot / "config.txt", contract)
        script_bytes = (image / "etc/initramfs-tools/scripts/init-premount/octessera-boot-splash").read_bytes()
        _write(boot / f"octessera/initrd.img-{contract.kernel_release}", make_splash_initramfs(script_bytes, runtime_bytes))
        initramfs_path = boot / f"octessera/initrd.img-{contract.kernel_release}"
        original_lsinitramfs = PROOF._run_lsinitramfs
        lsinitramfs_paths: list[Path] = []

        def fake_lsinitramfs(path: Path) -> str:
            lsinitramfs_paths.append(path)
            if path.resolve() == initramfs_path.resolve():
                return original_run(["lsinitramfs", "-l", str(path)], capture_output=True, text=True, check=True).stdout
            return "stock\n"

        PROOF._run_lsinitramfs = fake_lsinitramfs
        try:
            if getattr(os, "geteuid", lambda: -1)() == 0:
                proved = PROOF.prove_root(image, package, checksum, provenance_path, contract)
            else:
                proved = None
                PROOF._verify_selected_initramfs(boot, initramfs_path)
                PROOF._verify_stock_recovery(image, boot)
        finally:
            PROOF._run_lsinitramfs = original_lsinitramfs
        if proved is not None:
            assert proved["package"]["sha256"] == inventory["package"]["sha256"]
            assert proved["boot_layer"]["device_config_validator"]["size"] == validator_path.stat().st_size
            composer_path = image / "usr/local/sbin/octessera-usb-gadget"
            assert proved["boot_layer"]["usb_gadget_composer"]["size"] == composer_path.stat().st_size
            validator_bytes = validator_path.read_bytes()
            validator_path.write_bytes(bytes([validator_bytes[0] ^ 1]) + validator_bytes[1:])
            _expect("stale installed device config validator", lambda: PROOF.prove_root(image, package, checksum, provenance_path, contract))
            validator_path.write_bytes(validator_bytes[:-1])
            _expect("short installed device config validator", lambda: PROOF.prove_root(image, package, checksum, provenance_path, contract))
            validator_path.write_bytes(validator_bytes)
            composer_bytes = composer_path.read_bytes()
            composer_path.write_bytes(bytes([composer_bytes[0] ^ 1]) + composer_bytes[1:])
            _expect("stale installed USB gadget composer", lambda: PROOF.prove_root(image, package, checksum, provenance_path, contract))
            composer_path.write_bytes(composer_bytes[:-1])
            _expect("short installed USB gadget composer", lambda: PROOF.prove_root(image, package, checksum, provenance_path, contract))
            composer_path.write_bytes(composer_bytes)
            os.chmod(composer_path, 0o644)
            _expect("wrong mode installed USB gadget composer", lambda: PROOF.prove_root(image, package, checksum, provenance_path, contract))
            os.chmod(composer_path, 0o755)
        PROOF._verify_payload(image, boot, package, inventory)
        stock_manifest_path = boot / "octessera/recovery-stock/manifest.json"
        stock_manifest_content = stock_manifest_path.read_text(encoding="utf-8")
        stock_entries = json.loads(stock_manifest_content)
        stock_initrd_entry = next(entry for entry in stock_entries if entry["path"] == f"boot/initrd.img-{contract.kernel_release}")
        stock_recovery_path = image / stock_initrd_entry["recovery_path"]
        non_initrd_entry = next(entry for entry in stock_entries if entry["path"] != stock_initrd_entry["path"])
        non_initrd_path = image / non_initrd_entry["path"]
        non_initrd_original = non_initrd_path.read_bytes()
        assert stock_initrd_path.read_bytes() == stock_initrd_regenerated
        assert stock_recovery_path.read_bytes() == stock_initrd_original
        assert stock_initrd_entry["sha256"] == STAGE_INSTALLER.sha256_file(stock_recovery_path)
        assert {path.resolve() for path in lsinitramfs_paths} == {initramfs_path.resolve(), stock_initrd_path.resolve(), stock_recovery_path.resolve()}
        stock_initrd_path.write_bytes(b"\x1f\x8bcorrupt")
        _expect("corrupt retained stock initramfs", lambda: PROOF._verify_stock_recovery(image, boot))
        stock_initrd_path.write_bytes(stock_initrd_regenerated)
        PROOF._run_lsinitramfs = lambda path: ""
        try:
            _expect("empty retained stock initramfs listing", lambda: PROOF._verify_stock_recovery(image, boot))
        finally:
            PROOF._run_lsinitramfs = original_lsinitramfs
        stock_initrd_path.unlink()
        _expect("missing retained stock initramfs", lambda: PROOF._verify_stock_recovery(image, boot))
        stock_initrd_path.write_bytes(stock_initrd_regenerated)
        stock_initrd_path.unlink()
        stock_initrd_path.symlink_to(stock_recovery_path)
        _expect("symlink retained stock initramfs", lambda: PROOF._verify_stock_recovery(image, boot))
        stock_initrd_path.unlink()
        stock_initrd_path.write_bytes(stock_initrd_regenerated)
        stock_recovery_original = stock_recovery_path.read_bytes()
        stock_recovery_path.write_bytes(b"tampered recovery")
        _expect("tampered stock recovery", lambda: PROOF._verify_stock_recovery(image, boot))
        stock_recovery_path.write_bytes(stock_recovery_original)
        stock_recovery_path.unlink()
        stock_recovery_path.symlink_to(stock_initrd_path)
        _expect("symlink stock recovery", lambda: PROOF._verify_stock_recovery(image, boot))
        stock_recovery_path.unlink()
        stock_recovery_path.write_bytes(stock_recovery_original)
        tampered_manifest = [dict(entry) for entry in stock_entries]
        next(entry for entry in tampered_manifest if entry["path"] == stock_initrd_entry["path"])["recovery_path"] = "../escaped-initrd"
        stock_manifest_path.write_text(json.dumps(tampered_manifest), encoding="utf-8")
        _expect("escaped stock recovery", lambda: PROOF._verify_stock_recovery(image, boot))
        stock_manifest_path.write_text(stock_manifest_content, encoding="utf-8")
        non_initrd_path.write_bytes(non_initrd_original + b"tampered")
        _expect("tampered retained non-initrd", lambda: PROOF._verify_stock_recovery(image, boot))
        non_initrd_path.write_bytes(non_initrd_original)
        _write(initramfs_path, b"\x1f\x8bcorrupt")
        _expect("corrupt compressed initramfs", lambda: PROOF._verify_selected_initramfs(boot, initramfs_path))
        escape = work / "initramfs-escape"
        _write(escape, b"outside boot")
        initramfs_path.unlink()
        initramfs_path.symlink_to(escape)
        _expect("initramfs symlink escape", lambda: PROOF._verify_selected_initramfs(boot, initramfs_path))
        initramfs_path.unlink()
        _write(initramfs_path, b"initramfs")

        final_root = work / "actual-finalizer-root"
        final_boot = final_root / "boot/firmware"
        hardware_block = "# --- octessera additions ---\n" + (HERE / "stage4-octessera/files/boot/config.txt.append").read_text(encoding="utf-8")
        _write(
            final_boot / "config.txt",
            "kernel=old.img\nkernel=conflict.img\nauto_initramfs=1\ndtoverlay=i2s-dac-no20\n\n" + hardware_block,
        )
        _write(final_boot / "kernel8-stock.img", b"stock")
        _write(final_boot / "octessera/overlays/i2s-dac-no20.dtbo", b"i2s")
        final_hooks = _make_hooks(final_root)
        final_hook_original = [_hook_metadata(path) for path in final_hooks]
        final_artifacts = final_root / "var/lib/octessera/rpi-kernel"
        final_artifacts.mkdir(parents=True)
        shutil.copy2(package, final_artifacts / package.name)
        shutil.copy2(checksum, final_artifacts / "SHA256SUMS")
        shutil.copy2(provenance_path, final_artifacts / "provenance.json")
        final_lib = final_root / "usr/local/lib/octessera"
        final_lib.mkdir(parents=True)
        finalizer = HERE / "stage3-octessera-kernel/files/root/usr/local/sbin/octessera-finalize-rpi-kernel"
        final_sbin = final_root / "usr/local/sbin"
        final_sbin.mkdir(parents=True)
        shutil.copy2(finalizer, final_sbin / "octessera-finalize-rpi-kernel")
        for helper in ("install-rpi-kernel.py", "rpi_kernel_contract.py", "rpi_kernel_image.py", "raspi_firmware_hook_mask.py"):
            shutil.copy2(finalizer.parent.parent / "lib/octessera" / helper, final_lib / helper)
        fake_bin = work / "fake-bin"
        fake_bin.mkdir()
        fake_chroot = fake_bin / "chroot"
        fake_script = (
            "#!/bin/sh\nset -eu\n"
            "root=\"$1\"\n"
            "case \"$2\" in\n"
            f"  depmod) mkdir -p \"$root/lib/modules/{contract.kernel_release}\"; printf '%s\\n' fixture > \"$root/lib/modules/{contract.kernel_release}/modules.dep\" ;;\n"
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
        STAGE_INSTALLER.verify_selectors(final_boot / "config.txt", contract)
        assert (final_boot / f"octessera/initrd.img-{contract.kernel_release}").is_file()

        _expect("duplicate selectors", lambda: _write(boot / "config.txt", (boot / "config.txt").read_text() + "kernel=other.img\n") or PROOF._verify_selectors(boot / "config.txt"))
        STAGE_INSTALLER._write_selectors(boot / "config.txt", contract)
        _write(boot / "octessera/kernel8.img", b"tampered")
        _expect("selected kernel hash", lambda: PROOF._verify_payload(image, boot, package, inventory))
        STAGE_INSTALLER._write_selectors(boot / "config.txt", contract)
        _write(boot / "octessera/kernel8.img", inventory_kernel(inventory, package))
        wrong_checksum = work / "wrong-SHA256SUMS"
        wrong_checksum.write_text(f"{'0' * 64}  {package.name}\n", encoding="utf-8")
        _expect("package hash", lambda: STAGE_INSTALLER.verify_package_inputs(package, wrong_checksum, provenance_path, contract))
        wrong_provenance = work / "wrong-provenance.json"
        tampered_provenance = dict(provenance)
        tampered_provenance["package"] = dict(provenance["package"])
        tampered_provenance["package"]["sha256"] = "0" * 64
        wrong_provenance.write_text(json.dumps(tampered_provenance), encoding="utf-8")
        _expect("provenance package hash", lambda: STAGE_INSTALLER.verify_package_inputs(package, checksum, wrong_provenance, contract))
        _expect("missing provenance", lambda: STAGE_INSTALLER.verify_package_inputs(package, checksum, work / "missing-provenance.json", contract))
        _expect("missing selected DTB", lambda: (boot / "octessera/bcm2710-rpi-zero-2-w.dtb").unlink() or PROOF._verify_payload(image, boot, package, inventory))
        subprocess.run(["dpkg-deb", "-x", str(package), str(image)], check=True, capture_output=True)
        _expect("module payload", lambda: (image / f"lib/modules/{contract.kernel_release}/kernel/fixture/usb_f_uac2.ko").unlink() or PROOF._verify_payload(image, boot, package, inventory))
        _expect("auto initramfs", lambda: _write(boot / "config.txt", "auto_initramfs=1\n") or PROOF._verify_selectors(boot / "config.txt"))
        _expect("missing package", lambda: STAGE_INSTALLER.verify_package_inputs(work / "missing.deb", checksum, provenance_path, contract))
        PROOF._run_lsinitramfs = lambda path: "drwxr-xr-x root/root 0 1970-01-01 00:00 .\n"
        try:
            PROOF._verify_selected_initramfs(boot, initramfs_path)
        finally:
            PROOF._run_lsinitramfs = original_lsinitramfs
    print("Raspberry kernel image synthetic tests passed")
    return 0


def inventory_kernel(inventory: dict[str, Any], package: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-kernel-image-bytes-") as temporary:
        extracted = Path(temporary) / "root"
        subprocess.run(["dpkg-deb", "-x", str(package), str(extracted)], check=True, capture_output=True)
        return STAGE_INSTALLER.assert_firmware_kernel(extracted / inventory["kernel_image"]["package_path"])[0]


if __name__ == "__main__":
    raise SystemExit(_main())
