from __future__ import annotations

from dataclasses import dataclass
from math import cos, radians, sin
from typing import cast

import cadquery as cq

try:
    from .protective_case_bottom_latches import bottom_latch_components
    from .protective_case_corner_restraints import corner_restraint_components
    from .protective_case_foam_landing_pads import _bbox_distance, _source_hardware_components, foam_landing_pad_components
    from .protective_case_geometry import (
        FACE_DOWN_TRANSPORT_TRANSLATION,
        build_device_insertion,
        corner_restraints,
        dimensions,
        face_down_transport_xy,
        face_down_transport_feature_xy,
        load_source_parameters,
        wall_planes,
    )
    from .protective_case_mating_interface import deep_receiver_components, main_cavity_prism
except ImportError:
    from protective_case_bottom_latches import bottom_latch_components
    from protective_case_corner_restraints import corner_restraint_components
    from protective_case_foam_landing_pads import _bbox_distance, _source_hardware_components, foam_landing_pad_components
    from protective_case_geometry import FACE_DOWN_TRANSPORT_TRANSLATION, build_device_insertion, corner_restraints, dimensions, face_down_transport_feature_xy, face_down_transport_xy, load_source_parameters, wall_planes
    from protective_case_mating_interface import deep_receiver_components, main_cavity_prism


TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7
GUIDE_CENTER = (127.8, 74.2)
GUIDE_SCALE = 0.75
GUIDE_STROKE = 0.8
GUIDE_Z_MIN = 2.18
GUIDE_Z_MAX = 2.60
INLAY_Z_MIN = 1.80
INLAY_Z_MAX = 2.20
INLAY_CUT_Z_MAX = 2.22
TRELLIS_CASE_ORIGIN = (124.75, 17.5)
TURNAROUND_SOURCE_CENTER = (232.0, 74.2)
TURNAROUND_CENTER = TURNAROUND_SOURCE_CENTER
TURNAROUND_CENTERLINE_RADIUS = 7.0
TURNAROUND_ARC_INNER_RADIUS = 6.5
TURNAROUND_ARC_OUTER_RADIUS = 7.5
TURNAROUND_ARC_STROKE = 1.0
TURNAROUND_ARCS = ((20.0, 155.0), (200.0, 335.0))
TURNAROUND_ARROW_LENGTH = 3.2
TURNAROUND_ARROW_WIDTH = 3.2
TURNAROUND_ARROW_FLAT_TIP = 0.8
TURNAROUND_ARROW_OVERLAP = 0.4
TURNAROUND_EXPECTED_BBOX = (224.109991243, 239.890008757, 66.7, 81.7)
GUIDE_EXPECTED_BBOX = (34.8, 239.890008757, 21.7, 126.7)
CONTROL_SIDE_GUIDE_GAP = 3.309991243
CONTROL_SIDE_WALL_MARGIN = 13.509991243
CONTROL_SIDE_SUPPORT_CLEARANCE = 52.590086506
CONTROL_SIDE_FOAM_CLEARANCE = 27.222194187
CONTROL_SIDE_DEEP_CATCH_BRIDGE_CLEARANCE = 87.854942406
CONTROL_SIDE_HINGE_CLEARANCE = 83.001945739
CONTROL_SIDE_RECEIVER_CLEARANCE = 51.529388491
CONTROL_SIDE_CANONICAL_CLEARANCE = 27.4
CONTROL_SIDE_NOMINAL_CLEARANCE = 4.9
TURNAROUND_FLOOR_ATTACHMENT_VOLUME = 0.449862846


@dataclass(frozen=True)
class RoundedOutline:
    center: tuple[float, float]
    width: float
    height: float
    radius: float


@dataclass(frozen=True)
class OrientationGuideLayout:
    center: tuple[float, float]
    scale: float
    outer: RoundedOutline
    screen: RoundedOutline
    encoders: tuple[tuple[tuple[float, float], float], ...]
    neokeys: tuple[RoundedOutline, ...]
    trellis: tuple[tuple[int, int, tuple[float, float]], ...]


