from __future__ import annotations

import cadquery as cq

from top_body_assembly import (
    build_body_model,
    rounded_plate,
    west_extended_footprint,
    west_extension_solid,
)
from top_branding_variants import (
    BRANDING_RAISE,
    ORANGE_PI_EAST_USB_CENTER_X,
    ORANGE_PI_USB_WIDTH,
    build_branding_marking,
    build_flush_branding_marking,
    build_flush_port_marking_cutters,
    build_flush_port_marking_parts,
    build_flush_port_markings,
    build_flush_top_branding_marking,
    build_flush_top_branding_parts,
    orange_pi_top_params,
)
from top_enclosure_config import (
    ARTIFACT_ROOT,
    ORANGE_PI_STEP_OUT,
    ORANGE_PI_STL_OUT,
    PARAMS,
    ROOT,
    STEP_OUT,
    STL_OUT,
    load_params,
)
from top_enclosure_export import build_branded_export_model, export_top_variant
from top_faceplate_features import (
    NEOKEY_DECK_TOP_Z,
    NEOKEY_KEYCAP_RECESS_DEPTH,
    NEOKEY_MX_LATCH_PLATE_THICKNESS,
    NEOKEY_MX_MOUNTING_GRID_Z_DROP,
    NEOKEY_MX_UNDERSIDE_CLEARANCE,
    NEOKEY_PANEL_X_OFFSET,
    NEOKEY_PANEL_Y_OFFSET,
    NEOKEY_SEAT_BOTTOM_Z,
    NEOKEY_SEAT_OVERLAP,
    NEOKEY_TOP_Z,
    NEOKEY_WAVE_HOLLOW_SOUTH_EXTRA,
    OLED_SCREEN_CUTOUT_X_SHIFT,
    OLED_SCREEN_CUTOUT_Y_SHIFT,
    add_cutouts,
    add_neokey_cutouts,
    circle_cutter,
    crater_cutter,
    local_to_case,
    neokey_key_centers,
    neokey_seat_bounds,
    neokey_slot_bounds,
    rect_cutter,
    rect_prism,
)
from top_wall_port_cutouts import add_top_wall_port_cutouts
from port_markings_cadquery import MARK_CUT_CLEARANCE
from top_wave_geometry import (
    EXTENDED_SLOPE_RIGHT_X,
    HIGH_UNDERSIDE_Z,
    HIGH_Z,
    LOWER_TO_TIER2_RAMP_END_X,
    LOWER_TO_TIER2_RAMP_START_X,
    LOWER_WAVE_HIGH_UNDERSIDE_Z,
    LOWER_WAVE_HIGH_Z,
    LOW_Z,
    SOUTH_ROOF_LOW_WALL_BAND,
    SOUTH_SHOULDER_PLAN_WIDTH,
    TIER1_WAVE_SEAM_OVERLAP,
    UNDERSIDE_Z,
    WEST_EXTENSION,
    add_guidance_slots,
    curve_pair_at_y,
    east_wave_ramp_loft,
    first_y_at_x,
    left_region_prism,
    load_guidance_slots,
    quarter_circle_ease,
    rectangular_lower_wave_slope_loft,
    right_region_prism,
    shoulder_profile_wire,
    shoulder_loft,
    shoulder_plan_prism,
    south_edge_samples,
    slot_cutter,
    trimmed_curve_pairs,
    west_wave_wall,
    x_at_y,
    x_band_prism,
    y_band_prism,
)


def build_model(params: dict) -> cq.Workplane:
    return build_body_model(params)


def main() -> None:
    params = load_params()
    for variant_params, step_out, stl_out in [
        (params, STEP_OUT, STL_OUT),
        (orange_pi_top_params(params), ORANGE_PI_STEP_OUT, ORANGE_PI_STL_OUT),
    ]:
        export_top_variant(variant_params, step_out, stl_out)


if __name__ == "__main__":
    main()
