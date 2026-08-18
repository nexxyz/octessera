#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
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
        "StartLimitIntervalSec=30s",
        "StartLimitBurst=3",
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
        "Restart=on-failure",
        "RestartPreventExitStatus=78",
        "RestartSec=5s",
    ):
        require(line in service_content, f"production service is missing: {line}")
    require("AmbientCapabilities=" not in service_content and "CapabilityBoundingSet=" not in service_content, "production service grants ambient priority capability")
    require("Restart=always" not in service_content, "production service restarts always")
    requires = [line for line in service_content.splitlines() if line.startswith("Requires=")]
    require(
        requires == [
            "Requires=octessera-device-apply-reboot.socket",
            "Requires=octessera-provision-musical-default.service",
            "Requires=octessera-update-recovery.service",
        ],
        "production service has an unexpected dependency",
    )
    require(not any(line.startswith(prefix) for line in service_content.splitlines() for prefix in ("StartLimitAction=", "OnFailure=", "Requisite=", "BindsTo=", "PartOf=")), "production service has an unapproved failure dependency")
    require("LimitRTPRIO=80" not in service_content, "production service grants an overly broad realtime priority")
    require("PrivateDevices=yes" not in service_content and "DevicePolicy=" not in service_content, "production service blocks hardware access")
    require(
        not any(
            "octessera-update-" in line
            and line not in {
                "Requires=octessera-update-recovery.service",
                "After=octessera-update-recovery.service",
            }
            for line in service_content.splitlines()
        ),
        "production service claims unsupported updater behavior",
    )
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
        "Type=oneshot",
        "User=octessera-runtime",
        "Group=octessera-runtime",
        "ProtectSystem=strict",
        "ReadWritePaths=/run/octessera-boot",
        "DevicePolicy=closed",
        "DeviceAllow=/dev/spidev1.0 rw",
        "DeviceAllow=/dev/gpiochip1 rw",
        "ExecStart=/bin/true",
        "ExecStop=/bin/sh -c 'sleep 4; /usr/local/sbin/octessera-orange-oled-logo off || true'",
        "RemainAfterExit=yes",
        "TimeoutStopSec=8",
    ):
        require(line in content, f"Orange shutdown service is missing: {line}")
    require("Before=" not in content and "WantedBy=shutdown.target" not in content and "WantedBy=reboot.target" not in content and "WantedBy=halt.target" not in content, "Orange shutdown service retains target choreography")
    require("orange-oled-logo shutdown" not in content and "orange-oled-logo boot" not in content, "Orange shutdown service writes a logo")
    enabled = root / "etc/systemd/system/multi-user.target.wants/octessera-orange-oled-shutdown.service"
    require(enabled.is_symlink() and enabled.readlink().as_posix() in {"../octessera-orange-oled-shutdown.service", "/etc/systemd/system/octessera-orange-oled-shutdown.service"}, "Orange shutdown service is not enabled at multi-user")


def require_orange_suspend_service(root: Path, require: Require) -> None:
    service = root / "etc/systemd/system/octessera-orange-oled-suspend.service"
    enabled = root / "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service"
    content = service.read_text(encoding="utf-8")
    for line in (
        "After=octessera.service",
        "Requisite=octessera.service",
        "Before=sleep.target",
        "RequiredBy=sleep.target",
        "StopWhenUnneeded=yes",
        "Type=oneshot",
        "RemainAfterExit=yes",
        "User=octessera-runtime",
        "Group=octessera-runtime",
        "RuntimeDirectory=octessera-oled-suspend",
        "RuntimeDirectoryMode=0700",
        "RestrictAddressFamilies=AF_UNIX",
        "ExecStart=/usr/local/sbin/octessera-orange-oled-suspend prepare",
        "ExecStop=/usr/local/sbin/octessera-orange-oled-suspend resume",
        "TimeoutStartSec=8",
        "TimeoutStopSec=8",
    ):
        require(line in content, f"Orange suspend service is missing: {line}")
    require("SupplementaryGroups=audio i2c spi gpio" not in content, "Orange suspend service requires unavailable supplementary groups")
    require("Conflicts=" not in content and "systemctl" not in content and "BusName=" not in content, "Orange suspend service has an unsafe lifecycle dependency")
    require(enabled.is_symlink() and enabled.readlink().as_posix() in {"../octessera-orange-oled-suspend.service", "/etc/systemd/system/octessera-orange-oled-suspend.service"}, "Orange suspend service is not enabled at sleep.target")
    stale_wants = root / "etc/systemd/system/sleep.target.wants/octessera-orange-oled-suspend.service"
    require(not stale_wants.exists() and not stale_wants.is_symlink(), "Orange suspend service retains a non-required sleep target enablement")
    require(
        all(
            not path.exists() and not path.is_symlink()
            for path in (
                root / "usr/lib/systemd/system-sleep/octessera-orange-oled",
                root / "lib/systemd/system-sleep/octessera-orange-oled",
            )
        ),
        "obsolete Orange system-sleep hook remains",
    )


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


