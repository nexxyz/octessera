from __future__ import annotations

import re
from pathlib import Path
from typing import Any


def _require_input(
    root: Path,
    repository_root: Path,
    construction: dict[str, Any],
    source_relative: str,
    installed_relative: str,
    mode: int,
    require: Any,
    sha256_file: Any,
    require_owner_mode: Any,
) -> Path:
    expected = next((item for item in construction["exact_inputs"] if item["path"] == source_relative), None)
    require(expected is not None, f"Orange SD source identity is missing: {source_relative}")
    if expected is None:
        raise ValueError(f"Orange SD source identity is missing: {source_relative}")
    source = repository_root / source_relative
    installed = root / installed_relative
    require(source.is_file() and not source.is_symlink(), f"Orange SD source is missing or symlinked: {source_relative}")
    require(installed.is_file() and not installed.is_symlink(), f"Orange installed SD asset is missing or symlinked: {installed_relative}")
    require(sha256_file(source) == expected["sha256"] and source.stat().st_size == expected["size"], f"Orange SD source identity changed: {source_relative}")
    require(installed.read_bytes() == source.read_bytes(), f"Orange installed SD asset differs from its canonical source: {installed_relative}")
    require_owner_mode(installed, 0, 0, mode, require)
    return installed


def _config_value(config_lines: list[str], symbol: str, require: Any) -> str:
    matches = [line.removeprefix(f"{symbol}=") for line in config_lines if re.fullmatch(fr"{re.escape(symbol)}=[ym]", line)]
    require(len(matches) == 1, f"Orange kernel config must contain exactly one enabled {symbol}")
    return matches[0]


