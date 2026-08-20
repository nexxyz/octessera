from __future__ import annotations

import cadquery as cq

from top_wall_port_geometry import (
    PORT_CUT_EPS,
    PORT_FACE_RECESS_SPAN_PAD,
    PORT_FACE_RECESS_Z_PAD,
    PORT_RECESS_BACK_LAND,
    WEST_EXTENSION,
    make_horizontal_wall_port_profile_wire,
    make_west_wall_port_profile_wire,
    wall_port_z_bounds,
)
from top_wall_port_indent_geometry import quarter_circle_ease


def make_left_wall_face_recess(
    params: dict,
    y0: float,
    y1: float,
    height: float,
    depth_x: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    center_y = (y0 + y1) / 2.0
    center_z = (z0 + z1) / 2.0
    half_y_inner = (y1 - y0) / 2.0 + PORT_RECESS_BACK_LAND + half_span_adjust
    half_z_inner = (z1 - z0) / 2.0
    half_y_outer = half_y_inner + PORT_FACE_RECESS_SPAN_PAD
    half_z_outer = half_z_inner + PORT_FACE_RECESS_Z_PAD
    wires = []
    start_x = -WEST_EXTENSION - PORT_CUT_EPS
    for index in range(17):
        x = start_x + (depth_x - start_x) * index / 16
        t = (x - start_x) / (depth_x - start_x)
        ease = quarter_circle_ease(t)
        half_y = half_y_outer + (half_y_inner - half_y_outer) * ease
        half_z = half_z_outer
        wires.append(
            make_west_wall_port_profile_wire(x, center_y - half_y, center_y + half_y, center_z - half_z, center_z + half_z)
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_south_wall_face_recess(
    params: dict,
    x0: float,
    x1: float,
    height: float,
    depth_y: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    center_x = (x0 + x1) / 2.0
    center_z = (z0 + z1) / 2.0
    half_x_inner = (x1 - x0) / 2.0 + PORT_RECESS_BACK_LAND + half_span_adjust
    half_z_inner = (z1 - z0) / 2.0
    half_x_outer = half_x_inner + PORT_FACE_RECESS_SPAN_PAD
    half_z_outer = half_z_inner + PORT_FACE_RECESS_Z_PAD
    wires = []
    for index in range(17):
        y = -PORT_CUT_EPS + (depth_y + PORT_CUT_EPS) * index / 16
        t = (y + PORT_CUT_EPS) / (depth_y + PORT_CUT_EPS)
        ease = quarter_circle_ease(t)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * ease
        half_z = half_z_outer
        wires.append(
            make_horizontal_wall_port_profile_wire(center_x - half_x, center_x + half_x, y, center_z - half_z, center_z + half_z)
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def make_north_wall_face_recess(
    params: dict,
    x0: float,
    x1: float,
    height: float,
    depth_y: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    _, case_depth = params["case_size_v21"]
    z0, z1 = wall_port_z_bounds(height, z_shift=z_shift)
    center_x = (x0 + x1) / 2.0
    center_z = (z0 + z1) / 2.0
    half_x_inner = (x1 - x0) / 2.0 + PORT_RECESS_BACK_LAND + half_span_adjust
    half_z_inner = (z1 - z0) / 2.0
    half_x_outer = half_x_inner + PORT_FACE_RECESS_SPAN_PAD
    half_z_outer = half_z_inner + PORT_FACE_RECESS_Z_PAD
    inner_y = depth_y
    wires = []
    for index in range(17):
        y = case_depth + PORT_CUT_EPS - (case_depth + PORT_CUT_EPS - inner_y) * index / 16
        t = (case_depth + PORT_CUT_EPS - y) / (case_depth + PORT_CUT_EPS - inner_y)
        ease = quarter_circle_ease(t)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * ease
        half_z = half_z_outer
        wires.append(
            make_horizontal_wall_port_profile_wire(center_x - half_x, center_x + half_x, y, center_z - half_z, center_z + half_z)
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()
