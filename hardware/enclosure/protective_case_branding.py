from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, cast

import cadquery as cq

try:
    from .branding_marking_cadquery import Point, branding_marking_parts_placed, logo_marking_placed
    from .protective_case_device_orientation import fused_device_orientation_guide_inlay
    from .protective_case_geometry import dimensions
except ImportError:
    from branding_marking_cadquery import Point, branding_marking_parts_placed, logo_marking_placed
    from protective_case_device_orientation import fused_device_orientation_guide_inlay
    from protective_case_geometry import dimensions


@dataclass(frozen=True)
class BrandingDimensions:
    logo_target_size: float
    wordmark_width: float
    wordmark_height: float
    vertical_gap: float
    shallow_lid_lockup_rotation_degrees: float
    combined_center: tuple[float, float]
    deep_tub_logo_target_size: float
    deboss_depth: float
    flush_depth: float


def branding_dimensions(params: dict) -> BrandingDimensions:
    spec = params["branding"]
    return BrandingDimensions(
        logo_target_size=spec["logo_target_size"],
        wordmark_width=spec["wordmark_width"],
        wordmark_height=spec["wordmark_height"],
        vertical_gap=spec["vertical_gap"],
        shallow_lid_lockup_rotation_degrees=spec["shallow_lid_lockup_rotation_degrees"],
        combined_center=tuple(spec["combined_center"]),
        deep_tub_logo_target_size=spec["deep_tub_logo_target_size"],
        deboss_depth=spec["deboss_depth"],
        flush_depth=spec["flush_depth"],
    )


def mirror_for_negative_z_exterior_view(part: cq.Workplane, center_x: float) -> cq.Workplane:
    return part.mirror("YZ", basePointVector=(center_x, 0.0, 0.0))


def rotate_lockup_group(parts: list[tuple[str, cq.Workplane]], center: tuple[float, float], degrees: float) -> list[tuple[str, cq.Workplane]]:
    axis_start = (center[0], center[1], 0.0)
    axis_end = (center[0], center[1], 1.0)
    return [(name, part.rotate(axis_start, axis_end, degrees)) for name, part in parts]


def _part_box(part: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, part.val()).BoundingBox()


def _lockup_parts(params: dict, z0: float, height: float) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    center_x, center_y = branding.combined_center
    logo_seed = logo_marking_placed(
        branding.logo_target_size,
        Point(center_x, 0.0),
        z0,
        height,
    )
    logo_height = _part_box(logo_seed).ymax - _part_box(logo_seed).ymin
    total_height = logo_height + branding.vertical_gap + branding.wordmark_height
    top_y = center_y + total_height / 2.0
    logo_center_y = top_y - logo_height / 2.0
    wordmark_center_y = top_y - logo_height - branding.vertical_gap - branding.wordmark_height / 2.0
    return branding_marking_parts_placed(
        branding.logo_target_size,
        Point(center_x, logo_center_y),
        branding.wordmark_width,
        branding.wordmark_height,
        Point(center_x, wordmark_center_y),
        z0,
        height,
    )


def _deep_tub_logo(params: dict, z0: float, height: float) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    return [
        (
            "protective_case_deep_tub_logo",
            mirror_for_negative_z_exterior_view(
                logo_marking_placed(
                    branding.deep_tub_logo_target_size,
                    Point(branding.combined_center[0], branding.combined_center[1]),
                    z0,
                    height,
                ),
                branding.combined_center[0],
            ),
        )
    ]


def _shallow_lid_lockup(params: dict, z0: float, height: float) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    return [
        (f"protective_case_shallow_lid_{name.removeprefix('octessera_')}", part)
        for name, part in rotate_lockup_group(_lockup_parts(params, z0, height), branding.combined_center, branding.shallow_lid_lockup_rotation_degrees)
    ]


def build_deep_tub_flush_branding_parts(params: dict) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    return _deep_tub_logo(params, 0.0, branding.flush_depth)


def build_deep_tub_multicolor_branding_parts(params: dict) -> list[tuple[str, cq.Workplane]]:
    return build_deep_tub_flush_branding_parts(params) + [
        ("protective_case_deep_tub_device_orientation_guide", fused_device_orientation_guide_inlay(params))
    ]


def _deep_tub_multicolor_cutting_parts(params: dict) -> list[tuple[str, cq.Workplane]]:
    return build_deep_tub_flush_branding_parts(params) + [
        ("device_orientation_guide_cutter", fused_device_orientation_guide_inlay(params, True))
    ]


def build_deep_tub_debossed_branding_parts(params: dict) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    return _deep_tub_logo(params, 0.0, branding.deboss_depth)


def build_shallow_lid_flush_branding_parts(params: dict) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    return _shallow_lid_lockup(
        params,
        dimensions(params).height - branding.flush_depth,
        branding.flush_depth,
    )


def build_shallow_lid_debossed_branding_parts(params: dict) -> list[tuple[str, cq.Workplane]]:
    branding = branding_dimensions(params)
    return _shallow_lid_lockup(
        params,
        dimensions(params).height - branding.deboss_depth,
        branding.deboss_depth,
    )


def _compose_debossed(
    params: dict,
    builder: Callable[[dict], cq.Workplane],
    branding_builder: Callable[[dict], list[tuple[str, cq.Workplane]]],
) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    branding_parts = branding_builder(params)
    model = builder(params)
    for _, part in branding_parts:
        for solid in part.solids().vals():
            model = model.cut(cq.Workplane("XY").add(solid)).clean()
    return model, branding_parts


def _compose_multicolor(
    params: dict,
    builder: Callable[[dict], cq.Workplane],
    branding_builder: Callable[[dict], list[tuple[str, cq.Workplane]]],
    cutting_builder: Callable[[dict], list[tuple[str, cq.Workplane]]],
) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    branding_parts = branding_builder(params)
    model = builder(params)
    for _, part in cutting_builder(params):
        for solid in part.solids().vals():
            model = model.cut(cq.Workplane("XY").add(solid)).clean()
    return model, branding_parts


def compose_debossed_deep_tub(params: dict, builder: Callable[[dict], cq.Workplane]) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return _compose_debossed(params, builder, build_deep_tub_debossed_branding_parts)


def compose_multicolor_deep_tub(params: dict, builder: Callable[[dict], cq.Workplane]) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return _compose_multicolor(
        params,
        builder,
        build_deep_tub_multicolor_branding_parts,
        _deep_tub_multicolor_cutting_parts,
    )


def compose_debossed_shallow_lid(params: dict, builder: Callable[[dict], cq.Workplane]) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return _compose_debossed(params, builder, build_shallow_lid_debossed_branding_parts)


def compose_multicolor_shallow_lid(params: dict, builder: Callable[[dict], cq.Workplane]) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return _compose_multicolor(params, builder, build_shallow_lid_flush_branding_parts, build_shallow_lid_flush_branding_parts)
