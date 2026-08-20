from __future__ import annotations

import cadquery as cq

from top_wall_port_geometry import (
    PORT_RECESS_BACK_LAND,
    centered_indent_z_shift,
    make_left_wall_port_hole,
    make_north_wall_port_hole,
    make_south_wall_port_hole,
    wall_port_z_center,
)
from top_wall_port_indent_geometry import (
    make_audio_jack_port_cutter,
    make_left_wall_indent_wall,
    make_north_wall_indent_wall,
    make_south_wall_indent_wall,
)
from top_wall_port_recess_geometry import (
    make_left_wall_face_recess,
    make_north_wall_face_recess,
    make_south_wall_face_recess,
)


OLED_SD_X0 = 58.63
OLED_SD_X1 = 75.63
OLED_SD_HEIGHT = 4.5
OLED_SD_Z_SHIFT = -0.25
POWER_Z_SHIFT = -0.25
AUDIO_Z_SHIFT = 0.3
PI_Z_SHIFT = 7.0
PI_TOP_TRIM_Z_SHIFT = PI_Z_SHIFT - 1.5
PI_HDMI_Z_SHIFT = PI_TOP_TRIM_Z_SHIFT
PI_USB_Z_SHIFT = PI_TOP_TRIM_Z_SHIFT - 0.65
PI_SD_HOLE_Z_SHIFT = PI_Z_SHIFT - 4.3
PI_HDMI_HEIGHT = 5.0
PI_HDMI_INDENT_HEIGHT = 4.5
PI_USB_HEIGHT = 4.6
PI_USB_INDENT_HEIGHT = 4.6
PI_SD_HEIGHT = 3.0
PI_SD_INDENT_HEIGHT = 2.0


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
    pi_sd_indent_z_shift = centered_indent_z_shift(PI_SD_HEIGHT, PI_SD_HOLE_Z_SHIFT, PI_SD_INDENT_HEIGHT)
    pi_sd_indent_span_adjust = -2.5
    pi_hdmi_indent_z_shift = centered_indent_z_shift(PI_HDMI_HEIGHT, PI_HDMI_Z_SHIFT, PI_HDMI_INDENT_HEIGHT)
    pi_usb_indent_z_shift = centered_indent_z_shift(PI_USB_HEIGHT, PI_USB_Z_SHIFT, PI_USB_INDENT_HEIGHT)
    additions = []
    cuts = []
    for port in params["ports_v21"]:
        label = port["label"]
        if label == "audio 3.5mm":
            center_y = (port["a"] + port["b"]) / 2.0
            audio_indent_y0 = center_y - 4.1
            audio_indent_y1 = center_y + 4.1
            audio_indent_z_shift = wall_port_z_center(8.2, AUDIO_Z_SHIFT) - wall_port_z_center(5.2)
            additions.append(
                make_left_wall_indent_wall(
                    params,
                    audio_indent_y0,
                    audio_indent_y1,
                    5.2,
                    left_flush_x,
                    z_shift=audio_indent_z_shift,
                )
            )
            cuts.append(
                make_left_wall_face_recess(
                    params,
                    audio_indent_y0,
                    audio_indent_y1,
                    5.2,
                    left_flush_recess_x,
                    z_shift=audio_indent_z_shift,
                )
            )
            cuts.append(make_audio_jack_port_cutter(params, (port["a"] + port["b"]) / 2.0, left_flush_x, AUDIO_Z_SHIFT))
        elif label == "USB-C power":
            additions.append(make_left_wall_indent_wall(params, port["a"], port["b"], 4.6, left_flush_x, z_shift=POWER_Z_SHIFT))
            cuts.append(make_left_wall_face_recess(params, port["a"], port["b"], 4.6, left_flush_recess_x, z_shift=POWER_Z_SHIFT))
            cuts.append(make_left_wall_port_hole(params, port["a"], port["b"], 4.6, left_flush_x, z_shift=POWER_Z_SHIFT))
        elif label == "Pi microSD":
            additions.append(
                make_left_wall_indent_wall(
                    params,
                    port["a"],
                    port["b"],
                    PI_SD_INDENT_HEIGHT,
                    left_pi_x,
                    z_shift=pi_sd_indent_z_shift,
                    half_span_adjust=pi_sd_indent_span_adjust,
                )
            )
            cuts.append(
                make_left_wall_face_recess(
                    params,
                    port["a"],
                    port["b"],
                    PI_SD_INDENT_HEIGHT,
                    left_pi_recess_x,
                    z_shift=pi_sd_indent_z_shift,
                    half_span_adjust=pi_sd_indent_span_adjust,
                )
            )
            cuts.append(make_left_wall_port_hole(params, port["a"], port["b"], PI_SD_HEIGHT, left_pi_x, z_shift=PI_SD_HOLE_Z_SHIFT))
        elif label == "Pi mini-HDMI":
            additions.append(make_south_wall_indent_wall(params, port["a"], port["b"], PI_HDMI_INDENT_HEIGHT, south_pi_y, z_shift=pi_hdmi_indent_z_shift))
            cuts.append(make_south_wall_face_recess(params, port["a"], port["b"], PI_HDMI_INDENT_HEIGHT, south_pi_recess_y, z_shift=pi_hdmi_indent_z_shift))
            cuts.append(make_south_wall_port_hole(params, port["a"], port["b"], PI_HDMI_HEIGHT, south_pi_y, z_shift=PI_HDMI_Z_SHIFT))
        elif label in ("Pi USB data", "Orange Pi USB host"):
            additions.append(make_south_wall_indent_wall(params, port["a"], port["b"], PI_USB_INDENT_HEIGHT, south_pi_y, z_shift=pi_usb_indent_z_shift))
            cuts.append(make_south_wall_face_recess(params, port["a"], port["b"], PI_USB_INDENT_HEIGHT, south_pi_recess_y, z_shift=pi_usb_indent_z_shift))
            cuts.append(make_south_wall_port_hole(params, port["a"], port["b"], PI_USB_HEIGHT, south_pi_y, z_shift=PI_USB_Z_SHIFT))

    additions.append(make_north_wall_indent_wall(params, OLED_SD_X0, OLED_SD_X1, OLED_SD_HEIGHT, north_flush_y, z_shift=OLED_SD_Z_SHIFT))
    cuts.append(make_north_wall_face_recess(params, OLED_SD_X0, OLED_SD_X1, OLED_SD_HEIGHT, north_flush_recess_y, z_shift=OLED_SD_Z_SHIFT))
    cuts.append(make_north_wall_port_hole(params, OLED_SD_X0, OLED_SD_X1, OLED_SD_HEIGHT, north_flush_y, z_shift=OLED_SD_Z_SHIFT))

    for addition in additions:
        model = model.union(addition)
    for cutter in cuts:
        model = model.cut(cutter)
    return model.clean()