def _scale_point(point: tuple[float, float], center: tuple[float, float]) -> tuple[float, float]:
    return (
        center[0] + (point[0] - center[0]) * GUIDE_SCALE,
        center[1] + (point[1] - center[1]) * GUIDE_SCALE,
    )


def _trellis_case_center(source: dict, row: int, column: int) -> tuple[float, float]:
    return face_down_transport_xy(
        (
            FACE_DOWN_TRANSPORT_TRANSLATION[0] + TRELLIS_CASE_ORIGIN[0] + column * source["neotrellis_pitch"],
            FACE_DOWN_TRANSPORT_TRANSLATION[1] - (TRELLIS_CASE_ORIGIN[1] + row * source["neotrellis_pitch"]),
        )
    )


def orientation_guide_layout(params: dict) -> OrientationGuideLayout:
    source = load_source_parameters()
    center = tuple(params["device"]["center"])
    envelope = params["device"]["envelope"]
    outer = RoundedOutline(center, envelope["width"] * GUIDE_SCALE, envelope["depth"] * GUIDE_SCALE, envelope["radius"] * GUIDE_SCALE)
    screen_center = _scale_point(
        face_down_transport_feature_xy(
            source,
            source["features_local"]["oled_screen_center"],
            -0.5,
            -0.3,
        ),
        center,
    )
    screen_width, screen_height = source["screen_cutout"]
    screen = RoundedOutline(screen_center, screen_width * GUIDE_SCALE, screen_height * GUIDE_SCALE, source["screen_cutout_r"] * GUIDE_SCALE)
    encoders = tuple(
        (
            _scale_point(face_down_transport_feature_xy(source, point), center),
            (source["encoder_crater_flat_d"][name] / 2.0 + source["encoder_crater_slope_w"]) * GUIDE_SCALE,
        )
        for name, point in source["features_local"]["encoders"].items()
    )
    key_width, key_height = source["key_cutout"]
    neokey_size = (key_width * GUIDE_SCALE, key_height * GUIDE_SCALE)
    neokeys = tuple(
        RoundedOutline(
            _scale_point(
                face_down_transport_feature_xy(source, point, -0.25, -1.0),
                center,
            ),
            neokey_size[0],
            neokey_size[1],
            source["key_cutout_r"] * GUIDE_SCALE,
        )
        for point in source["features_local"]["neokey_key_centers"]
    )
    trellis = tuple(
        (
            row,
            column,
            _scale_point(_trellis_case_center(source, row, column), center),
        )
        for row in range(8)
        for column in range(8)
    )
    return OrientationGuideLayout(center, GUIDE_SCALE, outer, screen, encoders, neokeys, trellis)


def _rounded_prism(outline: RoundedOutline, z_min: float = GUIDE_Z_MIN, z_max: float = GUIDE_Z_MAX) -> cq.Workplane:
    x0 = outline.center[0] - outline.width / 2.0
    y0 = outline.center[1] - outline.height / 2.0
    return (
        cq.Workplane("XY")
        .box(outline.width, outline.height, z_max - z_min, centered=(False, False, False))
        .translate((x0, y0, z_min))
        .edges("|Z")
        .fillet(outline.radius)
    )


def _outline(outline: RoundedOutline, z_min: float = GUIDE_Z_MIN, z_max: float = GUIDE_Z_MAX) -> cq.Workplane:
    inner = RoundedOutline(
        outline.center,
        outline.width - 2.0 * GUIDE_STROKE,
        outline.height - 2.0 * GUIDE_STROKE,
        outline.radius - GUIDE_STROKE,
    )
    return _rounded_prism(outline, z_min, z_max).cut(_rounded_prism(inner, z_min, z_max)).clean()


def _ring(center: tuple[float, float], radius: float, z_min: float = GUIDE_Z_MIN, z_max: float = GUIDE_Z_MAX) -> cq.Workplane:
    return (
        cq.Workplane("XY")
        .circle(radius)
        .extrude(z_max - z_min)
        .translate((center[0], center[1], z_min))
        .cut(
            cq.Workplane("XY")
            .circle(radius - GUIDE_STROKE)
            .extrude(z_max - z_min)
            .translate((center[0], center[1], z_min))
        )
        .clean()
    )


