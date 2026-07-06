from __future__ import annotations

import json
import math
from pathlib import Path

import cadquery as cq

from wave_guidance import (
    SLOPE_PROFILE_STEPS,
    SOUTH_ROOF_LOW_WALL_BAND,
    SOUTH_SHOULDER_PLAN_WIDTH,
    load_guidance_slots,
    south_edge_samples,
)


ROOT = Path(__file__).resolve().parent
PARAMS = ROOT / "enclosure_params.json"
STEP_OUT = ROOT / "case_top_two_level_cadquery.step"
STL_OUT = ROOT / "case_top_two_level_cadquery.stl"


LOW_Z = 12.0
HIGH_Z = 26.0
UNDERSIDE_Z = 9.0
HIGH_UNDERSIDE_Z = 23.0
EXTENDED_SLOPE_RIGHT_X = 115.0
NEOKEY_PANEL_Y_OFFSET = 2.0
NEOKEY_CAP_RELIEF_BOTTOM_Z = 16.0
NEOKEY_SEAT_BOTTOM_Z = UNDERSIDE_Z
NEOKEY_SEAT_OVERLAP = 3.0
LOWER_WAVE_HEIGHT_SCALE = 0.5
LOWER_WAVE_HIGH_UNDERSIDE_Z = UNDERSIDE_Z + (HIGH_UNDERSIDE_Z - UNDERSIDE_Z) * LOWER_WAVE_HEIGHT_SCALE
LOWER_WAVE_HIGH_Z = LOW_Z + (HIGH_Z - LOW_Z) * LOWER_WAVE_HEIGHT_SCALE
LOWER_TO_TIER2_RAMP_START_X = 105.0
LOWER_TO_TIER2_RAMP_END_X = 115.0

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
    _, low = south_edge_samples()
    points = [(-margin, -margin), (-margin, depth + margin), (low[-1][0] + 0.2, depth + margin)]
    points += [(x + 0.2, y) for x, y in reversed(low)]
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
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    prism = cq.Workplane("XY").placeSketch(sketch).extrude(z1 - z0)
    return prism.translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))


def west_rounded_rect_prism(
    x0: float, y0: float, x1: float, y1: float, radius: float, z0: float, z1: float
) -> cq.Workplane:
    arc_offset = radius * (1.0 - 2.0**-0.5)
    return (
        cq.Workplane("XY")
        .moveTo(x1, y0)
        .lineTo(x1, y1)
        .lineTo(x0 + radius, y1)
        .threePointArc((x0 + arc_offset, y1 - arc_offset), (x0, y1 - radius))
        .lineTo(x0, y0 + radius)
        .threePointArc((x0 + arc_offset, y0 + arc_offset), (x0 + radius, y0))
        .close()
        .extrude(z1 - z0)
        .translate((0, 0, z0))
    )


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
    cutter = (
        cq.Workplane("XY", origin=(0, 0, bottom_z))
        .circle(flat_r)
        .workplane(offset=depth + 0.05)
        .circle(outer_r)
        .loft(combine=True)
    )
    return cutter.translate((x, y, 0))


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


