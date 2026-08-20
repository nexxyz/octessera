from __future__ import annotations

import cadquery as cq

from faceplate_insert_pillars import add_faceplate_insert_pillars, subtract_faceplate_insert_holes
from faceplate_walls import perimeter_wall_skirts
from top_faceplate_features import (
    NEOKEY_WAVE_HOLLOW_SOUTH_EXTRA,
    add_cutouts,
    add_neokey_cutouts,
    neokey_key_centers,
    neokey_seat_bounds,
)
from top_wall_port_cutouts import add_top_wall_port_cutouts
from top_wave_geometry import (
    EXTENDED_SLOPE_RIGHT_X,
    HIGH_UNDERSIDE_Z,
    HIGH_Z,
    LOWER_TO_TIER2_RAMP_END_X,
    LOWER_TO_TIER2_RAMP_START_X,
    LOWER_WAVE_HIGH_UNDERSIDE_Z,
    LOWER_WAVE_HIGH_Z,
    LOW_Z,
    SOUTH_SHOULDER_PLAN_WIDTH,
    TIER1_WAVE_SEAM_OVERLAP,
    UNDERSIDE_Z,
    WEST_EXTENSION,
    add_guidance_slots,
    east_wave_ramp_loft,
    first_y_at_x,
    left_region_prism,
    rectangular_lower_wave_slope_loft,
    right_region_prism,
    shoulder_loft,
    shoulder_plan_prism,
    south_edge_samples,
    west_wave_wall,
    x_band_prism,
    y_band_prism,
)


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
    high_plate = rounded_plate(width, depth, radius, HIGH_UNDERSIDE_Z, top_thick).intersect(right_region)
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
