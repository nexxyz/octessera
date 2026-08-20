from __future__ import annotations

import math

import cadquery as cq

from wave_guidance import (
    PI_BLOCK_NORTH_Y,
    SLOPE_PROFILE_STEPS,
    SOUTH_ROOF_LOW_WALL_BAND,
    SOUTH_SHOULDER_PLAN_WIDTH,
    load_guidance_slots,
    south_edge_samples,
)


LOW_Z = 12.0
HIGH_Z = 17.0
UNDERSIDE_Z = 9.0
HIGH_UNDERSIDE_Z = 14.0
EXTENDED_SLOPE_RIGHT_X = 115.0
WEST_EXTENSION = 1.0
LOWER_WAVE_HEIGHT_SCALE = 1.0
LOWER_WAVE_HIGH_UNDERSIDE_Z = UNDERSIDE_Z + (HIGH_UNDERSIDE_Z - UNDERSIDE_Z) * LOWER_WAVE_HEIGHT_SCALE
LOWER_WAVE_HIGH_Z = LOW_Z + (HIGH_Z - LOW_Z) * LOWER_WAVE_HEIGHT_SCALE
LOWER_TO_TIER2_RAMP_START_X = 105.0
LOWER_TO_TIER2_RAMP_END_X = 115.0
TIER1_WAVE_SEAM_OVERLAP = 2.4


def x_at_y(points: list[tuple[float, float]], y: float) -> float:
    sorted_points = sorted(points, key=lambda point: point[1])
    if y <= sorted_points[0][1]:
        return sorted_points[0][0]
    for (x0, y0), (x1, y1) in zip(sorted_points, sorted_points[1:]):
        if y <= y1:
            if y1 == y0:
                return x1
            return x0 + (x1 - x0) * ((y - y0) / (y1 - y0))
    return sorted_points[-1][0]


def first_y_at_x(points: list[tuple[float, float]], x: float) -> float:
    sorted_points = sorted(points, key=lambda point: point[1])
    for (x0, y0), (x1, y1) in zip(sorted_points, sorted_points[1:]):
        if (x0 <= x <= x1) or (x1 <= x <= x0):
            if x1 == x0:
                return y1
            return y0 + (y1 - y0) * ((x - x0) / (x1 - x0))
    return sorted_points[-1][1]


def y_band_prism(width: float, y0: float, y1: float, margin: float, z_height: float) -> cq.Workplane:
    points = [
        (-margin, y0),
        (-margin, y1),
        (width + margin, y1),
        (width + margin, y0),
    ]
    return cq.Workplane("XY").polyline(points).close().extrude(z_height).translate((0, 0, -1))


def x_band_prism(x0: float, x1: float, depth: float, margin: float, z_height: float) -> cq.Workplane:
    points = [
        (x0, -margin),
        (x0, depth + margin),
        (x1, depth + margin),
        (x1, -margin),
    ]
    return cq.Workplane("XY").polyline(points).close().extrude(z_height).translate((0, 0, -1))


def right_region_prism(width: float, depth: float, margin: float, z_height: float) -> cq.Workplane:
    high, _ = south_edge_samples()
    points = [(x, y) for x, y in high]
    points += [
        (EXTENDED_SLOPE_RIGHT_X, depth + margin),
        (width + margin, depth + margin),
        (width + margin, -margin),
        (points[0][0], -margin),
    ]
    return cq.Workplane("XY").polyline(points).close().extrude(z_height).translate((0, 0, -1))


def left_region_prism(width: float, depth: float, margin: float, z_height: float) -> cq.Workplane:
    high, _ = south_edge_samples()
    points = [(-margin, -margin), (-margin, depth + margin), (high[-1][0] + TIER1_WAVE_SEAM_OVERLAP, depth + margin)]
    points += [(x + TIER1_WAVE_SEAM_OVERLAP, y) for x, y in reversed(high)]
    points.append((-margin, -margin))
    return cq.Workplane("XY").polyline(points).close().extrude(z_height).translate((0, 0, -1))


def curve_pair_at_y(
    low: list[tuple[float, float]], high: list[tuple[float, float]], y: float
) -> tuple[tuple[float, float], tuple[float, float]]:
    return (x_at_y(low, y), y), (x_at_y(high, y), y)


