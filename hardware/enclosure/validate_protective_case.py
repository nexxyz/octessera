from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_3mf_validation import (
        validate_deep_tub_debossed_package,
        validate_deep_tub_multicolor_package,
        validate_shallow_lid_debossed_package,
        validate_shallow_lid_multicolor_package,
    )
    from .protective_case_branding import (
        branding_dimensions,
        build_deep_tub_multicolor_branding_parts,
        build_shallow_lid_flush_branding_parts,
    )
    from .protective_case_branding_validation import validate_branding
    from .protective_case_bottom_latches import validate_bottom_latches, validate_source_box_clip_ownership
    from .protective_case_corner_restraints import validate_corner_restraints
    from .protective_case_device_fit import validate_reference_top_fit
    from .protective_case_foam_landing_pads import add_foam_landing_pads, validate_foam_landing_pads
    from .protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface, transformed_hinge_profile, validate_hinge_profile_preservation, validate_mating_interface
    from .protective_case_geometry import (
        artifact_paths,
        build_device_insertion,
        corner_restraints,
        dimensions,
        hinge_axis,
        hinge_centers,
        load_parameters,
        normalize_source_half,
        parametric_box_measures,
    )
except ImportError:
    import generate_protective_case_cadquery as cad
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_3mf_validation import validate_deep_tub_debossed_package, validate_deep_tub_multicolor_package, validate_shallow_lid_debossed_package, validate_shallow_lid_multicolor_package
    from protective_case_branding import branding_dimensions, build_deep_tub_multicolor_branding_parts, build_shallow_lid_flush_branding_parts
    from protective_case_branding_validation import validate_branding
    from protective_case_bottom_latches import validate_bottom_latches, validate_source_box_clip_ownership
    from protective_case_corner_restraints import validate_corner_restraints
    from protective_case_device_fit import validate_reference_top_fit
    from protective_case_foam_landing_pads import add_foam_landing_pads, validate_foam_landing_pads
    from protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface, transformed_hinge_profile, validate_hinge_profile_preservation, validate_mating_interface
    from protective_case_geometry import artifact_paths, build_device_insertion, corner_restraints, dimensions, hinge_axis, hinge_centers, load_parameters, normalize_source_half, parametric_box_measures


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


def prism(x: float, y: float, z: float, width: float, depth: float, height: float) -> cq.Workplane:
    return cq.Workplane("XY").box(width, depth, height, centered=(False, False, False)).translate((x, y, z))


def valid_single(model: cq.Workplane, label: str) -> None:
    require(cast(cq.Shape, model.val()).isValid() and len(model.solids().vals()) == 1, f"{label} must be one valid solid")


def edge_radius(edge) -> float:
    return getattr(edge, "radius")()


