from __future__ import annotations

import math
from typing import cast

import cadquery as cq

try:
    from .protective_case_geometry import CornerRestraint, corner_restraints, dimensions, wall_planes
    from .protective_case_keepouts import control_contact_keepouts
except ImportError:
    from protective_case_geometry import CornerRestraint, corner_restraints, dimensions, wall_planes
    from protective_case_keepouts import control_contact_keepouts


VOLUME_TOLERANCE = 1.0e-7


def _box(width: float, depth: float, height: float, x: float, y: float, z: float) -> cq.Workplane:
    return cq.Workplane("XY").box(width, depth, height, centered=(False, False, False)).translate((x, y, z))


def _solid(bounds: tuple[float, float, float, float], z_min: float, z_max: float) -> cq.Workplane:
    x_min, x_max, y_min, y_max = bounds
    return _box(x_max - x_min, y_max - y_min, z_max - z_min, x_min, y_min, z_min)


def _unit(vector: tuple[float, float]) -> tuple[float, float]:
    length = math.hypot(*vector)
    return vector[0] / length, vector[1] / length


def _shelf_entry_exit(restraint: CornerRestraint, radius: float) -> tuple[tuple[float, float], tuple[float, float], tuple[float, float]]:
    points = restraint.shelf_polygon
    corner_index = points.index(restraint.device_corner)
    corner = points[corner_index]
    previous = points[(corner_index - 1) % len(points)]
    following = points[(corner_index + 1) % len(points)]
    previous_unit = _unit((previous[0] - corner[0], previous[1] - corner[1]))
    following_unit = _unit((following[0] - corner[0], following[1] - corner[1]))
    bisector = _unit((previous_unit[0] + following_unit[0], previous_unit[1] + following_unit[1]))
    interior_half_sine = math.sqrt((1.0 - (previous_unit[0] * following_unit[0] + previous_unit[1] * following_unit[1])) / 2.0)
    center = (corner[0] + bisector[0] * radius / interior_half_sine, corner[1] + bisector[1] * radius / interior_half_sine)
    entry = (corner[0] + previous_unit[0] * radius, corner[1] + previous_unit[1] * radius)
    exit = (corner[0] + following_unit[0] * radius, corner[1] + following_unit[1] * radius)
    midpoint = (center[0] - bisector[0] * radius, center[1] - bisector[1] * radius)
    return entry, midpoint, exit


def _rounded_shelf_plan(restraint: CornerRestraint, radius: float) -> cq.Workplane:
    points = restraint.shelf_polygon
    corner_index = points.index(restraint.device_corner)
    entry, midpoint, exit = _shelf_entry_exit(restraint, radius)
    wire = cq.Workplane("XY").moveTo(*entry).threePointArc(midpoint, exit)
    for offset in range(1, len(points)):
        wire = wire.lineTo(*points[(corner_index + offset) % len(points)])
    return wire.close()


def _device_facing_top_edges(shelf: cq.Workplane, restraint: CornerRestraint, radius: float) -> list[cq.Shape]:
    entry, _, exit = _shelf_entry_exit(restraint, radius)
    selected = []
    for edge in cast(list[cq.Shape], shelf.edges().vals()):
        box = edge.BoundingBox()
        if abs(box.zmin - box.zmax) > 0.05 or abs(box.zmax - restraint.contact_z) > 0.05:
            continue
        if any(
            math.hypot(vertex.Center().x - point[0], vertex.Center().y - point[1]) <= 0.05
            for vertex in edge.Vertices()
            for point in (entry, exit)
        ):
            selected.append(edge)
    return selected


def _edge_connects(edge: cq.Shape, first: tuple[float, float], second: tuple[float, float]) -> bool:
    vertices = edge.Vertices()
    if len(vertices) != 2:
        return False
    points = {(round(vertex.Center().x, 5), round(vertex.Center().y, 5)) for vertex in vertices}
    return points == {(round(first[0], 5), round(first[1], 5)), (round(second[0], 5), round(second[1], 5))}


