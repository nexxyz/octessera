from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal, cast

import cadquery as cq
from svgelements import Circle, Close, Line, Move, Path as SvgPath, Point, Polygon, Rect, SVG

from top_wall_port_cutouts import (
    OLED_SD_X0,
    OLED_SD_X1,
    PI_HDMI_Z_SHIFT,
    PI_SD_HOLE_Z_SHIFT,
    PI_USB_Z_SHIFT,
    PORT_INDENT_Z_PAD,
    POWER_Z_SHIFT,
    WEST_EXTENSION,
    z_bounds,
    z_center,
    z_shift_centered_on,
)


Side = Literal["west", "south", "north"]
ROOT = Path(__file__).resolve().parent
ICON_ROOT = ROOT / "port-icons"
MARK_DEPTH = 0.65
MARK_FACE_OVERLAP = 0.08
MARK_CUT_CLEARANCE = 0.08
ICON_HEIGHT = 4.8
SOUTH_ICON_HEIGHT = ICON_HEIGHT * 1.4
SCREEN_ICON_HEIGHT = SOUTH_ICON_HEIGHT * 0.7
ICON_GAP_BELOW_INDENT = 0.5
DOT_RADIUS = 0.48
DOT_GAP = 1.25
CURVE_STEPS = 18


@dataclass(frozen=True)
class IconShape:
    points: list[tuple[float, float]]
    cut: bool


def compound(parts: list[cq.Workplane]) -> cq.Workplane:
    solids = cast(list[cq.Shape], [solid for part in parts for solid in part.solids().vals()])
    return cq.Workplane("XY").add(cq.Compound.makeCompound(solids))


def face_depth(params: dict, side: Side, flush: bool, cut_clearance: float = 0.0) -> tuple[float, float]:
    _, depth = params["case_size_v21"]
    if side == "west":
        face = -WEST_EXTENSION
        return (face - cut_clearance, face + MARK_DEPTH + cut_clearance) if flush else (face - MARK_DEPTH, face + MARK_FACE_OVERLAP)
    if side == "south":
        face = 0.0
        return (face - cut_clearance, face + MARK_DEPTH + cut_clearance) if flush else (face - MARK_DEPTH, face + MARK_FACE_OVERLAP)
    face = depth
    return (face - MARK_DEPTH - cut_clearance, face + cut_clearance) if flush else (face - MARK_FACE_OVERLAP, face + MARK_DEPTH)


def prism_between(side: Side, points: list[tuple[float, float]], d0: float, d1: float) -> cq.Workplane:
    def point(depth_value: float, local: tuple[float, float]) -> cq.Vector:
        u, z = local
        return cq.Vector(depth_value, u, z) if side == "west" else cq.Vector(u, depth_value, z)

    wire0 = cq.Wire.makePolygon([point(d0, local) for local in [*points, points[0]]])
    wire1 = cq.Wire.makePolygon([point(d1, local) for local in [*points, points[0]]])
    return cq.Workplane("XY").add(cq.Solid.makeLoft([wire0, wire1], ruled=True)).clean()


def as_pair(point: Point) -> tuple[float, float]:
    return required_float(point.x), required_float(point.y)


def required_float(value: Any) -> float:
    if value is None:
        raise ValueError("expected numeric SVG coordinate, got None")
    return float(value)


def points_equal(a: tuple[float, float], b: tuple[float, float]) -> bool:
    return abs(a[0] - b[0]) < 1e-6 and abs(a[1] - b[1]) < 1e-6


def flatten_path(path: SvgPath) -> list[list[tuple[float, float]]]:
    subpaths: list[list[tuple[float, float]]] = []
    points: list[tuple[float, float]] = []
    start: tuple[float, float] | None = None
    for segment in path:
        if isinstance(segment, Move):
            if len(points) >= 3:
                subpaths.append(points)
            points = [as_pair(segment.end)]
            start = points[0]
        elif isinstance(segment, Close):
            if start is not None and points and not points_equal(points[-1], start):
                points.append(start)
            if len(points) >= 3:
                subpaths.append(points)
            points = []
            start = None
        else:
            steps = 1 if segment.__class__.__name__ == "Line" else CURVE_STEPS
            for step in range(1, steps + 1):
                points.append(as_pair(segment.point(step / steps)))
    if len(points) >= 3:
        subpaths.append(points)
    return [remove_duplicate_tail(path_points) for path_points in subpaths]