def validate_parameter_relationships(params: dict) -> None:
    dims = dimensions(params)
    box = params["parametric_box"]
    hinge = box["hinge"]
    clip = box["clip"]
    pins = params["hinge_pins"]
    require(
        box == {
            "width": 255.6,
            "depth": 148.4,
            "height": 64.85,
            "thickness": 2.2,
            "clearance": 0.25,
            "top_height": 54.85,
            "chamfer_z": 8.0,
            "chamfer_xy": 0.0,
            "text": "",
            "hinge": hinge,
            "clip": clip,
        },
        "parametric box dimensions changed",
    )
    require(hinge["kunkle_size"] == 6.0 and hinge["number_of_kunkles"] == 5 and hinge["clearance"] == 0.5, "upstream hinge measures changed")
    require(hinge["leaf_width"] == 6.0 and hinge["leaf_height"] == 2.0 and hinge["pivot_radius"] == 0.875, "upstream hinge leaf measures changed")
    require(clip == {"length": 10.0, "positions_x": [-55.0, 55.0]}, "upstream clip placement changed")
    require(
        params["bottom_latches"]
        == {
            "root_bridge": {"depth": 0.2, "host_profile_overlap": 0.1},
            "actuator": {"width": 19.4, "z": [56.95, 60.7]},
            "catch": {"width": 23.4, "z": [47.6, 51.6]},
        },
        "bottom latch bridge dimensions changed",
    )
    require(box["chamfer_xy"] >= 0.0, "horizontal chamfer must be nonnegative")
    require(abs(hinge["number_of_kunkles"] * hinge["kunkle_size"] + hinge["clearance"] - 30.5) <= TOLERANCE, "upstream leaf depth changed")
    close(dims.width, 255.6, "case_width")
    close(dims.depth, 148.4, "case_depth")
    close(dims.height, 64.85, "case_height")
    require(dims.inner_width == 251.2 and dims.inner_depth == 144.0, "inner wall dimensions changed")
    require(hinge_axis(params) == (-1.0, 54.85), "transformed hinge axis changed")
    require(hinge_centers(params) == (63.9, 191.7), "transformed hinge centers changed")
    require(pins == {"piece_count": 2, "piece_length": 34.5, "end_exposure": 2.0, "nominal_diameter": 1.75, "paddle_envelope": {"axial_length": 2.0, "vertical_span": 4.0, "outward_depth": 2.0, "inward_allowance": 0.4}}, "hinge pin measures changed")
    require(params["wall_planes"] == {"west_x": 2.2, "east_x": 253.4, "south_y": 2.2, "north_y": 146.2}, "inner wall planes changed")
    require(len(corner_restraints(params)) == 4, "expected four corner restraints")
    restraint_spec = params["corner_restraints"]
    require(
        restraint_spec["support_lower_z"] == 2.0
        and restraint_spec["shelf_overlap"] == 0.10
        and restraint_spec["shelf_plan_radius"] == 1.0
        and restraint_spec["shelf_top_edge_radius"] == 0.6
        and restraint_spec["support_contact_length_min"] == 3.0
        and restraint_spec["wall_intersection_volume_min"] == 30.0,
        "corner restraint elevations or shared spans changed",
    )
    require(params["device"]["envelope"] == {"width": 248.0, "depth": 140.0, "radius": 8.0, "height": 45.0}, "device envelope changed")
    require(params["device"]["check_fit"] == {"width": 249.0, "depth": 141.0, "radius": 8.0, "height": 45.0}, "device check-fit changed")
    device = params["device"]
    require(
        device["reference_top_variants"] == ["raspberry-pi-zero-2w", "orange-pi-zero-2w"]
        and device["face_down_transform"] == {"rotate_y_degrees": 180.0, "translate": [250.8, 4.2, 42.0]},
        "reference top variants or face-down transform changed",
    )
    require("measured_origin" not in device, "legacy measured device origin must remain absent")
    require(
        device["xy_retention"]
        == {
            "method": "adhesive_foam_strips",
            "thickness_range": [1.0, 2.0],
            "nominal_thickness_by_wall": {"west": 1.5, "east": 1.5, "south": 2.0, "north": 2.0},
            "straight_inner_wall_spans": ["west", "east", "south", "north"],
            "strips_per_span": 1,
            "material": "soft_closed_cell_adhesive_foam",
            "stacking": False,
            "integrated_hard_locators": False,
        },
        "XY retention contract changed",
    )
    nominal_x_clearance = (dims.inner_width - device["envelope"]["width"]) / 2.0
    nominal_y_clearance = (dims.inner_depth - device["envelope"]["depth"]) / 2.0
    check_fit_x_clearance = (dims.inner_width - device["check_fit"]["width"]) / 2.0
    check_fit_y_clearance = (dims.inner_depth - device["check_fit"]["depth"]) / 2.0
    close(nominal_x_clearance, 1.6, "nominal_hard_wall_clearance_x")
    close(nominal_y_clearance, 2.0, "nominal_hard_wall_clearance_y")
    close(check_fit_x_clearance, 1.1, "check_fit_hard_wall_clearance_x")
    close(check_fit_y_clearance, 1.5, "check_fit_hard_wall_clearance_y")
    branding = branding_dimensions(params)
    require(branding.deboss_depth == 0.4 and branding.flush_depth == 0.4, "branding depths changed")
    require(len(artifact_paths(params)) == 8 and len({path.name for path in artifact_paths(params)}) == 8, "expected eight exact artifact paths")
    print(f"case={dims.width:.1f}x{dims.depth:.1f}x{dims.height:.2f} inner={dims.inner_width:.1f}x{dims.inner_depth:.1f} axis=Y{hinge_axis(params)[0]:.2f}/Z{hinge_axis(params)[1]:.2f}")
    print(f"xy_retention=adhesive_foam_strips spans=west,east,south,north strips_per_span=1 nominal_hard_clearance=+/-{nominal_x_clearance:.1f}X/+/-{nominal_y_clearance:.1f}Y check_fit_hard_clearance=+/-{check_fit_x_clearance:.1f}X/+/-{check_fit_y_clearance:.1f} integrated_hard_locators=false physical_compression=UNMEASURED")