def _polar_point(center: tuple[float, float], radius: float, degrees: float) -> tuple[float, float]:
    angle = radians(degrees)
    return center[0] + radius * cos(angle), center[1] + radius * sin(angle)


def _annular_arc(
    center: tuple[float, float],
    start_degrees: float,
    end_degrees: float,
    z_min: float,
    z_max: float,
) -> cq.Workplane:
    midpoint = (start_degrees + end_degrees) / 2.0
    outer_start = _polar_point(center, TURNAROUND_ARC_OUTER_RADIUS, start_degrees)
    outer_mid = _polar_point(center, TURNAROUND_ARC_OUTER_RADIUS, midpoint)
    outer_end = _polar_point(center, TURNAROUND_ARC_OUTER_RADIUS, end_degrees)
    inner_end = _polar_point(center, TURNAROUND_ARC_INNER_RADIUS, end_degrees)
    inner_mid = _polar_point(center, TURNAROUND_ARC_INNER_RADIUS, midpoint)
    inner_start = _polar_point(center, TURNAROUND_ARC_INNER_RADIUS, start_degrees)
    return (
        cq.Workplane("XY")
        .moveTo(*outer_start)
        .threePointArc(outer_mid, outer_end)
        .lineTo(*inner_end)
        .threePointArc(inner_mid, inner_start)
        .close()
        .extrude(z_max - z_min)
        .translate((0.0, 0.0, z_min))
        .clean()
    )


def _arrowhead(
    center: tuple[float, float],
    end_degrees: float,
    z_min: float,
    z_max: float,
) -> cq.Workplane:
    angle = radians(end_degrees)
    endpoint = _polar_point(center, TURNAROUND_CENTERLINE_RADIUS, end_degrees)
    direction = (-sin(angle), cos(angle))
    normal = (cos(angle), sin(angle))
    base_center = (
        endpoint[0] - TURNAROUND_ARROW_OVERLAP * direction[0],
        endpoint[1] - TURNAROUND_ARROW_OVERLAP * direction[1],
    )
    tip_center = (
        base_center[0] + TURNAROUND_ARROW_LENGTH * direction[0],
        base_center[1] + TURNAROUND_ARROW_LENGTH * direction[1],
    )
    points = (
        (base_center[0] - normal[0] * TURNAROUND_ARROW_WIDTH / 2.0, base_center[1] - normal[1] * TURNAROUND_ARROW_WIDTH / 2.0),
        (base_center[0] + normal[0] * TURNAROUND_ARROW_WIDTH / 2.0, base_center[1] + normal[1] * TURNAROUND_ARROW_WIDTH / 2.0),
        (tip_center[0] + normal[0] * TURNAROUND_ARROW_FLAT_TIP / 2.0, tip_center[1] + normal[1] * TURNAROUND_ARROW_FLAT_TIP / 2.0),
        (tip_center[0] - normal[0] * TURNAROUND_ARROW_FLAT_TIP / 2.0, tip_center[1] - normal[1] * TURNAROUND_ARROW_FLAT_TIP / 2.0),
    )
    return cq.Workplane("XY").polyline(points).close().extrude(z_max - z_min).translate((0.0, 0.0, z_min)).clean()


def _turnaround_arrow_components(z_min: float, z_max: float) -> tuple[tuple[str, cq.Workplane], ...]:
    return tuple(
        (
            f"device_orientation_turnaround_arrow_{index + 1}",
            _annular_arc(TURNAROUND_CENTER, start, end, z_min, z_max).union(_arrowhead(TURNAROUND_CENTER, end, z_min, z_max)).clean(),
        )
        for index, (start, end) in enumerate(TURNAROUND_ARCS)
    )


def device_orientation_turnaround_arrow_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    return _turnaround_arrow_components(GUIDE_Z_MIN, GUIDE_Z_MAX)