def remove_duplicate_tail(points: list[tuple[float, float]]) -> list[tuple[float, float]]:
    if len(points) > 1 and points_equal(points[0], points[-1]):
        return points[:-1]
    return points


def rect_points(rect: Rect) -> list[tuple[float, float]]:
    bbox = rect.bbox()
    if bbox is None:
        raise ValueError("rect has no SVG bounds")
    min_x, min_y, max_x, max_y = (required_float(value) for value in bbox)
    return [(min_x, min_y), (max_x, min_y), (max_x, max_y), (min_x, max_y)]


def stroked_rect_shapes(rect: Rect) -> list[list[tuple[float, float]]]:
    bbox = rect.bbox()
    if bbox is None:
        raise ValueError("rect has no SVG bounds")
    min_x, min_y, max_x, max_y = (required_float(value) for value in bbox)
    half_stroke = stroke_width(rect) / 2.0
    outer = [
        (min_x - half_stroke, min_y - half_stroke),
        (max_x + half_stroke, min_y - half_stroke),
        (max_x + half_stroke, max_y + half_stroke),
        (min_x - half_stroke, max_y + half_stroke),
    ]
    inner = [
        (min_x + half_stroke, min_y + half_stroke),
        (max_x - half_stroke, min_y + half_stroke),
        (max_x - half_stroke, max_y - half_stroke),
        (min_x + half_stroke, max_y - half_stroke),
    ]
    return [outer, inner] if min_x + half_stroke < max_x - half_stroke and min_y + half_stroke < max_y - half_stroke else [outer]


def line_points(line: Line) -> list[tuple[float, float]]:
    start = as_pair(line.start)
    end = as_pair(line.end)
    dx = end[0] - start[0]
    dy = end[1] - start[1]
    length = (dx * dx + dy * dy) ** 0.5
    if length == 0.0:
        raise ValueError("line has zero length")
    half_stroke = stroke_width(line) / 2.0
    nx = -dy / length * half_stroke
    ny = dx / length * half_stroke
    return [
        (start[0] + nx, start[1] + ny),
        (end[0] + nx, end[1] + ny),
        (end[0] - nx, end[1] - ny),
        (start[0] - nx, start[1] - ny),
    ]


def stroke_width(element: object) -> float:
    return required_float(getattr(element, "stroke_width", None) or 1.0)


def polygon_points(polygon: Polygon) -> list[tuple[float, float]]:
    return [as_pair(point) for point in polygon.points]


def circle_points(circle: Circle) -> list[tuple[float, float]]:
    center = (required_float(circle.cx), required_float(circle.cy))
    radius = required_float(circle.rx)
    unit_circle = [
        (1.0, 0.0),
        (0.866, 0.5),
        (0.5, 0.866),
        (0.0, 1.0),
        (-0.5, 0.866),
        (-0.866, 0.5),
        (-1.0, 0.0),
        (-0.866, -0.5),
        (-0.5, -0.866),
        (0.0, -1.0),
        (0.5, -0.866),
        (0.866, -0.5),
    ]
    return [(center[0] + radius * x, center[1] + radius * y) for x, y in unit_circle]


def bounds(points: list[tuple[float, float]]) -> tuple[float, float, float, float]:
    xs = [x for x, _ in points]
    ys = [y for _, y in points]
    return min(xs), min(ys), max(xs), max(ys)


def polygon_area(points: list[tuple[float, float]]) -> float:
    return sum(
        x0 * y1 - x1 * y0
        for (x0, y0), (x1, y1) in zip(points, [*points[1:], points[0]])
    ) / 2.0


def bounds_contains(outer: tuple[float, float, float, float], inner: tuple[float, float, float, float]) -> bool:
    return outer[0] <= inner[0] and outer[1] <= inner[1] and outer[2] >= inner[2] and outer[3] >= inner[3]


def point_in_polygon(point: tuple[float, float], polygon: list[tuple[float, float]]) -> bool:
    inside = False
    previous = polygon[-1]
    for current in polygon:
        if (current[1] > point[1]) != (previous[1] > point[1]):
            x_at_y = (previous[0] - current[0]) * (point[1] - current[1]) / (previous[1] - current[1]) + current[0]
            if point[0] < x_at_y:
                inside = not inside
        previous = current
    return inside


def is_visible_shape(element) -> bool:
    fill = getattr(element, "fill", None)
    stroke = getattr(element, "stroke", None)
    has_fill = fill is not None and str(fill).lower() not in {"none", "transparent"}
    has_stroke = stroke is not None and str(stroke).lower() not in {"none", "transparent"}
    return has_fill or has_stroke