def validate_upstream_default() -> None:
    source_box = upstream.build_box(upstream.default_measures())
    expected = {
        "top": (15683.8187, (100.0, 67.0, 18.8146)),
        "bottom": (23552.5168, (100.0, 65.4350, 23.0)),
    }
    for name, part in (("top", source_box.top()), ("bottom", source_box.bottom())):
        valid_single(part, f"upstream default {name}")
        bbox = shape_box(part)
        close(volume(part), expected[name][0], f"upstream_default_{name}_volume")
        for actual, expected_size, axis in zip((bbox.xmax - bbox.xmin, bbox.ymax - bbox.ymin, bbox.zmax - bbox.zmin), expected[name][1], "xyz"):
            close(actual, expected_size, f"upstream_default_{name}_{axis}")
    print("upstream_default=top15683.8187 bottom23552.5168 bbox_regression=PASS")


def validate_source_halves(params: dict) -> tuple[cq.Workplane, cq.Workplane]:
    deep = cad.build_pristine_deep_tub(params)
    shallow = cad.build_pristine_shallow_lid(params)
    source_box = cad.build_source_box(params)
    validate_source_box_clip_ownership(source_box)
    for name, part in (("deep_tub", deep), ("shallow_lid", shallow)):
        valid_single(part, name)
    for name, part, expected in (
        ("deep_body", normalize_source_half(source_box.source_top(), params), (0.0, 255.6, 0.0, 148.4, 0.0, 54.6)),
        ("shallow_body", normalize_source_half(source_box.source_bottom(), params), (0.0, 255.6, 0.0, 148.4, 52.65, 64.85)),
        ("deep_pristine", deep, (0.0, 255.6, -4.0, 148.4, 0.0, 57.85)),
        ("shallow_pristine", shallow, (0.0, 255.6, -4.0, 148.4, 51.85, 64.85)),
    ):
        bbox = shape_box(part)
        for actual, expected_value, axis in zip((bbox.xmin, bbox.xmax, bbox.ymin, bbox.ymax, bbox.zmin, bbox.zmax), expected, ("xmin", "xmax", "ymin", "ymax", "zmin", "zmax")):
            close(actual, expected_value, f"{name}_{axis}")
    deep_body = normalize_source_half(source_box.source_top(), params)
    floor_probe = prism(80.0, 40.0, 0.0, 95.6, 68.4, 2.2)
    require(abs(volume(deep_body.intersect(floor_probe)) - volume(floor_probe)) <= VOLUME_TOLERANCE, "deep tub inner floor is not flat at Z2.2")
    require(volume(deep_body.intersect(prism(80.0, 40.0, 2.2, 95.6, 68.4, 0.05))) <= VOLUME_TOLERANCE, "deep tub floor rises above Z2.2")
    deep_augmented = cad.build_deep_tub(params)
    shallow_augmented = cad.build_shallow_lid(params)
    deep_with_foam = add_foam_landing_pads(params, apply_deep_mating_interface(params, deep))
    shallow_corrected = apply_shallow_mating_interface(params, shallow)
    for name, before, after, probe in (
        ("deep_side", deep, deep_augmented, prism(40.0, 0.0, 0.0, 175.6, 3.0, 54.6)),
        ("deep_mating", deep, deep_augmented, prism(40.0, 40.0, 53.5, 175.6, 68.4, 1.0)),
        ("shallow_side", shallow, shallow_augmented, prism(40.0, 0.0, 52.65, 175.6, 3.0, 12.2)),
        ("shallow_mating", shallow, shallow_augmented, prism(40.0, 40.0, 52.65, 175.6, 68.4, 1.0)),
    ):
        expected = deep_with_foam if name.startswith("deep_") else shallow_corrected
        close(volume(expected.intersect(probe)), volume(after.intersect(probe)), f"{name}_section_volume")
    for name, model, branding_parts in (
        ("deep", deep_with_foam, build_deep_tub_multicolor_branding_parts(params)),
        ("shallow", shallow_augmented, build_shallow_lid_flush_branding_parts(params)),
    ):
        multicolor = cad.build_multicolor_deep_tub(params)[0] if name == "deep" else cad.build_multicolor_shallow_lid(params)[0]
        removed = volume(model.cut(multicolor))
        branding_volume = sum(volume(part) for _, part in branding_parts)
        require(removed > 0.0 and removed <= branding_volume + TOLERANCE, f"{name} multicolor cut exceeds its branding inlay")
        valid_single(multicolor, f"{name} multicolor body")
    deep_augmented = cad.build_deep_tub(params)
    shallow_augmented = cad.build_shallow_lid(params)
    body_frame = cq.Compound.makeCompound([cast(cq.Shape, source_box.source_top().val()), cast(cq.Shape, source_box.source_bottom().val())]).BoundingBox()
    close(body_frame.xmax - body_frame.xmin, 255.6, "combined_body_width")
    close(body_frame.ymax - body_frame.ymin, 148.4, "combined_body_depth")
    close(body_frame.zmax - body_frame.zmin, 64.85, "combined_body_height")
    print(f"halves=upstream_valid clip_positions=() body_frame=255.6x148.4x64.85 deep_pristine_y=-4.0..148.4 shallow_pristine_y=-4.0..148.4 additions_cut=NONE branding_inlay=BOUNDED")
    return deep, shallow