def _guide_components(params: dict, z_min: float, z_max: float) -> tuple[tuple[str, cq.Workplane], ...]:
    layout = orientation_guide_layout(params)
    components = [
        ("device_orientation_outer_outline", _outline(layout.outer, z_min, z_max)),
        ("device_orientation_screen_outline", _outline(layout.screen, z_min, z_max)),
    ]
    components.extend(
        (f"device_orientation_encoder_ring_{index + 1}", _ring(center, radius, z_min, z_max))
        for index, (center, radius) in enumerate(layout.encoders)
    )
    components.extend(
        (f"device_orientation_neokey_outline_{index + 1}", _outline(outline, z_min, z_max))
        for index, outline in enumerate(layout.neokeys)
    )
    components.extend(
        (
            f"device_orientation_trellis_cell_r{row}_c{column}",
            _outline(RoundedOutline(center, trellis_cell_size(), trellis_cell_size(), trellis_corner_radius()), z_min, z_max),
        )
        for row, column, center in layout.trellis
    )
    return tuple(components)


def device_orientation_guide_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    return _guide_components(params, GUIDE_Z_MIN, GUIDE_Z_MAX) + device_orientation_turnaround_arrow_components(params)


def device_orientation_guide_inlay_components(params: dict, cutter: bool = False) -> tuple[tuple[str, cq.Workplane], ...]:
    z_max = INLAY_CUT_Z_MAX if cutter else INLAY_Z_MAX
    return _guide_components(params, INLAY_Z_MIN, z_max) + _turnaround_arrow_components(INLAY_Z_MIN, z_max)


def fused_device_orientation_guide_inlay(params: dict, cutter: bool = False) -> cq.Workplane:
    components = device_orientation_guide_inlay_components(params, cutter)
    result = cq.Workplane("XY").add(cast(cq.Shape, components[0][1].val()))
    for _, component in components[1:]:
        result = result.union(component).clean()
    return result


def trellis_cell_size() -> float:
    return load_source_parameters()["neotrellis_button_cutout"] * GUIDE_SCALE


def trellis_corner_radius() -> float:
    return load_source_parameters()["neotrellis_button_r"] * GUIDE_SCALE


def add_device_orientation_guide(params: dict, model: cq.Workplane) -> cq.Workplane:
    result = model
    for _, component in device_orientation_guide_components(params):
        result = result.union(component).clean()
    return result.clean()


def _volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def _intersection_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return _volume(first.intersect(second))


def _minimum_component_distance(first: tuple[cq.Workplane, ...], second: tuple[cq.Workplane, ...]) -> float:
    return min(cast(cq.Shape, left.val()).distance(cast(cq.Shape, right.val())) for left in first for right in second)


def _probe(x_min: float, x_max: float, y_min: float, y_max: float, z_min: float, z_max: float) -> cq.Workplane:
    return cq.Workplane("XY").box(x_max - x_min, y_max - y_min, z_max - z_min, centered=(False, False, False)).translate((x_min, y_min, z_min))


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def _bbox_tuple(model: cq.Workplane) -> tuple[float, float, float, float, float, float]:
    box = _shape_box(model)
    return box.xmin, box.xmax, box.ymin, box.ymax, box.zmin, box.zmax


def _check_outline(name: str, component: cq.Workplane, outline: RoundedOutline) -> None:
    shape = cast(cq.Shape, component.val())
    _require(shape.isValid() and len(component.solids().vals()) == 1, f"{name} must be one valid solid")
    box = _shape_box(component)
    for actual, expected, axis in (
        (box.xmin, outline.center[0] - outline.width / 2.0, "xmin"),
        (box.xmax, outline.center[0] + outline.width / 2.0, "xmax"),
        (box.ymin, outline.center[1] - outline.height / 2.0, "ymin"),
        (box.ymax, outline.center[1] + outline.height / 2.0, "ymax"),
        (box.zmin, GUIDE_Z_MIN, "zmin"),
        (box.zmax, GUIDE_Z_MAX, "zmax"),
    ):
        _require(abs(actual - expected) <= TOLERANCE, f"{name}_{axis}={actual:.6f}, expected={expected:.6f}")


