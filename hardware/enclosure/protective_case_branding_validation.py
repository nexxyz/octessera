from __future__ import annotations

from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .branding_marking_cadquery import Point, branding_marking_parts_placed, logo_marking_placed
    from .protective_case_branding import (
        branding_dimensions,
        build_deep_tub_debossed_branding_parts,
        build_deep_tub_flush_branding_parts,
        build_deep_tub_multicolor_branding_parts,
        build_shallow_lid_debossed_branding_parts,
        build_shallow_lid_flush_branding_parts,
        mirror_for_negative_z_exterior_view,
        rotate_lockup_group,
    )
    from .protective_case_geometry import dimensions
    from .protective_case_device_orientation import GUIDE_EXPECTED_BBOX, INLAY_Z_MAX, INLAY_Z_MIN, device_orientation_guide_inlay_components, fused_device_orientation_guide_inlay
except ImportError:
    import generate_protective_case_cadquery as cad
    from branding_marking_cadquery import Point, branding_marking_parts_placed, logo_marking_placed
    from protective_case_branding import branding_dimensions, build_deep_tub_debossed_branding_parts, build_deep_tub_flush_branding_parts, build_deep_tub_multicolor_branding_parts, build_shallow_lid_debossed_branding_parts, build_shallow_lid_flush_branding_parts, mirror_for_negative_z_exterior_view, rotate_lockup_group
    from protective_case_geometry import dimensions
    from protective_case_device_orientation import GUIDE_EXPECTED_BBOX, INLAY_Z_MAX, INLAY_Z_MIN, device_orientation_guide_inlay_components, fused_device_orientation_guide_inlay


TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def close(actual: float, expected: float, label: str) -> None:
    require(abs(actual - expected) <= TOLERANCE, f"{label}={actual:.4f}, expected={expected:.4f}")


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def intersection_volume(first: cq.Workplane, *others: cq.Workplane) -> float:
    result = first
    for other in others:
        result = result.intersect(other)
    return volume(result)


def symmetric_difference_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    if not first.solids().vals():
        return volume(second)
    if not second.solids().vals():
        return volume(first)
    return volume(first.cut(second)) + volume(second.cut(first))


def validate_deep_multicolor_geometry(params: dict, body: cq.Workplane, parts: list[tuple[str, cq.Workplane]]) -> None:
    expected_names = ["protective_case_checkfit_deep_tub_logo", "protective_case_checkfit_deep_tub_device_orientation_guide"]
    require([name for name, _ in parts] == expected_names, "deep multicolor parts must contain logo and orientation guide")
    logo, guide = (part for _, part in parts)
    expected_guide = fused_device_orientation_guide_inlay(params)
    source_components = device_orientation_guide_inlay_components(params)
    require(len(source_components) == 76 and len({name for name, _ in source_components}) == 76, "deep multicolor orientation guide logical component count changed")
    require(all(cast(cq.Shape, component.val()).isValid() and len(component.solids().vals()) == 1 for _, component in source_components), "deep multicolor orientation guide source component changed")
    require(symmetric_difference_volume(guide, expected_guide) <= VOLUME_TOLERANCE, "deep multicolor orientation guide geometry changed")
    require(len(guide.solids().vals()) == len(expected_guide.solids().vals()) == 73, "deep multicolor orientation guide fused solid count changed")
    guide_box = shape_box(guide)
    require(abs(guide_box.zmin - INLAY_Z_MIN) <= TOLERANCE and abs(guide_box.zmax - INLAY_Z_MAX) <= TOLERANCE, "deep multicolor orientation guide Z changed")
    guide_xmin, guide_xmax, guide_ymin, guide_ymax = GUIDE_EXPECTED_BBOX
    require(abs(guide_box.xmin - guide_xmin) <= 0.01 and abs(guide_box.xmax - guide_xmax) <= 0.01 and abs(guide_box.ymin - guide_ymin) <= 0.01 and abs(guide_box.ymax - guide_ymax) <= 0.01, "deep multicolor orientation guide XY changed")
    web = guide_box.zmin - shape_box(logo).zmax
    require(web >= 1.4 - TOLERANCE, f"deep multicolor solid web is too thin: {web:.3f}mm")
    body_shape = cast(cq.Shape, body.val())
    require(body_shape.isValid() and len(body.solids().vals()) == 1 and len(body.shells().vals()) == 1, "deep multicolor body must be one valid solid and shell")
    require(intersection_volume(body, logo) <= VOLUME_TOLERANCE and intersection_volume(body, guide) <= VOLUME_TOLERANCE, "deep multicolor body intersects a marking")
    require(intersection_volume(logo, guide) <= VOLUME_TOLERANCE, "deep multicolor logo intersects the orientation guide")


