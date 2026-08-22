from __future__ import annotations

import re
import subprocess
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

Require = Callable[[bool, str], None]
HashFile = Callable[[Path], str]
OwnerMode = Callable[[Path, int, int, int, Require], None]


def _read_metadata(path: Path, require: Require) -> dict[str, str]:
    values: dict[str, str] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        require(False, f"Orange build metadata is unreadable: {error}")
        return values
    for line in lines:
        key, separator, value = line.partition("=")
        require(bool(separator and key and key not in values), f"malformed or duplicate Orange build metadata field: {line}")
        values[key] = value
    return values


def verify_audio_overlay(
    root: Path,
    selected_dtb: Path,
    repository_root: Path,
    construction: dict[str, Any],
    require: Require,
    sha256_file: HashFile,
    require_owner_mode: OwnerMode,
    package: dict[str, Any],
) -> dict[str, str]:
    source_relative = "userpatches/overlay/usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts"
    installed_relative = "usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts"
    dtbo_relative = "boot/overlay-user/octessera-ahub0-pcm5102.dtbo"
    expected = next((item for item in construction["exact_inputs"] if item["path"] == source_relative), None)
    require(expected is not None, "canonical Orange audio DTS input identity is missing")
    if expected is None:
        raise ValueError("canonical Orange audio DTS input identity is missing")
    source = repository_root / source_relative
    installed = root / installed_relative
    dtbo = root / dtbo_relative
    require(source.is_file() and not source.is_symlink(), "canonical Orange audio DTS is missing or symlinked")
    require(installed.is_file() and not installed.is_symlink(), "installed Orange audio DTS is missing or symlinked")
    require(dtbo.is_file() and not dtbo.is_symlink(), "installed Orange audio DTBO is missing or symlinked")
    require(sha256_file(source) == expected["sha256"] and source.stat().st_size == expected["size"], "canonical Orange audio DTS input identity changed")
    require(installed.read_bytes() == source.read_bytes(), "installed Orange audio DTS differs from its canonical input")
    require_owner_mode(installed, 0, 0, 0o644, require)
    require_owner_mode(dtbo, 0, 0, 0o644, require)
    metadata = _read_metadata(root / "etc/octessera/build-metadata.env", require)
    require(metadata.get("OCTESSERA_AHUB0_PCM5102_DTS_SHA256") == sha256_file(installed), "installed Orange audio DTS hash metadata is not exact")
    require(metadata.get("OCTESSERA_AHUB0_PCM5102_DTBO_SHA256") == sha256_file(dtbo), "installed Orange audio DTBO hash metadata is not exact")
    env = root / "boot/armbianEnv.txt"
    require(env.is_file(), "Orange Armbian environment is missing")
    lines = env.read_text(encoding="utf-8").splitlines()
    require(not any(re.search(r"(^|[^_A-Za-z0-9])(user_overlays|overlays)\s*=", line) and not line.startswith("user_overlays=") and not line.startswith("overlays=") for line in lines), "Orange Armbian overlay assignment is malformed or commented")
    user_assignments = [line.partition("=")[2].split() for line in lines if line.startswith("user_overlays=")]
    overlay_assignments = [line.partition("=")[2].split() for line in lines if line.startswith("overlays=")]
    required_user_tokens = ["octessera-h618-spi1-cs0", "octessera-h618-input-routing", "octessera-ahub0-pcm5102"]
    require(user_assignments == [required_user_tokens], "Orange Armbian user_overlays assignment is not exact")
    require(overlay_assignments == [["i2c1-pi"]], "Orange Armbian overlays assignment is not exact")
    spi_validation = repository_root / "userpatches/overlay/usr/local/share/octessera/device-tree/spi-overlay-validation.sh"
    input_validation = repository_root / "userpatches/overlay/usr/local/share/octessera/device-tree/input-routing-overlay-validation.sh"
    audio_validation = repository_root / "userpatches/overlay/usr/local/share/octessera/device-tree/orange-ahub-overlay-validation.sh"
    stock_dtbo = selected_dtb.parent / "overlay/sun50i-h616-i2c1-pi.dtbo"
    require(stock_dtbo.is_file() and not stock_dtbo.is_symlink(), "stock i2c1-pi DTBO is missing or symlinked")
    require(stock_dtbo.relative_to(root).as_posix() == package["stock_i2c1_dtbo_path"], "selected stock i2c1-pi DTBO path is not package-bound")
    require(sha256_file(stock_dtbo) == package["stock_i2c1_dtbo_sha256"] and stock_dtbo.read_bytes() == package["stock_i2c1_dtbo"], "installed stock i2c1-pi DTBO differs from the supplied linux-dtb package")
    shell = "set -e; source \"$1\"; source \"$2\"; source \"$3\"; fdtoverlay -i \"$4\" -o \"$9/stock.dtb\" \"$5\"; fdtoverlay -i \"$9/stock.dtb\" -o \"$9/spi.dtb\" \"$6\"; fdtoverlay -i \"$9/spi.dtb\" -o \"$9/spi-input.dtb\" \"$7\"; fdtoverlay -i \"$9/spi-input.dtb\" -o \"$9/merged.dtb\" \"$8\"; octessera_assert_orange_preserved_peripherals \"$4\" \"$9/stock.dtb\" Orange-proof-stock; octessera_assert_spi1_merge \"$9/stock.dtb\" \"$9/merged.dtb\" \"$(fdtget -t s \"$4\" /__symbols__ spi1)\" \"$(fdtget -t s \"$4\" /__symbols__ spi1_pins)\" \"$(fdtget -t s \"$4\" /__symbols__ spi1_cs0_pin)\" \"$(fdtget -t s \"$4\" /__symbols__ spi0)\" \"$(fdtget -t s \"$4\" /__symbols__ i2c1)\" Orange-proof; octessera_assert_input_routing_merge \"$9/spi.dtb\" \"$9/merged.dtb\" \"$(fdtget -t s \"$4\" /__symbols__ uart0)\" \"$(fdtget -t s \"$4\" /__symbols__ pio)\" /chosen Orange-proof; octessera_assert_orange_audio_merge \"$9/spi-input.dtb\" \"$9/merged.dtb\" Orange-proof; octessera_assert_orange_preserved_peripherals \"$4\" \"$9/merged.dtb\" Orange-proof-full"
    try:
        with tempfile.TemporaryDirectory(prefix="octessera-orange-audio-proof-") as work:
            subprocess.run(["bash", "-c", shell, "orange-audio-proof", str(spi_validation), str(input_validation), str(audio_validation), str(selected_dtb), str(stock_dtbo), str(root / "boot/overlay-user/octessera-h618-spi1-cs0.dtbo"), str(root / "boot/overlay-user/octessera-h618-input-routing.dtbo"), str(dtbo), work], check=True, capture_output=True, text=True)
    except FileNotFoundError:
        require(False, "Orange audio DTBO topology or overlay composition proof failed")
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or "").strip().splitlines()
        require(False, f"Orange audio DTBO topology or overlay composition proof failed: {detail[-1] if detail else 'no diagnostic'}")
    return {"dts_sha256": sha256_file(installed), "dtbo_sha256": sha256_file(dtbo), "dtbo_path": dtbo_relative}