def raw_icon_paths(name: str) -> list[list[tuple[float, float]]]:
    raw_shapes: list[list[tuple[float, float]]] = []
    for element in SVG.parse(ICON_ROOT / f"{name}.svg").elements():
        if not is_visible_shape(element):
            continue
        fill = getattr(element, "fill", None)
        fill_visible = fill is not None and str(fill).lower() not in {"none", "transparent"}
        if isinstance(element, SvgPath):
            raw_shapes.extend(flatten_path(element))
        elif isinstance(element, Polygon):
            raw_shapes.append(remove_duplicate_tail(polygon_points(element)))
        elif isinstance(element, Rect):
            raw_shapes.extend([rect_points(element)] if fill_visible else stroked_rect_shapes(element))
        elif isinstance(element, Circle):
            raw_shapes.append(circle_points(element))
        elif isinstance(element, Line):
            raw_shapes.append(line_points(element))
    return [shape for shape in raw_shapes if len(shape) >= 3]


def icon_shapes(name: str) -> list[IconShape]:
    raw_shapes = raw_icon_paths(name)
    shape_bounds = [bounds(shape) for shape in raw_shapes]
    shape_areas = [abs(polygon_area(shape)) for shape in raw_shapes]
    result = []
    for index, shape in enumerate(raw_shapes):
        center = (sum(x for x, _ in shape) / len(shape), sum(y for _, y in shape) / len(shape))
        containing_larger = [
            other_index
            for other_index, other in enumerate(raw_shapes)
            if other_index != index
            and shape_areas[other_index] > shape_areas[index]
            and bounds_contains(shape_bounds[other_index], shape_bounds[index])
            and point_in_polygon(center, other)
        ]
        result.append(IconShape(shape, len(containing_larger) % 2 == 1))
    return result


def outside_u(side: Side, center_u: float, u: float) -> float:
    return center_u - (u - center_u) if side in {"west", "north"} else u


def icon_bounds(shapes: list[IconShape]) -> tuple[float, float, float, float]:
    return bounds([point for shape in shapes for point in shape.points])


def transform_shape(
    side: Side,
    center_u: float,
    bottom_z: float,
    icon_height: float,
    icon_bound: tuple[float, float, float, float],
    points: list[tuple[float, float]],
) -> list[tuple[float, float]]:
    min_x, min_y, max_x, max_y = icon_bound
    scale = icon_height / (max_y - min_y)
    source_center_x = (min_x + max_x) / 2.0
    transformed = [
        (outside_u(side, center_u, center_u + (x - source_center_x) * scale), bottom_z + (max_y - y) * scale)
        for x, y in points
    ]
    return list(reversed(transformed)) if side in {"west", "north"} else transformed


def icon_part(
    side: Side,
    name: str,
    center_u: float,
    bottom_z: float,
    d0: float,
    d1: float,
    icon_height: float = ICON_HEIGHT,
) -> cq.Workplane:
    shapes = icon_shapes(name)
    icon_bound = icon_bounds(shapes)
    positives = [
        prism_between(side, transform_shape(side, center_u, bottom_z, icon_height, icon_bound, shape.points), d0, d1)
        for shape in shapes
        if not shape.cut
    ]
    negatives = [
        prism_between(side, transform_shape(side, center_u, bottom_z, icon_height, icon_bound, shape.points), d0 - 0.02, d1 + 0.02)
        for shape in shapes
        if shape.cut
    ]
    result = positives[0]
    for positive in positives[1:]:
        result = result.union(positive)
    for negative in negatives:
        result = result.cut(negative)
    return result.clean()


def dot_part(side: Side, center_u: float, center_z: float, d0: float, d1: float) -> cq.Workplane:
    unit_circle = [
        (1.0, 0.0),
        (0.866, 0.5),
        (0.5, 0.866),
        (0.0, 1.0),
        (-0.5, 0.866),
        (-0.866, 0.5),
        (-1.0, 0.0),
        (-0.866, -0.5),
        (-0.5, -0.866),
        (0.0, -1.0),
        (0.5, -0.866),
        (0.866, -0.5),
    ]
    points = [(center_u + DOT_RADIUS * x, center_z + DOT_RADIUS * z) for x, z in unit_circle]
    return prism_between(side, list(reversed(points)) if side in {"west", "north"} else points, d0, d1)