def validate_branding(params: dict) -> None:
    branding = branding_dimensions(params)
    deep_flush = build_deep_tub_flush_branding_parts(params)
    deep_debossed = build_deep_tub_debossed_branding_parts(params)
    shallow_flush = build_shallow_lid_flush_branding_parts(params)
    shallow_debossed = build_shallow_lid_debossed_branding_parts(params)
    deep_multicolor = build_deep_tub_multicolor_branding_parts(params)
    require(len(deep_flush) == len(deep_debossed) == 1, "deep tub branding must contain logo only")
    require(len(shallow_flush) == len(shallow_debossed) == 2, "shallow lid branding must contain logo and wordmark")
    require(not any("orientation" in name for name, _ in shallow_flush + shallow_debossed), "shallow lid must not contain an orientation diagram")
    dims = dimensions(params)

    def canonical_parts(z_min: float, height: float, mirror: bool, deep_logo: bool = False) -> list[tuple[str, cq.Workplane]]:
        center_x, center_y = branding.combined_center
        if deep_logo:
            expected = [("octessera_logo", logo_marking_placed(branding.deep_tub_logo_target_size, Point(center_x, center_y), z_min, height))]
            if mirror:
                expected = [(name, mirror_for_negative_z_exterior_view(part, branding.combined_center[0])) for name, part in expected]
            return expected
        logo_seed = logo_marking_placed(branding.logo_target_size, Point(center_x, 0.0), z_min, height)
        logo_height = shape_box(logo_seed).ymax - shape_box(logo_seed).ymin
        total_height = logo_height + branding.vertical_gap + branding.wordmark_height
        top_y = center_y + total_height / 2.0
        expected = branding_marking_parts_placed(
            branding.logo_target_size,
            Point(center_x, top_y - logo_height / 2.0),
            branding.wordmark_width,
            branding.wordmark_height,
            Point(center_x, top_y - logo_height - branding.vertical_gap - branding.wordmark_height / 2.0),
            z_min,
            height,
        )
        expected = rotate_lockup_group(expected, branding.combined_center, branding.shallow_lid_lockup_rotation_degrees)
        if mirror:
            expected = [(name, mirror_for_negative_z_exterior_view(part, branding.combined_center[0])) for name, part in expected]
        return expected

    def bbox_text(box: cq.BoundBox) -> str:
        return f"({box.xmin:.6f},{box.ymin:.6f},{box.zmin:.6f},{box.xmax:.6f},{box.ymax:.6f},{box.zmax:.6f})"

    face_sets = (
        ("deep_debossed", deep_debossed, canonical_parts(0.0, branding.deboss_depth, True, True), cad.build_deep_tub(params), 0.0, branding.deboss_depth),
        ("deep_flush", deep_flush, canonical_parts(0.0, branding.flush_depth, True, True), cad.build_deep_tub(params), 0.0, branding.flush_depth),
        ("shallow_debossed", shallow_debossed, canonical_parts(dims.height - branding.deboss_depth, branding.deboss_depth, False), cad.build_pristine_shallow_lid(params), dims.height - branding.deboss_depth, dims.height),
        ("shallow_flush", shallow_flush, canonical_parts(dims.height - branding.flush_depth, branding.flush_depth, False), cad.build_pristine_shallow_lid(params), dims.height - branding.flush_depth, dims.height),
    )
    reported_boxes = {}
    for name, actual_parts, expected_parts, body, z_min, z_max in face_sets:
        owner_name = "deep_tub" if name.startswith("deep_") else "shallow_lid"
        expected_names = [f"protective_case_checkfit_{owner_name}_{mark_name.removeprefix('octessera_')}" for mark_name, _ in expected_parts]
        require([part_name for part_name, _ in actual_parts] == expected_names, f"{name} branding names changed")
        body_box = shape_box(body)
        for (actual_name, actual), (_, expected) in zip(actual_parts, expected_parts):
            actual_box = shape_box(actual)
            expected_box = shape_box(expected)
            require(symmetric_difference_volume(actual, expected) <= VOLUME_TOLERANCE, f"{name} {actual_name} differs from canonical placement")
            for axis in ("xmin", "ymin", "xmax", "ymax"):
                close(getattr(actual_box, axis), getattr(expected_box, axis), f"{name}_{actual_name}_{axis}")
            close(actual_box.zmin, z_min, f"{name}_{actual_name}_zmin")
            close(actual_box.zmax, z_max, f"{name}_{actual_name}_zmax")
            require(body_box.xmin - TOLERANCE <= actual_box.xmin <= actual_box.xmax <= body_box.xmax + TOLERANCE, f"{name} {actual_name} exceeds body X footprint")
            require(body_box.ymin - TOLERANCE <= actual_box.ymin <= actual_box.ymax <= body_box.ymax + TOLERANCE, f"{name} {actual_name} exceeds body Y footprint")
            require(body_box.zmin - TOLERANCE <= actual_box.zmin <= actual_box.zmax <= body_box.zmax + TOLERANCE, f"{name} {actual_name} exceeds body Z footprint")
            reported_boxes[f"{name}_{actual_name.removeprefix('protective_case_checkfit_')}"] = actual_box
        if len(actual_parts) == 2:
            require(intersection_volume(actual_parts[0][1], actual_parts[1][1]) <= VOLUME_TOLERANCE, f"{name} logo and wordmark overlap")

    for debossed, flush in ((deep_debossed, deep_flush), (shallow_debossed, shallow_flush)):
        for (_, debossed_part), (_, flush_part) in zip(debossed, flush):
            debossed_box = shape_box(debossed_part)
            flush_box = shape_box(flush_part)
            for axis in ("xmin", "ymin", "xmax", "ymax"):
                close(getattr(debossed_box, axis), getattr(flush_box, axis), f"branding_xy_{axis}")

    for name, parts in (("deep", deep_debossed), ("shallow", shallow_debossed)):
        model = cad.build_debossed_deep_tub(params)[0] if name == "deep" else cad.build_debossed_shallow_lid(params)[0]
        require(cast(cq.Shape, model.val()).isValid() and len(model.solids().vals()) == 1, f"{name} debossed model must be one valid solid")
        base = cad.build_deep_tub(params) if name == "deep" else cad.build_shallow_lid(params)
        branding_volume = sum(volume(part) for _, part in parts)
        require(abs(branding_volume - sum(intersection_volume(part, base) for _, part in parts)) <= VOLUME_TOLERANCE, f"{name} debossed branding cuts outside the shell")
        require(volume(model.cut(base)) <= VOLUME_TOLERANCE, f"{name} debossed branding is proud of the shell")
        require(all(intersection_volume(part, model) <= VOLUME_TOLERANCE for _, part in parts), f"{name} debossed marks intersect the cut body")
    for name, model, parts in (
        ("deep", cad.build_multicolor_deep_tub(params)[0], deep_multicolor),
        ("shallow", cad.build_multicolor_shallow_lid(params)[0], shallow_flush),
    ):
        if name == "deep":
            validate_deep_multicolor_geometry(params, model, parts)
        require(all(intersection_volume(part, model) <= VOLUME_TOLERANCE for _, part in parts), f"{name} flush marks intersect the cut body")
    print("branding=deep_logo_only deep_multicolor_orientation_guide shallow_canonical_lockup debossed0.40 flush0.40=PASS")
    print("branding_bboxes=" + ",".join(f"{name}:{bbox_text(box)}" for name, box in reported_boxes.items()))
