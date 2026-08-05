#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from unittest.mock import patch


MODULE_PATH = Path(__file__).parent / "stage4-octessera/files/root/usr/local/lib/octessera/rpi_uart_release.py"
MODULE_SPEC = importlib.util.spec_from_file_location("rpi_uart_release", MODULE_PATH)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
uart = importlib.util.module_from_spec(MODULE_SPEC)
MODULE_SPEC.loader.exec_module(uart)


def _write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    os.chown(path, 0, 0)  # type: ignore[attr-defined]


def _root(work: Path, firmware: bool = True) -> tuple[Path, Path]:
    root = work / ("firmware" if firmware else "boot")
    if root.exists():
        shutil.rmtree(root)
    boot = root / "boot/firmware" if firmware else root / "boot"
    _write(boot / "config.txt", b"dtoverlay=disable-bt\nenable_uart=0\n")
    _write(boot / "cmdline.txt", b"console=tty1 console=serial0,115200 root=x console=ttyAMA0,9600\tconsole=ttyS0,38400 quiet\n")
    (root / "etc/systemd/system/getty.target.wants").mkdir(parents=True, exist_ok=True)
    (root / "etc/systemd/system").mkdir(parents=True, exist_ok=True)
    return root, boot


def _expect_failure(operation: object) -> None:
    try:
        operation()  # type: ignore[operator]
    except (OSError, uart.UartReleaseError):
        return
    raise AssertionError("invalid Raspberry UART layout was accepted")


