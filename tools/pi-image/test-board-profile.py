#!/usr/bin/env python3
import importlib.util
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("package-rpi-imager-zip.py")
SPEC = importlib.util.spec_from_file_location("package_rpi_imager_zip", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

MODULE.require_raspberry_board_profile(MODULE.RASPBERRY_PI_ZERO_2W_PROFILE_ID)
repository = Path(__file__).parents[2]
provision_script = (repository / "tools/pi/provision/provision.sh").read_text(encoding="utf-8")
if 'if [ "$SERVICE" != octessera.service ]' not in provision_script:
    raise AssertionError("Shell provisioning does not reject non-default service names")
for value in (
    MODULE.ORANGE_PI_ZERO_2W_PROFILE_ID,
    "opi-zero-2w",
    "rpi-zero-2w",
    "pi-zero-2w",
):
    try:
        MODULE.require_raspberry_board_profile(value)
    except SystemExit:
        pass
    else:
        raise AssertionError(f"Raspberry packaging accepted non-canonical profile {value}")

print("Raspberry board profile validation passed")