def upstream_outer_envelope(params: dict) -> cq.Workplane:
    measures = parametric_box_measures(params)
    box = cq.Workplane(origin=(0, 0, 0)).box(measures.width, measures.depth, measures.height).edges("|Z").chamfer(measures.chamfer_z)
    if measures.chamfer_xy > 0:
        box = box.edges("#Z").chamfer(measures.chamfer_xy)
    outer_half = box.faces(">Z").workplane(offset=-(measures.top_height - measures.clearance)).split(keepTop=True)
    outer_half = outer_half.translate((0.0, -measures.depth / 2.0 - measures.thickness, 0.0))
    return normalize_source_half(outer_half, params)


def validate_insertion(params: dict, deep: cq.Workplane, shallow: cq.Workplane) -> None:
    dims = dimensions(params)
    insertion_spec = params["insertion"]
    descent = insertion_spec["descent_sample_z"]
    close(descent[0], insertion_spec["entry_base_z"], "descent_entry_base_z")
    close(descent[-1], insertion_spec["seated_base_z"], "descent_seated_base_z")
    require(all(first - second <= 1.0 + TOLERANCE for first, second in zip(descent, descent[1:])), "insertion descent samples exceed 1 mm spacing")
    require(all(first > second for first, second in zip(descent, descent[1:])), "insertion descent samples are not descending")
    closed_collisions = []
    for name, check_fit in (("device", False), ("check_fit", True)):
        seated = build_device_insertion(params, check_fit)
        valid_single(seated, f"{name} seated insertion")
        seated_box = shape_box(seated)
        close(seated_box.zmin, insertion_spec["seated_base_z"], f"{name}_seated_base_z")
        close(seated_box.zmax, insertion_spec["seated_base_z"] + dims.device_height, f"{name}_seated_top_z")
        expected_width = dims.check_fit_width if check_fit else dims.device_width
        expected_depth = dims.check_fit_depth if check_fit else dims.device_depth
        bbox = seated_box
        close(bbox.xmax - bbox.xmin, expected_width, f"{name}_width")
        close(bbox.ymax - bbox.ymin, expected_depth, f"{name}_depth")
        closed_collisions.append(intersection_volume(seated, shallow))
        require(closed_collisions[-1] <= VOLUME_TOLERANCE, f"{name} seated insertion collides with the closed shallow lid")
        for base_z in descent:
            candidate = build_device_insertion(params, check_fit, base_z)
            require(intersection_volume(candidate, deep) <= VOLUME_TOLERANCE, f"{name} intersects deep tub at base Z{base_z}")
    seated_top = insertion_spec["seated_base_z"] + dims.device_height
    gross_clearance = insertion_spec["entry_base_z"] - seated_top
    require(gross_clearance > 0.0, "seated insertion reaches the entry seam")
    print(f"insertion=device248x140R8 check_fit249x141R8 seated_base_z={insertion_spec['seated_base_z']:.2f} descent_samples={len(descent)} deep_collision=NONE closed_lid_collision_volumes={closed_collisions} gross_clearance_to_seam={gross_clearance:.2f}mm")