def validate_device_orientation(
    params: dict,
    final_tub: cq.Workplane,
    canonical_device: cq.Workplane,
    base_tub: cq.Workplane | None = None,
) -> None:
    layout = orientation_guide_layout(params)
    _require(layout.center == GUIDE_CENTER and layout.scale == GUIDE_SCALE, "orientation guide center or scale changed")
    control_centers = (layout.screen.center,) + tuple(center for center, _ in layout.encoders)
    _require(all(center[0] > layout.center[0] for center in control_centers), "screen and encoders must remain on the control side")
    final_shape = cast(cq.Shape, final_tub.val())
    _require(final_shape.isValid() and len(final_tub.solids().vals()) == 1, "oriented final deep tub must be one valid solid")
    if base_tub is not None:
        for actual, expected in zip(_bbox_tuple(final_tub), _bbox_tuple(base_tub)):
            _require(abs(actual - expected) <= TOLERANCE, "orientation guide changed final tub bbox")
        first_layer = _probe(0.0, dimensions(params).width, 0.0, dimensions(params).depth, -0.01, 0.20)
        _require(abs(_volume(final_tub.intersect(first_layer)) - _volume(base_tub.intersect(first_layer))) <= VOLUME_TOLERANCE, "orientation guide changed the first layer")
    components = device_orientation_guide_components(params)
    names = [name for name, _ in components]
    expected_names = ["device_orientation_outer_outline", "device_orientation_screen_outline"]
    expected_names += [f"device_orientation_encoder_ring_{index}" for index in range(1, 5)]
    expected_names += [f"device_orientation_neokey_outline_{index}" for index in range(1, 5)]
    expected_names += [f"device_orientation_trellis_cell_r{row}_c{column}" for row in range(8) for column in range(8)]
    expected_names += [f"device_orientation_turnaround_arrow_{index}" for index in range(1, 3)]
    _require(names == expected_names, "orientation guide component names or order changed")
    _require(len(components) == 76, f"orientation guide component count changed: {len(components)}")
    _check_outline(names[0], components[0][1], layout.outer)
    _check_outline(names[1], components[1][1], layout.screen)
    for name, (center, radius), (_, component) in zip(names[2:6], layout.encoders, components[2:6]):
        box = _shape_box(component)
        _require(cast(cq.Shape, component.val()).isValid() and len(component.solids().vals()) == 1, f"{name} must be one valid solid")
        _require(abs(box.xmin - (center[0] - radius)) <= TOLERANCE and abs(box.xmax - (center[0] + radius)) <= TOLERANCE, f"{name} layout changed")
        _require(abs(box.ymin - (center[1] - radius)) <= TOLERANCE and abs(box.ymax - (center[1] + radius)) <= TOLERANCE, f"{name} layout changed")
    for name, outline, (_, component) in zip(names[6:10], layout.neokeys, components[6:10]):
        _check_outline(name, component, outline)
    for name, (_, _, center), (_, component) in zip(names[10:74], layout.trellis, components[10:74]):
        _check_outline(name, component, RoundedOutline(center, trellis_cell_size(), trellis_cell_size(), trellis_corner_radius()))

    arrows = components[74:]
    _require(TURNAROUND_ARC_STROKE == 1.0 and TURNAROUND_ARC_INNER_RADIUS == 6.5 and TURNAROUND_ARC_OUTER_RADIUS == 7.5, "turnaround arc dimensions changed")
    _require(TURNAROUND_ARC_OUTER_RADIUS - TURNAROUND_ARC_INNER_RADIUS == TURNAROUND_ARC_STROKE, "turnaround arc stroke changed")
    _require(TURNAROUND_ARCS == ((20.0, 155.0), (200.0, 335.0)), "turnaround arc angles changed")
    _require(len(arrows) == 2 and [name for name, _ in arrows] == [f"device_orientation_turnaround_arrow_{index}" for index in range(1, 3)], "turnaround arrow components changed")
    for _, arrow in arrows:
        arrow_shape = cast(cq.Shape, arrow.val())
        _require(arrow_shape.isValid() and len(arrow.solids().vals()) == 1, "turnaround arrow must be one valid solid")
    arrow_boxes = [_shape_box(arrow) for _, arrow in arrows]
    outer_box = _shape_box(components[0][1])
    control_side_gap = min(box.xmin for box in arrow_boxes) - outer_box.xmax
    _require(abs(control_side_gap - CONTROL_SIDE_GUIDE_GAP) <= 0.01, f"turnaround arrows must remain on the screen/encoder control side: gap={control_side_gap:.9f}")
    for actual, expected in (
        (min(box.xmin for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[0]),
        (max(box.xmax for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[1]),
        (min(box.ymin for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[2]),
        (max(box.ymax for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[3]),
    ):
        _require(abs(actual - expected) <= 0.01, f"turnaround arrow bbox changed: {actual:.6f}, expected {expected:.6f}")
    _require(_intersection_volume(arrows[0][1], arrows[1][1]) <= VOLUME_TOLERANCE, "turnaround arrows overlap")
    combined_bbox = (
        min(box.xmin for box in arrow_boxes),
        max(box.xmax for box in arrow_boxes),
        min(box.ymin for box in arrow_boxes),
        max(box.ymax for box in arrow_boxes),
    )
    _require(all(abs(actual - expected) <= 0.01 for actual, expected in zip(combined_bbox, TURNAROUND_EXPECTED_BBOX)), "turnaround arrow group bbox changed")
    floor = _probe(0.0, dimensions(params).width, 0.0, dimensions(params).depth, GUIDE_Z_MIN, 2.2)
    for name, arrow in arrows:
        arrow_box = _shape_box(arrow)
        _require(abs(arrow_box.zmin - GUIDE_Z_MIN) <= TOLERANCE and abs(arrow_box.zmax - GUIDE_Z_MAX) <= TOLERANCE, f"{name} Z changed")
        _require(abs(_intersection_volume(arrow, floor) - TURNAROUND_FLOOR_ATTACHMENT_VOLUME) <= 1.0e-6, f"{name} floor attachment changed")

    guide = cq.Workplane("XY").newObject([cast(cq.Shape, component.val()) for _, component in components])
    guide_boxes = [_shape_box(component) for _, component in components]
    guide_bbox = (
        min(box.xmin for box in guide_boxes),
        max(box.xmax for box in guide_boxes),
        min(box.ymin for box in guide_boxes),
        max(box.ymax for box in guide_boxes),
    )
    _require(all(abs(actual - expected) <= 0.01 for actual, expected in zip(guide_bbox, GUIDE_EXPECTED_BBOX)), "orientation guide group bbox changed")
    floor = _probe(0.0, dimensions(params).width, 0.0, dimensions(params).depth, GUIDE_Z_MIN, 2.2)
    _require(all(_intersection_volume(component, floor) > 0.0 for _, component in components), "orientation guide does not overlap the floor")
    upper_guide = guide.intersect(_probe(0.0, dimensions(params).width, 0.0, dimensions(params).depth, 2.2, GUIDE_Z_MAX))
    _require(_intersection_volume(final_tub, upper_guide) > 0.0, "final deep tub is missing the orientation guide")
    _require(_volume(upper_guide.cut(final_tub)) <= VOLUME_TOLERANCE, "final deep tub orientation guide is incomplete")
    cavity = main_cavity_prism(params, "deep")
    _require(_volume(upper_guide.cut(cavity)) <= VOLUME_TOLERANCE, "orientation guide leaves the deep cavity")

    pads = foam_landing_pad_components(params)
    restraints = tuple(component for _, component in corner_restraint_components(params))
    source_hardware = _source_hardware_components(params)
    latch_hardware = tuple(component for _, component in bottom_latch_components(params, "deep"))
    receivers = tuple(receiver for _, receiver in deep_receiver_components(params))
    collisions = restraints + tuple(pad for _, pad in pads) + source_hardware + latch_hardware + receivers + (canonical_device,)
    _require(all(_intersection_volume(guide, collision) <= VOLUME_TOLERANCE for collision in collisions), "orientation guide has a prohibited collision")
    grid = tuple(component for _, component in components[:74])
    arrow_shapes = tuple(component for _, component in arrows)
    _require(all(_intersection_volume(arrow, grid_component) <= VOLUME_TOLERANCE for arrow in arrow_shapes for grid_component in grid), "turnaround arrow intersects the device orientation guide")
    _require(all(_intersection_volume(arrow, collision) <= VOLUME_TOLERANCE for arrow in arrow_shapes for collision in collisions), "turnaround arrow has a prohibited collision")
    nominal = build_device_insertion(params)
    nominal_clearance = _minimum_component_distance(arrow_shapes, (nominal,))
    canonical_clearance = _minimum_component_distance(arrow_shapes, (canonical_device,))
    control_side_support_clearance = _minimum_component_distance(arrow_shapes, restraints)
    control_side_foam_clearance = _minimum_component_distance(arrow_shapes, tuple(pad for _, pad in pads))
    hinge_clearance = _minimum_component_distance(arrow_shapes, source_hardware)
    deep_catch_bridge_clearance = _minimum_component_distance(arrow_shapes, latch_hardware)
    receiver_clearance = _minimum_component_distance(arrow_shapes, receivers)
    guide_device_clearance = _minimum_component_distance(grid, (canonical_device,))
    control_side_wall_margin = wall_planes(params)["east_x"] - max(_shape_box(arrow).xmax for arrow in arrow_shapes)
    _require(nominal_clearance >= CONTROL_SIDE_NOMINAL_CLEARANCE - TOLERANCE, f"orientation guide nominal clearance is too small: {nominal_clearance:.6f}")
    _require(guide_device_clearance >= 19.4 - TOLERANCE, f"orientation guide canonical clearance is too small: {guide_device_clearance:.6f}")
    _require(canonical_clearance >= CONTROL_SIDE_CANONICAL_CLEARANCE - TOLERANCE, f"turnaround arrow canonical clearance is too small: {canonical_clearance:.6f}")
    _require(control_side_support_clearance >= CONTROL_SIDE_SUPPORT_CLEARANCE - TOLERANCE, f"turnaround arrow control-side support clearance is too small: {control_side_support_clearance:.6f}")
    _require(control_side_foam_clearance >= CONTROL_SIDE_FOAM_CLEARANCE - TOLERANCE, f"turnaround arrow control-side foam clearance is too small: {control_side_foam_clearance:.6f}")
    _require(hinge_clearance >= CONTROL_SIDE_HINGE_CLEARANCE - TOLERANCE, f"turnaround arrow hinge clearance is too small: {hinge_clearance:.6f}")
    _require(abs(deep_catch_bridge_clearance - CONTROL_SIDE_DEEP_CATCH_BRIDGE_CLEARANCE) <= TOLERANCE, f"deep catch and bridge clearance changed: {deep_catch_bridge_clearance:.6f}")
    _require(receiver_clearance >= CONTROL_SIDE_RECEIVER_CLEARANCE - TOLERANCE, f"turnaround arrow receiver clearance is too small: {receiver_clearance:.6f}")
    _require(abs(control_side_wall_margin - CONTROL_SIDE_WALL_MARGIN) <= 0.01, f"turnaround arrow control-side wall margin changed: {control_side_wall_margin:.9f}")
    print(f"device_orientation_guide=components=76 grid_cells=64 turnaround_arrows=2 stroke={GUIDE_STROKE:.1f}mm z={GUIDE_Z_MIN:.2f}..{GUIDE_Z_MAX:.2f} turnaround_bbox=X{TURNAROUND_EXPECTED_BBOX[0]:.9f}..{TURNAROUND_EXPECTED_BBOX[1]:.9f}/Y{TURNAROUND_EXPECTED_BBOX[2]:.1f}..{TURNAROUND_EXPECTED_BBOX[3]:.1f} guide_gap={control_side_gap:.9f}mm nominal_clearance={nominal_clearance:.9f}mm guide_clearance={guide_device_clearance:.3f}mm arrow_clearance={canonical_clearance:.9f}mm support_clearance={control_side_support_clearance:.9f}mm foam_clearance={control_side_foam_clearance:.9f}mm hinge_clearance={hinge_clearance:.9f}mm deep_catch_bridge_clearance={deep_catch_bridge_clearance:.9f}mm receiver_clearance={receiver_clearance:.9f}mm control_side_wall_margin={control_side_wall_margin:.9f}mm floor_attachment={TURNAROUND_FLOOR_ATTACHMENT_VOLUME:.6f}mm3 collision=NONE")
