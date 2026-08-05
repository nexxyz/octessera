#!/usr/bin/env python3
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path


Require = Callable[[bool, str], None]


def read_kv_records(path: Path, field_count: int, require: Require) -> list[list[str]]:
    records: list[list[str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split(":")
        require(len(fields) == field_count, f"malformed account record: {path}")
        records.append(fields)
    return records


def require_owner_mode(path: Path, uid: int, gid: int, mode: int, require: Require) -> None:
    stat = path.lstat()
    require(not path.is_symlink(), f"runtime path is a symlink: {path}")
    require(stat.st_uid == uid and stat.st_gid == gid, f"runtime ownership is wrong: {path}")
    require(stat.st_mode & 0o777 == mode, f"runtime mode is wrong: {path}")


def runtime_account(root: Path, require: Require) -> tuple[int, int]:
    passwd = read_kv_records(root / "etc/passwd", 7, require)
    interactive = [record for record in passwd if record[0] == "octessera"]
    runtime = [record for record in passwd if record[0] == "octessera-runtime"]
    require(len(interactive) == 1 and interactive[0][5] == "/home/octessera" and interactive[0][6] == "/bin/bash", "interactive octessera account changed")
    require(len(runtime) == 1, "octessera-runtime account is missing or duplicated")
    runtime_record = runtime[0]
    require(runtime_record[2].isdigit() and runtime_record[3].isdigit(), "octessera-runtime account IDs are malformed")
    uid = int(runtime_record[2])
    gid = int(runtime_record[3])
    require(uid < 1000 and runtime_record[5] == "/nonexistent" and runtime_record[6] == "/usr/sbin/nologin", "octessera-runtime is not a system no-shell account")
    shadow = read_kv_records(root / "etc/shadow", 9, require)
    runtime_shadow = [record for record in shadow if record[0] == "octessera-runtime"]
    require(len(runtime_shadow) == 1 and (runtime_shadow[0][1] == "" or runtime_shadow[0][1].startswith(("!", "*")) or runtime_shadow[0][1] == "x"), "octessera-runtime password is not locked")
    groups = read_kv_records(root / "etc/group", 4, require)
    runtime_group = [record for record in groups if record[0] == "octessera-runtime"]
    require(len(runtime_group) == 1 and runtime_group[0][2].isdigit(), "octessera-runtime primary group is malformed")
    require(int(runtime_group[0][2]) == gid, "octessera-runtime primary group is invalid")
    for name in ("audio", "i2c", "spi", "gpio"):
        group = [record for record in groups if record[0] == name]
        require(len(group) == 1 and "octessera-runtime" in group[0][3].split(","), f"octessera-runtime is missing from group {name}")
    for name in ("sudo", "admin"):
        for group in groups:
            require(name != group[0] or "octessera-runtime" not in group[0][3].split(","), f"octessera-runtime is in protected group {name}")
    sudoers = [root / "etc/sudoers"]
    sudoers_dir = root / "etc/sudoers.d"
    if sudoers_dir.is_dir() and not sudoers_dir.is_symlink():
        sudoers.extend(path for path in sudoers_dir.rglob("*") if path.is_file() and not path.is_symlink())
    for path in sudoers:
        if path.is_file():
            require("octessera-runtime" not in path.read_text(encoding="utf-8"), f"octessera-runtime appears in sudoers: {path}")
    return uid, gid


def require_runtime_service(root: Path, require: Require) -> None:
    service = root / "etc/systemd/system/octessera.service"
    enabled = root / "etc/systemd/system/multi-user.target.wants/octessera.service"
    service_content = service.read_text(encoding="utf-8")
    for line in (
        "User=octessera-runtime",
        "Group=octessera-runtime",
        "Environment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w",
        "Environment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets",
        "Environment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples",
        "Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json",
        "Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1",
        "NoNewPrivileges=yes",
        "ProtectSystem=strict",
        "ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot",
        "PrivateTmp=yes",
        "ProtectHome=yes",
        "RuntimeDirectory=octessera",
        "LimitRTPRIO=70",
        "LimitMEMLOCK=infinity",
        "ExecStart=/usr/local/bin/octessera-pi",
    ):
        require(line in service_content, f"production service is missing: {line}")
    require("AmbientCapabilities=" not in service_content and "CapabilityBoundingSet=" not in service_content, "production service grants ambient priority capability")
    require("LimitRTPRIO=80" not in service_content, "production service grants an overly broad realtime priority")
    require("PrivateDevices=yes" not in service_content and "DevicePolicy=" not in service_content, "production service blocks hardware access")
    require("octessera-update" not in service_content, "production service claims unsupported updater behavior")
    require(
        enabled.is_symlink() and enabled.readlink().as_posix() in {"../octessera.service", "/etc/systemd/system/octessera.service"},
        "production service is not enabled",
    )


def require_orange_boot_service(root: Path, require: Require) -> None:
    service = root / "etc/systemd/system/octessera-orange-boot-splash.service"
    enabled = root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service"
    content = service.read_text(encoding="utf-8")
    for line in (
        "User=octessera-runtime",
        "Group=octessera-runtime",
        "ExecStart=/usr/local/sbin/octessera-orange-oled-logo boot-loop",
        "RuntimeDirectory=octessera-boot",
        "RuntimeDirectoryMode=0750",
        "RuntimeDirectoryPreserve=yes",
        "ProtectSystem=strict",
        "DevicePolicy=closed",
        "DeviceAllow=/dev/spidev1.0 rw",
        "DeviceAllow=/dev/gpiochip1 rw",
        "After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target",
    ):
        require(line in content, f"Orange boot service is missing: {line}")
    require("Conflicts=" not in content, "Orange boot service conflicts with runtime")
    require(enabled.is_symlink() and enabled.readlink().as_posix() in {"../octessera-orange-boot-splash.service", "/etc/systemd/system/octessera-orange-boot-splash.service"}, "Orange boot service is not enabled at sysinit")


def require_orange_shutdown_service(root: Path, require: Require) -> None:
    service = root / "etc/systemd/system/octessera-orange-oled-shutdown.service"
    content = service.read_text(encoding="utf-8")
    for line in (
        "After=octessera.service",
        "Before=shutdown.target reboot.target halt.target",
        "User=octessera-runtime",
        "Group=octessera-runtime",
        "SupplementaryGroups=audio i2c spi gpio",
        "ProtectSystem=strict",
        "ReadWritePaths=/run/octessera-boot",
        "DevicePolicy=closed",
        "DeviceAllow=/dev/spidev1.0 rw",
        "DeviceAllow=/dev/gpiochip1 rw",
        "ExecStart=-/usr/local/sbin/octessera-orange-oled-logo shutdown",
        "TimeoutStartSec=5",
    ):
        require(line in content, f"Orange shutdown service is missing: {line}")


def require_runtime_udev_rule(root: Path, require: Require) -> None:
    rule = root / "etc/udev/rules.d/70-octessera-orange-runtime.rules"
    require(rule.is_file() and not rule.is_symlink(), "Orange runtime udev rule is not a regular file")
    require_owner_mode(rule, 0, 0, 0o644, require)
    expected = (
        'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\n'
        'KERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\n'
        'KERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"\n'
    )
    require(rule.read_text(encoding="utf-8") == expected, "Orange runtime udev rule content is not exact")


def reject_unsupported_updater(root: Path, require: Require) -> None:
    for relative in (
        "etc/systemd/system/octessera-update-recovery.service",
        "etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service",
        "usr/local/sbin/octessera-update",
        "usr/local/sbin/octessera-update-guard",
        "usr/local/sbin/octessera-update-recovery",
        "usr/local/lib/octessera/updater_protocol.py",
        "usr/local/lib/octessera/updater_state.py",
        "usr/local/lib/octessera/updater_assets.py",
        "usr/local/lib/octessera/updater_guard.py",
        "usr/local/lib/octessera/updater_cli.py",
        "etc/sudoers.d/octessera-update",
    ):
        path = root / relative
        require(not path.exists() and not path.is_symlink(), f"production image contains unsupported updater path: {relative}")