def neokey_support_block(params: dict, key_centers: list[tuple[float, float]]) -> cq.Workplane:
    seat_x0, seat_y0, seat_x1, seat_y1 = neokey_seat_bounds(params, key_centers)
    return west_rounded_rect_prism(
        seat_x0,
        seat_y0,
        seat_x1,
        seat_y1,
        params["key_cutout_r"],
        NEOKEY_SEAT_BOTTOM_Z,
        HIGH_Z,
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
    slot_x0, slot_y0, slot_x1, slot_y1 = neokey_slot_bounds(params, key_centers)
    model = model.union(neokey_support_block(params, key_centers))
    model = model.cut(
        rect_cutter_from_z(
            slot_x0,
            slot_y0,
            slot_x1,
            slot_y1,
            params["key_cutout_r"],
            NEOKEY_CAP_RELIEF_BOTTOM_Z,
        )
    )
    mx_cutout = params["mx_switch_retention_cutout"]
    for x, y in key_centers:
        model = model.cut(
            rect_cutter(
                x - mx_cutout / 2,
                y - mx_cutout / 2,
                x + mx_cutout / 2,
                y + mx_cutout / 2,
                params["mx_switch_retention_r"],
            )
        )
    return model


def add_cutouts(model: cq.Workplane, params: dict) -> cq.Workplane:
    screen_cx, screen_cy = local_to_case(params, params["features_local"]["oled_screen_center"])
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
            y = 22.5 + row * neo_pitch
            model = model.cut(rect_cutter(x - neo_d / 2, y - neo_d / 2, x + neo_d / 2, y + neo_d / 2, params["neotrellis_button_r"]))

    return model


def build_model(params: dict) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    radius = params["corner_r"]
    top_thick = params["top_thick"]
    neokey_seat_x0, _, _, neokey_seat_top_y = neokey_seat_bounds(
        params, neokey_key_centers(params)
    )
    _, low_edge = south_edge_samples()
    lower_wave_top_y = first_y_at_x(low_edge, neokey_seat_x0)

    footprint = rounded_plate(width, depth, radius, 0, 40)
    low_plate = rounded_plate(width, depth, radius, UNDERSIDE_Z, top_thick).intersect(
        left_region_prism(width, depth, 5, 40)
    )
    right_region = right_region_prism(width, depth, 5, 40)
    wave_strip_region = right_region.intersect(
        x_band_prism(-5.0, EXTENDED_SLOPE_RIGHT_X, depth, 5, 40)
    )
    high_plate = rounded_plate(width, depth, radius, HIGH_UNDERSIDE_Z, top_thick).intersect(
        right_region
    )
    high_plate = high_plate.cut(
        wave_strip_region.intersect(y_band_prism(width, -5.0, neokey_seat_top_y, 5, 40))
    )
    ramp_region = x_band_prism(
        LOWER_TO_TIER2_RAMP_START_X, LOWER_TO_TIER2_RAMP_END_X, depth, 5, 40
    ).intersect(y_band_prism(width, -5.0, lower_wave_top_y, 5, 40))
    high_plate = high_plate.cut(ramp_region)
    lower_wave_plate = rounded_plate(
        width, depth, radius, LOWER_WAVE_HIGH_UNDERSIDE_Z, top_thick
    ).intersect(
        wave_strip_region.intersect(y_band_prism(width, -5.0, lower_wave_top_y, 5, 40))
    )
    lower_wave_plate = lower_wave_plate.cut(ramp_region)
    wave_flat_ramp = east_wave_ramp_loft(0.0, lower_wave_top_y)
    flat_faceplate = add_cutouts(
        low_plate.union(lower_wave_plate).union(wave_flat_ramp).union(high_plate).clean(),
        params,
    ).clean()
    upper_shoulder = shoulder_loft(neokey_seat_top_y, depth).intersect(footprint).clean()
    lower_wave = (
        shoulder_loft(0.0, lower_wave_top_y, LOWER_WAVE_HEIGHT_SCALE)
        .intersect(footprint)
        .clean()
    )
    shoulder = upper_shoulder.union(lower_wave).clean()
    model = flat_faceplate.union(shoulder).clean()
    model = add_neokey_cutouts(model, params).clean()
    return add_guidance_slots(model).clean()


def main() -> None:
    params = json.loads(PARAMS.read_text())
    model = build_model(params)
    cq.exporters.export(model, str(STEP_OUT))
    cq.exporters.export(model, str(STL_OUT), tolerance=0.08, angularTolerance=0.12)
    from generate_wave_review_view import main as generate_review_view

    generate_review_view()
    print(f"wrote {STEP_OUT}")
    print(f"wrote {STL_OUT}")


if __name__ == "__main__":
    main()
