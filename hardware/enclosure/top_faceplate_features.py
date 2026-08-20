from __future__ import annotations

import cadquery as cq

from faceplate_neokey_support import (
    neokey_deck_cap,
    neokey_raised_cap,
    neokey_south_slot_fill,
    neokey_support_block,
)
from top_wave_geometry import EXTENDED_SLOPE_RIGHT_X, HIGH_Z, LOW_Z, UNDERSIDE_Z


NEOKEY_PANEL_X_OFFSET = -0.25
NEOKEY_PANEL_Y_OFFSET = -1.0
NEOKEY_TOP_Z = 16.0
NEOKEY_DECK_TOP_Z = HIGH_Z + 3.0
NEOKEY_KEYCAP_RECESS_DEPTH = 1.0
NEOKEY_MX_LATCH_PLATE_THICKNESS = 1.5
NEOKEY_MX_UNDERSIDE_CLEARANCE = 2.6
NEOKEY_MX_MOUNTING_GRID_Z_DROP = 2.0
NEOKEY_SEAT_BOTTOM_Z = UNDERSIDE_Z + 1.0
NEOKEY_SEAT_OVERLAP = 3.0
NEOKEY_WAVE_HOLLOW_SOUTH_EXTRA = 1.0
OLED_SCREEN_CUTOUT_X_SHIFT = -0.5
OLED_SCREEN_CUTOUT_Y_SHIFT = -0.3


def rect_cutter(x0: float, y0: float, x1: float, y1: float, radius: float) -> cq.Workplane:
    width = x1 - x0
    depth = y1 - y0
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    cutter = cq.Workplane("XY").placeSketch(sketch).extrude(40)
    return cutter.translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, -2))


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
        (
            local_to_case(params, point)[0] + NEOKEY_PANEL_X_OFFSET,
            local_to_case(params, point)[1] + NEOKEY_PANEL_Y_OFFSET,
        )
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
