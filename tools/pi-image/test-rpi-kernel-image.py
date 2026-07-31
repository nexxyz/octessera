#!/usr/bin/env python3
from __future__ import annotations

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
            return SimpleNamespace(stdout=json.dumps({"blockdevices": [{"name": "/dev/loop0", "type": "loop", "children": [{"name": "/dev/loop0p2", "type": "part", "fstype": "ext4", "label": "rootfs"}, {"name": "/dev/loop0p1", "type": "part", "fstype": "vfat", "label": "bootfs"}]}]}))
        return original_run(command, **kwargs)
    PROOF.subprocess.run = fake_lsblk
    try:
        assert PROOF._expected_partitions("/dev/loop0") == ("/dev/loop0p1", "/dev/loop0p2")
    finally:
        PROOF.subprocess.run = original_run
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-image-test-") as temporary:
        work = Path(temporary)
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
        _write(image / "boot/firmware/config.txt", "kernel=kernel8.img\nauto_initramfs=1\n")
        _write(image / "boot/firmware/kernel8.img", b"stock-kernel")
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
        original_root_command = STAGE_INSTALLER._run_in_root
        def fake_root_command(rootfs: Path, command: list[str]) -> None:
            if command[0] == "update-initramfs":
                _write(rootfs / f"boot/initrd.img-{contract.kernel_release}", b"generated-initramfs")
        STAGE_INSTALLER._run_in_root = fake_root_command
        try:
            evidence = STAGE_INSTALLER._finalize(image, package, checksum, provenance_path, contract)
        finally:
            STAGE_INSTALLER._run_in_root = original_root_command
        assert evidence["initramfs"]["path"] == f"octessera/initrd.img-{contract.kernel_release}"
        STAGE_INSTALLER.verify_selectors(boot / "config.txt", contract)
        _write(boot / f"octessera/initrd.img-{contract.kernel_release}", b"initramfs")
        PROOF._run_lsinitramfs = lambda path: f"{contract.kernel_release} " + " ".join(contract.required_modules)
        proved = PROOF.prove_root(image, package, checksum, provenance_path, contract)
        assert proved["package"]["sha256"] == inventory["package"]["sha256"]
        PROOF._verify_payload(image, boot, package, inventory)

        final_root = work / "actual-finalizer-root"
        final_boot = final_root / "boot/firmware"
        _write(final_boot / "config.txt", "kernel=old.img\nkernel=conflict.img\nauto_initramfs=1\ndtoverlay=i2s-dac-no20\n")
        _write(final_boot / "kernel8-stock.img", b"stock")
        _write(final_boot / "octessera/overlays/i2s-dac-no20.dtbo", b"i2s")
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
        for helper in ("install-rpi-kernel.py", "rpi_kernel_contract.py", "rpi_kernel_image.py"):
            shutil.copy2(finalizer.parent.parent / "lib/octessera" / helper, final_lib / helper)
        fake_bin = work / "fake-bin"
        fake_bin.mkdir()
        fake_chroot = fake_bin / "chroot"
        fake_script = (
            "#!/bin/sh\nset -eu\n"
            "root=\"$1\"\n"
            "case \"$2\" in\n"
            f"  depmod) mkdir -p \"$root/lib/modules/{contract.kernel_release}\"; printf '%s\\n' fixture > \"$root/lib/modules/{contract.kernel_release}/modules.dep\" ;;\n"
            f"  update-initramfs) mkdir -p \"$root/boot\"; printf '%s\\n' fixture > \"$root/boot/initrd.img-{contract.kernel_release}\" ;;\n"
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
        PROOF._verify_initramfs(boot / f"octessera/initrd.img-{contract.kernel_release}", contract.kernel_release, contract.required_modules)
    print("Raspberry kernel image synthetic tests passed")
    return 0


def inventory_kernel(inventory: dict[str, Any], package: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-kernel-image-bytes-") as temporary:
        extracted = Path(temporary) / "root"
        subprocess.run(["dpkg-deb", "-x", str(package), str(extracted)], check=True, capture_output=True)
        return STAGE_INSTALLER.assert_firmware_kernel(extracted / inventory["kernel_image"]["package_path"])[0]


if __name__ == "__main__":
    raise SystemExit(_main())