def main() -> int:
    if os.geteuid() != 0:  # type: ignore[attr-defined]
        print("Raspberry UART tests require root for ownership assertions")
        return 0
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-uart-") as temporary:
        work = Path(temporary)
        for firmware in (True, False):
            root, boot = _root(work, firmware)
            cmdline = boot / "cmdline.txt"
            config = boot / "config.txt"
            uart.release_uart(root)
            result = cmdline.read_bytes()
            assert b"console=tty1" in result and b"root=x" in result and b"quiet" in result
            assert all(token not in result for token in (b"console=serial0", b"console=ttyAMA0", b"console=ttyS0"))
            assert result.count(b"console=tty1") == 1
            assert config.read_bytes().endswith(uart.UART_BLOCK)
            metadata = cmdline.stat()
            assert metadata.st_uid == 0 and metadata.st_mode & 0o777 == 0o644
            assert all((root / "etc/systemd/system" / f"serial-getty@{unit}.service").is_symlink() for unit in uart.MASK_UNITS)
            before = result
            config_before = config.read_bytes()
            uart.release_uart(root)
            assert cmdline.read_bytes() == before
            assert config.read_bytes() == config_before

        root, boot = _root(work)
        _write(boot / "config.txt", b"# keep\r\n[all]\r\ndtoverlay=disable-bt\r\nenable_uart=1\r\nkeep=value\r\n[pi4]\r\nenable_uart=1\r\n")
        uart.release_uart(root)
        config = boot / "config.txt"
        config_result = config.read_bytes()
        assert config_result.endswith(uart.UART_BLOCK)
        assert b"enable_uart=1" not in config_result
        assert b"keep=value\r\n[pi4]\r\n" in config_result
        config_before = config_result
        uart.release_uart(root)
        assert config.read_bytes() == config_before

        root, boot = _root(work)
        _write(boot / "config.txt", b"[all\n")
        _expect_failure(lambda: uart.release_uart(root))
        _write(boot / "config.txt", b"enable_uart=2\n")
        _expect_failure(lambda: uart.release_uart(root))
        _write(boot / "config.txt", b"enable_uart=1\0\n")
        _expect_failure(lambda: uart.release_uart(root))
        (boot / "config.txt").unlink()
        (boot / "config.txt").symlink_to("missing")
        _expect_failure(lambda: uart.release_uart(root))

        root, boot = _root(work)
        _write(boot / "cmdline.txt", b"console=serial01 console=ttyAMA0-debug console=ttyS0foo console=serial0,115200 root=x\n")
        uart.release_uart(root)
        result = (boot / "cmdline.txt").read_bytes()
        assert b"console=serial01" in result and b"console=ttyAMA0-debug" in result and b"console=ttyS0foo" in result
        assert b"console=serial0,115200" not in result

        root, boot = _root(work)
        for unit in uart.MASK_UNITS:
            mask = root / "etc/systemd/system" / f"serial-getty@{unit}.service"
            if mask.is_symlink():
                mask.unlink()
            mask.write_text("unexpected", encoding="utf-8")
        _expect_failure(lambda: uart.release_uart(root))
        for unit in uart.MASK_UNITS:
            (root / "etc/systemd/system" / f"serial-getty@{unit}.service").unlink()

        root, _ = _root(work)
        (root / "boot/config.txt").write_bytes(b"boot")
        os.chown(root / "boot/config.txt", 0, 0)  # type: ignore[attr-defined]
        _expect_failure(lambda: uart.select_boot_directory(root))
        (root / "boot/config.txt").unlink()

        root, boot = _root(work)
        boot.joinpath("cmdline.txt").write_bytes(b"one\ntwo\n")
        _expect_failure(lambda: uart.release_uart(root))
        boot.joinpath("cmdline.txt").write_bytes(b"one\0two")
        _expect_failure(lambda: uart.release_uart(root))
        boot.joinpath("cmdline.txt").write_bytes(b"   \n")
        _expect_failure(lambda: uart.release_uart(root))
        boot.joinpath("cmdline.txt").unlink()
        boot.joinpath("cmdline.txt").symlink_to("missing")
        _expect_failure(lambda: uart.release_uart(root))
        boot.joinpath("cmdline.txt").unlink()

        root, boot = _root(work)
        boot.joinpath("cmdline.txt").write_bytes(b"console=tty1\n")
        os.chown(boot / "cmdline.txt", 0, 0)  # type: ignore[attr-defined]
        calls: list[tuple[list[str], dict[str, object]]] = []

        def systemctl(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            calls.append((command, options))
            if command[1] == "show":
                return subprocess.CompletedProcess(command, 0, "not-found\n", "")
            return subprocess.CompletedProcess(command, 0)

        with patch.object(uart.subprocess, "run", side_effect=systemctl):
            uart.release_uart(root, live=True)
        commands = [command for command, _ in calls]
        expected = [["systemctl", "show", "--property=LoadState", "--value", unit] for unit in uart.BLUETOOTH_UNITS]
        expected.extend([["systemctl", action, f"serial-getty@{unit}.service"] for unit in uart.MASK_UNITS for action in ("stop", "disable")])
        assert commands == [*expected, ["systemctl", "daemon-reload"]]
        assert all(options.get("timeout") == 10 for _, options in calls)

        root, _ = _root(work)
        masked_calls: list[list[str]] = []

        def masked_systemctl(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            masked_calls.append(command)
            if command[1] == "show":
                return subprocess.CompletedProcess(command, 0, "masked\n", "")
            return subprocess.CompletedProcess(command, 0)

        with patch.object(uart.subprocess, "run", side_effect=masked_systemctl):
            uart.release_uart(root, live=True)
        assert not any(command[1:3] == ["disable", "--now"] for command in masked_calls)

        root, _ = _root(work)
        loaded_calls: list[list[str]] = []

        def loaded_systemctl(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            loaded_calls.append(command)
            if command[1] == "show":
                return subprocess.CompletedProcess(command, 0, "loaded\n", "")
            return subprocess.CompletedProcess(command, 0)

        with patch.object(uart.subprocess, "run", side_effect=loaded_systemctl):
            uart.release_uart(root, live=True)
        assert all(["systemctl", "disable", "--now", unit] in loaded_calls for unit in uart.BLUETOOTH_UNITS)

        root, _ = _root(work)

        def query_failure(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            if command[1] == "show":
                return subprocess.CompletedProcess(command, 1, "", "query failed")
            return subprocess.CompletedProcess(command, 0)

        with patch.object(uart.subprocess, "run", side_effect=query_failure):
            _expect_failure(lambda: uart.release_uart(root, live=True))

        root, _ = _root(work)

        def daemon_reload_failure(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            if command[1] == "daemon-reload":
                return subprocess.CompletedProcess(command, 1)
            if command[1] == "show":
                return subprocess.CompletedProcess(command, 0, "not-found\n", "")
            return subprocess.CompletedProcess(command, 0)

        with patch.object(uart.subprocess, "run", side_effect=daemon_reload_failure):
            _expect_failure(lambda: uart.release_uart(root, live=True))

        root, _ = _root(work)

        def owned_unit_failure(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            if command[1] == "show":
                return subprocess.CompletedProcess(command, 0, "loaded\n", "")
            if command[1] == "disable" and command[3] == "bluetooth.service":
                return subprocess.CompletedProcess(command, 1)
            return subprocess.CompletedProcess(command, 0)

        with patch.object(uart.subprocess, "run", side_effect=owned_unit_failure):
            _expect_failure(lambda: uart.release_uart(root, live=True))
    print("Raspberry UART release tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
