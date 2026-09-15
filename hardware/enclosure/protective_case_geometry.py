from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import cadquery as cq

try:
    from .upstream.andy_wings_parametric_box import Measures
except ImportError:
    from upstream.andy_wings_parametric_box import Measures


ROOT = Path(__file__).resolve().parent
PARAMS = ROOT / "protective_case_params.json"
SOURCE_PARAMS = ROOT / "enclosure_params.json"
FACE_DOWN_TRANSPORT_TRANSLATION = (4.8, 144.2, 42.0)
FACE_DOWN_TRANSPORT_XY_CENTER = (127.8, 74.2)


@dataclass(frozen=True)
class CornerRestraint:
    name: str
    shelf_polygon: tuple[tuple[float, float], ...]
    shelf_tabs: dict[str, tuple[float, float, float, float]]
    support_lower_polygon: tuple[tuple[float, float], ...]
    support_upper_polygon: tuple[tuple[float, float], ...]
    device_corner: tuple[float, float]
    nominal_shelf_base_z: float
    contact_z: float


@dataclass(frozen=True)
class CaseDimensions:
    width: float
    depth: float
    height: float
    wall: float
    inner_width: float
    inner_depth: float
    device_width: float
    device_depth: float
    device_height: float
    device_radius: float
    check_fit_width: float
    check_fit_depth: float
    check_fit_radius: float
    bed_width: float
    bed_depth: float


def load_parameters(path: Path = PARAMS) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_source_parameters(path: Path = SOURCE_PARAMS) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def dimensions(params: dict) -> CaseDimensions:
    box = params["parametric_box"]
    device = params["device"]
    envelope = device["envelope"]
    check_fit = device["check_fit"]
    bed_width, bed_depth = params["print_bed_xy"]
    return CaseDimensions(
        width=box["width"],
        depth=box["depth"],
        height=box["height"],
        wall=box["thickness"],
        inner_width=box["width"] - 2.0 * box["thickness"],
        inner_depth=box["depth"] - 2.0 * box["thickness"],
        device_width=envelope["width"],
        device_depth=envelope["depth"],
        device_height=envelope["height"],
        device_radius=envelope["radius"],
        check_fit_width=check_fit["width"],
        check_fit_depth=check_fit["depth"],
        check_fit_radius=check_fit["radius"],
        bed_width=bed_width,
        bed_depth=bed_depth,
    )


def parametric_box_measures(params: dict, clip_positions: tuple[float, ...] | None = None):
    box = params["parametric_box"]
    hinge = box["hinge"]
    clip = box["clip"]
    positions = tuple(clip["positions_x"]) if clip_positions is None else clip_positions
    return Measures(
        width=box["width"],
        depth=box["depth"],
        height=box["height"],
        thickness=box["thickness"],
        clearance=box["clearance"],
        top_height=box["top_height"],
        chamfer_z=box["chamfer_z"],
        chamfer_xy=box["chamfer_xy"],
        text=box["text"],
        hinge=Measures(**hinge),
        clip=Measures(length=clip["length"], positions_x=positions),
    )


def normalize_source_half(model: cq.Workplane, params: dict) -> cq.Workplane:
    normalization = params["normalization"]
    translated = model.rotate(
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        normalization["rotate_x_degrees"],
    )
    return translated.translate(tuple(normalization["translate"]))


def hinge_axis(params: dict) -> tuple[float, float]:
    box = params["parametric_box"]
    hinge = box["hinge"]
    source_y = 2.0 * hinge["thickness"] - hinge["leaf_height"] - box["thickness"]
    source_z = box["height"] - box["top_height"] - box["height"] / 2.0
    translate_x, translate_y, translate_z = params["normalization"]["translate"]
    return -source_y + translate_y, -source_z + translate_z


def hinge_centers(params: dict) -> tuple[float, ...]:
    box = params["parametric_box"]
    translate_x = params["normalization"]["translate"][0]
    return tuple(translate_x + sign * box["width"] / 4.0 for sign in (-1.0, 1.0))


def device_origin(params: dict) -> tuple[float, float]:
    return tuple(params["device"]["measured_origin"])


def local_to_case(source_params: dict, point: list[float]) -> tuple[float, float]:
    _, case_depth = source_params["case_size_v21"]
    offset_x, offset_y = source_params["offset_v21"]
    return offset_x + point[0], case_depth - (offset_y + point[1])


def face_down_transport_transform(model: cq.Workplane) -> cq.Workplane:
    face_down = model.rotate((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), 180.0).translate(FACE_DOWN_TRANSPORT_TRANSLATION)
    center_x, center_y = FACE_DOWN_TRANSPORT_XY_CENTER
    return face_down.rotate((center_x, center_y, 0.0), (center_x, center_y, 1.0), 180.0)


def face_down_transport_xy(point: tuple[float, float]) -> tuple[float, float]:
    center_x, center_y = FACE_DOWN_TRANSPORT_XY_CENTER
    return 2.0 * center_x - point[0], 2.0 * center_y - point[1]


def face_down_transport_feature_xy(
    source_params: dict, point: list[float], x_offset: float = 0.0, y_offset: float = 0.0
) -> tuple[float, float]:
    face_up_x, face_up_y = local_to_case(source_params, point)
    return face_down_transport_xy((
        FACE_DOWN_TRANSPORT_TRANSLATION[0] + face_up_x + x_offset,
        FACE_DOWN_TRANSPORT_TRANSLATION[1] - (face_up_y + y_offset),
    ))


def v21_to_protective(point: tuple[float, float], params: dict) -> tuple[float, float]:
    origin_x, origin_y = device_origin(params)
    return origin_x + point[0], origin_y + point[1]


def local_to_protective(source_params: dict, point: list[float], params: dict) -> tuple[float, float]:
    return v21_to_protective(local_to_case(source_params, point), params)


def build_device_insertion(params: dict, check_fit: bool = False, base_z: float | None = None) -> cq.Workplane:
    dims = dimensions(params)
    width = dims.check_fit_width if check_fit else dims.device_width
    depth = dims.check_fit_depth if check_fit else dims.device_depth
    radius = dims.check_fit_radius if check_fit else dims.device_radius
    center_x, center_y = params["device"]["center"]
    origin_x = center_x - width / 2.0
    origin_y = center_y - depth / 2.0
    floor_z = params["insertion"]["seated_base_z"] if base_z is None else base_z
    height = dims.device_height
    return (
        cq.Workplane("XY")
        .box(width, depth, height, centered=(False, False, False))
        .translate((origin_x, origin_y, floor_z))
        .edges("|Z")
        .fillet(radius)
    )


def corner_restraints(params: dict) -> tuple[CornerRestraint, ...]:
    return tuple(
        CornerRestraint(
            name=record["name"],
            shelf_polygon=tuple(tuple(point) for point in record["shelf_polygon"]),
            shelf_tabs={side: tuple(bounds) for side, bounds in record["shelf_tabs"].items()},
            support_lower_polygon=tuple(tuple(point) for point in record["support_lower_polygon"]),
            support_upper_polygon=tuple(tuple(point) for point in record["support_upper_polygon"]),
            device_corner=tuple(record["device_corner"]),
            nominal_shelf_base_z=record["nominal_shelf_base_z"],
            contact_z=record["contact_z"],
        )
        for record in params["corner_restraints"]["records"]
    )


def wall_planes(params: dict) -> dict[str, float]:
    return dict(params["wall_planes"])


def artifact_paths(params: dict) -> tuple[Path, ...]:
    return tuple(ROOT.parent.parent / Path(path) for path in params["artifacts"])
