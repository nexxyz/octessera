from __future__ import annotations

import gzip


def _cpio_record(name: str, payload: bytes, mode: int, links: int = 1) -> bytes:
    fields = (1, mode, 0, 0, links, 0, len(payload), 0, 0, 0, 0, len(name) + 1, 0)
    header = b"070701" + b"".join(f"{value:08x}".encode() for value in fields)
    named = header + name.encode() + b"\0"
    return named + b"\0" * (-len(named) % 4) + payload + b"\0" * (-len(payload) % 4)


def canonical_command_records() -> tuple[tuple[str, bytes, int, int], ...]:
    return (
        ("bin", b"usr/bin", 0o120777, 1),
        ("usr/bin/sh", b"dash", 0o120777, 1),
        ("usr/bin/dash", b"fixture", 0o100755, 1),
        ("usr/bin/setsid", b"fixture", 0o100755, 1),
        ("usr/bin/sleep", b"fixture", 0o100755, 1),
        ("usr/bin/cat", b"fixture", 0o100755, 1),
        ("usr/bin/mv", b"fixture", 0o100755, 1),
        ("usr/bin/chmod", b"fixture", 0o100755, 1),
        ("usr/bin/chown", b"fixture", 0o100755, 1),
        ("usr/bin/rm", b"fixture", 0o100755, 1),
    )


def make_splash_initramfs(
    script: bytes,
    binary: bytes,
    script_path: str = "scripts/init-premount/octessera-boot-splash",
    command_records: tuple[tuple[str, bytes, int, int], ...] | None = None,
    runtime_record: tuple[bytes, int, int] | None = None,
) -> bytes:
    command_records = canonical_command_records() if command_records is None else command_records
    runtime_payload, runtime_mode, runtime_links = runtime_record or (binary, 0o100755, 1)
    record_names = {name for name, _, _, _ in command_records}
    directories: list[str] = []
    for path in (
        script_path,
        "usr/local/bin/octessera-pi",
        *(name for name, _, _, _ in command_records),
        "lib/modules/fixture/spi-bcm2835.ko",
    ):
        parts = path.split("/")[:-1]
        directories.extend("/".join(parts[:index]) for index in range(1, len(parts) + 1))
    directories = [directory for directory in dict.fromkeys(directories) if directory not in record_names]
    archive = b"".join(_cpio_record(directory, b"", 0o040755) for directory in directories)
    archive += _cpio_record(script_path, script, 0o100755)
    archive += _cpio_record("usr/local/bin/octessera-pi", runtime_payload, runtime_mode, runtime_links)
    archive += b"".join(_cpio_record(name, payload, mode, links) for name, payload, mode, links in command_records)
    archive += b"".join(
        _cpio_record(name, b"fixture", 0o100755)
        for name in (
            "lib/modules/fixture/spi-bcm2835.ko",
            "lib/modules/fixture/spidev.ko",
        )
    )
    archive += _cpio_record("TRAILER!!!", b"", 0)
    return gzip.compress(archive, mtime=0)
