from __future__ import annotations

import json
import math
from pathlib import Path
from typing import cast

import cadquery as cq

from branding_marking_cadquery import branding_marking_parts, make_branding_marking
from faceplate_insert_pillars import add_faceplate_insert_pillars, subtract_faceplate_insert_holes
from faceplate_neokey_support import neokey_deck_cap, neokey_raised_cap, neokey_south_slot_fill, neokey_support_block
from faceplate_walls import perimeter_wall_skirts
from port_markings_cadquery import MARK_CUT_CLEARANCE, make_port_markings, port_marking_parts
from top_wall_port_cutouts import add_top_wall_port_cutouts
from wave_guidance import (
    PI_BLOCK_NORTH_Y,
    SLOPE_PROFILE_STEPS,
    SOUTH_ROOF_LOW_WALL_BAND,
    SOUTH_SHOULDER_PLAN_WIDTH,
    load_guidance_slots,
    south_edge_samples,
)


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
PARAMS = ROOT / "enclosure_params.json"
STEP_OUT = ARTIFACT_ROOT / "step" / "case_top_two_level_cadquery.step"
STL_OUT = ARTIFACT_ROOT / "stl" / "case_top_two_level_cadquery.stl"


LOW_Z = 12.0
HIGH_Z = 17.0
UNDERSIDE_Z = 9.0
HIGH_UNDERSIDE_Z = 14.0
EXTENDED_SLOPE_RIGHT_X = 115.0
WEST_EXTENSION = 1.0
NEOKEY_PANEL_Y_OFFSET = -0.5
NEOKEY_TOP_Z = 16.0
NEOKEY_DECK_TOP_Z = HIGH_Z + 3.0
NEOKEY_KEYCAP_RECESS_DEPTH = 1.0
NEOKEY_MX_LATCH_PLATE_THICKNESS = 1.5
BRANDING_RAISE = 0.65
NEOKEY_MX_UNDERSIDE_CLEARANCE = 2.6
NEOKEY_MX_MOUNTING_GRID_Z_DROP = 2.0
NEOKEY_SEAT_BOTTOM_Z = UNDERSIDE_Z + 1.0
NEOKEY_SEAT_OVERLAP = 3.0
LOWER_WAVE_HEIGHT_SCALE = 1.0
LOWER_WAVE_HIGH_UNDERSIDE_Z = UNDERSIDE_Z + (HIGH_UNDERSIDE_Z - UNDERSIDE_Z) * LOWER_WAVE_HEIGHT_SCALE
LOWER_WAVE_HIGH_Z = LOW_Z + (HIGH_Z - LOW_Z) * LOWER_WAVE_HEIGHT_SCALE
LOWER_TO_TIER2_RAMP_START_X = 105.0
LOWER_TO_TIER2_RAMP_END_X = 115.0
TIER1_WAVE_SEAM_OVERLAP = 2.4
NEOKEY_WAVE_HOLLOW_SOUTH_EXTRA = 1.0
OLED_SCREEN_CUTOUT_X_SHIFT = -0.5
OLED_SCREEN_CUTOUT_Y_SHIFT = 0.2

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
def rounded_plate(width: float, depth: float, radius: float, z0: float, thickness: float) -> cq.Workplane:
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    return (
        cq.Workplane("XY")
        .placeSketch(sketch)
        .extrude(thickness)
        .translate((width / 2.0, depth / 2.0, z0))
    )


def west_extension_solid(width: float, depth: float, radius: float, z0: float, z1: float) -> cq.Workplane:
    extended = rounded_plate(width + WEST_EXTENSION, depth, radius, z0, z1 - z0).translate((-WEST_EXTENSION, 0, 0))
    original = rounded_plate(width, depth, radius, z0 - 0.1, z1 - z0 + 0.2)
    return extended.cut(original).clean()


