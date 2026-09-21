#!/usr/bin/env python3
import importlib.util
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("package-rpi-imager-zip.py")
SPEC = importlib.util.spec_from_file_location("package_rpi_imager_zip", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

MODULE.require_raspberry_board_profile(MODULE.RASPBERRY_PI_ZERO_2W_PROFILE_ID)
manifest = MODULE.build_manifest(
    version="0.8.7",
    image_url="https://example.invalid/image.zip",
    icon_url="https://example.invalid/icon.png",
    release_date="2026-09-18",
    extract_size=1,
    extract_sha256="0" * 64,
    image_download_size=1,
)
if manifest["os_list"][0]["description"] != (
    "Ready-to-flash Raspberry Pi OS Lite image with octessera hardware services preinstalled. "
    "Raspberry Pi Imager may configure SSH, hostname, and Wi-Fi, but the username must remain pi."
):
    raise AssertionError("Raspberry Imager manifest does not require the fixed pi username")
repository = Path(__file__).parents[2]
provision_script = (repository / "tools/pi/provision/provision.sh").read_text(encoding="utf-8")
if 'if [ "$SERVICE" != octessera.service ]' not in provision_script:
    raise AssertionError("Shell provisioning does not reject non-default service names")
for value in (
    MODULE.ORANGE_PI_ZERO_2W_PROFILE_ID,
    "opi-zero-2w",
    "unknown-board",
    "pi-zero-2w",
):
    try:
        MODULE.require_raspberry_board_profile(value)
    except SystemExit:
        pass
    else:
        raise AssertionError(f"Raspberry packaging accepted non-canonical profile {value}")

print("Raspberry board profile validation passed")
