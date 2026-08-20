from __future__ import annotations

import math

import cadquery as cq


WALL_PORT_TOP_Z = 7.5
PORT_CUT_EPS = 1.0
WEST_EXTENSION = 1.0
PORT_INDENT_RAMP = 6.0
PORT_INDENT_Z_PAD = 4.2
PORT_INDENT_SPAN_PAD = 6.0
PORT_INDENT_WALL_SPAN_PAD = 1.0
PORT_INDENT_WALL_PROFILE_EXTRA = 0.8
PORT_FACE_RECESS_SPAN_PAD = 5.0
PORT_FACE_RECESS_Z_PAD = 3.0
PORT_RECESS_BACK_LAND = 1.0
PORT_RECESS_VERTICAL_LAND = 1.2
PORT_INDENT_WALL_OVERLAP = 0.4
WEST_PORT_INDENT_BACK_OVERLAP = 0.5
PORT_INDENT_CORNER_R = 1.2
PORT_HOLE_CORNER_R = 0.55
ROUNDED_CORNER_STEPS = 4


def wall_port_z_bounds(height: float, pad: float = 0.0, z_shift: float = 0.0) -> tuple[float, float]:
    return WALL_PORT_TOP_Z - height - pad + z_shift, WALL_PORT_TOP_Z + pad + z_shift


def wall_port_z_center(height: float, z_shift: float = 0.0) -> float:
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    return (z0 + z1) / 2.0


def centered_indent_z_shift(hole_height: float, hole_z_shift: float, indent_height: float) -> float:
    return wall_port_z_center(hole_height, hole_z_shift) - wall_port_z_center(indent_height)


def make_left_wall_port_hole(
    params: dict, y0: float, y1: float, height: float, x1: float | None = None, z_shift: float = 0.0
) -> cq.Workplane:
    wall = params["wall"]
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    x_min = -WEST_EXTENSION - PORT_CUT_EPS
    x_max = (x1 or wall) + PORT_CUT_EPS
    return (
        cq.Workplane("XY")
        .add(
            cq.Solid.makeLoft(
                [
                    make_west_wall_port_profile_wire(x_min, y0, y1, z0, z1, PORT_HOLE_CORNER_R),
                    make_west_wall_port_profile_wire(x_max, y0, y1, z0, z1, PORT_HOLE_CORNER_R),
                ],
                ruled=True,
            )
        )
        .clean()
    )


def make_south_wall_port_hole(
    params: dict, x0: float, x1: float, height: float, y1: float | None = None, z_shift: float = 0.0
) -> cq.Workplane:
    wall = params["wall"]
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    y_min = -PORT_CUT_EPS
    y_max = (y1 or wall) + PORT_CUT_EPS
    return (
        cq.Workplane("XY")
        .add(
            cq.Solid.makeLoft(
                [
                    make_horizontal_wall_port_profile_wire(x0, x1, y_min, z0, z1, PORT_HOLE_CORNER_R),
                    make_horizontal_wall_port_profile_wire(x0, x1, y_max, z0, z1, PORT_HOLE_CORNER_R),
                ],
                ruled=True,
            )
        )
        .clean()
    )


def make_north_wall_port_hole(
    params: dict, x0: float, x1: float, height: float, y0: float | None = None, z_shift: float = 0.0
) -> cq.Workplane:
    _, depth = params["case_size_v21"]
    wall = params["wall"]
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    y_min = (y0 or depth - wall) - PORT_CUT_EPS
    y_max = depth + PORT_CUT_EPS
    return (
        cq.Workplane("XY")
        .add(
            cq.Solid.makeLoft(
                [
                    make_horizontal_wall_port_profile_wire(x0, x1, y_min, z0, z1, PORT_HOLE_CORNER_R),
                    make_horizontal_wall_port_profile_wire(x0, x1, y_max, z0, z1, PORT_HOLE_CORNER_R),
                ],
                ruled=True,
            )
        )
        .clean()
    )


def rounded_wall_port_profile_points(
    u0: float, u1: float, z0: float, z1: float, radius: float = PORT_INDENT_CORNER_R
) -> list[tuple[float, float]]:
    half_u = (u1 - u0) / 2.0
    half_z = (z1 - z0) / 2.0
    r = min(radius, half_u - 0.05, half_z - 0.05)
    if r <= 0.05:
        return [(u0, z0), (u1, z0), (u1, z1), (u0, z1), (u0, z0)]

    corners = [
        (u1 - r, z0 + r, -90.0, 0.0),
        (u1 - r, z1 - r, 0.0, 90.0),
        (u0 + r, z1 - r, 90.0, 180.0),
        (u0 + r, z0 + r, 180.0, 270.0),
    ]
    points: list[tuple[float, float]] = []
    for cx, cz, a0, a1 in corners:
        for step in range(ROUNDED_CORNER_STEPS + 1):
            angle = math.radians(a0 + (a1 - a0) * step / ROUNDED_CORNER_STEPS)
            points.append((cx + math.cos(angle) * r, cz + math.sin(angle) * r))
    points.append(points[0])
    return points


def make_west_wall_port_profile_wire(
    x: float, y0: float, y1: float, z0: float, z1: float, radius: float = PORT_INDENT_CORNER_R
) -> cq.Wire:
    return cq.Wire.makePolygon([cq.Vector(x, y, z) for y, z in rounded_wall_port_profile_points(y0, y1, z0, z1, radius)])


def make_horizontal_wall_port_profile_wire(
    x0: float, x1: float, y: float, z0: float, z1: float, radius: float = PORT_INDENT_CORNER_R
) -> cq.Wire:
    return cq.Wire.makePolygon([cq.Vector(x, y, z) for x, z in rounded_wall_port_profile_points(x0, x1, z0, z1, radius)])