def west_extended_footprint(width: float, depth: float, radius: float, z0: float, thickness: float) -> cq.Workplane:
    return rounded_plate(width + WEST_EXTENSION, depth, radius, z0, thickness).translate((-WEST_EXTENSION, 0, 0))


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
            bottom_z = (
                UNDERSIDE_Z
                + (HIGH_UNDERSIDE_Z - UNDERSIDE_Z) * height_scale * bottom_eased
            )
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


def rect_cutter(x0: float, y0: float, x1: float, y1: float, radius: float) -> cq.Workplane:
    width = x1 - x0
    depth = y1 - y0
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    cutter = cq.Workplane("XY").placeSketch(sketch).extrude(40)
    return cutter.translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, -2))


def rect_cutter_from_z(x0: float, y0: float, x1: float, y1: float, radius: float, z0: float) -> cq.Workplane:
    width = x1 - x0
    depth = y1 - y0
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    cutter = cq.Workplane("XY").placeSketch(sketch).extrude(40)
    return cutter.translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))


def rect_prism(x0: float, y0: float, x1: float, y1: float, radius: float, z0: float, z1: float) -> cq.Workplane:
    width = x1 - x0
    depth = y1 - y0
    if radius <= 0.0:
        return cq.Workplane("XY").rect(width, depth).extrude(z1 - z0).translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    prism = cq.Workplane("XY").placeSketch(sketch).extrude(z1 - z0)
    return prism.translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))


def circle_cutter(x: float, y: float, radius: float) -> cq.Workplane:
    return cq.Workplane("XY").circle(radius).extrude(40).translate((x, y, -2))


def slot_cutter(start: tuple[float, float], end: tuple[float, float], width: float) -> cq.Workplane:
    x0, y0 = start
    x1, y1 = end
    dx = x1 - x0
    dy = y1 - y0
    length = (dx * dx + dy * dy) ** 0.5
    if length == 0.0:
        return circle_cutter(x0, y0, width / 2.0)
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
        return circle_cutter(x0, y0, width / 2.0)
    return cutter


def add_guidance_slots(model: cq.Workplane) -> cq.Workplane:
    for start, end in load_guidance_slots():
        model = model.cut(slot_cutter(start, end, 2.0))
    return model


def crater_cutter(x: float, y: float, flat_d: float, depth: float, slope_w: float, top_z: float) -> cq.Workplane:
    bottom_z = top_z - depth
    flat_r = flat_d / 2.0
    outer_r = flat_r + slope_w
    return (
        cq.Workplane("XY", origin=(0, 0, bottom_z))
        .circle(flat_r)
        .workplane(offset=depth + 0.05)
        .circle(outer_r)
        .loft(combine=True)
        .translate((x, y, 0))
    )


def neokey_slot_bounds(params: dict, key_centers: list[tuple[float, float]]) -> tuple[float, float, float, float]:
    key_w, key_h = params["key_cutout"]
    key_x_values = [x for x, _ in key_centers]
    key_y_values = [y for _, y in key_centers]
    return (
        min(key_x_values) - key_w / 2,
        min(key_y_values) - key_h / 2,
        max(key_x_values) + key_w / 2,
        max(key_y_values) + key_h / 2,
    )


def neokey_seat_bounds(params: dict, key_centers: list[tuple[float, float]]) -> tuple[float, float, float, float]:
    slot_x0, slot_y0, slot_x1, slot_y1 = neokey_slot_bounds(params, key_centers)
    return (
        slot_x0 - NEOKEY_SEAT_OVERLAP,
        slot_y0 - NEOKEY_SEAT_OVERLAP,
        min(slot_x1 + NEOKEY_SEAT_OVERLAP, EXTENDED_SLOPE_RIGHT_X),
        slot_y1 + NEOKEY_SEAT_OVERLAP,
    )


def local_to_case(params: dict, point: list[float]) -> tuple[float, float]:
    _, case_depth = params["case_size_v21"]
    offset_x, offset_y = params["offset_v21"]
    return offset_x + point[0], case_depth - (offset_y + point[1])


