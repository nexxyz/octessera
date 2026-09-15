from __future__ import annotations

import hashlib
import re


def normalize_kernel_config(config: bytes) -> bytes:
    lines = config.splitlines(keepends=True)
    rustc_lines: list[int] = []
    for index, line in enumerate(lines):
        content = line.rstrip(b"\r\n")
        if re.match(rb"^(?:#\s*)?CONFIG_RUSTC_VERSION(?:=|\s|$)", content):
            if not re.fullmatch(rb"CONFIG_RUSTC_VERSION=[0-9]+", content):
                raise ValueError("kernel config contains a malformed CONFIG_RUSTC_VERSION line")
            rustc_lines.append(index)
    if len(rustc_lines) != 1:
        raise ValueError("kernel config must contain exactly one CONFIG_RUSTC_VERSION=[0-9]+ line")
    index = rustc_lines[0]
    content = lines[index].rstrip(b"\r\n")
    lines[index] = b"CONFIG_RUSTC_VERSION=<normalized>" + lines[index][len(content) :]
    return b"".join(lines)


def kernel_config_hashes(config: bytes) -> tuple[str, str]:
    normalized = normalize_kernel_config(config)
    return hashlib.sha256(config).hexdigest(), hashlib.sha256(normalized).hexdigest()
