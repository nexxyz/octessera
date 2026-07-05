from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent
GUIDANCE_SVG = ROOT / "wave_curve_guidance.svg"

SLOPE_PROFILE_STEPS = 12
SOUTH_SHOULDER_SAMPLES = 64
SOUTH_SHOULDER_PLAN_WIDTH = 8.5
SOUTH_ROOF_LOW_EAVE_WIDTH = 0.0
SOUTH_ROOF_LOW_WALL_BAND = 3.0
SVG_TEMPLATE_HEIGHT = 900.0
SVG_TEMPLATE_MARGIN = 30.0
SVG_TEMPLATE_SCALE = 6.0

FALLBACK_SOUTH_HIGH_EDGE_PATH = [
    ("line", (650.0, 30.0), (650.0, 450.0)),
    ("cubic", (650.0, 450.0), (650.0, 620.0), (600.0, 695.0), (380.0, 695.0)),
    ("cubic", (380.0, 695.0), (250.0, 695.0), (130.0, 720.0), (130.0, 869.0)),
]
FALLBACK_SOUTH_LOW_EDGE_PATH = [
    ("line", (600.0, 30.0), (600.0, 400.0)),
    ("cubic", (600.0, 400.0), (600.0, 550.0), (600.0, 650.0), (330.0, 645.0)),
    ("cubic", (330.0, 645.0), (200.0, 645.0), (80.0, 700.0), (80.0, 869.0)),
]


def smootherstep(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0)


def svg_point_to_case(point: tuple[float, float]) -> tuple[float, float]:
    x, y = point
    return (
        (x - SVG_TEMPLATE_MARGIN) / SVG_TEMPLATE_SCALE,
        (SVG_TEMPLATE_HEIGHT - SVG_TEMPLATE_MARGIN - y) / SVG_TEMPLATE_SCALE,
    )


def cubic_point(
    p0: tuple[float, float],
    p1: tuple[float, float],
    p2: tuple[float, float],
    p3: tuple[float, float],
    t: float,
) -> tuple[float, float]:
    u = 1.0 - t
    return (
        u**3 * p0[0] + 3.0 * u**2 * t * p1[0] + 3.0 * u * t**2 * p2[0] + t**3 * p3[0],
        u**3 * p0[1] + 3.0 * u**2 * t * p1[1] + 3.0 * u * t**2 * p2[1] + t**3 * p3[1],
    )


def parse_path_d(d: str) -> tuple:
    numbers = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", d)]
    if " C " in f" {d} ":
        return (
            "cubic",
            (numbers[0], numbers[1]),
            (numbers[2], numbers[3]),
            (numbers[4], numbers[5]),
            (numbers[6], numbers[7]),
        )
    return ("line", (numbers[0], numbers[1]), (numbers[2], numbers[3]))


def load_guidance_paths() -> tuple[list[tuple], list[tuple]]:
    if not GUIDANCE_SVG.exists():
        return FALLBACK_SOUTH_HIGH_EDGE_PATH, FALLBACK_SOUTH_LOW_EDGE_PATH
    high = []
    low = []
    for match in re.finditer(r'<path\s+[^>]*d="([^"]+)"[^>]*stroke="([^"]+)"', GUIDANCE_SVG.read_text(encoding="utf-8")):
        d, stroke = match.groups()
        segment = parse_path_d(d)
        first_point = segment[1]
        if stroke == "green" or first_point[0] >= 625.0:
            high.append(segment)
        elif stroke == "brown":
            low.append(segment)
    return high or FALLBACK_SOUTH_HIGH_EDGE_PATH, low or FALLBACK_SOUTH_LOW_EDGE_PATH


def load_guidance_slots() -> list[tuple[tuple[float, float], tuple[float, float]]]:
    if not GUIDANCE_SVG.exists():
        return []
    slots = []
    pattern = r'<line\s+[^>]*x1="([^"]+)"\s+y1="([^"]+)"\s+x2="([^"]+)"\s+y2="([^"]+)"[^>]*stroke="#333"'
    for match in re.finditer(pattern, GUIDANCE_SVG.read_text(encoding="utf-8")):
        x1, y1, x2, y2 = (float(value) for value in match.groups())
        slots.append((svg_point_to_case((x1, y1)), svg_point_to_case((x2, y2))))
    return slots


def sample_svg_path(path: list[tuple]) -> list[tuple[float, float]]:
    points = []
    steps_per_segment = max(2, SOUTH_SHOULDER_SAMPLES // len(path))
    for segment_index, segment in enumerate(path):
        kind = segment[0]
        for step in range(steps_per_segment + 1):
            if segment_index > 0 and step == 0:
                continue
            t = step / steps_per_segment
            if kind == "line":
                _, p0, p1 = segment
                x = p0[0] + (p1[0] - p0[0]) * t
                y = p0[1] + (p1[1] - p0[1]) * t
            else:
                _, p0, p1, p2, p3 = segment
                x, y = cubic_point(p0, p1, p2, p3, t)
            points.append(svg_point_to_case((x, y)))
    return points


def normalize_curve_direction(points: list[tuple[float, float]]) -> list[tuple[float, float]]:
    if points[0][1] > points[-1][1]:
        return list(reversed(points))
    return points


def offset_curve_right(points: list[tuple[float, float]], distance: float) -> list[tuple[float, float]]:
    offset_points = []
    for index, (x, y) in enumerate(points):
        if index == 0:
            x0, y0 = points[index]
            x1, y1 = points[index + 1]
        elif index == len(points) - 1:
            x0, y0 = points[index - 1]
            x1, y1 = points[index]
        else:
            x0, y0 = points[index - 1]
            x1, y1 = points[index + 1]
        dx = x1 - x0
        dy = y1 - y0
        length = (dx * dx + dy * dy) ** 0.5
        if length == 0.0:
            offset_points.append((x + distance, y))
            continue
        offset_points.append((x + dy / length * distance, y - dx / length * distance))
    return offset_points


def south_edge_samples() -> tuple[list[tuple[float, float]], list[tuple[float, float]]]:
    _, low_path = load_guidance_paths()
    low = normalize_curve_direction(sample_svg_path(low_path))
    high = offset_curve_right(low, SOUTH_SHOULDER_PLAN_WIDTH)
    return high, low


def extend_low_edge_outward(
    low: list[tuple[float, float]], high: list[tuple[float, float]], distance: float
) -> list[tuple[float, float]]:
    outer = []
    for low_point, high_point in zip(low, high):
        low_x, low_y = low_point
        high_x, high_y = high_point
        dx = low_x - high_x
        dy = low_y - high_y
        length = (dx * dx + dy * dy) ** 0.5
        if length == 0.0:
            outer.append(low_point)
        else:
            outer.append((low_x + dx / length * distance, low_y + dy / length * distance))
    return outer


def south_roof_outer_samples() -> list[tuple[float, float]]:
    high, low = south_edge_samples()
    return extend_low_edge_outward(low, high, SOUTH_ROOF_LOW_EAVE_WIDTH)
