from __future__ import annotations

from copy import deepcopy

import cadquery as cq

from branding_marking_cadquery import branding_marking_parts, make_branding_marking
from port_markings_cadquery import MARK_CUT_CLEARANCE, make_port_markings, port_marking_parts
from top_enclosure_config import load_params
from top_wave_geometry import LOW_Z


BRANDING_RAISE = 0.65
ORANGE_PI_EAST_USB_CENTER_X = 64.1
ORANGE_PI_USB_WIDTH = 11.5


def orange_pi_top_params(params: dict) -> dict:
    variant_params = deepcopy(params)
    variant_params["host_variant"] = "orange_pi_zero_2w"
    half_width = ORANGE_PI_USB_WIDTH / 2.0
    variant_params["ports_v21"] = [
        *variant_params["ports_v21"],
        {
            "side": "bottom",
            "a": ORANGE_PI_EAST_USB_CENTER_X - half_width,
            "b": ORANGE_PI_EAST_USB_CENTER_X + half_width,
            "z0": 8.5,
            "z1": 19.5,
            "label": "Orange Pi USB host",
        },
    ]
    return variant_params


def build_branding_marking(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    case_params = params or load_params()
    return make_branding_marking(LOW_Z, BRANDING_RAISE).union(make_port_markings(case_params, model_bottom_z)).clean()


def build_flush_top_branding_marking() -> cq.Workplane:
    return make_branding_marking(LOW_Z - BRANDING_RAISE, BRANDING_RAISE).clean()


def build_flush_top_branding_parts() -> list[tuple[str, cq.Workplane]]:
    return branding_marking_parts(LOW_Z - BRANDING_RAISE, BRANDING_RAISE)


def build_flush_port_markings(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    case_params = params or load_params()
    return make_port_markings(case_params, model_bottom_z, flush=True).clean()


def build_flush_port_marking_parts(params: dict | None = None, model_bottom_z: float = -10.0) -> list[tuple[str, cq.Workplane]]:
    case_params = params or load_params()
    return port_marking_parts(case_params, model_bottom_z, flush=True)


def build_flush_port_marking_cutters(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    case_params = params or load_params()
    return make_port_markings(case_params, model_bottom_z, flush=True, cut_clearance=MARK_CUT_CLEARANCE).clean()


def build_flush_branding_marking(params: dict | None = None, model_bottom_z: float = -10.0) -> cq.Workplane:
    return build_flush_top_branding_marking().union(build_flush_port_markings(params, model_bottom_z)).clean()
