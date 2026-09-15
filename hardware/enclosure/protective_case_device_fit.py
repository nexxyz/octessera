from __future__ import annotations

import importlib
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .protective_case_corner_restraints import _minimum_support_slope, continuous_shelf_support, corner_restraint_components
    from .protective_case_device_orientation import validate_device_orientation
    from .protective_case_foam_landing_pads import foam_landing_pad_components
    from .protective_case_geometry import (
        corner_restraints,
        dimensions,
        face_down_transport_transform,
    )
    from .protective_case_keepouts import control_contact_keepouts
except ImportError:
    import generate_protective_case_cadquery as cad
    from protective_case_corner_restraints import _minimum_support_slope, continuous_shelf_support, corner_restraint_components
    from protective_case_device_orientation import validate_device_orientation
    from protective_case_foam_landing_pads import foam_landing_pad_components
    from protective_case_geometry import corner_restraints, dimensions, face_down_transport_transform
    from protective_case_keepouts import control_contact_keepouts


ROOT = Path(__file__).resolve().parent
TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7
EXPECTED_TUB_BBOX = (0.0, 255.6, -4.0, 149.834972, 0.0, 57.85)
EXPECTED_DEVICE_BBOX = (3.8, 251.8, 4.2, 144.2, 22.0, 52.0)
EXPECTED_SHELF_ELEVATIONS = {
    "NW": (23.5, 25.0),
    "SW": (23.5, 25.0),
    "NE": (28.5, 30.0),
    "SE": (23.5, 25.0),
}
EXPECTED_CONTACTS = {
    "NW": (90.391, (3.8, 15.0, 134.9, 144.2), 25.0),
    "SW": (90.391, (3.8, 15.0, 4.2, 13.5), 25.0),
    "NE": (199.751, (236.1, 251.8, 130.6, 144.2), 30.0),
    "SE": (199.751, (236.1, 251.8, 4.2, 17.8), 25.0),
}
CURRENT_BASE_VOLUME = 185846.03981308394
CURRENT_FINAL_VOLUME = 186788.02738391187
EXPECTED_BASE_VOLUME_DELTA = -0.023217
EXPECTED_FINAL_VOLUME_DELTA = 0.019609


@dataclass(frozen=True)
class SupportContact:
    name: str
    area: float
    bbox: tuple[float, float, float, float]
    z: float


def _source_top_module():
    module_name = "generate_two_level_enclosure_cadquery"
    source_root = str(ROOT)
    added = source_root not in sys.path
    if added:
        sys.path.insert(0, source_root)
    try:
        return importlib.import_module(module_name)
    finally:
        if added:
            sys.path.remove(source_root)


def build_canonical_face_down_device() -> cq.Workplane:
    source = _source_top_module()
    return face_down_transport_transform(source.build_model(source.load_params()))


def _shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def _volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def _intersection_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return _volume(first.intersect(second))


def _prism(bounds: tuple[float, float, float, float], z_min: float, z_max: float) -> cq.Workplane:
    x_min, y_min, x_max, y_max = bounds
    return cq.Workplane("XY").box(x_max - x_min, y_max - y_min, z_max - z_min, centered=(False, False, False)).translate((x_min, y_min, z_min))


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _close(actual: float, expected: float, label: str, tolerance: float = TOLERANCE) -> None:
    _require(abs(actual - expected) <= tolerance, f"{label}={actual:.6f}, expected={expected:.6f}")


def _bbox_tuple(model: cq.Workplane) -> tuple[float, float, float, float, float, float]:
    box = _shape_box(model)
    return box.xmin, box.xmax, box.ymin, box.ymax, box.zmin, box.zmax


