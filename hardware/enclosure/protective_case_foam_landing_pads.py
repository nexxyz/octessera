from __future__ import annotations

import math
from dataclasses import dataclass
from typing import cast

import cadquery as cq

try:
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_bottom_latches import bottom_latch_components
    from .protective_case_corner_restraints import corner_restraint_components, outside_volume
    from .protective_case_geometry import build_device_insertion, dimensions, normalize_source_half, parametric_box_measures, wall_planes
    from .protective_case_keepouts import control_contact_keepouts
except ImportError:
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_bottom_latches import bottom_latch_components
    from protective_case_corner_restraints import corner_restraint_components, outside_volume
    from protective_case_geometry import build_device_insertion, dimensions, normalize_source_half, parametric_box_measures, wall_planes
    from protective_case_keepouts import control_contact_keepouts


TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


@dataclass(frozen=True)
class FoamLandingPadSpec:
    name: str
    plane: str
    embedded_plane: float
    flat_plane: float
    tangent_center: float


def foam_landing_pad_specs(params: dict) -> tuple[FoamLandingPadSpec, ...]:
    spec = params["foam_landing_pads"]
    center_x, center_y = params["device"]["center"]
    walls = spec["walls"]
    return (
        FoamLandingPadSpec("foam_landing_pad_west", "YZ", walls["west"]["embedded_plane"], walls["west"]["flat_plane"], center_y),
        FoamLandingPadSpec("foam_landing_pad_east", "YZ", walls["east"]["embedded_plane"], walls["east"]["flat_plane"], center_y),
        FoamLandingPadSpec("foam_landing_pad_south", "XZ", walls["south"]["embedded_plane"], walls["south"]["flat_plane"], center_x),
        FoamLandingPadSpec("foam_landing_pad_north", "XZ", walls["north"]["embedded_plane"], walls["north"]["flat_plane"], center_x),
    )


def _rounded_wire(plane: str, origin: tuple[float, float, float], width: float, height: float, radius: float) -> cq.Wire:
    half_width = width / 2.0
    half_height = height / 2.0
    diagonal = radius / math.sqrt(2.0)
    return cast(
        cq.Wire,
        cq.Workplane(plane, origin=origin)
        .moveTo(-half_width + radius, -half_height)
        .lineTo(half_width - radius, -half_height)
        .threePointArc((half_width - radius + diagonal, -half_height + radius - diagonal), (half_width, -half_height + radius))
        .lineTo(half_width, half_height - radius)
        .threePointArc((half_width - radius + diagonal, half_height - radius + diagonal), (half_width - radius, half_height))
        .lineTo(-half_width + radius, half_height)
        .threePointArc((-half_width + radius - diagonal, half_height - radius + diagonal), (-half_width, half_height - radius))
        .lineTo(-half_width, -half_height + radius)
        .threePointArc((-half_width + radius - diagonal, -half_height + radius - diagonal), (-half_width + radius, -half_height))
        .close()
        .val(),
    )


def _profile_origin(pad: FoamLandingPadSpec, params: dict, plane: float, center_z: float) -> tuple[float, float, float]:
    return (plane, pad.tangent_center, center_z) if pad.plane == "YZ" else (pad.tangent_center, plane, center_z)


def _ruled_pad(params: dict, pad: FoamLandingPadSpec) -> cq.Workplane:
    spec = params["foam_landing_pads"]
    center_z = spec["center_z"]
    embedded = spec["embedded_profile"]
    flat = spec["flat_profile"]
    embedded_wire = _rounded_wire(pad.plane, _profile_origin(pad, params, pad.embedded_plane, center_z), embedded["tangent_width"], embedded["vertical_height"], embedded["corner_radius"])
    flat_wire = _rounded_wire(pad.plane, _profile_origin(pad, params, pad.flat_plane, center_z), flat["tangent_width"], flat["vertical_height"], flat["corner_radius"])
    return cq.Workplane("XY").newObject([embedded_wire, flat_wire]).toPending().loft(combine=True, ruled=True)


def foam_landing_pad_components(params: dict) -> tuple[tuple[str, cq.Workplane], ...]:
    return tuple((pad.name, _ruled_pad(params, pad)) for pad in foam_landing_pad_specs(params))


def add_foam_landing_pads(params: dict, model: cq.Workplane) -> cq.Workplane:
    result = model
    for _, pad in foam_landing_pad_components(params):
        result = result.union(pad).clean()
    return result.clean()


def foam_landing_pad_probe(params: dict, pad: FoamLandingPadSpec) -> cq.Workplane:
    spec = params["foam_landing_pads"]["patch"]
    center_z = params["foam_landing_pads"]["center_z"]
    wire = _rounded_wire(pad.plane, _profile_origin(pad, params, pad.flat_plane, center_z), spec["tangent_width"], spec["vertical_height"], spec["corner_radius"])
    return cq.Workplane("XY").newObject([cq.Face.makeFromWires(wire)])