def shelf_contact_pad(restraint: CornerRestraint, params: dict) -> cq.Workplane:
    spec = params["corner_restraints"]
    z_min = restraint.nominal_shelf_base_z - spec["shelf_overlap"]
    shelf_plan = _rounded_shelf_plan(restraint, spec["shelf_plan_radius"])
    rear_edges = [edge for edge in cast(list[cq.Shape], shelf_plan.edges().vals()) if _edge_connects(edge, restraint.shelf_polygon[-1], restraint.shelf_polygon[0])]
    if len(rear_edges) != 1 or rear_edges[0].geomType() != "LINE":
        raise ValueError(f"{restraint.name} shelf diagonal rear edge is not square")
    shelf = shelf_plan.extrude(restraint.contact_z - z_min).translate((0.0, 0.0, z_min))
    top_edges = _device_facing_top_edges(shelf, restraint, spec["shelf_plan_radius"])
    if len(top_edges) != 3:
        raise ValueError(f"{restraint.name} shelf expected three device-facing top edges, found {len(top_edges)}")
    return shelf.newObject(top_edges).fillet(spec["shelf_top_edge_radius"]).clean()


def shelf_contact_component(restraint: CornerRestraint, params: dict) -> cq.Workplane:
    spec = params["corner_restraints"]
    z_min = restraint.nominal_shelf_base_z - spec["shelf_overlap"]
    shelf = shelf_contact_pad(restraint, params)
    for bounds in restraint.shelf_tabs.values():
        shelf = shelf.union(_solid(bounds, z_min, restraint.contact_z)).clean()
    return shelf


def continuous_shelf_support(restraint: CornerRestraint, params: dict) -> cq.Workplane:
    spec = params["corner_restraints"]
    lower_z = spec["support_lower_z"]
    upper_z = restraint.nominal_shelf_base_z - spec["shelf_overlap"]
    return (
        cq.Workplane("XY")
        .workplane(offset=lower_z)
        .polyline(list(restraint.support_lower_polygon))
        .close()
        .workplane(offset=upper_z - lower_z)
        .polyline(list(restraint.support_upper_polygon))
        .close()
        .loft(combine=True, ruled=True)
    )


def corner_restraint_component(restraint: CornerRestraint, params: dict) -> cq.Workplane:
    return continuous_shelf_support(restraint, params).union(shelf_contact_component(restraint, params)).clean()


def corner_restraint_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    return tuple(
        (f"corner_restraint_{restraint.name.lower()}", corner_restraint_component(restraint, params))
        for restraint in corner_restraints(params)
    )


def add_corner_restraints(params: dict, model: cq.Workplane) -> cq.Workplane:
    result = model
    for _, restraint in corner_restraint_components(params):
        result = result.union(restraint).clean()
    return result.clean()


def _volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def _intersection_volume(first: cq.Workplane, *others: cq.Workplane) -> float:
    result = first
    for other in others:
        result = result.intersect(other)
    return _volume(result)


def _prism(x: float, y: float, z: float, width: float, depth: float, height: float) -> cq.Workplane:
    return cq.Workplane("XY").box(width, depth, height, centered=(False, False, False)).translate((x, y, z))


def _valid_single(model: cq.Workplane, label: str) -> None:
    shape = cast(cq.Shape, model.val())
    if not (shape.isValid() and len(model.solids().vals()) == 1):
        raise ValueError(f"{label} must be one valid solid")


def _valid_shell_solid(model: cq.Workplane, label: str) -> None:
    _valid_single(model, label)
    if len(model.shells().vals()) != 1:
        raise ValueError(f"{label} must be one shell")


