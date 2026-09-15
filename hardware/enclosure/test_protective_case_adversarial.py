from __future__ import annotations

import unittest
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .protective_case_corner_restraints import corner_restraint_components, wall_witness
    from .protective_case_foam_landing_pads import add_foam_landing_pads
    from .protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface
    from .protective_case_geometry import corner_restraints, dimensions, hinge_axis, hinge_centers, load_parameters, normalize_source_half
    from .protective_case_keepouts import control_contact_keepouts
    from .validate_protective_case import filament_pin, hinge_attachment_proxy_probe, hinge_knuckle_centers, hinge_section_probe, intersection_volume, paddle_envelope, transformed_hinge_profile
except ImportError:
    import generate_protective_case_cadquery as cad
    from protective_case_corner_restraints import corner_restraint_components, wall_witness
    from protective_case_foam_landing_pads import add_foam_landing_pads
    from protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface
    from protective_case_geometry import corner_restraints, dimensions, hinge_axis, hinge_centers, load_parameters, normalize_source_half
    from protective_case_keepouts import control_contact_keepouts
    from validate_protective_case import filament_pin, hinge_attachment_proxy_probe, hinge_knuckle_centers, hinge_section_probe, intersection_volume, paddle_envelope, transformed_hinge_profile


VOLUME_TOLERANCE = 1.0e-7


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def prism(x: float, y: float, z: float, width: float, depth: float, height: float) -> cq.Workplane:
    return cq.Workplane("XY").box(width, depth, height, centered=(False, False, False)).translate((x, y, z))


class ProtectiveCaseAdversarialTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.dims = dimensions(cls.params)

    def test_project_additions_do_not_cut_upstream_geometry(self) -> None:
        params = self.params
        deep = cad.build_pristine_deep_tub(params)
        deep_with_foam = add_foam_landing_pads(params, apply_deep_mating_interface(params, deep))
        augmented = cad.build_deep_tub(params)
        shallow = cad.build_pristine_shallow_lid(params)
        shallow_corrected = apply_shallow_mating_interface(params, shallow)
        self.assertLessEqual(volume(apply_deep_mating_interface(params, deep).cut(augmented)), VOLUME_TOLERANCE)
        self.assertLessEqual(volume(shallow_corrected.cut(cad.build_shallow_lid(params))), VOLUME_TOLERANCE)
        for before, after, probe in (
            (deep_with_foam, augmented, prism(40.0, 0.0, 0.0, 175.6, 3.0, 54.6)),
            (deep_with_foam, augmented, prism(40.0, 40.0, 53.5, 175.6, 68.4, 1.0)),
            (shallow_corrected, cad.build_shallow_lid(params), prism(40.0, 0.0, 52.65, 175.6, 3.0, 12.2)),
            (shallow_corrected, cad.build_shallow_lid(params), prism(40.0, 40.0, 52.65, 175.6, 68.4, 1.0)),
        ):
            self.assertAlmostEqual(volume(before.intersect(probe)), volume(after.intersect(probe)), places=6)

    def test_hinge_sections_and_load_path_metrics(self) -> None:
        params = self.params
        source_box = cad.build_source_box(params)
        deep = apply_deep_mating_interface(params, normalize_source_half(source_box.top(), params))
        shallow = apply_shallow_mating_interface(params, normalize_source_half(source_box.bottom(), params))
        hinge = params["parametric_box"]["hinge"]
        section_areas = {"deep": [], "shallow": []}
        attachment_proxy_areas = {"deep": [], "shallow": []}
        for center in hinge_centers(params):
            for variant, half in (("deep", deep), ("shallow", shallow)):
                profile = transformed_hinge_profile(params, center, variant)
                self.assertEqual(len(profile.solids().vals()), 1)
                for local_x in hinge_knuckle_centers(variant):
                    section = half.intersect(hinge_section_probe(params, center, variant, local_x))
                    self.assertGreater(len(section.solids().vals()), 0, f"{variant} {center + local_x}")
                    section_areas[variant].append(volume(section) / 0.05)
                    attachment_proxy_areas[variant].append(volume(profile.intersect(hinge_attachment_proxy_probe(params, center, variant, local_x))) / 0.05)
        self.assertEqual(len(section_areas["deep"]), 4)
        self.assertEqual(len(section_areas["shallow"]), 6)
        self.assertGreaterEqual(min(section_areas["deep"]), 37.95)
        self.assertLessEqual(max(section_areas["deep"]), 38.06)
        self.assertAlmostEqual(sum(section_areas["deep"]) / len(section_areas["deep"]), 38.005673, places=5)
        self.assertGreaterEqual(min(section_areas["shallow"]), 38.31)
        self.assertLessEqual(max(section_areas["shallow"]), 38.42)
        self.assertAlmostEqual(sum(section_areas["shallow"]) / len(section_areas["shallow"]), 38.365673, places=5)
        self.assertGreaterEqual(min(attachment_proxy_areas["deep"]), 25.0)
        self.assertGreaterEqual(min(attachment_proxy_areas["shallow"]), 25.0)
        self.assertGreaterEqual(hinge["kunkle_size"] - hinge["clearance"], 5.0)

    def test_paddles_overlap_pins_and_clear_both_halves(self) -> None:
        params = self.params
        source_box = cad.build_source_box(params)
        deep = normalize_source_half(source_box.top(), params)
        shallow = normalize_source_half(source_box.bottom(), params)
        for center in hinge_centers(params):
            pin = filament_pin(params, center)
            for side in ("west", "east"):
                paddle = paddle_envelope(params, center, side)
                self.assertGreater(intersection_volume(pin, paddle), 0.0)
                self.assertLessEqual(intersection_volume(paddle, deep), VOLUME_TOLERANCE)
                self.assertLessEqual(intersection_volume(paddle, shallow), VOLUME_TOLERANCE)
                self.assertLessEqual(shape_box(paddle).ymax, hinge_axis(params)[0] + params["hinge_pins"]["paddle_envelope"]["inward_allowance"] + 0.001)

    def test_corner_witnesses_are_specific_and_keepouts_are_disjoint(self) -> None:
        params = self.params
        specs = params["corner_restraints"]
        pristine = cad.build_pristine_deep_tub(params)
        keepouts = control_contact_keepouts(params)
        adjacent = {"NW": ("west", "north"), "SW": ("west", "south"), "NE": ("east", "north"), "SE": ("east", "south")}
        for restraint in corner_restraints(params):
            component = dict(corner_restraint_components(params))[f"corner_restraint_{restraint.name.lower()}"]
            self.assertFalse(
                any(
                    intersection_volume(component, prism(keepout.bounds[0], keepout.bounds[1], 0.0, keepout.bounds[2] - keepout.bounds[0], keepout.bounds[3] - keepout.bounds[1], self.dims.height)) > VOLUME_TOLERANCE
                    for keepout in keepouts
                )
            )
            for side in adjacent[restraint.name]:
                witness = wall_witness(params, restraint, side)
                self.assertGreaterEqual(intersection_volume(component, pristine, witness), specs["wall_intersection_volume_min"])
                section = component.intersect(pristine).intersect(witness).intersect(prism(0.0, 0.0, 3.975, self.dims.width, self.dims.depth, 0.05))
                section_box = shape_box(section)
                length = section_box.ymax - section_box.ymin if side in ("west", "east") else section_box.xmax - section_box.xmin
                self.assertGreaterEqual(length, specs["support_contact_length_min"])


if __name__ == "__main__":
    unittest.main()
