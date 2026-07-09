from __future__ import annotations

import math
import re
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import cadquery as cq


ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets"
MARK_SVG = ASSETS / "octessera-mark.svg"
WORDMARK_SVG = ASSETS / "octessera-wordmark.svg"
WEST_TO_OLED_CENTER_X = 25.44
MARK_SIZE = 14.0
MARK_TOP_Y = 132.5
WORDMARK_WIDTH = 39.0
WORDMARK_TOP_Y = 120.0


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
        for cx, cy, radius in re.findall(r'<circle\s+cx="([^"]+)"\s+cy="([^"]+)"\s+r="([^"]+)"', svg)
    ]
    return paths, circles


def parse_wordmark_polygons() -> list[list[Point]]:
    svg = WORDMARK_SVG.read_text(encoding="utf-8")
    polygons = []
    for path_data in re.findall(r'<path\s+d="([^"]+)"', svg):
        nums = [parse_svg_number(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]
        if len(nums) >= 6:
            polygons.append([Point(nums[index], nums[index + 1]) for index in range(0, len(nums), 2)])
    return polygons


def bounds(points: list[Point]) -> tuple[float, float, float, float]:
    return min(point.x for point in points), min(point.y for point in points), max(point.x for point in points), max(point.y for point in points)


def bounds_contains(outer: tuple[float, float, float, float], inner: tuple[float, float, float, float]) -> bool:
    return outer[0] <= inner[0] and outer[1] <= inner[1] and outer[2] >= inner[2] and outer[3] >= inner[3]


def point_in_polygon(point: Point, polygon: list[Point]) -> bool:
    inside = False
    previous = polygon[-1]
    for current in polygon:
        if (current.y > point.y) != (previous.y > point.y):
            x_at_y = (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y) + current.x
            if point.x < x_at_y:
                inside = not inside
        previous = current
    return inside


def polygon_center(polygon: list[Point]) -> Point:
    return Point(sum(point.x for point in polygon) / len(polygon), sum(point.y for point in polygon) / len(polygon))


def transform(point: Point, source: tuple[float, float, float, float], scale: float, x0: float, top_y: float) -> Point:
    min_x, min_y, _, _ = source
    return Point(x0 + (point.x - min_x) * scale, top_y - (point.y - min_y) * scale)


def segment_marking(start: Point, end: Point, width: float, z0: float, height: float) -> cq.Workplane:
    dx = end.x - start.x
    dy = end.y - start.y
    length = math.hypot(dx, dy)
    angle = math.degrees(math.atan2(dy, dx))
    return (
        cq.Workplane("XY")
        .box(length, width, height, centered=(True, True, False))
        .rotate((0, 0, 0), (0, 0, 1), angle)
        .translate(((start.x + end.x) / 2, (start.y + end.y) / 2, z0))
    )


def compound(parts: list[cq.Workplane]) -> cq.Workplane:
    solids = cast(list[cq.Shape], [solid for part in parts for solid in part.solids().vals()])
    return cq.Workplane("XY").add(cq.Compound.makeCompound(solids))


def wordmark_marking(z0: float, height: float) -> cq.Workplane:
    polygons = parse_wordmark_polygons()
    word_points = [point for polygon in polygons for point in polygon]
    word_source = bounds(word_points)
    word_scale = WORDMARK_WIDTH / (word_source[2] - word_source[0])
    wordmark_x = WEST_TO_OLED_CENTER_X - WORDMARK_WIDTH / 2.0
    transformed = [
        [transform(point, word_source, word_scale, wordmark_x, WORDMARK_TOP_Y) for point in polygon]
        for polygon in polygons
    ]
    positives: list[cq.Workplane] = []
    negatives: list[cq.Workplane] = []
    polygon_bounds = [bounds(polygon) for polygon in transformed]
    for index, polygon in enumerate(transformed):
        center = polygon_center(polygon)
        depth = sum(
            1
            for other_index, other in enumerate(transformed)
            if other_index != index and bounds_contains(polygon_bounds[other_index], polygon_bounds[index]) and point_in_polygon(center, other)
        )
        solid = cq.Workplane("XY").polyline([(point.x, point.y) for point in polygon]).close().extrude(height).translate((0, 0, z0))
        if depth % 2 == 0:
            positives.append(solid)
        else:
            negatives.append(solid)

    result = positives[0]
    for positive in positives[1:]:
        result = result.union(positive)
    for negative in negatives:
        result = result.cut(negative)
    return result.clean()


def make_branding_marking(z0: float, height: float = 0.65) -> cq.Workplane:
    mark_x = WEST_TO_OLED_CENTER_X - MARK_SIZE / 2.0
    mark_paths, mark_circles = parse_mark()
    mark_points = [point for path in mark_paths for point in path]
    mark_points += [point for circle in mark_circles for point in [Point(circle.center.x - circle.radius, circle.center.y - circle.radius), Point(circle.center.x + circle.radius, circle.center.y + circle.radius)]]
    mark_source = bounds(mark_points)
    mark_scale = MARK_SIZE / max(mark_source[2] - mark_source[0], mark_source[3] - mark_source[1])
    parts: list[cq.Workplane] = []
    for path in mark_paths:
        transformed = [transform(point, mark_source, mark_scale, mark_x, MARK_TOP_Y) for point in path]
        for start, end in zip(transformed, transformed[1:]):
            parts.append(segment_marking(start, end, 6.5 * mark_scale, z0, height))
    for circle in mark_circles:
        center = transform(circle.center, mark_source, mark_scale, mark_x, MARK_TOP_Y)
        parts.append(cq.Workplane("XY").circle(circle.radius * mark_scale).extrude(height).translate((center.x, center.y, z0)))

    parts.append(wordmark_marking(z0, height))
    return compound(parts).clean()