def _volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def _intersection_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return _volume(first.intersect(second))


def _prism(x: float, y: float, z: float, width: float, depth: float, height: float) -> cq.Workplane:
    return cq.Workplane("XY").box(width, depth, height, centered=(False, False, False)).translate((x, y, z))


def foam_landing_pad_flat_face(pad: cq.Workplane, spec: FoamLandingPadSpec) -> cq.Shape:
    faces = cast(list[cq.Shape], pad.faces().vals())
    candidates = []
    for face in faces:
        box = face.BoundingBox()
        coordinate = box.xmin if spec.plane == "YZ" else box.ymin
        target = spec.flat_plane
        if face.geomType() == "PLANE" and abs(coordinate - target) <= TOLERANCE and (box.xmax - box.xmin <= TOLERANCE if spec.plane == "YZ" else box.ymax - box.ymin <= TOLERANCE):
            candidates.append(face)
    if len(candidates) != 1:
        raise ValueError(f"{spec.name} expected one flat face, found {len(candidates)}")
    return candidates[0]


def _minimum_pad_slope(pad: cq.Workplane) -> float:
    slopes = []
    for face in cast(list[cq.Shape], pad.faces().vals()):
        normal = face.normalAt()
        slope = math.degrees(math.acos(min(1.0, abs(normal.z))))
        if slope > 1.0:
            slopes.append(slope)
    return min(slopes)


def _source_hardware_components(params: dict) -> tuple[cq.Workplane, ...]:
    box = params["parametric_box"]
    measures = parametric_box_measures(params, ())
    rel_z = box["height"] - box["top_height"] - box["height"] / 2.0
    source_hinge = upstream.Hinge(measures.hinge)
    rel_y = 2.0 * box["hinge"]["thickness"] - box["hinge"]["leaf_height"]
    components = []
    translate_x = params["normalization"]["translate"][0]
    for center in (translate_x - box["width"] / 4.0, translate_x + box["width"] / 4.0):
        source_x = center - params["normalization"]["translate"][0]
        raw = source_hinge.vuoto.translate((source_x, measures.depth / 2.0 + rel_y, rel_z)).translate((0.0, -measures.depth / 2.0 - measures.thickness, 0.0))
        components.append(normalize_source_half(raw, params))
    return tuple(components)


def _bbox_distance(first: cq.Workplane, second: cq.Workplane) -> float:
    first_box = cast(cq.Shape, first.val()).BoundingBox()
    second_box = cast(cq.Shape, second.val()).BoundingBox()
    gaps = (
        max(first_box.xmin - second_box.xmax, second_box.xmin - first_box.xmax, 0.0),
        max(first_box.ymin - second_box.ymax, second_box.ymin - first_box.ymax, 0.0),
        max(first_box.zmin - second_box.zmax, second_box.zmin - first_box.zmax, 0.0),
    )
    return math.sqrt(sum(gap * gap for gap in gaps))


