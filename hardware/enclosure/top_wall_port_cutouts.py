from __future__ import annotations

import cadquery as cq


WALL_PORT_TOP_Z = 7.5
PORT_CUT_EPS = 1.0
OLED_SD_X0 = 57.88
OLED_SD_X1 = 78.88
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


def box_cutter(x0: float, y0: float, x1: float, y1: float, z0: float, z1: float) -> cq.Workplane:
    return (
        cq.Workplane("XY")
        .rect(x1 - x0, y1 - y0)
        .extrude(z1 - z0)
        .translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))
    )


def quarter_ease(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return 1.0 - (1.0 - t * t) ** 0.5


def z_bounds(height: float, pad: float = 0.0, z_shift: float = 0.0) -> tuple[float, float]:
    return WALL_PORT_TOP_Z - height - pad + z_shift, WALL_PORT_TOP_Z + pad + z_shift


def z_center(height: float, z_shift: float = 0.0) -> float:
    z0, z1 = z_bounds(height, z_shift=z_shift)
    return (z0 + z1) / 2.0


def left_wall_rect(params: dict, y0: float, y1: float, height: float, x1: float | None = None, z_shift: float = 0.0) -> cq.Workplane:
    wall = params["wall"]
    z0, z1 = z_bounds(height, z_shift=z_shift)
    return box_cutter(-PORT_CUT_EPS, y0, (x1 or wall) + PORT_CUT_EPS, y1, z0, z1)


def south_wall_rect(params: dict, x0: float, x1: float, height: float, y1: float | None = None, z_shift: float = 0.0) -> cq.Workplane:
    wall = params["wall"]
    z0, z1 = z_bounds(height, z_shift=z_shift)
    return box_cutter(x0, -PORT_CUT_EPS, x1, (y1 or wall) + PORT_CUT_EPS, z0, z1)


def north_wall_rect(params: dict, x0: float, x1: float, height: float, y0: float | None = None, z_shift: float = 0.0) -> cq.Workplane:
    _, depth = params["case_size_v21"]
    wall = params["wall"]
    z0, z1 = z_bounds(height, z_shift=z_shift)
    return box_cutter(x0, (y0 or depth - wall) - PORT_CUT_EPS, x1, depth + PORT_CUT_EPS, z0, z1)


def safe_span(start: float, end: float, low: float, high: float) -> tuple[float, float]:
    return max(start, low), min(end, high)


def left_wall_indent_wall(
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
    z0, z1 = z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    center_y = (y0 + y1) / 2.0
    half_y_inner = (
        (y1 - y0) / 2.0
        + PORT_RECESS_BACK_LAND
        + PORT_INDENT_WALL_PROFILE_EXTRA
        + half_span_adjust
    )
    half_y_outer = half_y_inner + PORT_FACE_RECESS_SPAN_PAD
    safe0 = params["corner_r"] + 0.5
    safe1 = depth - params["corner_r"] - 0.5
    wall_overlap = wall - PORT_INDENT_WALL_OVERLAP
    wires = []
    for index in range(17):
        x = wall_overlap + (target_x - wall_overlap) * index / 16
        t = (x - wall_overlap) / (target_x - wall_overlap)
        half_y = half_y_outer + (half_y_inner - half_y_outer) * quarter_ease(t)
        y_min = max(center_y - half_y, safe0)
        y_max = min(center_y + half_y, safe1)
        wires.append(cq.Wire.makePolygon([
            cq.Vector(x, y_min, z0),
            cq.Vector(x, y_max, z0),
            cq.Vector(x, y_max, z1),
            cq.Vector(x, y_min, z1),
            cq.Vector(x, y_min, z0),
        ]))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def south_wall_indent_wall(
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
    z0, z1 = z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    center_x = (x0 + x1) / 2.0
    half_x_inner = (
        (x1 - x0) / 2.0
        + PORT_RECESS_BACK_LAND
        + PORT_INDENT_WALL_PROFILE_EXTRA
        + half_span_adjust
    )
    half_x_outer = half_x_inner + PORT_FACE_RECESS_SPAN_PAD
    safe0 = params["corner_r"] + 0.5
    safe1 = width - params["corner_r"] - 0.5
    wall_overlap = wall - PORT_INDENT_WALL_OVERLAP
    wires = []
    for index in range(17):
        y = wall_overlap + (target_y - wall_overlap) * index / 16
        t = (y - wall_overlap) / (target_y - wall_overlap)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * quarter_ease(t)
        x_min = max(center_x - half_x, safe0)
        x_max = min(center_x + half_x, safe1)
        wires.append(cq.Wire.makePolygon([
            cq.Vector(x_min, y, z0),
            cq.Vector(x_max, y, z0),
            cq.Vector(x_max, y, z1),
            cq.Vector(x_min, y, z1),
            cq.Vector(x_min, y, z0),
        ]))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def north_wall_indent_wall(
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
    z0, z1 = z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    center_x = (x0 + x1) / 2.0
    half_x_inner = (
        (x1 - x0) / 2.0
        + PORT_RECESS_BACK_LAND
        + PORT_INDENT_WALL_PROFILE_EXTRA
        + half_span_adjust
    )
    half_x_outer = half_x_inner + PORT_FACE_RECESS_SPAN_PAD
    safe0 = params["corner_r"] + 0.5
    safe1 = width - params["corner_r"] - 0.5
    inner_overlap_y = inner_y + PORT_INDENT_WALL_OVERLAP
    wires = []
    for index in range(17):
        y = inner_overlap_y + (target_y - inner_overlap_y) * index / 16
        t = (inner_overlap_y - y) / (inner_overlap_y - target_y)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * quarter_ease(t)
        x_min = max(center_x - half_x, safe0)
        x_max = min(center_x + half_x, safe1)
        wires.append(cq.Wire.makePolygon([
            cq.Vector(x_min, y, z0),
            cq.Vector(x_max, y, z0),
            cq.Vector(x_max, y, z1),
            cq.Vector(x_min, y, z1),
            cq.Vector(x_min, y, z0),
        ]))
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def audio_jack_cutter(params: dict, y: float, x1: float) -> cq.Workplane:
    height = 8.2
    return (
        cq.Workplane("YZ")
        .circle(3.35)
        .extrude(x1 + 2 * PORT_CUT_EPS)
        .translate((-PORT_CUT_EPS, y, z_center(height)))
    )


def left_wall_indent(
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
    z0, z1 = z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    start = y0 - PORT_INDENT_SPAN_PAD - PORT_INDENT_RAMP + south_trim
    end = y1 + PORT_INDENT_SPAN_PAD + PORT_INDENT_RAMP
    stations = []
    for index in range(25):
        y = start + (end - start) * index / 24
        if y < y0 - PORT_INDENT_SPAN_PAD:
            t = (y - start) / PORT_INDENT_RAMP
            x = wall + (target_x - wall) * quarter_ease(t)
        elif y > y1 + PORT_INDENT_SPAN_PAD:
            t = (end - y) / PORT_INDENT_RAMP
            x = wall + (target_x - wall) * quarter_ease(t)
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


def south_wall_indent(params: dict, x0: float, x1: float, height: float, target_y: float, z_shift: float = 0.0) -> cq.Workplane:
    wall = params["wall"]
    wall_overlap = wall - PORT_INDENT_WALL_OVERLAP
    z0, z1 = z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    start = x0 - PORT_INDENT_SPAN_PAD - PORT_INDENT_RAMP
    end = x1 + PORT_INDENT_SPAN_PAD + PORT_INDENT_RAMP
    stations = []
    for index in range(25):
        x = start + (end - start) * index / 24
        if x < x0 - PORT_INDENT_SPAN_PAD:
            t = (x - start) / PORT_INDENT_RAMP
            y = wall + (target_y - wall) * quarter_ease(t)
        elif x > x1 + PORT_INDENT_SPAN_PAD:
            t = (end - x) / PORT_INDENT_RAMP
            y = wall + (target_y - wall) * quarter_ease(t)
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


def north_wall_indent(params: dict, x0: float, x1: float, height: float, target_y: float, z_shift: float = 0.0) -> cq.Workplane:
    _, depth = params["case_size_v21"]
    inner_y = depth - params["wall"]
    inner_overlap_y = inner_y + PORT_INDENT_WALL_OVERLAP
    z0, z1 = z_bounds(height, PORT_INDENT_Z_PAD, z_shift)
    start = x0 - PORT_INDENT_SPAN_PAD - PORT_INDENT_RAMP
    end = x1 + PORT_INDENT_SPAN_PAD + PORT_INDENT_RAMP
    stations = []
    for index in range(25):
        x = start + (end - start) * index / 24
        if x < x0 - PORT_INDENT_SPAN_PAD:
            t = (x - start) / PORT_INDENT_RAMP
            y = inner_y - (inner_y - target_y) * quarter_ease(t)
        elif x > x1 + PORT_INDENT_SPAN_PAD:
            t = (end - x) / PORT_INDENT_RAMP
            y = inner_y - (inner_y - target_y) * quarter_ease(t)
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


def left_wall_face_recess(
    params: dict,
    y0: float,
    y1: float,
    height: float,
    depth_x: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    z0, z1 = z_bounds(height, z_shift=z_shift)
    center_y = (y0 + y1) / 2.0
    center_z = (z0 + z1) / 2.0
    half_y_inner = (y1 - y0) / 2.0 + PORT_RECESS_BACK_LAND + half_span_adjust
    half_z_inner = (z1 - z0) / 2.0
    half_y_outer = half_y_inner + PORT_FACE_RECESS_SPAN_PAD
    half_z_outer = half_z_inner + PORT_FACE_RECESS_Z_PAD
    wires = []
    for index in range(17):
        x = -PORT_CUT_EPS + (depth_x + PORT_CUT_EPS) * index / 16
        t = (x + PORT_CUT_EPS) / (depth_x + PORT_CUT_EPS)
        ease = quarter_ease(t)
        half_y = half_y_outer + (half_y_inner - half_y_outer) * ease
        half_z = half_z_outer
        wires.append(
            cq.Wire.makePolygon(
                [
                    cq.Vector(x, center_y - half_y, center_z - half_z),
                    cq.Vector(x, center_y + half_y, center_z - half_z),
                    cq.Vector(x, center_y + half_y, center_z + half_z),
                    cq.Vector(x, center_y - half_y, center_z + half_z),
                    cq.Vector(x, center_y - half_y, center_z - half_z),
                ]
            )
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def south_wall_face_recess(
    params: dict,
    x0: float,
    x1: float,
    height: float,
    depth_y: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    z0, z1 = z_bounds(height, z_shift=z_shift)
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
        ease = quarter_ease(t)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * ease
        half_z = half_z_outer
        wires.append(
            cq.Wire.makePolygon(
                [
                    cq.Vector(center_x - half_x, y, center_z - half_z),
                    cq.Vector(center_x + half_x, y, center_z - half_z),
                    cq.Vector(center_x + half_x, y, center_z + half_z),
                    cq.Vector(center_x - half_x, y, center_z + half_z),
                    cq.Vector(center_x - half_x, y, center_z - half_z),
                ]
            )
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def north_wall_face_recess(
    params: dict,
    x0: float,
    x1: float,
    height: float,
    depth_y: float,
    z_shift: float = 0.0,
    half_span_adjust: float = 0.0,
) -> cq.Workplane:
    _, case_depth = params["case_size_v21"]
    z0, z1 = z_bounds(height, z_shift=z_shift)
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
        ease = quarter_ease(t)
        half_x = half_x_outer + (half_x_inner - half_x_outer) * ease
        half_z = half_z_outer
        wires.append(
            cq.Wire.makePolygon(
                [
                    cq.Vector(center_x - half_x, y, center_z - half_z),
                    cq.Vector(center_x + half_x, y, center_z - half_z),
                    cq.Vector(center_x + half_x, y, center_z + half_z),
                    cq.Vector(center_x - half_x, y, center_z + half_z),
                    cq.Vector(center_x - half_x, y, center_z - half_z),
                ]
            )
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True)).clean()


def add_top_wall_port_cutouts(model: cq.Workplane, params: dict) -> cq.Workplane:
    pcb_x0 = params["offset_v21"][0]
    pcb_y0 = params["offset_v21"][1]
    pcb_y1 = pcb_y0 + params["pcb_size"][1]
    left_flush_x = pcb_x0
    left_pi_x = pcb_x0 + 0.5
    south_pi_y = pcb_y0 - 0.5
    north_flush_y = pcb_y1
    left_flush_recess_x = left_flush_x - PORT_RECESS_BACK_LAND
    left_pi_recess_x = left_pi_x - PORT_RECESS_BACK_LAND
    south_pi_recess_y = south_pi_y - PORT_RECESS_BACK_LAND
    north_flush_recess_y = north_flush_y + PORT_RECESS_BACK_LAND
    power_z_shift = -0.5
    pi_z_shift = 7.0
    pi_top_trim_z_shift = pi_z_shift - 1.5
    pi_south_indent_z_shift = pi_top_trim_z_shift - 0.5
    pi_sd_hole_z_shift = pi_z_shift - 4.0
    pi_sd_indent_z_shift = pi_sd_hole_z_shift
    pi_sd_indent_span_adjust = -1.5
    additions = []
    cuts = []
    for port in params["ports_v21"]:
        label = port["label"]
        if label == "audio 3.5mm":
            center_y = (port["a"] + port["b"]) / 2.0
            audio_indent_y0 = center_y - 4.1
            audio_indent_y1 = center_y + 4.1
            additions.append(left_wall_indent_wall(params, audio_indent_y0, audio_indent_y1, 6.7, left_flush_x))
            cuts.append(left_wall_face_recess(params, audio_indent_y0, audio_indent_y1, 6.7, left_flush_recess_x))
            cuts.append(audio_jack_cutter(params, (port["a"] + port["b"]) / 2.0, left_flush_x))
        elif label == "USB-C power":
            additions.append(left_wall_indent_wall(params, port["a"], port["b"], 5.6, left_flush_x, z_shift=power_z_shift))
            cuts.append(left_wall_face_recess(params, port["a"], port["b"], 5.6, left_flush_recess_x, z_shift=power_z_shift))
            cuts.append(left_wall_rect(params, port["a"], port["b"], 5.6, left_flush_x, z_shift=power_z_shift))
        elif label == "Pi microSD":
            additions.append(
                left_wall_indent_wall(
                    params,
                    port["a"],
                    port["b"],
                    2.0,
                    left_pi_x,
                    z_shift=pi_sd_indent_z_shift,
                    half_span_adjust=pi_sd_indent_span_adjust,
                )
            )
            cuts.append(
                left_wall_face_recess(
                    params,
                    port["a"],
                    port["b"],
                    2.0,
                    left_pi_recess_x,
                    z_shift=pi_sd_indent_z_shift,
                    half_span_adjust=pi_sd_indent_span_adjust,
                )
            )
            cuts.append(left_wall_rect(params, port["a"], port["b"], 2.0, left_pi_x, z_shift=pi_sd_hole_z_shift))
        elif label == "Pi mini-HDMI":
            additions.append(south_wall_indent_wall(params, port["a"], port["b"], 2.5, south_pi_y, z_shift=pi_south_indent_z_shift))
            cuts.append(south_wall_face_recess(params, port["a"], port["b"], 2.5, south_pi_recess_y, z_shift=pi_south_indent_z_shift))
            cuts.append(south_wall_rect(params, port["a"], port["b"], 3.0, south_pi_y, z_shift=pi_top_trim_z_shift))
        elif label == "Pi USB data":
            additions.append(south_wall_indent_wall(params, port["a"], port["b"], 1.8, south_pi_y, z_shift=pi_south_indent_z_shift))
            cuts.append(south_wall_face_recess(params, port["a"], port["b"], 1.8, south_pi_recess_y, z_shift=pi_south_indent_z_shift))
            cuts.append(south_wall_rect(params, port["a"], port["b"], 2.3, south_pi_y, z_shift=pi_top_trim_z_shift))

    additions.append(north_wall_indent_wall(params, OLED_SD_X0, OLED_SD_X1, 5.0, north_flush_y))
    cuts.append(north_wall_face_recess(params, OLED_SD_X0, OLED_SD_X1, 5.0, north_flush_recess_y))
    cuts.append(north_wall_rect(params, OLED_SD_X0, OLED_SD_X1, 5.0, north_flush_y))

    for addition in additions:
        model = model.union(addition)
    for cutter in cuts:
        model = model.cut(cutter)
    return model.clean()