def filament_pin(params: dict, center: float) -> cq.Workplane:
    pin = params["hinge_pins"]
    return cq.Workplane("YZ").circle(pin["nominal_diameter"] / 2.0).extrude(pin["piece_length"]).translate((center - pin["piece_length"] / 2.0, *hinge_axis(params)))


def hinge_knuckle_centers(variant: str) -> tuple[float, ...]:
    if variant == "deep":
        return (-6.0, 6.0)
    if variant == "shallow":
        return (-12.0, 0.0, 12.0)
    raise ValueError(f"unknown hinge variant {variant}")


def hinge_section_probe(params: dict, center: float, variant: str, local_x: float) -> cq.Workplane:
    half_depth = 4.0 if variant == "deep" else 5.0
    axis_y, axis_z = hinge_axis(params)
    return prism(center + local_x - 0.025, axis_y - half_depth, axis_z - 8.0, 0.05, 2.0 * half_depth, 16.0)


def hinge_attachment_proxy_probe(params: dict, center: float, variant: str, local_x: float) -> cq.Workplane:
    axis_y, axis_z = hinge_axis(params)
    z_offset = 4.0 if variant == "deep" else 2.06
    return prism(center + local_x - 0.025, axis_y - (4.0 if variant == "deep" else 5.0), axis_z - z_offset, 0.05, 8.0 if variant == "deep" else 10.0, 20.0)


def paddle_envelope(params: dict, center: float, side: str) -> cq.Workplane:
    pin = params["hinge_pins"]
    paddle = pin["paddle_envelope"]
    pin_start = center - pin["piece_length"] / 2.0
    x = pin_start if side == "west" else center + pin["piece_length"] / 2.0 - paddle["axial_length"]
    axis_y, axis_z = hinge_axis(params)
    return prism(x, axis_y - paddle["outward_depth"], axis_z - paddle["vertical_span"] / 2.0, paddle["axial_length"], paddle["outward_depth"] + paddle["inward_allowance"], paddle["vertical_span"])


