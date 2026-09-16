from __future__ import annotations

from dataclasses import dataclass
from typing import cast

import cadquery as cq

try:
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_geometry import dimensions, hinge_axis, hinge_centers, normalize_source_half, parametric_box_measures
except ImportError:
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_geometry import dimensions, hinge_axis, hinge_centers, normalize_source_half, parametric_box_measures


TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


@dataclass(frozen=True)
class MatingRail:
    name: str
    bounds: tuple[float, float, float, float]


def _prism(bounds: tuple[float, float, float, float], z_min: float, z_max: float) -> cq.Workplane:
    x_min, x_max, y_min, y_max = bounds
    return cq.Workplane("XY").box(x_max - x_min, y_max - y_min, z_max - z_min, centered=(False, False, False)).translate((x_min, y_min, z_min))


def rounded_prism(bounds: tuple[float, float, float, float], radius: float, z_min: float, z_max: float) -> cq.Workplane:
    return _prism(bounds, z_min, z_max).edges("|Z").fillet(radius)


def _stable_bounds(params: dict) -> tuple[float, float, float, float]:
    planes = params["wall_planes"]
    return planes["west_x"], planes["east_x"], planes["south_y"], planes["north_y"]


def vertical_chamfer_outer_envelope(params: dict) -> cq.Workplane:
    box = params["parametric_box"]
    return cq.Workplane("XY").box(box["width"], box["depth"], box["height"], centered=(False, False, False)).edges("|Z").chamfer(box["chamfer_z"])


def _cavity_bounds(params: dict, cavity: dict) -> tuple[float, float, float, float]:
    center_x, center_y = cavity["center"]
    return center_x - cavity["width"] / 2.0, center_x + cavity["width"] / 2.0, center_y - cavity["depth"] / 2.0, center_y + cavity["depth"] / 2.0


def main_cavity_prism(params: dict, half: str) -> cq.Workplane:
    cavity = params["mating_interface"]["main_cavity"]
    z_min, z_max = cavity["deep_z"] if half == "deep" else cavity["shallow_z"]
    return rounded_prism(_cavity_bounds(params, cavity), cavity["radius"], z_min, z_max)


def opening_prism(params: dict, z_min: float | None = None, z_max: float | None = None) -> cq.Workplane:
    opening = params["mating_interface"]["opening"]
    default_z_min, default_z_max = opening["cut_z"]
    z_min = default_z_min if z_min is None else z_min
    z_max = default_z_max if z_max is None else z_max
    return rounded_prism(_cavity_bounds(params, opening), opening["radius"], z_min, z_max)


def _source_diagonal_intercept(pristine_shallow: cq.Workplane) -> float:
    section = pristine_shallow.intersect(_prism((0.0, 30.0, 0.0, 30.0), 53.2, 53.201))
    candidates = []
    for raw_edge in section.edges().vals():
        edge = cast(cq.Edge, raw_edge)
        vertices = edge.Vertices()
        if len(vertices) != 2:
            continue
        first, second = vertices
        dx = second.X - first.X
        dy = second.Y - first.Y
        if abs(dx + dy) <= 1.0e-5 and 4.0 <= first.X <= 11.0 and 4.0 <= first.Y <= 11.0:
            candidates.append((first.X + first.Y + second.X + second.Y) / 2.0)
    unique_candidates = {round(candidate, 6) for candidate in candidates}
    require(len(unique_candidates) == 1, "original diagonal source witness changed")
    return candidates[0]


def original_diagonal_clash(params: dict, pristine_shallow: cq.Workplane) -> tuple[float, float]:
    width = 246.3
    depth = 139.1
    radius = 8.0
    x_min = (params["parametric_box"]["width"] - width) / 2.0
    y_min = (params["parametric_box"]["depth"] - depth) / 2.0
    lip = rounded_prism((x_min, x_min + width, y_min, y_min + depth), radius, 53.0, 53.55)
    clash = _volume(lip.intersect(pristine_shallow))
    circle_tangent_intercept = 2.0 * (x_min + radius - radius / 2.0**0.5)
    normal_penetration = (_source_diagonal_intercept(pristine_shallow) - circle_tangent_intercept) / 2.0**0.5
    return clash, normal_penetration


