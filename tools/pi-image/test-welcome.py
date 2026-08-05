#!/usr/bin/env python3
from __future__ import annotations

import os
import pty
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WELCOME = ROOT / "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh"
MARKER = b"cellular automata -> music"


def _pty_output(command: str) -> bytes:
    master, slave = pty.openpty()  # type: ignore[attr-defined]
    try:
        process = subprocess.Popen(["bash", "--noprofile", "--norc", "-i", "-c", command], stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        output = bytearray()
        while True:
            try:
                chunk = os.read(master, 4096)
            except OSError:
                break
            if not chunk:
                break
            output.extend(chunk)
        process.wait(timeout=10)
        return bytes(output)
    finally:
        os.close(master)


def main() -> int:
    path = str(WELCOME)
    assert _pty_output(f"source {path}").count(MARKER) == 1
    assert _pty_output(f"source {path}; source {path}; ( source {path} )").count(MARKER) == 1
    assert MARKER not in _pty_output(f"OCTESSERA_WELCOME_SHOWN=1; export OCTESSERA_WELCOME_SHOWN; source {path}")
    with tempfile.TemporaryDirectory() as temporary:
        redirected = Path(temporary) / "welcome.txt"
        subprocess.run(["bash", "-c", f"source {path}"], check=True, stdout=redirected.open("wb"), stderr=subprocess.DEVNULL)
        assert redirected.read_bytes() == b""
        with redirected.open("wb") as output:
            subprocess.run(["bash", "-i", "-c", f"source {path}"], check=True, stdout=output, stderr=subprocess.DEVNULL)
        assert redirected.read_bytes() == b""
    print("Octessera welcome PTY and noninteractive tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
