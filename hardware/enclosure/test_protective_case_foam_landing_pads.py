from __future__ import annotations

import unittest
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .protective_case_bottom_latches import bottom_latch_components
    from .protective_case_corner_restraints import _downward_horizontal_faces, corner_restraint_components, outside_volume
    from .protective_case_foam_landing_pads import (
        _bbox_distance,
        _minimum_pad_slope,
        _source_hardware_components,
        foam_landing_pad_components,
        foam_landing_pad_flat_face,
        foam_landing_pad_probe,
        foam_landing_pad_specs,
    )
    from .protective_case_geometry import build_device_insertion, dimensions, load_parameters
    from .protective_case_keepouts import control_contact_keepouts
    from .validate_protective_case import intersection_volume, prism, upstream_outer_envelope
except ImportError:
    import generate_protective_case_cadquery as cad
    from protective_case_bottom_latches import bottom_latch_components
    from protective_case_corner_restraints import _downward_horizontal_faces, corner_restraint_components, outside_volume
    from protective_case_foam_landing_pads import _bbox_distance, _minimum_pad_slope, _source_hardware_components, foam_landing_pad_components, foam_landing_pad_flat_face, foam_landing_pad_probe, foam_landing_pad_specs
    from protective_case_geometry import build_device_insertion, dimensions, load_parameters
    from protective_case_keepouts import control_contact_keepouts
    from validate_protective_case import intersection_volume, prism, upstream_outer_envelope


def shape_box(model: cq.Workplane | cq.Shape) -> cq.BoundBox:
    return cast(cq.Shape, model.val() if isinstance(model, cq.Workplane) else model).BoundingBox()


def volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


class ProtectiveCaseFoamLandingPadTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.dims = dimensions(cls.params)
        cls.pristine = cad.build_pristine_deep_tub(cls.params)
        cls.outer = upstream_outer_envelope(cls.params)

    def test_four_pads_have_exact_wall_placements_profiles_and_solids(self) -> None:
        self.assertEqual([name for name, _ in foam_landing_pad_components(self.params)], ["foam_landing_pad_west", "foam_landing_pad_east", "foam_landing_pad_south", "foam_landing_pad_north"])
        expected = {
            "foam_landing_pad_west": (1.8, 62.2, 26.0, 2.6, 86.2, 38.0),
            "foam_landing_pad_east": (253.0, 62.2, 26.0, 253.8, 86.2, 38.0),
            "foam_landing_pad_south": (115.8, 1.8, 26.0, 139.8, 2.6, 38.0),
            "foam_landing_pad_north": (115.8, 145.8, 26.0, 139.8, 146.6, 38.0),
        }
        for name, pad in foam_landing_pad_components(self.params):
            shape = cast(cq.Shape, pad.val())
            self.assertTrue(shape.isValid())
            self.assertEqual(len(pad.solids().vals()), 1)
            self.assertEqual(len(pad.shells().vals()), 1)
            measured = shape_box(pad)
            for actual, target in zip((measured.xmin, measured.ymin, measured.zmin, measured.xmax, measured.ymax, measured.zmax), expected[name]):
                self.assertAlmostEqual(actual, target, places=4)
            radii = [getattr(edge, "radius")() for edge in pad.edges().vals() if edge.geomType() == "CIRCLE"]
            self.assertEqual(sum(abs(radius - 2.0) <= 0.01 for radius in radii), 4)
            self.assertEqual(sum(abs(radius - 1.0) <= 0.01 for radius in radii), 4)

    def test_flat_faces_and_foam_probes_are_usable(self) -> None:
        expected_patch = {
            "foam_landing_pad_west": (2.6, 64.2, 27.0, 2.6, 84.2, 37.0),
            "foam_landing_pad_east": (253.0, 64.2, 27.0, 253.0, 84.2, 37.0),
            "foam_landing_pad_south": (117.8, 2.6, 27.0, 137.8, 2.6, 37.0),
            "foam_landing_pad_north": (117.8, 145.8, 27.0, 137.8, 145.8, 37.0),
        }
        for (name, pad), spec in zip(foam_landing_pad_components(self.params), foam_landing_pad_specs(self.params)):
            flat_face = foam_landing_pad_flat_face(pad, spec)
            probe = foam_landing_pad_probe(self.params, spec)
            probe_box = shape_box(probe)
            for actual, target in zip((probe_box.xmin, probe_box.ymin, probe_box.zmin, probe_box.xmax, probe_box.ymax, probe_box.zmax), expected_patch[name]):
                self.assertAlmostEqual(actual, target, places=4)
            self.assertAlmostEqual(flat_face.Area(), 219.1416, places=3)
            self.assertAlmostEqual(flat_face.intersect(cast(cq.Shape, probe.val())).Area(), probe.val().Area(), places=3)

    def test_pads_meet_wall_keepout_shelf_and_insertion_contracts(self) -> None:
        pads = foam_landing_pad_components(self.params)
        keepouts = control_contact_keepouts(self.params)
        shelves = tuple(component for _, component in corner_restraint_components(self.params))
        for name, pad in pads:
            self.assertAlmostEqual(intersection_volume(pad, self.pristine), 107.0746, places=3)
            self.assertLessEqual(outside_volume(pad, self.outer), 1.0e-7)
            self.assertTrue(all(intersection_volume(pad, prism(keepout.bounds[0], keepout.bounds[1], 0.0, keepout.bounds[2] - keepout.bounds[0], keepout.bounds[3] - keepout.bounds[1], self.dims.height)) <= 1.0e-7 for keepout in keepouts), name)
            self.assertTrue(all(intersection_volume(pad, shelf) <= 1.0e-7 for shelf in shelves), name)
            self.assertGreaterEqual(_minimum_pad_slope(pad), 45.0)
            self.assertTrue(all(abs(face.normalAt().z) < 0.999 for face in cast(list[cq.Shape], pad.faces().vals())))
        for check_fit in (False, True):
            for base_z in self.params["insertion"]["descent_sample_z"]:
                insertion = build_device_insertion(self.params, check_fit, base_z)
                self.assertTrue(all(intersection_volume(insertion, pad) <= 1.0e-7 for _, pad in pads), (check_fit, base_z))
        proud = self.params["foam_landing_pads"]["cavity_proud"]
        for fit_name, expected in (("envelope", (1.2, 1.6)), ("check_fit", (0.7, 1.1))):
            fit = self.params["device"][fit_name]
            self.assertAlmostEqual((self.dims.inner_width - fit["width"]) / 2.0 - proud, expected[0], places=6)
            self.assertAlmostEqual((self.dims.inner_depth - fit["depth"]) / 2.0 - proud, expected[1], places=6)

    def test_final_tub_keeps_single_shell_and_existing_underside_contract(self) -> None:
        final_tub = cad.build_deep_tub(self.params)
        self.assertTrue(cast(cq.Shape, final_tub.val()).isValid())
        self.assertEqual(len(final_tub.solids().vals()), 1)
        self.assertEqual(len(final_tub.shells().vals()), 1)
        underside_faces = _downward_horizontal_faces(final_tub)
        self.assertEqual(len(underside_faces), 4)
        self.assertLessEqual(max(area for area, _ in underside_faces), 0.286)
        self.assertLessEqual(max(width for _, width in underside_faces), 0.30)
        seam_clearance = self.params["insertion"]["entry_base_z"] - max(shape_box(pad).zmax for _, pad in foam_landing_pad_components(self.params))
        self.assertGreaterEqual(seam_clearance, 16.8)
        source_features = _source_hardware_components(self.params)
        separation = min(_bbox_distance(pad, feature) for _, pad in foam_landing_pad_components(self.params) for feature in source_features)
        self.assertGreaterEqual(separation, 10.8)
        latch_features = tuple(component for _, component in bottom_latch_components(self.params, "deep"))
        latch_separation = min(_bbox_distance(pad, latch) for _, pad in foam_landing_pad_components(self.params) for latch in latch_features)
        self.assertAlmostEqual(latch_separation, 32.429, delta=0.05)


if __name__ == "__main__":
    unittest.main()
