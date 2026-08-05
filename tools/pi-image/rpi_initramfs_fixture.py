from __future__ import annotations

import gzip


def _cpio_record(name: str, payload: bytes, mode: int) -> bytes:
    fields = (1, mode, 0, 0, 1, 0, len(payload), 0, 0, 0, 0, len(name) + 1, 0)
    header = b"070701" + b"".join(f"{value:08x}".encode() for value in fields)
    named = header + name.encode() + b"\0"
    return named + b"\0" * (-len(named) % 4) + payload + b"\0" * (-len(payload) % 4)


def make_splash_initramfs(
    script: bytes,
    binary: bytes,
    script_path: str = "scripts/init-premount/octessera-boot-splash",
) -> bytes:
    directories: list[str] = []
    for path in (
        script_path,
        "usr/local/bin/octessera-pi",
        "usr/bin/setsid",
        "bin/sh",
        "lib/modules/fixture/spi-bcm2835.ko",
    ):
        parts = path.split("/")[:-1]
        directories.extend("/".join(parts[:index]) for index in range(1, len(parts) + 1))
    directories = list(dict.fromkeys(directories))
    archive = b"".join(_cpio_record(directory, b"", 0o040755) for directory in directories)
    archive += _cpio_record(script_path, script, 0o100755)
    archive += _cpio_record("usr/local/bin/octessera-pi", binary, 0o100755)
    archive += b"".join(
        _cpio_record(name, b"fixture", 0o100755)
        for name in (
            "usr/bin/setsid",
            "bin/sh",
            "bin/sleep",
            "bin/cat",
            "bin/mv",
            "bin/chmod",
            "bin/chown",
            "bin/rm",
            "lib/modules/fixture/spi-bcm2835.ko",
            "lib/modules/fixture/spidev.ko",
        )
    )
    archive += _cpio_record("TRAILER!!!", b"", 0)
    return gzip.compress(archive, mtime=0)
