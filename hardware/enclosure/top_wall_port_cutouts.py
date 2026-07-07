from __future__ import annotations

import cadquery as cq


TEMP_WALL_PORT_CENTER_Z = 6.0
PORT_CUT_EPS = 1.0
OLED_SD_X0 = 61.88
OLED_SD_X1 = 82.88


def box_cutter(x0: float, y0: float, x1: float, y1: float, z0: float, z1: float) -> cq.Workplane:
    return (
        cq.Workplane("XY")
        .rect(x1 - x0, y1 - y0)
        .extrude(z1 - z0)
        .translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))
    )


def left_wall_rect(params: dict, y0: float, y1: float, height: float) -> cq.Workplane:
    wall = params["wall"]
    z0 = TEMP_WALL_PORT_CENTER_Z - height / 2.0
    z1 = TEMP_WALL_PORT_CENTER_Z + height / 2.0
    return box_cutter(-PORT_CUT_EPS, y0, wall + PORT_CUT_EPS, y1, z0, z1)


def south_wall_rect(params: dict, x0: float, x1: float, height: float) -> cq.Workplane:
    wall = params["wall"]
    z0 = TEMP_WALL_PORT_CENTER_Z - height / 2.0
    z1 = TEMP_WALL_PORT_CENTER_Z + height / 2.0
    return box_cutter(x0, -PORT_CUT_EPS, x1, wall + PORT_CUT_EPS, z0, z1)


def north_wall_rect(params: dict, x0: float, x1: float, height: float) -> cq.Workplane:
    _, depth = params["case_size_v21"]
    wall = params["wall"]
    z0 = TEMP_WALL_PORT_CENTER_Z - height / 2.0
    z1 = TEMP_WALL_PORT_CENTER_Z + height / 2.0
    return box_cutter(x0, depth - wall - PORT_CUT_EPS, x1, depth + PORT_CUT_EPS, z0, z1)


def audio_jack_cutter(params: dict, y: float) -> cq.Workplane:
    wall = params["wall"]
    return (
        cq.Workplane("YZ")
        .circle(4.1)
        .extrude(wall + 2 * PORT_CUT_EPS)
        .translate((-PORT_CUT_EPS, y, TEMP_WALL_PORT_CENTER_Z))
    )


def add_top_wall_port_cutouts(model: cq.Workplane, params: dict) -> cq.Workplane:
    for port in params["ports_v21"]:
        label = port["label"]
        if label == "audio 3.5mm":
            model = model.cut(audio_jack_cutter(params, (port["a"] + port["b"]) / 2.0))
        elif label == "USB-C power":
            model = model.cut(left_wall_rect(params, port["a"], port["b"], 5.6))
        elif label == "Pi microSD":
            model = model.cut(left_wall_rect(params, port["a"], port["b"], 4.0))
        elif label == "Pi mini-HDMI":
            model = model.cut(south_wall_rect(params, port["a"], port["b"], 6.5))
        elif label == "Pi USB data":
            model = model.cut(south_wall_rect(params, port["a"], port["b"], 5.8))

    model = model.cut(north_wall_rect(params, OLED_SD_X0, OLED_SD_X1, 5.0))
    return model.clean()