def _shelf_elevation_mismatches(params: dict) -> tuple[str, ...]:
    mismatches = []
    for restraint in corner_restraints(params):
        expected_base, expected_contact = EXPECTED_SHELF_ELEVATIONS[restraint.name]
        if abs(restraint.contact_z - expected_contact) > TOLERANCE:
            direction = "penetration" if restraint.contact_z > expected_contact else "gap"
            mismatches.append(f"{restraint.name} {direction}={abs(restraint.contact_z - expected_contact):.3f}mm")
        if abs(restraint.nominal_shelf_base_z - expected_base) > TOLERANCE:
            mismatches.append(f"{restraint.name} base={restraint.nominal_shelf_base_z:.3f}mm expected={expected_base:.3f}mm")
    return tuple(mismatches)


def _support_contact(device: cq.Workplane, shelf: cq.Workplane, name: str) -> SupportContact:
    top_face = cast(cq.Shape, shelf.faces(">Z").val())
    common = device.intersect(cq.Workplane("XY").newObject([top_face]))
    faces = cast(list[cq.Shape], common.faces().vals())
    _require(bool(faces), f"{name} has no common planar support contact")
    _require(all(face.geomType() == "PLANE" for face in faces), f"{name} support contact is not planar")
    contact = cast(cq.Shape, common.val())
    box = contact.BoundingBox()
    return SupportContact(name, contact.Area(), (box.xmin, box.xmax, box.ymin, box.ymax), box.zmin)


def support_contacts(device: cq.Workplane, params: dict) -> tuple[SupportContact, ...]:
    return tuple(
        _support_contact(device, shelf, restraint.name)
        for restraint, (_, shelf) in zip(corner_restraints(params), corner_restraint_components(params))
    )


def _control_keepout_probes(params: dict) -> tuple[cq.Workplane, ...]:
    case_height = dimensions(params).height
    return tuple(
        _prism(keepout.bounds, 0.0, case_height)
        for keepout in control_contact_keepouts(params)
    )


def _insertion_sweep_offsets(params: dict) -> tuple[float, ...]:
    insertion = params["insertion"]
    descent = tuple(insertion["descent_sample_z"])
    _require(len(descent) == 49, f"insertion sweep must use 49 configured stations, found {len(descent)}")
    _require(abs(descent[0] - insertion["entry_base_z"]) <= TOLERANCE, "insertion sweep entry station changed")
    _require(abs(descent[-1] - insertion["seated_base_z"]) <= TOLERANCE, "insertion sweep terminal station changed")
    _require(all(first > second for first, second in zip(descent, descent[1:])), "insertion sweep stations are not descending")
    offsets = tuple(base_z - insertion["seated_base_z"] for base_z in descent)
    _require(abs(offsets[0] - 47.35) <= TOLERANCE and abs(offsets[-1]) <= TOLERANCE, "insertion sweep does not span 0..47.35mm")
    _require(any(offset > 32.65 for offset in offsets), "insertion sweep omits the critical interval beyond 32.65mm")
    return offsets