def require_production_updater(root: Path, construction: dict, repository_root: Path, version: str, require: Require) -> None:
    assets = (
        ("tools/device-update/updater_protocol.py", "usr/local/lib/octessera/updater_protocol.py", 0o644),
        ("tools/device-update/updater_state.py", "usr/local/lib/octessera/updater_state.py", 0o644),
        ("tools/device-update/updater_assets.py", "usr/local/lib/octessera/updater_assets.py", 0o644),
        ("tools/device-update/updater_guard.py", "usr/local/lib/octessera/updater_guard.py", 0o644),
        ("tools/device-update/updater_cli.py", "usr/local/lib/octessera/updater_cli.py", 0o644),
        ("tools/device-update/updater_profiles.py", "usr/local/lib/octessera/updater_profiles.py", 0o644),
        ("tools/device-update/octessera-update-broker", "usr/local/sbin/octessera-update-broker", 0o755),
        ("userpatches/overlay/usr/local/sbin/octessera-update", "usr/local/sbin/octessera-update", 0o755),
        ("userpatches/overlay/usr/local/sbin/octessera-update-guard", "usr/local/sbin/octessera-update-guard", 0o755),
        ("userpatches/overlay/usr/local/sbin/octessera-update-recovery", "usr/local/sbin/octessera-update-recovery", 0o755),
        ("userpatches/overlay/etc/systemd/system/octessera-update-guard.service", "etc/systemd/system/octessera-update-guard.service", 0o644),
        ("userpatches/overlay/etc/systemd/system/octessera-update-recovery.service", "etc/systemd/system/octessera-update-recovery.service", 0o644),
        ("userpatches/overlay/etc/systemd/system/octessera-update.socket", "etc/systemd/system/octessera-update.socket", 0o644),
        ("userpatches/overlay/etc/systemd/system/octessera-update@.service", "etc/systemd/system/octessera-update@.service", 0o644),
        ("userpatches/overlay/etc/sudoers.d/octessera-update", "etc/sudoers.d/octessera-update", 0o440),
    )
    exact_inputs = {item["path"]: item for item in construction["exact_inputs"]}
    for source_relative, installed_relative, mode in assets:
        source = repository_root / source_relative
        installed = root / installed_relative
        expected = exact_inputs.get(source_relative)
        require(expected is not None, f"Orange updater source identity is missing: {source_relative}")
        if expected is None:
            raise ValueError(f"Orange updater source identity is missing: {source_relative}")
        require(source.is_file() and not source.is_symlink(), f"Orange updater source is missing or symlinked: {source_relative}")
        require(installed.is_file() and not installed.is_symlink(), f"Orange updater asset is missing or symlinked: {installed_relative}")
        require(hashlib.sha256(source.read_bytes()).hexdigest() == expected["sha256"] and source.stat().st_size == expected["size"], f"Orange updater source identity changed: {source_relative}")
        require(installed.read_bytes() == source.read_bytes(), f"Orange updater asset differs from its canonical source: {installed_relative}")
        require_owner_mode(installed, 0, 0, mode, require)
    recovery = root / "etc/systemd/system/octessera-update-recovery.service"
    recovery_enabled = root / "etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service"
    require_owner_mode(recovery, 0, 0, 0o644, require)
    require(recovery_enabled.is_symlink() and recovery_enabled.lstat().st_uid == 0 and recovery_enabled.lstat().st_gid == 0 and recovery_enabled.readlink().as_posix() in {"../octessera-update-recovery.service", "/etc/systemd/system/octessera-update-recovery.service"}, "Orange updater recovery service is not enabled")
    socket = root / "etc/systemd/system/octessera-update.socket"
    socket_enabled = root / "etc/systemd/system/sockets.target.wants/octessera-update.socket"
    socket_content = socket.read_text(encoding="utf-8")
    require(
        all(line in socket_content for line in (
            "ListenStream=/run/octessera-update/update.sock",
            "SocketMode=0660",
            "SocketUser=root",
            "SocketGroup=octessera-runtime",
            "DirectoryMode=0755",
            "Accept=yes",
        )),
        "Orange update socket permissions are not narrow",
    )
    require(socket_enabled.is_symlink() and socket_enabled.lstat().st_uid == 0 and socket_enabled.lstat().st_gid == 0 and socket_enabled.readlink().as_posix() in {"../octessera-update.socket", "/etc/systemd/system/octessera-update.socket"}, "Orange update socket is not enabled")
    broker_service = root / "etc/systemd/system/octessera-update@.service"
    broker_service_content = broker_service.read_text(encoding="utf-8")
    require(
        all(line in broker_service_content for line in (
            "User=root",
            "Group=root",
            "StandardInput=socket",
            "StandardOutput=socket",
            "ExecStart=/usr/local/sbin/octessera-update-broker",
            "ProtectSystem=strict",
            "ReadWritePaths=/opt/octessera /usr/local/bin /run/octessera",
        )),
        "Orange update broker service is not root-owned and constrained",
    )
    sudoers = root / "etc/sudoers.d/octessera-update"
    require("octessera-runtime" not in sudoers.read_text(encoding="utf-8"), "Orange runtime account appears in updater sudoers")
    state_path = root / "opt/octessera/update-state.json"
    release = root / f"opt/octessera/releases/{version}"
    manifest_path = release / "update-manifest.json"
    require_owner_mode(state_path, 0, 0, 0o644, require)
    require_owner_mode(manifest_path, 0, 0, 0o444, require)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    require(manifest == {"schema_version": 2, "updater_protocol": 2, "candidate_health_protocol": 1, "updater_supported": True, "distribution": "runtime-updater", "tag": f"v{version}", "version": version, "board_profile": "orange-pi-zero-2w", "arch": "aarch64-unknown-linux-gnu", "binary": "octessera-pi", "platforms": ["orange-pi-zero-2w", "linux-aarch64-device"]}, "Orange updater manifest is not exact")
    state = json.loads(state_path.read_text(encoding="utf-8"))
    require(state == {"schema_version": 2, "phase": "committed", "current": version, "previous": None, "updated_at": "1970-01-01T00:00:00Z", "release": manifest, "asset": None}, "Orange updater state is not an exact committed initial state")