def _sample_face_normals(face: cq.Shape) -> tuple[cq.Vector, ...]:
    uv = [getattr(face, "paramAt")(vertex.Center()) for vertex in face.Vertices()]
    if not uv:
        return ()
    u_min, u_max = min(point[0] for point in uv), max(point[0] for point in uv)
    v_min, v_max = min(point[1] for point in uv), max(point[1] for point in uv)
    normals = []
    for u_fraction in (0.1, 0.5, 0.9):
        for v_fraction in (0.1, 0.5, 0.9):
            sample = getattr(face, "normalAt")(u_min + (u_max - u_min) * u_fraction, v_min + (v_max - v_min) * v_fraction)
            normals.append(sample[0] if isinstance(sample, tuple) else sample)
    return tuple(normals)


def _minimum_support_slope(support: cq.Workplane) -> float:
    slopes = []
    for face in cast(list[cq.Shape], support.faces().vals()):
        for normal in _sample_face_normals(face):
            angle = math.degrees(math.acos(min(1.0, abs(normal.z))))
            if angle > 1.0:
                slopes.append(angle)
    return min(slopes)


def _downward_horizontal_faces(model: cq.Workplane) -> tuple[tuple[float, float], ...]:
    result = []
    for face in cast(list[cq.Shape], model.faces().vals()):
        normal = getattr(face, "normalAt")()
        box = face.BoundingBox()
        if normal.z >= -0.999 or box.zmax <= 2.2 + 0.05:
            continue
        circle_edges = [edge for edge in cast(list[cq.Shape], face.Edges()) if edge.geomType() == "CIRCLE"]
        line_edges = [edge for edge in cast(list[cq.Shape], face.Edges()) if edge.geomType() == "LINE"]
        if len(circle_edges) != 1 or len(line_edges) != 1:
            raise ValueError("unexpected hard horizontal underside face")
        radius = getattr(circle_edges[0], "radius")()
        chord = getattr(line_edges[0], "Length")()
        unsupported_width = radius - math.sqrt(max(0.0, radius * radius - (chord / 2.0) ** 2))
        result.append((face.Area(), unsupported_width))
    return tuple(result)


def _outside_solids(component: cq.Workplane, outer_envelope: cq.Workplane) -> tuple[cq.Shape, ...]:
    envelope = cast(cq.Shape, outer_envelope.solids().vals()[0])
    return tuple(
        outside
        for solid in cast(list[cq.Shape], component.solids().vals())
        for outside in solid.cut(envelope).Solids()
    )


def outside_volume(component: cq.Workplane, outer_envelope: cq.Workplane) -> float:
    return sum(outside.Volume() for outside in _outside_solids(component, outer_envelope))


def outside_section_area(
    component: cq.Workplane,
    outer_envelope: cq.Workplane,
    z: float,
    case_width: float,
    case_depth: float,
) -> float:
    outside = _outside_solids(component, outer_envelope)
    if not outside:
        return 0.0
    section = cq.Workplane("XY").newObject(list(outside)).intersect(_prism(0.0, 0.0, z - 0.005, case_width, case_depth, 0.01))
    return _volume(section) / 0.01


def section_symmetric_difference_volume(
    first: cq.Workplane,
    second: cq.Workplane,
    z: float,
    case_width: float,
    case_depth: float,
) -> float:
    probe = _prism(0.0, 0.0, z - 0.005, case_width, case_depth, 0.01)
    first_section = first.intersect(probe)
    second_section = second.intersect(probe)
    if not first_section.solids().vals():
        return _volume(second_section)
    if not second_section.solids().vals():
        return _volume(first_section)
    return _volume(first_section.cut(second_section)) + _volume(second_section.cut(first_section))


def support_section_stations(restraint: CornerRestraint, params: dict) -> tuple[float, ...]:
    spec = params["corner_restraints"]
    lower_z = spec["support_lower_z"]
    upper_z = restraint.nominal_shelf_base_z - spec["shelf_overlap"]
    return (
        lower_z + 0.01,
        lower_z + 0.20,
        2.40,
        3.00,
        4.00,
        4.91127,
        7.80,
        8.00,
        (lower_z + upper_z) / 2.0,
        upper_z - 0.01,
    )


