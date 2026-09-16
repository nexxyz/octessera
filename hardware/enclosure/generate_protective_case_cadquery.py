from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import cast

import cadquery as cq

try:
    from . import generate_multimaterial_caps_3mf as multimaterial
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_branding import (
        compose_debossed_deep_tub,
        compose_debossed_shallow_lid,
        compose_multicolor_deep_tub,
        compose_multicolor_shallow_lid,
    )
    from .protective_case_bottom_latches import add_bottom_latches
    from .protective_case_corner_restraints import add_corner_restraints
    from .protective_case_device_orientation import add_device_orientation_guide
    from .protective_case_foam_landing_pads import add_foam_landing_pads
    from .protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface
    from .protective_case_geometry import PARAMS, artifact_paths, dimensions, load_parameters, normalize_source_half, parametric_box_measures
except ImportError:
    import generate_multimaterial_caps_3mf as multimaterial
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_branding import compose_debossed_deep_tub, compose_debossed_shallow_lid, compose_multicolor_deep_tub, compose_multicolor_shallow_lid
    from protective_case_bottom_latches import add_bottom_latches
    from protective_case_corner_restraints import add_corner_restraints
    from protective_case_device_orientation import add_device_orientation_guide
    from protective_case_foam_landing_pads import add_foam_landing_pads
    from protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface
    from protective_case_geometry import PARAMS, artifact_paths, dimensions, load_parameters, normalize_source_half, parametric_box_measures


def build_source_box(params: dict):
    return upstream.build_box(parametric_box_measures(params, ()))


def build_pristine_deep_tub(params: dict) -> cq.Workplane:
    return normalize_source_half(build_source_box(params).top(), params)


def build_pristine_shallow_lid(params: dict) -> cq.Workplane:
    return normalize_source_half(build_source_box(params).bottom(), params)


def build_deep_tub_base(params: dict) -> cq.Workplane:
    corrected = apply_deep_mating_interface(params, build_pristine_deep_tub(params))
    latched = add_bottom_latches(params, corrected, "deep")
    return add_foam_landing_pads(params, add_corner_restraints(params, latched))


def build_deep_tub(params: dict) -> cq.Workplane:
    return add_device_orientation_guide(params, build_deep_tub_base(params))


def build_shallow_lid(params: dict) -> cq.Workplane:
    corrected = apply_shallow_mating_interface(params, build_pristine_shallow_lid(params))
    return add_bottom_latches(params, corrected, "shallow")


def build_debossed_deep_tub(params: dict) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return compose_debossed_deep_tub(params, build_deep_tub)


def build_multicolor_deep_tub(params: dict) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return compose_multicolor_deep_tub(params, build_deep_tub_base)


def build_debossed_shallow_lid(params: dict) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return compose_debossed_shallow_lid(params, build_shallow_lid)


def build_multicolor_shallow_lid(params: dict) -> tuple[cq.Workplane, list[tuple[str, cq.Workplane]]]:
    return compose_multicolor_shallow_lid(params, build_shallow_lid)


@dataclass(frozen=True)
class PrintTransform:
    rotate_x_degrees: float
    translation: tuple[float, float, float]


def _rotated_for_print(model: cq.Workplane, rotate_x_degrees: float) -> cq.Workplane:
    if rotate_x_degrees == 0.0:
        return model
    return model.rotate((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), rotate_x_degrees)


def print_transform(owner_body: cq.Workplane, exterior: str) -> PrintTransform:
    if exterior == "positive_z":
        rotate_x_degrees = 180.0
    elif exterior == "negative_z":
        rotate_x_degrees = 0.0
    else:
        raise ValueError(f"unknown exterior orientation: {exterior}")
    bbox = cast(cq.Shape, _rotated_for_print(owner_body, rotate_x_degrees).val()).BoundingBox()
    return PrintTransform(rotate_x_degrees, (-bbox.xmin, -bbox.ymin, -bbox.zmin))


def apply_print_transform(model: cq.Workplane, transform: PrintTransform) -> cq.Workplane:
    return _rotated_for_print(model, transform.rotate_x_degrees).translate(transform.translation)


def orient_for_print(model: cq.Workplane, exterior: str) -> cq.Workplane:
    return apply_print_transform(model, print_transform(model, exterior))


def export_model(model: cq.Workplane, step_path: Path, stl_path: Path, exterior: str) -> None:
    step_path.parent.mkdir(parents=True, exist_ok=True)
    stl_path.parent.mkdir(parents=True, exist_ok=True)
    cq.exporters.export(model, str(step_path))
    transform = print_transform(model, exterior)
    cq.exporters.export(apply_print_transform(model, transform), str(stl_path), tolerance=0.08, angularTolerance=0.12)


def write_parts_3mf(path: Path, parts: list[tuple[str, cq.Workplane, int]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    multimaterial.write_3mf_parts(path, [multimaterial.ModelPart(name, model, extruder) for name, model, extruder in parts])
    print(f"wrote {path}")


def export_all(params: dict) -> None:
    paths = artifact_paths(params)
    debossed_deep_tub, _ = build_debossed_deep_tub(params)
    export_model(debossed_deep_tub, paths[0], paths[1], "negative_z")
    write_parts_3mf(paths[2], [("protective_case_deep_tub_debossed_logo", orient_for_print(debossed_deep_tub, "negative_z"), 1)])
    multicolor_deep_tub, deep_branding = build_multicolor_deep_tub(params)
    deep_transform = print_transform(multicolor_deep_tub, "negative_z")
    write_parts_3mf(
        paths[3],
        [("protective_case_deep_tub_body", apply_print_transform(multicolor_deep_tub, deep_transform), 1)]
        + [(name, apply_print_transform(part, deep_transform), 2) for name, part in deep_branding],
    )
    debossed_shallow_lid, _ = build_debossed_shallow_lid(params)
    export_model(debossed_shallow_lid, paths[4], paths[5], "positive_z")
    write_parts_3mf(paths[6], [("protective_case_shallow_lid_debossed_branding", orient_for_print(debossed_shallow_lid, "positive_z"), 1)])
    multicolor_shallow_lid, shallow_branding = build_multicolor_shallow_lid(params)
    shallow_transform = print_transform(multicolor_shallow_lid, "positive_z")
    write_parts_3mf(
        paths[7],
        [("protective_case_shallow_lid_body", apply_print_transform(multicolor_shallow_lid, shallow_transform), 1)]
        + [(name, apply_print_transform(part, shallow_transform), 2) for name, part in shallow_branding],
    )


def main() -> None:
    export_all(load_parameters(PARAMS))


if __name__ == "__main__":
    main()