def sd_mark(side: Side, center_u: float, bottom_z: float, dot_count: int, d0: float, d1: float) -> list[tuple[str, cq.Workplane]]:
    dot_offset = 3.15
    icon_u = center_u
    dot_direction = -1.0 if side in {"west", "north"} else 1.0
    dot_u = center_u + dot_direction * dot_offset
    if side == "north":
        dot_u -= 0.5
    dots = [("dot", dot_part(side, dot_u, bottom_z + ICON_HEIGHT / 2.0, d0, d1))]
    if dot_count == 2:
        dots = [
            ("dot_west", dot_part(side, dot_u - DOT_GAP / 2.0, bottom_z + ICON_HEIGHT / 2.0, d0, d1)),
            ("dot_east", dot_part(side, dot_u + DOT_GAP / 2.0, bottom_z + ICON_HEIGHT / 2.0, d0, d1)),
        ]
    return [("microsd_icon", icon_part(side, "microsd-card", icon_u, bottom_z, d0, d1)), *dots]


def icon_bottom_from_indent(indent_height: float, indent_z_shift: float, icon_height: float) -> float:
    indent_bottom, _ = z_bounds(indent_height, PORT_INDENT_Z_PAD, indent_z_shift)
    return indent_bottom - ICON_GAP_BELOW_INDENT - icon_height


def port_icon_bottoms() -> dict[str, float]:
    audio_indent_z_shift = z_center(8.2) - z_center(5.2)
    pi_usb_bottom = icon_bottom_from_indent(1.8, z_shift_centered_on(2.3, PI_USB_Z_SHIFT, 1.8), SOUTH_ICON_HEIGHT)
    south_icon_center = pi_usb_bottom + SOUTH_ICON_HEIGHT / 2.0
    return {
        "audio 3.5mm": icon_bottom_from_indent(5.2, audio_indent_z_shift, ICON_HEIGHT),
        "USB-C power": icon_bottom_from_indent(4.6, POWER_Z_SHIFT, ICON_HEIGHT),
        "Pi microSD": icon_bottom_from_indent(2.0, z_shift_centered_on(3.0, PI_SD_HOLE_Z_SHIFT, 2.0), ICON_HEIGHT),
        "Pi mini-HDMI": south_icon_center - SCREEN_ICON_HEIGHT / 2.0,
        "Pi USB data": pi_usb_bottom,
        "OLED SD": icon_bottom_from_indent(5.0, 0.0, ICON_HEIGHT),
    }


def port_marking_parts(params: dict, model_bottom_z: float, flush: bool = False, cut_clearance: float = 0.0) -> list[tuple[str, cq.Workplane]]:
    bottoms = port_icon_bottoms()
    west_d0, west_d1 = face_depth(params, "west", flush, cut_clearance)
    south_d0, south_d1 = face_depth(params, "south", flush, cut_clearance)
    north_d0, north_d1 = face_depth(params, "north", flush, cut_clearance)
    parts: list[tuple[str, cq.Workplane]] = []
    for port in params["ports_v21"]:
        center_u = (port["a"] + port["b"]) / 2.0
        label = port["label"]
        if label == "audio 3.5mm":
            parts.append(("west_audio_headphones", icon_part("west", "headphones", center_u, bottoms[label], west_d0, west_d1)))
        elif label == "USB-C power":
            parts.append(("west_power_lightning", icon_part("west", "lightning", center_u, bottoms[label], west_d0, west_d1)))
        elif label == "Pi microSD":
            parts.extend((f"west_pi_sd_{name}", part) for name, part in sd_mark("west", center_u, bottoms[label], 1, west_d0, west_d1))
        elif label == "Pi mini-HDMI":
            parts.append(("south_hdmi_monitor", icon_part("south", "monitor", center_u, bottoms[label], south_d0, south_d1, SCREEN_ICON_HEIGHT)))
        elif label == "Pi USB data":
            parts.append(("south_usb_icon", icon_part("south", "usb", center_u, bottoms[label], south_d0, south_d1, SOUTH_ICON_HEIGHT)))

    oled_center_u = (OLED_SD_X0 + OLED_SD_X1) / 2.0
    parts.extend((f"north_oled_sd_{name}", part) for name, part in sd_mark("north", oled_center_u, bottoms["OLED SD"], 2, north_d0, north_d1))
    return parts


def make_port_markings(params: dict, model_bottom_z: float, flush: bool = False, cut_clearance: float = 0.0) -> cq.Workplane:
    return compound([part for _, part in port_marking_parts(params, model_bottom_z, flush, cut_clearance)]).clean()