def validate_hinges(params: dict, deep_half: cq.Workplane, shallow_half: cq.Workplane) -> None:
    validate_hinge_profile_preservation(params, deep_half, shallow_half)
    box = params["parametric_box"]
    hinge = box["hinge"]
    source_hinge = upstream.Hinge(parametric_box_measures(params, ()).hinge)
    bore_radius = hinge["pivot_radius"] + hinge["clearance"] / 2.0
    source_hinge_shape = cast(cq.Shape, source_hinge.pieno.val())
    radii = [edge_radius(edge) for edge in source_hinge_shape.Edges() if edge.geomType() == "CIRCLE"]
    require(any(abs(radius - bore_radius) <= TOLERANCE for radius in radii), "upstream hinge bore radius is not 1.125 mm")
    knuckle_contact = hinge["kunkle_size"] - hinge["clearance"]
    require(knuckle_contact >= 5.0, "upstream knuckle X contact is below 5 mm")
    section_areas = {"deep": [], "shallow": []}
    attachment_proxy_areas = {"deep": [], "shallow": []}
    for center in hinge_centers(params):
        for variant, half in (("deep", deep_half), ("shallow", shallow_half)):
            profile = transformed_hinge_profile(params, center, variant)
            valid_single(profile, f"{variant} hinge profile {center}")
            for local_x in hinge_knuckle_centers(variant):
                probe = hinge_section_probe(params, center, variant, local_x)
                section = half.intersect(probe)
                require(cast(cq.Shape, section.val()).isValid() and len(section.solids().vals()) > 0, f"{variant} hinge section {center + local_x:.3f} must be valid")
                section_area = volume(section) / 0.05
                section_areas[variant].append(section_area)
                require((37.95 <= section_area <= 38.06 if variant == "deep" else 38.31 <= section_area <= 38.42), f"{variant} hinge section area is outside the observed load-path range")
                attachment_proxy = volume(profile.intersect(hinge_attachment_proxy_probe(params, center, variant, local_x))) / 0.05
                attachment_proxy_areas[variant].append(attachment_proxy)
                require(attachment_proxy >= 25.0, f"{variant} hinge section has no usable attachment proxy")
    require(len(section_areas["deep"]) == 4 and len(section_areas["shallow"]) == 6, "unexpected actual upstream knuckle section count")
    pin_spec = params["hinge_pins"]
    require(abs(bore_radius * 2.0 - 2.25) <= TOLERANCE, "actual hinge bore is not 2.25 mm")
    paddle_metrics = []
    for center in hinge_centers(params):
        pin = filament_pin(params, center)
        require(intersection_volume(pin, deep_half) <= VOLUME_TOLERANCE and intersection_volume(pin, shallow_half) <= VOLUME_TOLERANCE, f"filament pin {center} collides with a case half")
        for side in ("west", "east"):
            paddle = paddle_envelope(params, center, side)
            require(intersection_volume(pin, paddle) > 0.0, f"{side} paddle does not overlap filament pin {center}")
            require(intersection_volume(paddle, deep_half) <= VOLUME_TOLERANCE and intersection_volume(paddle, shallow_half) <= VOLUME_TOLERANCE, f"{side} paddle {center} collides with a case half")
            paddle_metrics.append((intersection_volume(pin, paddle), shape_box(paddle).ymin, shape_box(paddle).ymax))
    min_pin_overlap, paddle_y_min, paddle_y_max = min(paddle_metrics)
    print(f"hinges=2 axis=Y{hinge_axis(params)[0]:.2f}/Z{hinge_axis(params)[1]:.2f} bore=2.25 radial_clearance=0.25 leaf_depth=30.5 actual_knuckle_sections=10 knuckle_contact={knuckle_contact:.2f} section_area_deep_range={min(section_areas['deep']):.3f}..{max(section_areas['deep']):.3f} expected=38.005673 section_area_shallow_range={min(section_areas['shallow']):.3f}..{max(section_areas['shallow']):.3f} expected=38.365673 attachment_proxy_deep_min={min(attachment_proxy_areas['deep']):.3f} attachment_proxy_shallow_min={min(attachment_proxy_areas['shallow']):.3f} paddles=PASS")
    print(f"paddle_envelope=Y{paddle_y_min:.2f}..{paddle_y_max:.2f} Z-2.00..2.00 pin_overlap_min={min_pin_overlap:.3f} case_clearance_to_body_y0={-paddle_y_max:.2f} collision=NONE")


