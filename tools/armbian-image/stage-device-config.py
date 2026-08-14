#!/usr/bin/env python3
import os
import shutil
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
DEFAULT_OVERLAY = ROOT / "userpatches/overlay"


def main() -> None:
    overlay = Path(sys.argv[1]) if len(sys.argv) == 2 else DEFAULT_OVERLAY
    if len(sys.argv) > 2:
        raise SystemExit("usage: stage-device-config.py [overlay]")
    destination = overlay / "usr/local/lib/octessera/device_config.py"
    if not SOURCE.is_file() or SOURCE.is_symlink():
        raise SystemExit(f"Canonical device config source is missing or symlinked: {SOURCE}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(SOURCE, destination)
    os.chmod(destination, 0o644)
    if SOURCE.read_bytes() != destination.read_bytes():
        raise SystemExit("Staged device config differs from canonical source.")


if __name__ == "__main__":
    main()