def neokey_key_centers(params: dict) -> list[tuple[float, float]]:
    return [
        (local_to_case(params, point)[0], local_to_case(params, point)[1] + NEOKEY_PANEL_Y_OFFSET)
        for point in params["features_local"]["neokey_key_centers"]
    ]


def add_neokey_cutouts(model: cq.Workplane, params: dict) -> cq.Workplane:
    key_centers = neokey_key_centers(params)
    seat_bounds = neokey_seat_bounds(params, key_centers)
    model = model.union(neokey_south_slot_fill(seat_bounds, NEOKEY_WAVE_HOLLOW_SOUTH_EXTRA, LOW_Z, HIGH_Z))
    model = model.union(neokey_support_block(params, seat_bounds, NEOKEY_SEAT_BOTTOM_Z, LOW_Z, NEOKEY_TOP_Z))
    model = model.union(neokey_deck_cap(params, seat_bounds, NEOKEY_TOP_Z, HIGH_Z))
    model = model.union(neokey_raised_cap(params, seat_bounds, HIGH_Z, NEOKEY_DECK_TOP_Z))
    key_w = key_h = params["key_cutout"][0]
    mx_plate_top_z = NEOKEY_TOP_Z - NEOKEY_KEYCAP_RECESS_DEPTH - NEOKEY_MX_MOUNTING_GRID_Z_DROP
    for x, y in key_centers:
        model = model.cut(
            rect_prism(
                x - key_w / 2,
                y - key_h / 2,
                x + key_w / 2,
                y + key_h / 2,
                params["key_cutout_r"],
                mx_plate_top_z,
                NEOKEY_DECK_TOP_Z + 0.2,
            )
        )
    mx_cutout = params["mx_switch_retention_cutout"]
    mx_plate_bottom_z = mx_plate_top_z - NEOKEY_MX_LATCH_PLATE_THICKNESS
    mx_mounting_grid_bottom_z = NEOKEY_SEAT_BOTTOM_Z - NEOKEY_MX_MOUNTING_GRID_Z_DROP
    mx_underside_clearance = mx_cutout + NEOKEY_MX_UNDERSIDE_CLEARANCE
    for x, y in key_centers:
        model = model.cut(
            rect_prism(
                x - mx_underside_clearance / 2,
                y - mx_underside_clearance / 2,
                x + mx_underside_clearance / 2,
                y + mx_underside_clearance / 2,
                params["mx_switch_retention_r"],
                mx_mounting_grid_bottom_z - 0.1,
                mx_plate_bottom_z,
            )
        )
        model = model.cut(
            rect_prism(
                x - mx_cutout / 2.0,
                y - mx_cutout / 2.0,
                x + mx_cutout / 2.0,
                y + mx_cutout / 2.0,
                params["mx_switch_retention_r"],
                mx_plate_bottom_z - 0.05,
                NEOKEY_DECK_TOP_Z + 0.2,
            )
        )
    return model


def add_cutouts(model: cq.Workplane, params: dict) -> cq.Workplane:
    screen_cx, screen_cy = local_to_case(params, params["features_local"]["oled_screen_center"])
    screen_cx += OLED_SCREEN_CUTOUT_X_SHIFT
    screen_cy += OLED_SCREEN_CUTOUT_Y_SHIFT
    screen_w, screen_h = params["screen_cutout"]
    model = model.cut(
        rect_cutter(
            screen_cx - screen_w / 2,
            screen_cy - screen_h / 2,
            screen_cx + screen_w / 2,
            screen_cy + screen_h / 2,
            params["screen_cutout_r"],
        )
    )

    encoder_crater_flat_d = params["encoder_crater_flat_d"]
    for name, point in params["features_local"]["encoders"].items():
        x, y = local_to_case(params, point)
        model = model.cut(
            crater_cutter(
                x,
                y,
                encoder_crater_flat_d[name],
                params["encoder_crater_depth"],
                params["encoder_crater_slope_w"],
                LOW_Z,
            )
        )
        model = model.cut(circle_cutter(x, y, params["encoder_hole_d"] / 2.0))

    neo_pitch = params["neotrellis_pitch"]
    neo_d = params["neotrellis_button_cutout"]
    for row in range(8):
        for col in range(8):
            x = 124.75 + col * neo_pitch
            y = 17.5 + row * neo_pitch
            model = model.cut(rect_cutter(x - neo_d / 2, y - neo_d / 2, x + neo_d / 2, y + neo_d / 2, params["neotrellis_button_r"]))

    return model