def validate_bed_fit(params: dict) -> None:
    dims = dimensions(params)
    models = (
        ("deep_debossed", cad.build_debossed_deep_tub(params)[0], "negative_z"),
        ("deep_multicolor", cad.build_multicolor_deep_tub(params)[0], "negative_z"),
        ("shallow_debossed", cad.build_debossed_shallow_lid(params)[0], "positive_z"),
        ("shallow_multicolor", cad.build_multicolor_shallow_lid(params)[0], "positive_z"),
    )
    extents = []
    support_areas = []
    for name, model, exterior in models:
        valid_single(model, name)
        raw_box = shape_box(model)
        if exterior == "negative_z":
            require(raw_box.zmin >= -TOLERANCE, f"{name} has proud geometry below the deep exterior plane")
        else:
            require(raw_box.zmax <= dims.height + TOLERANCE, f"{name} has proud geometry above the shallow exterior plane")
        oriented = cad.orient_for_print(model, exterior)
        bbox = shape_box(oriented)
        sizes = (bbox.xmax - bbox.xmin, bbox.ymax - bbox.ymin, bbox.zmax - bbox.zmin)
        require(all(size <= dims.bed_width + TOLERANCE for size in sizes), f"{name} does not fit the 260x260 bed")
        support_probe = prism(0.0, 0.0, -TOLERANCE, dims.width, dims.depth, 0.2 + TOLERANCE)
        support_area = volume(oriented.intersect(support_probe)) / 0.2
        require(support_area >= dims.width * dims.depth * 0.25, f"{name} first-layer shell support is too small")
        support_areas.append((name, support_area))
        extents.append((name, sizes))
    print("bed=260x260 fit=PASS print_orientation=exterior_down")
    print("first_layer_shell_support_area=" + ", ".join(f"{name}:{area:.1f}mm2" for name, area in support_areas))
    print("final_extents=" + ", ".join(f"{name}:{sizes[0]:.3f}x{sizes[1]:.3f}x{sizes[2]:.3f}" for name, sizes in extents))