def wall_witness(params: dict, restraint: CornerRestraint, side: str) -> cq.Workplane:
    planes = wall_planes(params)
    support_points = restraint.support_lower_polygon + restraint.support_upper_polygon
    x_min = min(point[0] for point in support_points)
    x_max = max(point[0] for point in support_points)
    y_min = min(point[1] for point in support_points)
    y_max = max(point[1] for point in support_points)
    spec = params["corner_restraints"]
    z_min = spec["support_lower_z"]
    z_max = restraint.nominal_shelf_base_z - spec["shelf_overlap"]
    if side == "west":
        return _prism(planes["west_x"] - 0.6, y_min, z_min, 0.6, y_max - y_min, z_max - z_min)
    if side == "east":
        return _prism(planes["east_x"], y_min, z_min, 0.6, y_max - y_min, z_max - z_min)
    if side == "south":
        return _prism(x_min, planes["south_y"] - 0.6, z_min, x_max - x_min, 0.6, z_max - z_min)
    return _prism(x_min, planes["north_y"], z_min, x_max - x_min, 0.6, z_max - z_min)


def validate_corner_restraints(
    params: dict,
    deep: cq.Workplane,
    outer_envelope: cq.Workplane,
    final_tub: cq.Workplane,
) -> None:
    specs = params["corner_restraints"]
    keepouts = control_contact_keepouts(params)
    case_dims = dimensions(params)
    wall_metrics = []
    wall_gap_metrics = []
    support_metrics = []
    containment_metrics = []
    pad_metrics = []
    no_ridge_differences = []
    final_addition_differences = []
    adjacent_sides = {
        "NW": ("west", "north"),
        "SW": ("west", "south"),
        "NE": ("east", "north"),
        "SE": ("east", "south"),
    }
    for restraint, (name, component) in zip(corner_restraints(params), corner_restraint_components(params)):
        _valid_shell_solid(component, name)
        if len(restraint.support_lower_polygon) != 6 or len(restraint.support_upper_polygon) != 6:
            raise ValueError(f"{name} continuous shelf support must use six-vertex polygons")
        support = continuous_shelf_support(restraint, params)
        _valid_shell_solid(support, f"{name} continuous shelf support")
        slope = _minimum_support_slope(support)
        if slope < 45.0:
            raise ValueError(f"{name} support face slope is below 45 degrees: {slope:.3f}")
        support_metrics.append(f"{name}={slope:.3f}deg")
        pad = shelf_contact_pad(restraint, params)
        shelf = shelf_contact_component(restraint, params)
        _valid_single(pad, f"{name} shelf pad")
        _valid_shell_solid(shelf, f"{name} shelf")
        pad_box = cast(cq.Shape, pad.val()).BoundingBox()
        pad_z_min = restraint.nominal_shelf_base_z - specs["shelf_overlap"]
        pad_bounds = (min(point[0] for point in restraint.shelf_polygon), max(point[0] for point in restraint.shelf_polygon), min(point[1] for point in restraint.shelf_polygon), max(point[1] for point in restraint.shelf_polygon))
        for actual, expected, label in (
            (pad_box.xmin, pad_bounds[0], "xmin"),
            (pad_box.xmax, pad_bounds[1], "xmax"),
            (pad_box.ymin, pad_bounds[2], "ymin"),
            (pad_box.ymax, pad_bounds[3], "ymax"),
            (pad_box.zmin, pad_z_min, "zmin"),
            (pad_box.zmax, restraint.contact_z, "contact_z"),
        ):
            if abs(actual - expected) > 0.05:
                raise ValueError(f"{name}_contact_pad_{label}={actual:.4f}, expected={expected:.4f}")
        top_faces = pad.faces(">Z").vals()
        if len(top_faces) != 1:
            raise ValueError(f"{name} contact pad must have one top contact face")
        top_face = cast(cq.Shape, top_faces[0])
        if top_face.geomType() != "PLANE" or top_face.Area() <= 0.0:
            raise ValueError(f"{name} contact pad has no planar top contact face")
        edge_details = [
            (getattr(edge, "radius")(), cast(cq.Shape, edge).BoundingBox())
            for edge in cast(list[cq.Shape], pad.edges().vals())
            if edge.geomType() == "CIRCLE"
        ]
        plan_edges = sum(abs(radius - specs["shelf_plan_radius"]) <= 0.05 for radius, _ in edge_details)
        top_edges = sum(abs(radius - specs["shelf_top_edge_radius"]) <= 0.05 for radius, _ in edge_details)
        plan_bottom_edges = sum(abs(radius - specs["shelf_plan_radius"]) <= 0.05 and abs(box.zmin - pad_z_min) <= 0.05 for radius, box in edge_details)
        plan_top_edges = sum(abs(radius - specs["shelf_plan_radius"]) <= 0.05 and abs(box.zmin - (restraint.contact_z - specs["shelf_top_edge_radius"])) <= 0.05 for radius, box in edge_details)
        top_transition_edges = sum(abs(radius - specs["shelf_top_edge_radius"]) <= 0.05 and abs(box.zmin - (restraint.contact_z - specs["shelf_top_edge_radius"])) <= 0.05 and abs(box.zmax - restraint.contact_z) <= 0.05 for radius, box in edge_details)
        if (plan_edges, top_edges, plan_bottom_edges, plan_top_edges, top_transition_edges) != (2, 4, 1, 1, 4):
            raise ValueError(f"{name} shelf rounding edges shelf_r1={plan_edges} top_r0.6={top_edges} locations={plan_bottom_edges}/{plan_top_edges}/{top_transition_edges}")
        shelf_top_faces = shelf.faces(">Z").vals()
        if len(shelf_top_faces) != 1:
            raise ValueError(f"{name} shelf must have one top contact face")
        shelf_top_face = cast(cq.Shape, shelf_top_faces[0])
        shelf_top_box = shelf_top_face.BoundingBox()
        if shelf_top_face.geomType() != "PLANE" or shelf_top_face.Area() < (262.0 if name.endswith(("ne", "se")) else 131.0):
            raise ValueError(f"{name} shelf top contact face is too small or non-planar")
        pad_metrics.append(f"{name}=flat_top:{shelf_top_face.Area():.3f}mm2/{shelf_top_box.xmax - shelf_top_box.xmin:.3f}x{shelf_top_box.ymax - shelf_top_box.ymin:.3f}mm shelf_r1_edges:{plan_edges} top_r0.6_edges:{top_edges}")
        bbox = cast(cq.Shape, component.val()).BoundingBox()
        support_bounds = (
            min(point[0] for point in restraint.support_lower_polygon + restraint.support_upper_polygon),
            max(point[0] for point in restraint.support_lower_polygon + restraint.support_upper_polygon),
            min(point[1] for point in restraint.support_lower_polygon + restraint.support_upper_polygon),
            max(point[1] for point in restraint.support_lower_polygon + restraint.support_upper_polygon),
        )
        all_bounds = (support_bounds,) + tuple(restraint.shelf_tabs.values()) + (pad_bounds,)
        for actual, expected, label in (
            (bbox.xmin, min(bounds[0] for bounds in all_bounds), "xmin"),
            (bbox.xmax, max(bounds[1] for bounds in all_bounds), "xmax"),
            (bbox.ymin, min(bounds[2] for bounds in all_bounds), "ymin"),
            (bbox.ymax, max(bounds[3] for bounds in all_bounds), "ymax"),
            (bbox.zmin, specs["support_lower_z"], "zmin"),
            (bbox.zmax, restraint.contact_z, "contact_z"),
        ):
            if abs(actual - expected) > 0.05:
                raise ValueError(f"{name}_{label}={actual:.4f}, expected={expected:.4f}")
        if _intersection_volume(component, deep) <= 0.0:
            raise ValueError(f"{name} is not joined to the upstream tub")
        floor_join = _intersection_volume(component, deep, _prism(0.0, 0.0, specs["support_lower_z"], case_dims.width, case_dims.depth, 0.22))
        if floor_join <= 0.0:
            raise ValueError(f"{name} has no floor overlap")
        outside_volume_value = outside_volume(component, outer_envelope)
        if outside_volume_value > VOLUME_TOLERANCE:
            raise ValueError(f"{name} breaches the upstream exterior envelope by {outside_volume_value:.9f} mm3")
        support = continuous_shelf_support(restraint, params)
        support_section_z = support_section_stations(restraint, params)
        for z in support_section_z:
            difference = section_symmetric_difference_volume(component, support, z, case_dims.width, case_dims.depth)
            if difference > VOLUME_TOLERANCE:
                raise ValueError(f"{name} has a non-support ridge at Z{z:.5f}: symmetric difference={difference:.9e} mm3")
            no_ridge_differences.append(difference)
        section_z = support_section_z + (restraint.nominal_shelf_base_z, restraint.contact_z - 0.01)
        outside_sections = []
        for z in section_z:
            section = component.intersect(_prism(0.0, 0.0, z - 0.005, case_dims.width, case_dims.depth, 0.01))
            _valid_single(section, f"{name} horizontal section Z{z:.5f}")
            area = outside_section_area(component, outer_envelope, z, case_dims.width, case_dims.depth)
            if area > VOLUME_TOLERANCE:
                raise ValueError(f"{name} breaches the upstream exterior envelope at Z{z:.5f} by {area:.9f} mm2")
            outside_sections.append(area)
        shelf_z_min = restraint.nominal_shelf_base_z - specs["shelf_overlap"]
        for side in adjacent_sides[restraint.name]:
            tab_bounds = restraint.shelf_tabs[side]
            tab = _solid(tab_bounds, shelf_z_min, restraint.contact_z)
            if any(edge.geomType() == "CIRCLE" for edge in cast(list[cq.Shape], tab.edges().vals())):
                raise ValueError(f"{name} {side} wall tab has rounded edges")
            plane = wall_planes(params)[{"west": "west_x", "east": "east_x", "south": "south_y", "north": "north_y"}[side]]
            wall_gap = 0.0 if (tab_bounds[0] <= plane <= tab_bounds[1] if side in ("west", "east") else tab_bounds[2] <= plane <= tab_bounds[3]) else 1.0
            if wall_gap != 0.0:
                raise ValueError(f"{name} {side} shelf wall gap is not zero")
            wall_overlap = plane - tab_bounds[0] if side == "west" else tab_bounds[1] - plane if side == "east" else plane - tab_bounds[2] if side == "south" else tab_bounds[3] - plane
            if abs(wall_overlap - 0.58) > 0.05:
                raise ValueError(f"{name} {side} shelf wall overlap changed: {wall_overlap:.3f} mm")
            wall_gap_metrics.append(f"{name}/{side}=0.00mm")
            for z in (restraint.nominal_shelf_base_z, (restraint.nominal_shelf_base_z + restraint.contact_z) / 2.0, restraint.contact_z - 0.6, restraint.contact_z - 0.01):
                tab_slice = tab.intersect(_prism(0.0, 0.0, z - 0.005, case_dims.width, case_dims.depth, 0.01))
                if _volume(tab_slice.cut(component)) > VOLUME_TOLERANCE:
                    raise ValueError(f"{name} {side} shelf tab has a modeled gap at Z{z:.2f}")
            witness = wall_witness(params, restraint, side)
            witness_volume = _intersection_volume(component, deep, witness)
            if witness_volume < specs["wall_intersection_volume_min"]:
                raise ValueError(f"{name} {side} wall intersection is below 30 mm3")
            section = component.intersect(deep).intersect(witness).intersect(_prism(0.0, 0.0, 3.975, case_dims.width, case_dims.depth, 0.05))
            _valid_single(section, f"{name} {side} Z4 wall section")
            section_box = cast(cq.Shape, section.val()).BoundingBox()
            length = section_box.ymax - section_box.ymin if side in ("west", "east") else section_box.xmax - section_box.xmin
            if length < specs["support_contact_length_min"]:
                raise ValueError(f"{name} {side} Z4 support contact is below 3 mm")
            wall_metrics.append(f"{name}/{side}={witness_volume:.3f}mm3/{length:.3f}mm")
        keepout_collision = next(
            (
                keepout
                for keepout in keepouts
                if _intersection_volume(component, _prism(keepout.bounds[0], keepout.bounds[1], 0.0, keepout.bounds[2] - keepout.bounds[0], keepout.bounds[3] - keepout.bounds[1], case_dims.height)) > VOLUME_TOLERANCE
            ),
            None,
        )
        if keepout_collision is not None:
            raise ValueError(f"{name} intersects {keepout_collision.name}")
        containment_metrics.append(f"{name}=outside:{outside_volume_value:.3e}mm3 sections:{max(outside_sections):.3e}mm2 floor:{floor_join:.3f}mm3")
    final_additions = final_tub.cut(deep)
    for restraint, (name, component) in zip(corner_restraints(params), corner_restraint_components(params)):
        support = continuous_shelf_support(restraint, params)
        component_box = cast(cq.Shape, component.val()).BoundingBox()
        local_probe = _prism(
            component_box.xmin - 0.01,
            component_box.ymin - 0.01,
            0.0,
            component_box.xmax - component_box.xmin + 0.02,
            component_box.ymax - component_box.ymin + 0.02,
            case_dims.height,
        )
        expected_additions = support.cut(deep)
        for z in support_section_stations(restraint, params):
            difference = section_symmetric_difference_volume(
                final_additions.intersect(local_probe),
                expected_additions.intersect(local_probe),
                z,
                case_dims.width,
                case_dims.depth,
            )
            if difference > VOLUME_TOLERANCE:
                raise ValueError(f"{name} final tub has a support-plus-strip appendage at Z{z:.5f}: symmetric difference={difference:.9e} mm3")
            final_addition_differences.append(difference)
    _valid_shell_solid(final_tub, "deep tub with corner restraints")
    underside_faces = _downward_horizontal_faces(final_tub)
    if len(underside_faces) != 4:
        raise ValueError(f"expected four permitted horizontal underside faces, found {len(underside_faces)}")
    if any(area > 0.286 or width > 0.30 for area, width in underside_faces):
        raise ValueError(f"horizontal underside exceeds area/width limits: {underside_faces}")
    print(f"support_slopes=PASS metrics={','.join(support_metrics)}")
    print(f"undersides=PASS permitted={len(underside_faces)} max_area={max(area for area, _ in underside_faces):.6f}mm2 max_width={max(width for _, width in underside_faces):.6f}mm")
    print(f"contact_pads=PASS metrics={';'.join(pad_metrics)}")
    print(f"shelf_wall_gaps=PASS metrics={','.join(wall_gap_metrics)}")
    print(f"no_ridge_sections=PASS isolated_max_symmetric_difference={max(no_ridge_differences):.3e}mm3 final_additions_max_symmetric_difference={max(final_addition_differences):.3e}mm3")
    print(f"corner_restraints=4 containment=PASS metrics={';'.join(containment_metrics)}")
    print(f"corner_contacts=floor_join=PASS both_wall_witnesses=PASS control_keepouts=NONE final_solid=PASS metrics={','.join(wall_metrics)}")