def verify_orange_sd_card(
    root: Path,
    repository_root: Path,
    construction: dict[str, Any],
    config_lines: list[str],
    module_root: Path,
    require: Any,
    sha256_file: Any,
    require_owner_mode: Any,
) -> None:
    helper = _require_input(
        root,
        repository_root,
        construction,
        "tools/storage/octessera-sd-card",
        "usr/local/sbin/octessera-sd-card",
        0o755,
        require,
        sha256_file,
        require_owner_mode,
    )
    library = _require_input(
        root,
        repository_root,
        construction,
        "tools/storage/octessera-sd-card-lib.sh",
        "usr/local/lib/octessera/octessera-sd-card-lib.sh",
        0o644,
        require,
        sha256_file,
        require_owner_mode,
    )
    service = _require_input(
        root,
        repository_root,
        construction,
        "userpatches/overlay/etc/systemd/system/octessera-orange-sd-card.service",
        "etc/systemd/system/octessera-orange-sd-card.service",
        0o644,
        require,
        sha256_file,
        require_owner_mode,
    )
    rule = _require_input(
        root,
        repository_root,
        construction,
        "userpatches/overlay/etc/udev/rules.d/99-octessera-orange-sd-card.rules",
        "etc/udev/rules.d/99-octessera-orange-sd-card.rules",
        0o644,
        require,
        sha256_file,
        require_owner_mode,
    )
    storage_library = _require_input(
        root,
        repository_root,
        construction,
        "tools/storage/octessera-sd-card-lib.sh",
        "usr/local/lib/octessera/octessera-sd-card-lib.sh",
        0o644,
        require,
        sha256_file,
        require_owner_mode,
    )
    storage_helper = _require_input(
        root,
        repository_root,
        construction,
        "tools/storage/octessera-orange-storage",
        "usr/local/sbin/octessera-orange-storage",
        0o755,
        require,
        sha256_file,
        require_owner_mode,
    )
    control_helper = _require_input(
        root,
        repository_root,
        construction,
        "tools/storage/octessera-orange-storage-control",
        "usr/local/sbin/octessera-orange-storage-control",
        0o755,
        require,
        sha256_file,
        require_owner_mode,
    )
    socket = _require_input(
        root,
        repository_root,
        construction,
        "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control.socket",
        "etc/systemd/system/octessera-orange-storage-control.socket",
        0o644,
        require,
        sha256_file,
        require_owner_mode,
    )
    control_service = _require_input(
        root,
        repository_root,
        construction,
        "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control@.service",
        "etc/systemd/system/octessera-orange-storage-control@.service",
        0o644,
        require,
        sha256_file,
        require_owner_mode,
    )
    helper_text = helper.read_text(encoding="utf-8")
    require(". /usr/local/lib/octessera/octessera-sd-card-lib.sh" in helper_text, "Orange SD helper library path is not canonical")
    require(library.read_text(encoding="utf-8") == (repository_root / "tools/storage/octessera-sd-card-lib.sh").read_text(encoding="utf-8"), "Orange SD helper library differs from its canonical source")
    require("SD_MOUNT=${OCTESSERA_SD_MOUNT:?OCTESSERA_SD_MOUNT must be set}" in helper_text, "Orange SD helper mount configuration is not explicit")
    require("SD_OWNER=${OCTESSERA_SD_OWNER:?OCTESSERA_SD_OWNER must be set}" in helper_text, "Orange SD helper owner configuration is not explicit")
    require("/dev/disk/by-label/OCTESSERA_SD" not in helper_text and "mmcblk" not in helper_text, "Orange SD helper uses a device-index fallback")
    service_text = service.read_text(encoding="utf-8")
    for line in (
        "User=root",
        "Environment=OCTESSERA_SD_MOUNT=/var/lib/octessera/samples/sd-card",
        "Environment=OCTESSERA_SD_OWNER=octessera-runtime",
        "ExecStart=/usr/local/sbin/octessera-sd-card mount",
    ):
        require(line in service_text.splitlines(), f"Orange SD service is missing: {line}")
    require(
        rule.read_text(encoding="utf-8")
        == 'ACTION=="add|change", SUBSYSTEM=="block", ENV{DEVTYPE}=="partition", ENV{ID_FS_LABEL}=="OCTESSERA_SD", TAG+="systemd", ENV{SYSTEMD_WANTS}+="octessera-orange-sd-card.service"\n',
        "Orange SD udev rule is not label-bound",
    )
    require("/usr/local/lib/octessera/octessera-sd-card-lib.sh" in helper_text, "Orange SD helper does not use the shared library")
    storage_text = storage_helper.read_text(encoding="utf-8")
    require("/sys/kernel/config" in storage_text and "musb-hdrc.4.auto" in storage_text, "Orange storage helper is not fixed to configfs and the Orange UDC")
    require("--config" not in storage_text and not re.search(r"\bOCTESSERA_[A-Z0-9_]+\s*=", storage_text), "Orange storage helper exposes configurable paths")
    control_text = control_helper.read_text(encoding="utf-8")
    require("storage-start\\n" in control_text and "storage-stop\\n" in control_text, "Orange storage socket protocol is not exact")
    require("subprocess.run(\n            [STORAGE_PATH, action]" in control_text, "Orange storage socket invokes an unexpected command")
    require("sudo" not in control_text and "systemctl" not in control_text, "Orange storage socket is a generic privilege broker")
    socket_lines = socket.read_text(encoding="utf-8").splitlines()
    for line in (
        "ListenStream=/run/octessera-orange-storage-control/storage.sock",
        "SocketMode=0660",
        "SocketUser=root",
        "SocketGroup=octessera-runtime",
        "DirectoryMode=0755",
        "RemoveOnStop=yes",
        "Accept=yes",
    ):
        require(line in socket_lines, f"Orange storage socket is missing: {line}")
    service_lines = control_service.read_text(encoding="utf-8").splitlines()
    for line in (
        "User=root",
        "Group=root",
        "StandardInput=socket",
        "StandardOutput=socket",
        "ExecStart=/usr/local/sbin/octessera-orange-storage-control",
        "TimeoutStartSec=5s",
        "RestrictAddressFamilies=AF_UNIX",
    ):
        require(line in service_lines, f"Orange storage service is missing: {line}")
    require(not (root / "etc/sudoers.d/octessera-usb-storage").exists(), "Orange storage control has a sudoers entry")
    enabled_socket = root / "etc/systemd/system/sockets.target.wants/octessera-orange-storage-control.socket"
    require(enabled_socket.is_symlink() and enabled_socket.readlink().as_posix() == "../octessera-orange-storage-control.socket", "Orange storage socket is not enabled")
    module_load = root / "etc/modules-load.d/octessera-orange-usb-gadget.conf"
    require(module_load.is_file() and "usb_f_mass_storage" in module_load.read_text(encoding="utf-8").splitlines(), "Orange mass-storage module is not enabled")
    link = root / "etc/systemd/system/multi-user.target.wants/octessera-orange-sd-card.service"
    require(link.is_symlink() and link.readlink().as_posix() == "../octessera-orange-sd-card.service", "Orange SD service is not enabled")
    metadata = {}
    for line in (root / "etc/octessera/build-metadata.env").read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator:
            metadata[key] = value
    for key, path in (
        ("OCTESSERA_SPI1_OLED_SD2_DTS_SHA256", "usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts"),
        ("OCTESSERA_SPI1_OLED_SD2_DTBO_SHA256", "boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo"),
    ):
        require(metadata.get(key) == sha256_file(root / path), f"Orange SD device-tree hash metadata is not exact: {key}")
    require(_config_value(config_lines, "CONFIG_MMC", require) == "y", "Orange MMC core must be built in")
    require(_config_value(config_lines, "CONFIG_MMC_BLOCK", require) == "y", "Orange MMC block support must be built in")
    spi_value = _config_value(config_lines, "CONFIG_MMC_SPI", require)
    mmc_module = sorted(path for path in module_root.rglob("mmc_spi.ko*") if path.is_file())
    module_load = root / "etc/modules-load.d/octessera-orange-sd-card.conf"
    if spi_value == "m":
        require(len(mmc_module) == 1, "Orange kernel is missing exactly one mmc_spi module")
        require(module_load.is_file() and not module_load.is_symlink(), "Orange mmc_spi module-load file is missing")
        require_owner_mode(module_load, 0, 0, 0o644, require)
        require(module_load.read_text(encoding="utf-8") == "mmc_spi\n", "Orange mmc_spi module-load file is not exact")
    else:
        require(not mmc_module and not module_load.exists() and not module_load.is_symlink(), "Orange built-in mmc_spi must not be module-loaded")
