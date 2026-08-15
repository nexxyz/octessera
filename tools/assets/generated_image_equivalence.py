"""Strict semantic validation for the PNG and ICO files owned by the logo generator."""
from __future__ import annotations

import struct
import zlib
from pathlib import Path


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
PNG_SIZE = 128
PNG_IHDR_SIZE = 13
PNG_MAX_CHUNK = 1 << 20
PNG_RAW_SIZE = PNG_SIZE * (1 + PNG_SIZE * 4)
RGB565_SIZE = PNG_SIZE * PNG_SIZE * 2
ICO_HEADER_SIZE = 6
ICO_ENTRY_SIZE = 16


def _png_semantics(data: bytes) -> tuple[bytes, bytes]:
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError("invalid PNG signature")
    position = len(PNG_SIGNATURE)
    chunks: list[tuple[bytes, bytes]] = []
    while position < len(data):
        if len(data) - position < 12:
            raise ValueError("truncated PNG chunk")
        length = struct.unpack_from(">I", data, position)[0]
        if length > PNG_MAX_CHUNK:
            raise ValueError("PNG chunk is too large")
        end = position + 12 + length
        if end > len(data):
            raise ValueError("truncated PNG chunk data")
        kind = data[position + 4 : position + 8]
        payload = data[position + 8 : position + 8 + length]
        crc = struct.unpack_from(">I", data, position + 8 + length)[0]
        if zlib.crc32(kind + payload) & 0xFFFFFFFF != crc:
            raise ValueError("invalid PNG chunk CRC")
        chunks.append((kind, payload))
        position = end
        if kind == b"IEND":
            break
    if position != len(data) or [kind for kind, _ in chunks] != [b"IHDR", b"IDAT", b"IEND"]:
        raise ValueError("unexpected PNG chunk order or trailing data")
    if chunks[2][1]:
        raise ValueError("invalid PNG IEND payload")
    ihdr = chunks[0][1]
    if len(ihdr) != PNG_IHDR_SIZE or ihdr != struct.pack(">IIBBBBB", PNG_SIZE, PNG_SIZE, 8, 6, 0, 0, 0):
        raise ValueError("unsupported PNG layout")
    compressed = chunks[1][1]
    stream = zlib.decompressobj()
    try:
        raw = stream.decompress(compressed, PNG_RAW_SIZE + 1)
        if stream.unconsumed_tail:
            raise ValueError("PNG decompression exceeds bound")
        raw += stream.flush()
    except zlib.error as error:
        raise ValueError("invalid PNG zlib stream") from error
    if not stream.eof or stream.unused_data or stream.unconsumed_tail or len(raw) != PNG_RAW_SIZE:
        raise ValueError("invalid PNG zlib bounds")
    stride = PNG_SIZE * 4
    if any(raw[row * (stride + 1)] != 0 for row in range(PNG_SIZE)):
        raise ValueError("PNG uses unsupported row filter")
    # Keep the non-IDAT chunks in the compared metadata. Only IDAT's encoded
    # representation, length, and CRC are intentionally ignored.
    return data[8 : 8 + 12 + len(ihdr)] + data[-12:], raw


def _ico_semantics(data: bytes) -> tuple[bytes, bytes]:
    if len(data) < ICO_HEADER_SIZE + ICO_ENTRY_SIZE:
        raise ValueError("truncated ICO")
    reserved, kind, count = struct.unpack_from("<HHH", data)
    if (reserved, kind, count) != (0, 1, 1):
        raise ValueError("unsupported ICO header")
    width, height, colors, reserved_entry, planes, bits, size, offset = struct.unpack_from(
        "<BBBBHHII", data, ICO_HEADER_SIZE
    )
    if (width, height, colors, reserved_entry, planes, bits) != (128, 128, 0, 0, 1, 32):
        raise ValueError("unsupported ICO image layout")
    if offset != ICO_HEADER_SIZE + ICO_ENTRY_SIZE or size != len(data) - offset:
        raise ValueError("invalid ICO image bounds")
    return _png_semantics(data[offset:])


def _rgb565_semantics(data: bytes) -> tuple[bytes, bytes]:
    if len(data) != RGB565_SIZE:
        raise ValueError("unsupported RGB565 asset size")
    return b"rgb565", data


def image_semantics(data: bytes, kind: str) -> tuple[bytes, bytes]:
    if kind == "png":
        return _png_semantics(data)
    if kind == "ico":
        return _ico_semantics(data)
    if kind == "rgb565":
        return _rgb565_semantics(data)
    raise ValueError(f"unsupported generated image kind: {kind}")


def images_equivalent(left: bytes | Path, right: bytes | Path, kind: str) -> bool:
    def read(value: bytes | Path) -> bytes:
        return value if isinstance(value, bytes) else value.read_bytes()

    try:
        return image_semantics(read(left), kind) == image_semantics(read(right), kind)
    except (OSError, ValueError, struct.error):
        return False