def _corner_bounds(params: dict) -> tuple[tuple[str, tuple[float, float, float, float]], ...]:
    west, east, south, north = _stable_bounds(params)
    radius = params["mating_interface"]["main_cavity"]["radius"]
    return (
        ("sw", (west, west + radius, south, south + radius)),
        ("se", (east - radius, east, south, south + radius)),
        ("nw", (west, west + radius, north - radius, north)),
        ("ne", (east - radius, east, north - radius, north)),
    )


def corner_fill_components(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
    cavity = main_cavity_prism(params, half)
    z_min, z_max = params["mating_interface"]["main_cavity"]["deep_z"] if half == "deep" else params["mating_interface"]["main_cavity"]["shallow_z"]
    outer = vertical_chamfer_outer_envelope(params)
    return tuple((f"{half}_main_cavity_corner_{name}", _prism(bounds, z_min, z_max).cut(cavity).intersect(outer).clean()) for name, bounds in _corner_bounds(params))


def lip_corner_relief_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    west, east, south, north = _stable_bounds(params)
    relief_z_min, relief_z_max = params["mating_interface"]["lip_corner_relief"]["z"]
    outer = vertical_chamfer_outer_envelope(params)
    bounds = (
        ("sw", (west, west + 10.1, south, south + 10.5)),
        ("se", (east - 10.1, east, south, south + 10.5)),
        ("nw", (west, west + 10.1, north - 10.5, north)),
        ("ne", (east - 10.1, east, north - 10.5, north)),
    )
    return tuple((f"shallow_lip_corner_relief_{name}", _prism(corner_bounds, relief_z_min, relief_z_max).intersect(outer).clean()) for name, corner_bounds in bounds)


def mating_rails(params: dict) -> tuple[MatingRail, ...]:
    mating = params["mating_interface"]
    opening = mating["opening"]
    tongue = mating["tongue"]
    center_x, center_y = opening["center"]
    x_min = center_x - opening["width"] / 2.0
    x_max = center_x + opening["width"] / 2.0
    y_min = center_y - opening["depth"] / 2.0
    y_max = center_y + opening["depth"] / 2.0
    tangent_stop = opening["radius"] + tongue["corner_tangent_relief"]
    inner_hinge_span = params["parametric_box"]["hinge"]["number_of_kunkles"] * params["parametric_box"]["hinge"]["kunkle_size"] + params["parametric_box"]["hinge"]["clearance"]
    hinge_relief = inner_hinge_span / 2.0 + tongue["south_hinge_relief_margin"]
    hinge_centers = (center_x - params["parametric_box"]["width"] / 4.0, center_x + params["parametric_box"]["width"] / 4.0)
    return (
        MatingRail("mating_tongue_west", (x_min - tongue["thickness"], x_min, y_min + tangent_stop, y_max - tangent_stop)),
        MatingRail("mating_tongue_east", (x_max, x_max + tongue["thickness"], y_min + tangent_stop, y_max - tangent_stop)),
        MatingRail("mating_tongue_north", (x_min + tangent_stop, x_max - tangent_stop, y_max, y_max + tongue["thickness"])),
        MatingRail("mating_tongue_south_west", (x_min + tangent_stop, hinge_centers[0] - hinge_relief, y_min - tongue["thickness"] + 0.0, y_min)),
        MatingRail("mating_tongue_south_center", (hinge_centers[0] + hinge_relief, hinge_centers[1] - hinge_relief, y_min - tongue["thickness"] + 0.0, y_min)),
        MatingRail("mating_tongue_south_east", (hinge_centers[1] + hinge_relief, x_max - tangent_stop, y_min - tongue["thickness"] + 0.0, y_min)),
    )


def receiver_rails(params: dict) -> tuple[MatingRail, ...]:
    clearance = params["mating_interface"]["receiver"]["xy_clearance"]
    return tuple(
        MatingRail(
            f"mating_receiver_{rail.name.removeprefix('mating_tongue_')}",
            (rail.bounds[0] - clearance, rail.bounds[1] + clearance, rail.bounds[2] - clearance, rail.bounds[3] + clearance),
        )
        for rail in mating_rails(params)
    )


def shallow_tongue_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    z_min, z_max = params["mating_interface"]["tongue"]["z"]
    return tuple((rail.name, _prism(rail.bounds, z_min, z_max)) for rail in mating_rails(params))


def shallow_hinge_gap_bar_relief_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    rails = {rail.name: rail for rail in mating_rails(params)}
    opening_y_min = _cavity_bounds(params, params["mating_interface"]["opening"])[2]
    south_wall_y = params["wall_planes"]["south_y"]
    z_min, z_max = params["mating_interface"]["tongue"]["z"]
    bounds = (
        ("shallow_hinge_gap_bar_relief_west", (rails["mating_tongue_south_west"].bounds[1], rails["mating_tongue_south_center"].bounds[0], south_wall_y, opening_y_min)),
        ("shallow_hinge_gap_bar_relief_east", (rails["mating_tongue_south_center"].bounds[1], rails["mating_tongue_south_east"].bounds[0], south_wall_y, opening_y_min)),
    )
    return tuple((name, _prism(relief_bounds, z_min, z_max)) for name, relief_bounds in bounds)


def deep_receiver_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    z_min, z_max = params["mating_interface"]["receiver"]["cut_z"]
    return tuple((rail.name, _prism(rail.bounds, z_min, z_max)) for rail in receiver_rails(params))


def apply_shallow_mating_interface(params: dict, model: cq.Workplane) -> cq.Workplane:
    result = model.cut(opening_prism(params))
    for _, relief in lip_corner_relief_components(params):
        result = result.cut(relief)
    for _, relief in shallow_hinge_gap_bar_relief_components(params):
        result = result.cut(relief)
    result = result.cut(main_cavity_prism(params, "shallow"))
    for _, fill in corner_fill_components(params, "shallow"):
        result = result.union(fill)
    for _, rail in shallow_tongue_components(params):
        result = result.union(rail)
    return result.clean()


def apply_deep_mating_interface(params: dict, model: cq.Workplane) -> cq.Workplane:
    result = model.cut(main_cavity_prism(params, "deep"))
    for _, fill in corner_fill_components(params, "deep"):
        result = result.union(fill)
    for _, receiver in deep_receiver_components(params):
        result = result.cut(receiver)
    return result.clean()


def _volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def _intersection_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return _volume(first.intersect(second))


def _filament_pin(params: dict, center: float) -> cq.Workplane:
    pin = params["hinge_pins"]
    axis_y, axis_z = hinge_axis(params)
    return cq.Workplane("YZ").circle(pin["nominal_diameter"] / 2.0).extrude(pin["piece_length"]).translate((center - pin["piece_length"] / 2.0, axis_y, axis_z))


def transformed_hinge_profile(params: dict, center: float, variant: str) -> cq.Workplane:
    box = params["parametric_box"]
    source_box = parametric_box_measures(params, ())
    source_hinge = upstream.Hinge(source_box.hinge)
    source_x = center - params["normalization"]["translate"][0]
    rel_y = 2.0 * box["hinge"]["thickness"] - box["hinge"]["leaf_height"]
    rel_z = box["height"] - box["top_height"] - box["height"] / 2.0
    raw = source_hinge.vuoto if variant == "deep" else source_hinge.pieno
    return normalize_source_half(raw.translate((source_x, box["depth"] / 2.0 + rel_y, rel_z)).translate((0.0, -box["depth"] / 2.0 - box["thickness"], 0.0)), params)


def hinge_profile_symmetric_difference(pristine: cq.Workplane, corrected: cq.Workplane, profile: cq.Workplane) -> tuple[float, float]:
    pristine_intersection = pristine.intersect(profile)
    corrected_intersection = corrected.intersect(profile)
    return _volume(pristine_intersection.cut(corrected_intersection)), _volume(corrected_intersection.cut(pristine_intersection))


def validate_hinge_profile_preservation(params: dict, corrected_deep: cq.Workplane, corrected_shallow: cq.Workplane) -> None:
    source_box = upstream.build_box(parametric_box_measures(params, ()))
    pristine = {
        "deep": normalize_source_half(source_box.top(), params),
        "shallow": normalize_source_half(source_box.bottom(), params),
    }
    corrected = {"deep": corrected_deep, "shallow": corrected_shallow}
    maximum = 0.0
    for center in hinge_centers(params):
        for variant in ("deep", "shallow"):
            differences = hinge_profile_symmetric_difference(pristine[variant], corrected[variant], transformed_hinge_profile(params, center, variant))
            maximum = max(maximum, *differences)
            require(max(differences) <= VOLUME_TOLERANCE, f"{variant} hinge profile changed at {center}")
    print(f"hinge_profile_preservation=symmetric_difference_max={maximum:.3e} both_directions=PASS")


def _shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def _plan_edges(model: cq.Workplane) -> list[cq.Edge]:
    face = cast(cq.Face, model.faces(">Z").val())
    return [cast(cq.Edge, edge) for wire in face.Wires() for edge in wire.Edges()]


def receiver_exterior_lands(params: dict) -> tuple[tuple[str, float], ...]:
    width = params["parametric_box"]["width"]
    depth = params["parametric_box"]["depth"]
    lands = []
    for name, receiver in deep_receiver_components(params):
        box = _shape_box(receiver)
        if name == "mating_receiver_west":
            land = box.xmin
        elif name == "mating_receiver_east":
            land = width - box.xmax
        elif name == "mating_receiver_north":
            land = depth - box.ymax
        else:
            land = box.ymin
        lands.append((name, land))
    return tuple(lands)


def corrected_outer_bbox_delta(pristine: cq.Workplane, corrected: cq.Workplane) -> float:
    pristine_box = _shape_box(pristine)
    corrected_box = _shape_box(corrected)
    return max(abs(getattr(pristine_box, axis) - getattr(corrected_box, axis)) for axis in ("xmin", "xmax", "ymin", "ymax", "zmin", "zmax"))


def _union_components(components: tuple[tuple[str, cq.Workplane], ...]) -> cq.Workplane:
    result = components[0][1]
    for _, component in components[1:]:
        result = result.union(component)
    return result.clean()


def component_added_volumes(base: cq.Workplane, components: tuple[tuple[str, cq.Workplane], ...]) -> tuple[tuple[str, float], ...]:
    result = base
    additions = []
    for name, component in components:
        updated = result.union(component).clean()
        additions.append((name, _volume(updated.cut(result))))
        result = updated
    return tuple(additions)


def _add_components(base: cq.Workplane, components: tuple[tuple[str, cq.Workplane], ...]) -> cq.Workplane:
    result = base
    for _, component in components:
        result = result.union(component).clean()
    return result


def stable_square_symmetric_difference(model: cq.Workplane, expected: cq.Workplane, bounds: tuple[float, float, float, float], z: float) -> float:
    probe = _prism(bounds, z, z + 0.01)
    actual_section = model.intersect(probe)
    expected_section = expected.intersect(probe)
    return _volume(actual_section.cut(expected_section)) + _volume(expected_section.cut(actual_section))


def symmetric_difference_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return _volume(first.cut(second)) + _volume(second.cut(first))


def shallow_corner_shoulder_areas(params: dict) -> tuple[tuple[str, float], ...]:
    areas = []
    for name, fill in corner_fill_components(params, "shallow"):
        downward_faces = [cast(cq.Face, face) for face in fill.faces("<Z").vals() if cast(cq.Face, face).geomType() == "PLANE"]
        require(len(downward_faces) == 1, f"{name} must have one downward shoulder")
        areas.append((name, downward_faces[0].Area()))
    return tuple(areas)


def minimum_normal_corner_wall(params: dict) -> float:
    west, _, south, _ = _stable_bounds(params)
    radius = params["mating_interface"]["main_cavity"]["radius"]
    return (2.0 * (west + radius) - radius * 2.0**0.5 - params["parametric_box"]["chamfer_z"]) / 2.0**0.5


def validate_mating_interface(params: dict, pristine_deep: cq.Workplane, pristine_shallow: cq.Workplane, corrected_deep: cq.Workplane, corrected_shallow: cq.Workplane) -> None:
    mating = params["mating_interface"]
    opening = opening_prism(params)
    main = mating["main_cavity"]
    rails = shallow_tongue_components(params)
    receivers = deep_receiver_components(params)
    bar_reliefs = shallow_hinge_gap_bar_relief_components(params)
    shallow_z_min, shallow_z_max = main["shallow_z"]
    require(main == {"width": 251.2, "depth": 144.0, "radius": 9.5, "center": [127.8, 74.2], "deep_z": [2.2, 54.65], "shallow_z": [54.85, 60.65]}, "main cavity contract changed")
    require(mating["opening"] == {"width": 250.0, "depth": 142.0, "radius": 8.5, "center": [127.8, 74.2], "cut_z": [52.55, 54.85]}, "mating opening contract changed")
    require(mating["tongue"] == {"thickness": 1.3, "z": [52.65, 54.85], "corner_tangent_relief": 1.0, "south_hinge_relief_margin": 1.0}, "mating tongue contract changed")
    require(mating["receiver"] == {"xy_clearance": 0.25, "cut_z": [52.55, 54.65]}, "mating receiver contract changed")
    require(mating["lip_corner_relief"] == {"z": [52.55, 54.85]}, "lip corner relief contract changed")
    require([name for name, _ in bar_reliefs] == ["shallow_hinge_gap_bar_relief_west", "shallow_hinge_gap_bar_relief_east"], "hinge-gap bar relief names changed")
    require(_stable_bounds(params) == (2.2, 253.4, 2.2, 146.2), "stable mating bounds changed")
    original_clash, original_penetration = original_diagonal_clash(params, pristine_shallow)
    require(3.08 <= original_clash <= 3.19 and abs(original_clash - 3.135323) <= TOLERANCE, "original diagonal clash changed")
    require(abs(original_penetration - 0.417) <= 0.01, "original diagonal penetration changed")
    opening_box = _shape_box(opening)
    require(all(abs(actual - expected) <= TOLERANCE for actual, expected in zip((opening_box.xmin, opening_box.xmax, opening_box.ymin, opening_box.ymax, opening_box.zmin, opening_box.zmax), (2.8, 252.8, 3.2, 145.2, 52.55, 54.85))), "mating opening bounds changed")
    opening_edges = _plan_edges(opening)
    require(sum(edge.geomType() == "LINE" for edge in opening_edges) == 4 and sum(edge.geomType() == "CIRCLE" for edge in opening_edges) == 4, "mating opening profile is not four lines and four arcs")
    require(all(abs(getattr(edge, "radius")() - 8.5) <= TOLERANCE for edge in opening_edges if edge.geomType() == "CIRCLE"), "mating opening radius changed")
    main_profile = rounded_prism(_cavity_bounds(params, main), main["radius"], 0.0, 1.0)
    main_box = _shape_box(main_profile)
    require(all(abs(actual - expected) <= TOLERANCE for actual, expected in zip((main_box.xmin, main_box.xmax, main_box.ymin, main_box.ymax), (2.2, 253.4, 2.2, 146.2))), "main cavity bounds changed")
    main_edges = _plan_edges(main_profile)
    require(sum(edge.geomType() == "LINE" for edge in main_edges) == 4 and sum(edge.geomType() == "CIRCLE" for edge in main_edges) == 4, "main cavity profile is not four lines and four arcs")
    require(all(abs(getattr(edge, "radius")() - 9.5) <= TOLERANCE for edge in main_edges if edge.geomType() == "CIRCLE"), "main cavity radius changed")
    require(_volume(opening_prism(params, 52.55, 54.85).cut(rounded_prism(_cavity_bounds(params, main), main["radius"], 52.55, 54.85))) <= VOLUME_TOLERANCE, "lip opening is not contained in main cavity profile")
    outer = vertical_chamfer_outer_envelope(params)
    for half in ("deep", "shallow"):
        cavity = main_cavity_prism(params, half)
        for _, fill in corner_fill_components(params, half):
            require(_intersection_volume(fill, cavity) <= VOLUME_TOLERANCE and _volume(fill.cut(outer)) <= VOLUME_TOLERANCE, f"{half} corner fill is outside its ownership")
    deep_cavity = main_cavity_prism(params, "deep")
    deep_after_main = pristine_deep.cut(deep_cavity)
    deep_fills = corner_fill_components(params, "deep")
    deep_filled = _add_components(deep_after_main, deep_fills)
    expected_deep = deep_filled
    for _, receiver in receivers:
        expected_deep = expected_deep.cut(receiver).clean()
    shallow_after_lip = pristine_shallow.cut(opening)
    shallow_reliefs = lip_corner_relief_components(params)
    shallow_after_relief = shallow_after_lip
    for _, relief in shallow_reliefs:
        shallow_after_relief = shallow_after_relief.cut(relief).clean()
    shallow_after_bar = shallow_after_relief
    bar_removals = []
    for name, relief in bar_reliefs:
        updated = shallow_after_bar.cut(relief).clean()
        bar_removals.append((name, _volume(shallow_after_bar.cut(updated))))
        shallow_after_bar = updated
    shallow_after_main = shallow_after_bar.cut(main_cavity_prism(params, "shallow"))
    shallow_fills = corner_fill_components(params, "shallow")
    shallow_filled = _add_components(shallow_after_main, shallow_fills)
    expected_shallow = _add_components(shallow_filled, rails)
    require(abs(_volume(pristine_deep) - 173571.147583) <= TOLERANCE and abs(_volume(pristine_shallow) - 99669.924754) <= TOLERANCE, "zero-chamfer pristine volumes changed")
    require(abs(_volume(pristine_deep.intersect(deep_cavity)) - 877.492280) <= TOLERANCE, "deep main cavity removal changed")
    require(abs(_volume(pristine_shallow.intersect(opening)) - 2703.637698) <= TOLERANCE, "shallow lip removal changed")
    require(abs(_volume(shallow_after_lip.cut(shallow_after_relief)) - 50.763420) <= TOLERANCE and abs(sum(_volume(relief) for _, relief in shallow_reliefs) - 916.044) <= TOLERANCE, "shallow corner relief changed")
    require(all(abs(_volume(relief) - 71.5) <= TOLERANCE for _, relief in bar_reliefs) and all(abs(removed - 53.024822397) <= TOLERANCE for _, removed in bar_removals) and abs(sum(removed for _, removed in bar_removals) - 106.049644795) <= TOLERANCE, "shallow hinge-gap bar relief changed")
    require(abs(_volume(shallow_after_bar.cut(shallow_after_main)) - 2378.771609) <= TOLERANCE, "shallow main cavity removal changed")
    deep_fill_additions = component_added_volumes(deep_after_main, deep_fills)
    shallow_fill_additions = component_added_volumes(shallow_after_main, shallow_fills)
    require(all(abs(volume_added - 54.813064) <= TOLERANCE for _, volume_added in deep_fill_additions), "deep corner fill additions changed")
    require(all(abs(volume_added - 3.582536) <= TOLERANCE for _, volume_added in shallow_fill_additions), "shallow corner fill additions changed")
    require(abs(sum(volume_added for _, volume_added in deep_fill_additions) - 219.252257) <= TOLERANCE and abs(sum(volume_added for _, volume_added in shallow_fill_additions) - 14.330143) <= TOLERANCE, "corner fill totals changed")
    receiver_removed = _volume(deep_filled.cut(expected_deep))
    rail_added = _volume(expected_shallow.cut(shallow_filled))
    require(abs(receiver_removed - 930.905) <= TOLERANCE and abs(sum(_volume(receiver) for _, receiver in receivers) - 2441.88) <= TOLERANCE, "receiver removal changed")
    require(abs(rail_added - 994.51) <= TOLERANCE and abs(sum(_volume(rail) for _, rail in rails) - 1838.98) <= TOLERANCE, "rail addition changed")
    rail_attachment_min = min(_intersection_volume(rail, pristine_shallow) for _, rail in rails)
    require(rail_attachment_min >= 58.32, "mating rail attachment is below the required minimum")
    require(abs(_volume(pristine_deep.cut(expected_deep)) - 1808.397280) <= TOLERANCE and abs(_volume(expected_deep) - 171982.002559) <= TOLERANCE, "deep corrected volume changed")
    require(abs(_volume(pristine_shallow.cut(expected_shallow)) - 5239.222371) <= TOLERANCE and abs(_volume(expected_shallow.cut(pristine_shallow)) - 1008.840143) <= TOLERANCE and abs(_volume(expected_shallow) - 95439.542508) <= TOLERANCE, "shallow corrected volumes changed")
    deep_fill_union = _union_components(deep_fills)
    shallow_fill_union = _union_components(shallow_fills)
    expected_shallow_additions = _add_components(shallow_fill_union, rails).cut(pristine_shallow)
    require(symmetric_difference_volume(expected_deep, corrected_deep) <= VOLUME_TOLERANCE and symmetric_difference_volume(expected_shallow, corrected_shallow) <= VOLUME_TOLERANCE, "corrected half changed")
    require(symmetric_difference_volume(pristine_deep.cut(expected_deep), pristine_deep.cut(corrected_deep)) <= VOLUME_TOLERANCE and symmetric_difference_volume(pristine_shallow.cut(expected_shallow), pristine_shallow.cut(corrected_shallow)) <= VOLUME_TOLERANCE, "removed ownership changed")
    require(symmetric_difference_volume(corrected_deep.cut(pristine_deep), deep_fill_union.cut(pristine_deep)) <= VOLUME_TOLERANCE and symmetric_difference_volume(corrected_shallow.cut(pristine_shallow), expected_shallow_additions) <= VOLUME_TOLERANCE, "added ownership changed")
    require(_intersection_volume(deep_fill_union, _union_components(receivers)) <= VOLUME_TOLERANCE, "receiver cuts overlap deep fill additions")
    stable_bounds = _stable_bounds(params)
    deep_z_min, deep_z_max = main["deep_z"]
    deep_z = [deep_z_min + 0.01 + 0.05 * index for index in range(round((deep_z_max - deep_z_min) / 0.05))]
    shallow_z = [shallow_z_min + 0.01 + 0.05 * index for index in range(round((shallow_z_max - shallow_z_min) / 0.05))]
    require(all(stable_square_symmetric_difference(corrected_deep, deep_fill_union, stable_bounds, z) <= VOLUME_TOLERANCE for z in deep_z), "deep stable-square profiles changed")
    require(all(stable_square_symmetric_difference(corrected_shallow, shallow_fill_union, stable_bounds, z) <= VOLUME_TOLERANCE for z in shallow_z), "shallow stable-square profiles changed")
    passage_probes = []
    for fit, shifts in ((params["device"]["envelope"], ((0.0, 0.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0))), (params["device"]["check_fit"], ((0.0, 0.0), (0.5, 0.5), (-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5)))):
        for radius in (8.0, 8.5):
            for shift_x, shift_y in shifts:
                center_x, center_y = params["device"]["center"]
                bounds = (center_x + shift_x - fit["width"] / 2.0, center_x + shift_x + fit["width"] / 2.0, center_y + shift_y - fit["depth"] / 2.0, center_y + shift_y + fit["depth"] / 2.0)
                passage_probes.append(rounded_prism(bounds, radius, shallow_z_min, shallow_z_max))
    passage_probes.append(opening_prism(params, shallow_z_min, shallow_z_max))
    require(all(_intersection_volume(fill, probe) <= VOLUME_TOLERANCE for _, fill in shallow_fills for probe in passage_probes), "shallow corner fill blocks a guaranteed passage")
    hinge_profiles = tuple(transformed_hinge_profile(params, center, "shallow") for center in hinge_centers(params))
    pins = tuple(_filament_pin(params, center) for center in hinge_centers(params))
    relief_gaps = []
    for _, relief in bar_reliefs:
        require(_intersection_volume(corrected_shallow, relief) <= VOLUME_TOLERANCE, "shallow hinge-gap bar relief retains material")
        require(all(_intersection_volume(relief, rail) <= VOLUME_TOLERANCE for _, rail in rails), "shallow hinge-gap bar relief intersects a rail")
        require(all(_intersection_volume(relief, profile) <= VOLUME_TOLERANCE for profile in hinge_profiles), "shallow hinge-gap bar relief intersects a hinge profile")
        require(all(_intersection_volume(relief, pin) <= VOLUME_TOLERANCE for pin in pins), "shallow hinge-gap bar relief intersects a filament pin")
        require(all(_intersection_volume(relief, probe) <= VOLUME_TOLERANCE for probe in passage_probes), "shallow hinge-gap bar relief intersects a device passage")
        relief_box = _shape_box(relief)
        relief_gaps.extend(relief_box.ymin - _shape_box(profile).ymax for profile in hinge_profiles)
    require(min(relief_gaps) >= 0.2 - 1.0e-6, "shallow hinge-gap bar relief is too close to a hinge profile")
    shoulders = shallow_corner_shoulder_areas(params)
    require(len(shoulders) == 4 and all(abs(area - 12.887815753) <= 0.0001 for _, area in shoulders) and abs(sum(area for _, area in shoulders) - 51.551263014) <= 0.0001, "shallow corner shoulders changed")
    lands = receiver_exterior_lands(params)
    require(min(land for _, land in lands) >= 1.20, "receiver exterior land is below 1.20 mm")
    normal_corner_wall = minimum_normal_corner_wall(params)
    require(normal_corner_wall >= 1.2, "normal corner wall is below 1.2 mm")
    outer_bbox_delta = max(corrected_outer_bbox_delta(pristine_deep, corrected_deep), corrected_outer_bbox_delta(pristine_shallow, corrected_shallow))
    require(outer_bbox_delta <= TOLERANCE, "mating correction changed an outer bounding box")
    print(f"mating_interface=main_cavity=251.2x144R9.5 deepZ2.2..54.65 shallowZ54.85..60.65 opening=250x142R8.5 cutZ52.55..54.85 rails=6 receivers=6 pristine=deep173571.147583/shallow99669.924754 deep_corrected=171982.002559 shallow_corrected=95439.542508 lip_removed=2703.637698 relief_removed=50.763420 hinge_gap_bar_raw=71.500000/each hinge_gap_bar_removed=53.024822/each total=106.049645 main_shallow_removed=2378.771609 receiver_cut=930.905000 rail_added=994.510000 fills=deep219.252257/shallow14.330143 receiver_exterior_land_min={min(land for _, land in lands):.2f} normal_corner_wall={normal_corner_wall:.6f} original_diagonal_clash={original_clash:.6f} original_diagonal_penetration={original_penetration:.6f} hinge_gap_to_profile_min={min(relief_gaps):.6f} shoulders_total={sum(area for _, area in shoulders):.9f} outer_bbox_delta={outer_bbox_delta:.3e} rail_attachment_min={rail_attachment_min:.4f} PASS")