def trimmed_curve_pairs(
    low: list[tuple[float, float]], high: list[tuple[float, float]], y0: float, y1: float
) -> list[tuple[tuple[float, float], tuple[float, float]]]:
    pairs = []
    if y0 <= low[-1][1] and y1 >= low[0][1]:
        start_y = max(y0, low[0][1])
        end_y = min(y1, low[-1][1])
        pairs.append(curve_pair_at_y(low, high, start_y))
        pairs.extend(
            (low_point, high_point)
            for low_point, high_point in zip(low, high)
            if start_y < low_point[1] < end_y
        )
        pairs.append(curve_pair_at_y(low, high, end_y))
    return pairs


def shoulder_plan_prism(y0: float, y1: float, z_height: float, high_x_extra: float = 0.0) -> cq.Workplane:
    high, low = south_edge_samples()
    curve_pairs = trimmed_curve_pairs(low, high, y0, y1)
    low_points = [low_point for low_point, _ in curve_pairs]
    high_points = [(x + high_x_extra, y) for _, (x, y) in curve_pairs]
    points = [*low_points, *reversed(high_points), low_points[0]]
    return cq.Workplane("XY").polyline(points).close().extrude(z_height).translate((0, 0, -1))


def shoulder_loft(y0: float, y1: float, height_scale: float = 1.0) -> cq.Workplane:
    high, low = south_edge_samples()
    curve_pairs = trimmed_curve_pairs(low, high, y0, y1)
    wires = [
        shoulder_profile_wire(low_point, high_point, height_scale)
        for low_point, high_point in curve_pairs
    ]
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True))


def shoulder_profile_wire(
    low: tuple[float, float], high: tuple[float, float], height_scale: float = 1.0
) -> cq.Wire:
    low_x, low_y = low
    high_x, high_y = high
    top_points = []
    bottom_points = []
    bottom_band_t = min(0.45, SOUTH_ROOF_LOW_WALL_BAND / SOUTH_SHOULDER_PLAN_WIDTH)
    for index in range(SLOPE_PROFILE_STEPS + 1):
        t = index / SLOPE_PROFILE_STEPS
        eased = (1.0 - (1.0 - t) * (1.0 - t)) ** 0.5
        x = low_x + (high_x - low_x) * t
        y = low_y + (high_y - low_y) * t
        z = LOW_Z + (HIGH_Z - LOW_Z) * height_scale * eased
        top_points.append(cq.Vector(x, y, z))
        if t <= bottom_band_t:
            bottom_z = UNDERSIDE_Z
        else:
            bottom_t = (t - bottom_band_t) / (1.0 - bottom_band_t)
            bottom_eased = (1.0 - (1.0 - bottom_t) * (1.0 - bottom_t)) ** 0.5
            bottom_z = UNDERSIDE_Z + (HIGH_UNDERSIDE_Z - UNDERSIDE_Z) * height_scale * bottom_eased
        bottom_points.append(cq.Vector(x, y, bottom_z))
    points = [*top_points, *reversed(bottom_points), top_points[0]]
    return cq.Wire.makePolygon(points)


def quarter_circle_ease(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return 1.0 - (1.0 - t * t) ** 0.5


def east_wave_ramp_loft(y0: float, y1: float) -> cq.Workplane:
    wires = []
    samples = 16
    for index in range(samples + 1):
        t = index / samples
        eased = quarter_circle_ease(t)
        top_z = LOWER_WAVE_HIGH_Z + (HIGH_Z - LOWER_WAVE_HIGH_Z) * eased
        bottom_z = LOWER_WAVE_HIGH_UNDERSIDE_Z + (HIGH_UNDERSIDE_Z - LOWER_WAVE_HIGH_UNDERSIDE_Z) * eased
        x = LOWER_TO_TIER2_RAMP_START_X + (LOWER_TO_TIER2_RAMP_END_X - LOWER_TO_TIER2_RAMP_START_X) * t
        wires.append(
            cq.Wire.makePolygon(
                [
                    cq.Vector(x, y0, top_z),
                    cq.Vector(x, y1, top_z),
                    cq.Vector(x, y1, bottom_z),
                    cq.Vector(x, y0, bottom_z),
                    cq.Vector(x, y0, top_z),
                ]
            )
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True))