def validate_foam_landing_pads(params: dict, pristine: cq.Workplane, final_tub: cq.Workplane, outer_envelope: cq.Workplane) -> None:
    spec = params["foam_landing_pads"]
    dims = dimensions(params)
    pads = foam_landing_pad_components(params)
    require(len(pads) == 4, "foam landing pads must contain exactly four pads")
    keepouts = control_contact_keepouts(params)
    shelves = tuple(component for _, component in corner_restraint_components(params))
    planes = wall_planes(params)
    plane_by_wall = {"west": planes["west_x"], "east": planes["east_x"], "south": planes["south_y"], "north": planes["north_y"]}
    pad_metrics = []
    for pad_name, pad in pads:
        pad_spec = next(item for item in foam_landing_pad_specs(params) if item.name == pad_name)
        wall_name = pad_name.removeprefix("foam_landing_pad_")
        wall_plane = plane_by_wall[wall_name]
        direction = 1.0 if wall_name in ("west", "south") else -1.0
        require(abs(pad_spec.embedded_plane - (wall_plane - direction * spec["wall_overlap"])) <= TOLERANCE, f"{pad_name} embedded wall overlap changed")
        require(abs(pad_spec.flat_plane - (wall_plane + direction * spec["cavity_proud"])) <= TOLERANCE, f"{pad_name} cavity proud changed")
        require(abs(abs(pad_spec.flat_plane - pad_spec.embedded_plane) - 0.8) <= TOLERANCE, f"{pad_name} ruled-loft depth changed")
        shape = cast(cq.Shape, pad.val())
        require(shape.isValid() and len(pad.solids().vals()) == 1 and len(pad.shells().vals()) == 1, f"{pad_name} must be one valid solid and shell")
        box = shape.BoundingBox()
        require(abs(box.zmin - (spec["center_z"] - spec["embedded_profile"]["vertical_height"] / 2.0)) <= TOLERANCE, f"{pad_name} Z minimum changed")
        require(abs(box.zmax - (spec["center_z"] + spec["embedded_profile"]["vertical_height"] / 2.0)) <= TOLERANCE, f"{pad_name} Z maximum changed")
        require(_intersection_volume(pad, pristine) >= 100.0, f"{pad_name} wall intersection is below 100 mm3")
        require(outside_volume(pad, outer_envelope) <= VOLUME_TOLERANCE, f"{pad_name} breaches the exterior envelope")
        require(all(_intersection_volume(pad, keepout_probe) <= VOLUME_TOLERANCE for keepout_probe in (_prism(keepout.bounds[0], keepout.bounds[1], 0.0, keepout.bounds[2] - keepout.bounds[0], keepout.bounds[3] - keepout.bounds[1], dims.height) for keepout in keepouts)), f"{pad_name} intersects a control keepout")
        require(all(_intersection_volume(pad, shelf) <= VOLUME_TOLERANCE for shelf in shelves), f"{pad_name} intersects a shelf")
        flat_face = foam_landing_pad_flat_face(pad, pad_spec)
        require(abs(flat_face.Area() - 219.1416) <= 0.01, f"{pad_name} flat face area changed")
        require(_minimum_pad_slope(pad) >= 45.0, f"{pad_name} has a face slope below 45 degrees")
        require(all(abs(face.normalAt().z) < 0.999 for face in cast(list[cq.Shape], pad.faces().vals())), f"{pad_name} has a horizontal ledge face")
        pad_metrics.append(f"{pad_name}={_intersection_volume(pad, pristine):.4f}mm3/slope:{_minimum_pad_slope(pad):.2f}deg")
    for check_fit in (False, True):
        for base_z in params["insertion"]["descent_sample_z"]:
            insertion = build_device_insertion(params, check_fit, base_z)
            require(all(_intersection_volume(insertion, pad) <= VOLUME_TOLERANCE for _, pad in pads), f"{'check_fit' if check_fit else 'nominal'} insertion intersects a foam pad at Z{base_z}")
    nominal_x = (dims.inner_width - dims.device_width) / 2.0 - spec["cavity_proud"]
    nominal_y = (dims.inner_depth - dims.device_depth) / 2.0 - spec["cavity_proud"]
    check_fit_x = (dims.inner_width - dims.check_fit_width) / 2.0 - spec["cavity_proud"]
    check_fit_y = (dims.inner_depth - dims.check_fit_depth) / 2.0 - spec["cavity_proud"]
    for actual, expected, label in ((nominal_x, 1.2, "nominal_x"), (nominal_y, 1.6, "nominal_y"), (check_fit_x, 0.7, "check_fit_x"), (check_fit_y, 1.1, "check_fit_y")):
        require(abs(actual - expected) <= TOLERANCE, f"{label} hard clearance changed")
    seam_clearance = params["insertion"]["entry_base_z"] - max(cast(cq.Shape, pad.val()).BoundingBox().zmax for _, pad in pads)
    require(seam_clearance >= 16.8, f"foam pad seam clearance is below 16.8 mm: {seam_clearance:.3f}")
    source_hardware = _source_hardware_components(params)
    latch_hardware = tuple(component for _, component in bottom_latch_components(params, "deep"))
    separation = min(_bbox_distance(pad, feature) for _, pad in pads for feature in source_hardware)
    latch_separation = min(_bbox_distance(pad, latch) for _, pad in pads for latch in latch_hardware)
    require(separation >= 10.8, f"foam pad separation from hinge geometry is below 10.8 mm: {separation:.3f}")
    require(abs(latch_separation - 32.429) <= TOLERANCE, f"foam-to-latch bbox separation changed: {latch_separation:.3f}")
    require(cast(cq.Shape, final_tub.val()).isValid() and len(final_tub.solids().vals()) == 1 and len(final_tub.shells().vals()) == 1, "deep tub with foam pads must be one valid solid and shell")
    print(f"foam_landing_pads=4 valid=PASS metrics={';'.join(pad_metrics)} flat_face=219.1416mm2 insertion_stations=98 collision=NONE")
    print(f"foam_clearance=nominal:+/-{nominal_x:.1f}X/+/-{nominal_y:.1f}Y check_fit:+/-{check_fit_x:.1f}X/+/-{check_fit_y:.1f}Y seam={seam_clearance:.3f}mm hinge_separation={separation:.3f}mm latch_bbox_separation={latch_separation:.3f}mm pads=manual_closed_cell_adhesive_foam")
