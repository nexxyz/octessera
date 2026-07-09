#!/usr/bin/env python3
from __future__ import annotations

import re
import struct
import zlib
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets"
MARK_SVG = ASSETS / "octessera-mark.svg"
WORDMARK_SVG = ASSETS / "octessera-wordmark.svg"
SIZE = 128
SCALE = 4
WHITE = 255
BLACK = 0


@dataclass(frozen=True)
class Point:
    x: float
    y: float


@dataclass(frozen=True)
class Circle:
    center: Point
    radius: float


def parse_svg_number(value: str) -> float:
    return float(value.strip())


def parse_mark() -> tuple[list[list[Point]], list[Circle]]:
    svg = MARK_SVG.read_text(encoding="utf-8")
    paths = []
    for path_data in re.findall(r'<path\s+d="([^"]+)"', svg):
        nums = [parse_svg_number(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]
        paths.append([Point(nums[index], nums[index + 1]) for index in range(0, len(nums), 2)])
    circles = [
        Circle(Point(parse_svg_number(cx), parse_svg_number(cy)), parse_svg_number(radius))
        for cx, cy, radius in re.findall(
            r'<circle\s+cx="([^"]+)"\s+cy="([^"]+)"\s+r="([^"]+)"', svg
        )
    ]
    if not paths and not circles:
        raise SystemExit(f"No supported mark primitives found in {MARK_SVG}")
    return paths, circles


def parse_wordmark_text() -> str:
    svg = WORDMARK_SVG.read_text(encoding="utf-8")
    match = re.search(r">\s*([A-Z0-9 ]+)\s*</text>", svg)
    return match.group(1) if match else "OCTESSERA"


def parse_wordmark_polygons() -> list[list[Point]]:
    svg = WORDMARK_SVG.read_text(encoding="utf-8")
    polygons: list[list[Point]] = []
    for path_data in re.findall(r'<path\s+d="([^"]+)"', svg):
        nums = [parse_svg_number(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]
        if len(nums) >= 6:
            polygons.append([Point(nums[index], nums[index + 1]) for index in range(0, len(nums), 2)])
    if not polygons:
        raise SystemExit(f"No vectorized wordmark paths found in {WORDMARK_SVG}")
    return polygons


def primitive_bounds(paths: list[list[Point]], circles: list[Circle]) -> tuple[float, float, float, float]:
    xs: list[float] = []
    ys: list[float] = []
    for path in paths:
        for point in path:
            xs.append(point.x)
            ys.append(point.y)
    for circle in circles:
        xs.extend([circle.center.x - circle.radius, circle.center.x + circle.radius])
        ys.extend([circle.center.y - circle.radius, circle.center.y + circle.radius])
    return min(xs), min(ys), max(xs), max(ys)


def polygon_bounds(polygons: list[list[Point]]) -> tuple[float, float, float, float]:
    xs = [point.x for polygon in polygons for point in polygon]
    ys = [point.y for polygon in polygons for point in polygon]
    return min(xs), min(ys), max(xs), max(ys)


def make_canvas() -> list[list[int]]:
    return [[BLACK for _ in range(SIZE * SCALE)] for _ in range(SIZE * SCALE)]


def set_pixel(canvas: list[list[int]], x: int, y: int, value: int = WHITE) -> None:
    if 0 <= y < len(canvas) and 0 <= x < len(canvas[y]):
        canvas[y][x] = value


def draw_disk(canvas: list[list[int]], center: Point, radius: float) -> None:
    min_x = int(center.x - radius - 1)
    max_x = int(center.x + radius + 1)
    min_y = int(center.y - radius - 1)
    max_y = int(center.y + radius + 1)
    radius_sq = radius * radius
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            if (x + 0.5 - center.x) ** 2 + (y + 0.5 - center.y) ** 2 <= radius_sq:
                set_pixel(canvas, x, y)


def distance_to_segment(point: Point, start: Point, end: Point) -> float:
    dx = end.x - start.x
    dy = end.y - start.y
    length_sq = dx * dx + dy * dy
    if length_sq == 0:
        return ((point.x - start.x) ** 2 + (point.y - start.y) ** 2) ** 0.5
    t = max(0.0, min(1.0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_sq))
    projected = Point(start.x + t * dx, start.y + t * dy)
    return ((point.x - projected.x) ** 2 + (point.y - projected.y) ** 2) ** 0.5


def draw_segment(canvas: list[list[int]], start: Point, end: Point, width: float) -> None:
    radius = width / 2
    min_x = int(min(start.x, end.x) - radius - 1)
    max_x = int(max(start.x, end.x) + radius + 1)
    min_y = int(min(start.y, end.y) - radius - 1)
    max_y = int(max(start.y, end.y) + radius + 1)
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            if distance_to_segment(Point(x + 0.5, y + 0.5), start, end) <= radius:
                set_pixel(canvas, x, y)


def point_in_polygon(point: Point, polygon: list[Point]) -> bool:
    inside = False
    previous = polygon[-1]
    for current in polygon:
        crosses = (current.y > point.y) != (previous.y > point.y)
        if crosses:
            x_at_y = (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y) + current.x
            if point.x < x_at_y:
                inside = not inside
        previous = current
    return inside


def transform(point: Point, bounds: tuple[float, float, float, float], target: float, center: Point) -> Point:
    min_x, min_y, max_x, max_y = bounds
    scale = target / max(max_x - min_x, max_y - min_y)
    source_center = Point((min_x + max_x) / 2, (min_y + max_y) / 2)
    return Point(
        (point.x - source_center.x) * scale + center.x,
        (point.y - source_center.y) * scale + center.y,
    )


def draw_mark(canvas: list[list[int]], target_size: float, center_x: float, center_y: float) -> None:
    paths, circles = parse_mark()
    bounds = primitive_bounds(paths, circles)
    high_target = target_size * SCALE
    high_center = Point(center_x * SCALE, center_y * SCALE)
    min_x, min_y, max_x, max_y = bounds
    mark_scale = high_target / max(max_x - min_x, max_y - min_y)
    for path in paths:
        transformed = [transform(point, bounds, high_target, high_center) for point in path]
        for start, end in zip(transformed, transformed[1:]):
            draw_segment(canvas, start, end, 6.5 * mark_scale)
    for circle in circles:
        draw_disk(canvas, transform(circle.center, bounds, high_target, high_center), circle.radius * mark_scale)


def draw_wordmark(canvas: list[list[int]], target_width: float, center_x: float, center_y: float) -> None:
    polygons = parse_wordmark_polygons()
    bounds = polygon_bounds(polygons)
    min_x, min_y, max_x, max_y = bounds
    source_width = max_x - min_x
    source_height = max_y - min_y
    scale = (target_width * SCALE) / source_width
    target_height = source_height * scale
    source_center = Point((min_x + max_x) / 2, (min_y + max_y) / 2)
    target_center = Point(center_x * SCALE, center_y * SCALE)
    transformed = [
        [
            Point(
                (point.x - source_center.x) * scale + target_center.x,
                (point.y - source_center.y) * scale + target_center.y,
            )
            for point in polygon
        ]
        for polygon in polygons
    ]
    min_draw_x = round(center_x * SCALE - target_width * SCALE / 2) - 1
    max_draw_x = round(center_x * SCALE + target_width * SCALE / 2) + 1
    min_draw_y = round(center_y * SCALE - target_height / 2) - 1
    max_draw_y = round(center_y * SCALE + target_height / 2) + 1
    for y in range(min_draw_y, max_draw_y + 1):
        for x in range(min_draw_x, max_draw_x + 1):
            point = Point(x + 0.5, y + 0.5)
            if sum(1 for polygon in transformed if point_in_polygon(point, polygon)) % 2 == 1:
                set_pixel(canvas, x, y)


def downsample_grayscale(canvas: list[list[int]]) -> bytes:
    rows = []
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            total = 0
            for sy in range(SCALE):
                for sx in range(SCALE):
                    total += canvas[y * SCALE + sy][x * SCALE + sx]
            value = round(total / (SCALE * SCALE))
            row.extend((value, value, value, WHITE))
        rows.append(bytes(row))
    return b"".join(rows)


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


def write_png(path: Path, rgba: bytes) -> None:
    raw_rows = []
    stride = SIZE * 4
    for y in range(SIZE):
        raw_rows.append(b"\x00" + rgba[y * stride : (y + 1) * stride])
    data = b"\x89PNG\r\n\x1a\n"
    data += png_chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    data += png_chunk(b"IDAT", zlib.compress(b"".join(raw_rows), 9))
    data += png_chunk(b"IEND", b"")
    path.write_bytes(data)


def save_mark(path: Path) -> None:
    canvas = make_canvas()
    draw_mark(canvas, target_size=80, center_x=64, center_y=64)
    write_png(path, downsample_grayscale(canvas))


def save_stacked_logo(path: Path) -> None:
    canvas = make_canvas()
    draw_mark(canvas, target_size=58, center_x=64, center_y=52)
    draw_wordmark(canvas, target_width=104, center_x=64, center_y=92)
    write_png(path, downsample_grayscale(canvas))


def main() -> None:
    _ = parse_wordmark_text()
    save_stacked_logo(ASSETS / "octessera-pi-manifest.png")
    save_mark(ASSETS / "octessera-pi-sleeping.png")
    save_mark(ASSETS / "octessera-pi-shutdown.png")
    save_stacked_logo(ASSETS / "octessera-pi-booting.png")


if __name__ == "__main__":
    main()