def validate_artifact_paths(params: dict, check_files: bool = False) -> None:
    paths = artifact_paths(params)
    require(len(paths) == 8, f"expected 8 protective-case artifacts, found {len(paths)}")
    require(len({path.name for path in paths}) == 8, "protective-case artifact names are not unique")
    repo_root = Path(__file__).resolve().parents[2]
    three_mf_paths = [path for path in paths if path.suffix == ".3mf"]
    debossed_3mf_paths = [path for path in three_mf_paths if "debossed" in path.name]
    multicolor_3mf_paths = [path for path in three_mf_paths if "multicolor" in path.name]
    require(len(three_mf_paths) == 4 and len(debossed_3mf_paths) == 2 and len(multicolor_3mf_paths) == 2, "expected two debossed and two multicolor 3MF artifacts")
    require(all(path.relative_to(repo_root).parent.as_posix() == "release-artifacts/enclosure/3mf-single-material" for path in debossed_3mf_paths), "debossed 3MF artifacts must use the single-material directory")
    require(all(path.relative_to(repo_root).parent.as_posix() == "release-artifacts/enclosure/3mf-multicolor" for path in multicolor_3mf_paths), "multicolor 3MF artifacts must use the multicolor directory")
    if not check_files:
        print("artifact_contract=8 exact_paths=SOURCE_ONLY 3mf=debossed2/multicolor2")
        return
    for path in paths:
        require(path.is_file() and path.stat().st_size > 0, f"missing or empty exported artifact {path}")
    def bbox_tuple(model: cq.Workplane) -> tuple[float, float, float, float, float, float]:
        box = shape_box(model)
        return box.xmin, box.ymin, box.zmin, box.xmax, box.ymax, box.zmax

    def printed_parts(body: cq.Workplane, parts: list[tuple[str, cq.Workplane]], exterior: str) -> tuple[tuple[float, float, float, float, float, float], dict[int, tuple[float, float, float, float, float, float]]]:
        transform = cad.print_transform(body, exterior)
        printed_body = cad.apply_print_transform(body, transform)
        printed_marks = [cad.apply_print_transform(part, transform) for _, part in parts]
        return bbox_tuple(printed_body), {1: bbox_tuple(printed_body), **{index + 2: bbox_tuple(mark) for index, mark in enumerate(printed_marks)}}

    deep_debossed_body, deep_debossed_parts = cad.build_debossed_deep_tub(params)
    deep_multicolor_body, deep_multicolor_parts = cad.build_multicolor_deep_tub(params)
    shallow_debossed_body, shallow_debossed_parts = cad.build_debossed_shallow_lid(params)
    shallow_multicolor_body, shallow_multicolor_parts = cad.build_multicolor_shallow_lid(params)
    expected_packages = {
        "deep_tub_debossed": printed_parts(deep_debossed_body, [], "negative_z"),
        "deep_tub_multicolor": printed_parts(deep_multicolor_body, deep_multicolor_parts, "negative_z"),
        "shallow_lid_debossed": printed_parts(shallow_debossed_body, [], "positive_z"),
        "shallow_lid_multicolor": printed_parts(shallow_multicolor_body, shallow_multicolor_parts, "positive_z"),
    }
    for path in (path for path in paths if path.suffix == ".3mf"):
        if "deep_tub_debossed" in path.name:
            expected_bbox, expected_parts = expected_packages["deep_tub_debossed"]
            validate_deep_tub_debossed_package(path, expected_bbox, expected_parts)
        elif "deep_tub_multicolor" in path.name:
            expected_bbox, expected_parts = expected_packages["deep_tub_multicolor"]
            validate_deep_tub_multicolor_package(path, expected_bbox, expected_parts)
        elif "shallow_lid_debossed" in path.name:
            expected_bbox, expected_parts = expected_packages["shallow_lid_debossed"]
            validate_shallow_lid_debossed_package(path, expected_bbox, expected_parts)
        elif "shallow_lid_multicolor" in path.name:
            expected_bbox, expected_parts = expected_packages["shallow_lid_multicolor"]
            validate_shallow_lid_multicolor_package(path, expected_bbox, expected_parts)
        else:
            raise ValueError(f"unexpected protective-case 3MF path {path}")
    print("artifacts=8 exact_paths=PASS step_stl=present_nonempty 3mf=validated geometry=not_inspected_for_step_stl")
    print("artifact_contract=8 exact_paths=SOURCE_ONLY")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-artifacts", action="store_true", help="verify exported artifact presence and 3MF packages")
    args = parser.parse_args()
    params = load_parameters()
    validate_parameter_relationships(params)
    validate_upstream_default()
    deep, shallow = validate_source_halves(params)
    validate_insertion(params, deep, shallow)
    corrected_deep = apply_deep_mating_interface(params, deep)
    corrected_shallow = apply_shallow_mating_interface(params, shallow)
    validate_bottom_latches(params, corrected_deep, corrected_shallow)
    validate_hinges(params, corrected_deep, corrected_shallow)
    outer_envelope = upstream_outer_envelope(params)
    final_tub = cad.build_deep_tub(params)
    validate_corner_restraints(params, deep, outer_envelope, final_tub)
    validate_foam_landing_pads(params, deep, final_tub, outer_envelope)
    validate_mating_interface(params, deep, shallow, corrected_deep, corrected_shallow)
    validate_branding(params)
    validate_bed_fit(params)
    validate_reference_top_fit(params)
    validate_artifact_paths(params, check_files=args.check_artifacts)
    print("PASS")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        raise