def rectangular_lower_wave_slope_loft(x0: float, x1: float, low_y: float) -> cq.Workplane:
    high_y = low_y - SOUTH_SHOULDER_PLAN_WIDTH
    wires = []
    samples = 32
    bottom_band_t = min(0.45, SOUTH_ROOF_LOW_WALL_BAND / SOUTH_SHOULDER_PLAN_WIDTH)
    for index in range(samples + 1):
        x = x0 + (x1 - x0) * index / samples
        top_points = []
        bottom_points = []
        for profile_index in range(SLOPE_PROFILE_STEPS + 1):
            t = profile_index / SLOPE_PROFILE_STEPS
            eased = (1.0 - (1.0 - t) * (1.0 - t)) ** 0.5
            y = low_y + (high_y - low_y) * t
            z = LOW_Z + (HIGH_Z - LOW_Z) * eased
            top_points.append(cq.Vector(x, y, z))
            if t <= bottom_band_t:
                bottom_z = UNDERSIDE_Z
            else:
                bottom_t = (t - bottom_band_t) / (1.0 - bottom_band_t)
                bottom_eased = (1.0 - (1.0 - bottom_t) * (1.0 - bottom_t)) ** 0.5
                bottom_z = UNDERSIDE_Z + (HIGH_UNDERSIDE_Z - UNDERSIDE_Z) * bottom_eased
            bottom_points.append(cq.Vector(x, y, bottom_z))
        wires.append(cq.Wire.makePolygon([*top_points, *reversed(bottom_points), top_points[0]]))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True))


def west_wave_wall(params: dict, footprint: cq.Workplane) -> cq.Workplane:
    wall = params["wall"]
    low_y = PI_BLOCK_NORTH_Y
    high_y = PI_BLOCK_NORTH_Y - SOUTH_SHOULDER_PLAN_WIDTH
    wires = []
    samples = 24
    for index in range(samples + 1):
        y = PI_BLOCK_NORTH_Y * index / samples
        if y <= high_y:
            top_z = HIGH_Z
        else:
            t = (low_y - y) / (low_y - high_y)
            eased = (1.0 - (1.0 - t) * (1.0 - t)) ** 0.5
            top_z = LOW_Z + (HIGH_Z - LOW_Z) * eased
        wires.append(
            cq.Wire.makePolygon(
                [
                    cq.Vector(-WEST_EXTENSION, y, LOW_Z - 0.05),
                    cq.Vector(wall + 0.3, y, LOW_Z - 0.05),
                    cq.Vector(wall + 0.3, y, top_z),
                    cq.Vector(-WEST_EXTENSION, y, top_z),
                    cq.Vector(-WEST_EXTENSION, y, LOW_Z - 0.05),
                ]
            )
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).intersect(footprint).clean()


def slot_cutter(start: tuple[float, float], end: tuple[float, float], width: float) -> cq.Workplane:
    x0, y0 = start
    x1, y1 = end
    dx = x1 - x0
    dy = y1 - y0
    length = (dx * dx + dy * dy) ** 0.5
    if length == 0.0:
        return cq.Workplane("XY").circle(width / 2.0).extrude(40).translate((x0, y0, -2))
    tangent_x = dx / length
    tangent_y = dy / length
    normal_x = -tangent_y
    normal_y = tangent_x
    amplitude = width * 0.9
    samples = 28
    cutter = None
    for index in range(samples + 1):
        t = index / samples
        offset = amplitude * math.sin(2.0 * math.pi * (t - 0.15))
        x = x0 + dx * t + normal_x * offset
        y = y0 + dy * t + normal_y * offset
        disk = (
            cq.Workplane("XY")
            .circle(width / 2.0)
            .extrude(HIGH_Z - UNDERSIDE_Z + 8.0)
            .translate((x, y, UNDERSIDE_Z - 4.0))
        )
        cutter = disk if cutter is None else cutter.union(disk)
    if cutter is None:
        return cq.Workplane("XY").circle(width / 2.0).extrude(40).translate((x0, y0, -2))
    return cutter


def add_guidance_slots(model: cq.Workplane) -> cq.Workplane:
    for start, end in load_guidance_slots():
        model = model.cut(slot_cutter(start, end, 2.0))
    return model