def build_body_model(params: dict) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    radius = params["corner_r"]
    top_thick = params["top_thick"]
    neokey_seat_x0, neokey_seat_y0, neokey_seat_x1, neokey_seat_top_y = neokey_seat_bounds(
        params, neokey_key_centers(params)
    )
    _, low_edge = south_edge_samples()
    lower_wave_top_y = first_y_at_x(low_edge, neokey_seat_x0)

    footprint = west_extended_footprint(width, depth, radius, 0, 40)
    west_extension = west_extension_solid(width, depth, radius, -10.0, LOW_Z)
    low_plate = rounded_plate(width, depth, radius, UNDERSIDE_Z, top_thick).intersect(left_region_prism(width, depth, 5, 40))
    right_region = right_region_prism(width, depth, 5, 40)
    wave_strip_region = right_region.intersect(
        x_band_prism(-5.0, EXTENDED_SLOPE_RIGHT_X, depth, 5, 40)
    )
    neokey_seat_region = x_band_prism(neokey_seat_x0, neokey_seat_x1, depth, 5, 40).intersect(
        y_band_prism(width, neokey_seat_y0 - NEOKEY_WAVE_HOLLOW_SOUTH_EXTRA, neokey_seat_top_y, 5, 40)
    )
    low_plate = low_plate.cut(neokey_seat_region)
    high_plate = rounded_plate(width, depth, radius, HIGH_UNDERSIDE_Z, top_thick).intersect(
        right_region
    )
    high_plate = high_plate.cut(
        wave_strip_region.intersect(y_band_prism(width, -5.0, neokey_seat_top_y, 5, 40))
    )
    ramp_region = x_band_prism(
        LOWER_TO_TIER2_RAMP_START_X, LOWER_TO_TIER2_RAMP_END_X, depth, 5, 40
    ).intersect(y_band_prism(width, -5.0, lower_wave_top_y, 5, 40))
    lower_wave_slope_clearance = x_band_prism(-5.0, EXTENDED_SLOPE_RIGHT_X, depth, 5, 40).intersect(
        y_band_prism(
            width,
            lower_wave_top_y - SOUTH_SHOULDER_PLAN_WIDTH,
            lower_wave_top_y,
            5,
            40,
        )
    )
    upper_shoulder_clearance = shoulder_plan_prism(
        neokey_seat_top_y,
        depth,
        40,
        TIER1_WAVE_SEAM_OVERLAP + 0.3,
    ).intersect(footprint)
    east_ramp_clearance = ramp_region
    low_plate = low_plate.cut(lower_wave_slope_clearance)
    low_plate = low_plate.cut(upper_shoulder_clearance)
    low_plate = low_plate.cut(east_ramp_clearance)
    high_plate = high_plate.cut(ramp_region)
    lower_wave_plate = rounded_plate(
        width, depth, radius, LOWER_WAVE_HIGH_UNDERSIDE_Z, top_thick
    ).intersect(
        wave_strip_region.intersect(y_band_prism(width, -5.0, lower_wave_top_y, 5, 40))
    )
    lower_wave_plate = lower_wave_plate.cut(lower_wave_slope_clearance)
    lower_wave_plate = lower_wave_plate.cut(ramp_region)
    lower_wave_plate = lower_wave_plate.cut(neokey_seat_region)
    wave_flat_ramp = east_wave_ramp_loft(0.0, lower_wave_top_y)
    wave_flat_ramp = wave_flat_ramp.cut(neokey_seat_region)
    west_wall = west_wave_wall(params, footprint)
    flat_faceplate = add_cutouts(
        low_plate.union(lower_wave_plate).union(wave_flat_ramp).union(west_wall).union(high_plate).union(west_extension).clean(),
        params,
    ).clean()
    upper_shoulder = shoulder_loft(neokey_seat_top_y, depth).intersect(footprint).clean()
    lower_wave = (
        rectangular_lower_wave_slope_loft(-WEST_EXTENSION, EXTENDED_SLOPE_RIGHT_X, lower_wave_top_y)
        .intersect(footprint)
        .cut(neokey_seat_region)
        .clean()
    )
    shoulder = upper_shoulder.union(lower_wave).clean()
    skirts = perimeter_wall_skirts(
        params,
        west_extended_footprint,
        EXTENDED_SLOPE_RIGHT_X,
        LOWER_TO_TIER2_RAMP_START_X,
        LOWER_TO_TIER2_RAMP_END_X,
        LOW_Z,
        LOWER_WAVE_HIGH_Z,
        HIGH_Z,
    )
    model = flat_faceplate.union(shoulder).union(skirts).clean()
    model = add_faceplate_insert_pillars(model, params)
    model = add_neokey_cutouts(model, params).clean()
    model = add_guidance_slots(model).clean()
    return subtract_faceplate_insert_holes(add_top_wall_port_cutouts(model.union(skirts).clean(), params), params)