def validate_actual_device_fit(params: dict) -> None:
    mismatches = _shelf_elevation_mismatches(params)
    _require(not mismatches, "face-down shelf elevations changed: " + ", ".join(mismatches))
    device = build_canonical_face_down_device()
    _require(cast(cq.Shape, device.val()).isValid() and len(device.solids().vals()) == 1, "canonical face-down device must be one valid solid")
    for actual, expected, axis in zip(_bbox_tuple(device), EXPECTED_DEVICE_BBOX, ("xmin", "xmax", "ymin", "ymax", "zmin", "zmax")):
        _close(actual, expected, f"face_down_device_{axis}")

    final_tub = cad.build_deep_tub(params)
    _require(_intersection_volume(device, final_tub) <= VOLUME_TOLERANCE, "canonical device has a prohibited deep-tub intersection")
    contacts = support_contacts(device, params)
    contact_metrics = []
    for contact in contacts:
        expected_area, expected_bbox, expected_z = EXPECTED_CONTACTS[contact.name]
        _close(contact.area, expected_area, f"{contact.name}_contact_area", 0.01)
        for actual, expected, axis in zip(contact.bbox, expected_bbox, ("xmin", "xmax", "ymin", "ymax")):
            _close(actual, expected, f"{contact.name}_contact_{axis}")
        _close(contact.z, expected_z, f"{contact.name}_contact_z")
        contact_metrics.append(f"{contact.name}={contact.area:.3f}mm2/bbox={','.join(f'{value:.1f}' for value in contact.bbox)}/Z{contact.z:.1f}")

    insertion_offsets = _insertion_sweep_offsets(params)
    insertion_collisions = []
    for offset in insertion_offsets:
        insertion_collisions.append(_intersection_volume(device.translate((0.0, 0.0, float(offset))), final_tub))
    _require(max(insertion_collisions) <= VOLUME_TOLERANCE, "canonical device insertion sweep has a prohibited intersection")

    shallow_lid = cad.build_shallow_lid(params)
    shallow_collision = _intersection_volume(device, shallow_lid)
    shallow_distance = cast(cq.Shape, device.val()).distance(cast(cq.Shape, shallow_lid.val()))
    _require(shallow_collision <= VOLUME_TOLERANCE, "canonical device collides with the shallow lid")
    _close(shallow_distance, 1.192686, "canonical_device_shallow_lid_distance", 0.001)

    restraints = corner_restraint_components(params)
    pads = foam_landing_pad_components(params)
    keepout_probes = _control_keepout_probes(params)
    for restraint_name, restraint in restraints:
        _require(_intersection_volume(restraint, final_tub) > 0.0, f"{restraint_name} is not joined to the final tub")
        _require(all(_intersection_volume(restraint, probe) <= VOLUME_TOLERANCE for probe in keepout_probes), f"{restraint_name} intersects a control keepout")
        _require(all(_intersection_volume(restraint, pad) <= VOLUME_TOLERANCE for _, pad in pads), f"{restraint_name} intersects a foam landing pad")
    _require(all(_intersection_volume(device, pad) <= VOLUME_TOLERANCE for _, pad in pads), "canonical device intersects a foam landing pad")

    _require(cast(cq.Shape, final_tub.val()).isValid() and len(final_tub.solids().vals()) == 1, "final deep tub must be one valid solid")
    for actual, expected, axis in zip(_bbox_tuple(final_tub), EXPECTED_TUB_BBOX, ("xmin", "xmax", "ymin", "ymax", "zmin", "zmax")):
        _close(actual, expected, f"final_tub_{axis}")
    base_tub = cad.build_deep_tub_base(params)
    base_volume_delta = _volume(base_tub) - CURRENT_BASE_VOLUME
    final_volume_delta = _volume(final_tub) - CURRENT_FINAL_VOLUME
    _close(base_volume_delta, EXPECTED_BASE_VOLUME_DELTA, "deep_base_volume_delta", 0.001)
    _close(final_volume_delta, EXPECTED_FINAL_VOLUME_DELTA, "deep_final_volume_delta", 0.001)
    validate_device_orientation(params, final_tub, device, base_tub)
    slopes = tuple(f"{restraint.name}={_minimum_support_slope(continuous_shelf_support(restraint, params)):.3f}deg" for restraint in corner_restraints(params))
    print("actual_device=source_built transform=x=250.8-x,y=4.2+y,z=42.0-z bbox=PASS contacts=all4")
    print(f"actual_device_contacts={';'.join(contact_metrics)}")
    print(f"actual_device_support_slopes={','.join(slopes)} deep_base_volume_delta={base_volume_delta:+.6f}mm3 deep_final_volume_delta={final_volume_delta:+.6f}mm3")
    print(f"actual_device_insertion_sweep=stations={len(insertion_offsets)} offsets={min(insertion_offsets):.2f}..{max(insertion_offsets):.2f}mm critical_interval=>32.65mm collision=NONE shallow_lid_collision={shallow_collision:.3e}mm3 min_distance={shallow_distance:.6f}mm")
    print("actual_device_fit=solid=PASS bbox=UNCHANGED supports=JOINED controls=NONE foam=NONE")
