from __future__ import annotations

import cadquery as cq

from top_wall_port_geometry import (
    PORT_CUT_EPS,
    PORT_FACE_RECESS_SPAN_PAD,
    PORT_INDENT_RAMP,
    PORT_INDENT_SPAN_PAD,
    PORT_INDENT_WALL_OVERLAP,
    PORT_INDENT_WALL_PROFILE_EXTRA,
    PORT_INDENT_Z_PAD,
    PORT_RECESS_BACK_LAND,
    WEST_EXTENSION,
    WEST_PORT_INDENT_BACK_OVERLAP,
    make_horizontal_wall_port_profile_wire,
    make_west_wall_port_profile_wire,
    wall_port_z_bounds,
    wall_port_z_center,
)


def quarter_circle_ease(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return 1.0 - (1.0 - t * t) ** 0.5


def make_left_wall_indent_wall(
    params: dict,
    y0: float,
    y1: float,
    height: float,
    target_x: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    _, depth = params["case_size_v21"]
    wall = params["wall"]
    z0, z1 = wall_port_z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    center_y = (y0 + y1) / 2.0
    half_y_inner = (y1 - y0) / 2.0 + PORT_RECESS_BACK_LAND + PORT_INDENT_WALL_PROFILE_EXTRA + half_span_adjust
    half_y_outer = half_y_inner + PORT_FACE_RECESS_SPAN_PAD
    safe0 = params["corner_r"] + 0.5
    safe1 = depth - params["corner_r"] - 0.5
    wall_overlap = wall - WEST_EXTENSION - PORT_INDENT_WALL_OVERLAP
    target_with_overlap = target_x + WEST_PORT_INDENT_BACK_OVERLAP
    wires = []
    for index in range(17):
        x = wall_overlap + (target_with_overlap - wall_overlap) * index / 16
        t = (x - wall_overlap) / (target_x - wall_overlap)
        half_y = half_y_outer + (half_y_inner - half_y_outer) * quarter_circle_ease(t)
        y_min = max(center_y - half_y, safe0)
        y_max = min(center_y + half_y, safe1)
        wires.append(make_west_wall_port_profile_wire(x, y_min, y_max, z0, z1))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_south_wall_indent_wall(
    params: dict,
    x0: float,
    x1: float,
    height: float,
    target_y: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    width, _ = params["case_size_v21"]
    wall = params["wall"]
    z0, z1 = wall_port_z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    center_x = (x0 + x1) / 2.0
    half_x_inner = (x1 - x0) / 2.0 + PORT_RECESS_BACK_LAND + PORT_INDENT_WALL_PROFILE_EXTRA + half_span_adjust
    half_x_outer = half_x_inner + PORT_FACE_RECESS_SPAN_PAD
    safe0 = params["corner_r"] + 0.5
    safe1 = width - params["corner_r"] - 0.5
    wall_overlap = wall - PORT_INDENT_WALL_OVERLAP
    wires = []
    for index in range(17):
        y = wall_overlap + (target_y - wall_overlap) * index / 16
        t = (y - wall_overlap) / (target_y - wall_overlap)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * quarter_circle_ease(t)
        x_min = max(center_x - half_x, safe0)
        x_max = min(center_x + half_x, safe1)
        wires.append(make_horizontal_wall_port_profile_wire(x_min, x_max, y, z0, z1))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_north_wall_indent_wall(
    params: dict,
    x0: float,
    x1: float,
    height: float,
    target_y: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    inner_y = depth - params["wall"]
    z0, z1 = wall_port_z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    center_x = (x0 + x1) / 2.0
    half_x_inner = (x1 - x0) / 2.0 + PORT_RECESS_BACK_LAND + PORT_INDENT_WALL_PROFILE_EXTRA + half_span_adjust
    half_x_outer = half_x_inner + PORT_FACE_RECESS_SPAN_PAD
    safe0 = params["corner_r"] + 0.5
    safe1 = width - params["corner_r"] - 0.5
    inner_overlap_y = inner_y + PORT_INDENT_WALL_OVERLAP
    wires = []
    for index in range(17):
        y = inner_overlap_y + (target_y - inner_overlap_y) * index / 16
        t = (inner_overlap_y - y) / (inner_overlap_y - target_y)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * quarter_circle_ease(t)
        x_min = max(center_x - half_x, safe0)
        x_max = min(center_x + half_x, safe1)
        wires.append(make_horizontal_wall_port_profile_wire(x_min, x_max, y, z0, z1))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_audio_jack_port_cutter(params: dict, y: float, x1: float, z_shift: float) -> cq.Workplane:
    height = 8.2
    return (
        cq.Workplane("YZ")
        .circle(3.35)
        .extrude(x1 + WEST_EXTENSION + 2 * PORT_CUT_EPS)
        .translate((-WEST_EXTENSION - PORT_CUT_EPS, y, wall_port_z_center(height, z_shift)))
    )


def make_left_wall_indent(
    params: dict,
    y0: float,
    y1: float,
    height: float,
    target_x: float,
    south_trim: float = 0.0,
    z_shift: float = 0.0,
) -> cq.Workplane:
    wall = params["wall"]
    wall_overlap = wall - PORT_INDENT_WALL_OVERLAP
    z0, z1 = wall_port_z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    start = y0 - PORT_INDENT_SPAN_PAD - PORT_INDENT_RAMP + south_trim
    end = y1 + PORT_INDENT_SPAN_PAD + PORT_INDENT_RAMP
    stations = []
    for index in range(25):
        y = start + (end - start) * index / 24
        if y < y0 - PORT_INDENT_SPAN_PAD:
            t = (y - start) / PORT_INDENT_RAMP
            x = wall + (target_x - wall) * quarter_circle_ease(t)
        elif y > y1 + PORT_INDENT_SPAN_PAD:
            t = (end - y) / PORT_INDENT_RAMP
            x = wall + (target_x - wall) * quarter_circle_ease(t)
        else:
            x = target_x
        if x > wall_overlap + 0.05:
            stations.append((y, x))
    wires = [
        cq.Wire.makePolygon(
            [
                cq.Vector(wall_overlap, y, z0),
                cq.Vector(x, y, z0),
                cq.Vector(x, y, z1),
                cq.Vector(wall_overlap, y, z1),
                cq.Vector(wall_overlap, y, z0),
            ]
        )
        for y, x in stations
    ]
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_south_wall_indent(params: dict, x0: float, x1: float, height: float, target_y: float, z_shift: float = 0.0) -> cq.Workplane:
    wall = params["wall"]
    wall_overlap = wall - PORT_INDENT_WALL_OVERLAP
    z0, z1 = wall_port_z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    start = x0 - PORT_INDENT_SPAN_PAD - PORT_INDENT_RAMP
    end = x1 + PORT_INDENT_SPAN_PAD + PORT_INDENT_RAMP
    stations = []
    for index in range(25):
        x = start + (end - start) * index / 24
        if x < x0 - PORT_INDENT_SPAN_PAD:
            t = (x - start) / PORT_INDENT_RAMP
            y = wall + (target_y - wall) * quarter_circle_ease(t)
        elif x > x1 + PORT_INDENT_SPAN_PAD:
            t = (end - x) / PORT_INDENT_RAMP
            y = wall + (target_y - wall) * quarter_circle_ease(t)
        else:
            y = target_y
        if y > wall_overlap + 0.05:
            stations.append((x, y))
    wires = [
        cq.Wire.makePolygon(
            [
                cq.Vector(x, wall_overlap, z0),
                cq.Vector(x, y, z0),
                cq.Vector(x, y, z1),
                cq.Vector(x, wall_overlap, z1),
                cq.Vector(x, wall_overlap, z0),
            ]
        )
        for x, y in stations
    ]
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_north_wall_indent(params: dict, x0: float, x1: float, height: float, target_y: float, z_shift: float = 0.0) -> cq.Workplane:
    _, depth = params["case_size_v21"]
    inner_y = depth - params["wall"]
    inner_overlap_y = inner_y + PORT_INDENT_WALL_OVERLAP
    z0, z1 = wall_port_z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    start = x0 - PORT_INDENT_SPAN_PAD - PORT_INDENT_RAMP
    end = x1 + PORT_INDENT_SPAN_PAD + PORT_INDENT_RAMP
    stations = []
    for index in range(25):
        x = start + (end - start) * index / 24
        if x < x0 - PORT_INDENT_SPAN_PAD:
            t = (x - start) / PORT_INDENT_RAMP
            y = inner_y - (inner_y - target_y) * quarter_circle_ease(t)
        elif x > x1 + PORT_INDENT_SPAN_PAD:
            t = (end - x) / PORT_INDENT_RAMP
            y = inner_y - (inner_y - target_y) * quarter_circle_ease(t)
        else:
            y = target_y
        if y < inner_overlap_y - 0.05:
            stations.append((x, y))
    wires = [
        cq.Wire.makePolygon(
            [
                cq.Vector(x, y, z0),
                cq.Vector(x, inner_overlap_y, z0),
                cq.Vector(x, inner_overlap_y, z1),
                cq.Vector(x, y, z1),
                cq.Vector(x, y, z0),
            ]
        )
        for x, y in stations
    ]
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()
