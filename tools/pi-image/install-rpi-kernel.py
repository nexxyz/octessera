#!/usr/bin/env python3
from pathlib import Path
from runpy import run_path

CANONICAL = Path(__file__).resolve().parent / "stage3-octessera-kernel/files/root/usr/local/lib/octessera/install-rpi-kernel.py"
globals().update(run_path(str(CANONICAL), run_name=__name__))