def build_branding_marking(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    case_params = params or load_params()
    return make_branding_marking(LOW_Z, BRANDING_RAISE).union(make_port_markings(case_params, model_bottom_z)).clean()


def build_flush_top_branding_marking() -> cq.Workplane:
    return make_branding_marking(LOW_Z - BRANDING_RAISE, BRANDING_RAISE).clean()


def build_flush_top_branding_parts() -> list[tuple[str, cq.Workplane]]:
    return branding_marking_parts(LOW_Z - BRANDING_RAISE, BRANDING_RAISE)


def build_flush_port_markings(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    case_params = params or load_params()
    return make_port_markings(case_params, model_bottom_z, flush=True).clean()


def build_flush_port_marking_parts(params: dict | None = None, model_bottom_z: float = -10.0) -> list[tuple[str, cq.Workplane]]:
    case_params = params or load_params()
    return port_marking_parts(case_params, model_bottom_z, flush=True)


def build_flush_port_marking_cutters(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    case_params = params or load_params()
    return make_port_markings(case_params, model_bottom_z, flush=True, cut_clearance=MARK_CUT_CLEARANCE).clean()


def build_flush_branding_marking(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    return build_flush_top_branding_marking().union(build_flush_port_markings(params, model_bottom_z)).clean()


def load_params() -> dict:
    return json.loads(PARAMS.read_text())


def build_model(params: dict) -> cq.Workplane:
    return build_body_model(params)


def build_branded_export_model(params: dict) -> cq.Workplane:
    body = build_body_model(params)
    branding = build_branding_marking(params, cast(cq.Shape, body.val()).BoundingBox().zmin)
    solids = cast(list[cq.Shape], [*body.solids().vals(), *branding.solids().vals()])
    return cq.Workplane("XY").add(cq.Compound.makeCompound(solids))


def main() -> None:
    params = load_params()
    model = build_branded_export_model(params)
    STEP_OUT.parent.mkdir(parents=True, exist_ok=True)
    STL_OUT.parent.mkdir(parents=True, exist_ok=True)
    cq.exporters.export(model, str(STEP_OUT))
    cq.exporters.export(model, str(STL_OUT), tolerance=0.08, angularTolerance=0.12)
    print(f"wrote {STEP_OUT}")
    print(f"wrote {STL_OUT}")
if __name__ == "__main__":
    main()
