from __future__ import annotations

import unittest
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .protective_case_geometry import hinge_centers, load_parameters, normalize_source_half
    from .protective_case_mating_interface import (
        apply_deep_mating_interface,
        apply_shallow_mating_interface,
        corner_fill_components,
        corrected_outer_bbox_delta,
        deep_receiver_components,
        hinge_profile_symmetric_difference,
        main_cavity_prism,
        opening_prism,
        minimum_normal_corner_wall,
        original_diagonal_clash,
        receiver_exterior_lands,
        shallow_hinge_gap_bar_relief_components,
        lip_corner_relief_components,
        shallow_tongue_components,
        transformed_hinge_profile,
        validate_mating_interface,
    )
    from .validate_protective_case import intersection_volume
except ImportError:
    import generate_protective_case_cadquery as cad
    from protective_case_geometry import hinge_centers, load_parameters, normalize_source_half
    from protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface, corner_fill_components, corrected_outer_bbox_delta, deep_receiver_components, hinge_profile_symmetric_difference, lip_corner_relief_components, main_cavity_prism, minimum_normal_corner_wall, opening_prism, original_diagonal_clash, receiver_exterior_lands, shallow_hinge_gap_bar_relief_components, shallow_tongue_components, transformed_hinge_profile, validate_mating_interface
    from validate_protective_case import intersection_volume


VOLUME_TOLERANCE = 1.0e-7


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def fit_candidate(params: dict, fit: dict, shift_x: float, shift_y: float) -> cq.Workplane:
    center_x, center_y = params["device"]["center"]
    return (
        cq.Workplane("XY")
        .box(fit["width"], fit["depth"], 7.38, centered=(True, True, False))
        .translate((center_x + shift_x, center_y + shift_y, 52.55))
        .edges("|Z")
        .fillet(fit["radius"])
    )


class ProtectiveCaseMatingInterfaceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.pristine_deep = cad.build_pristine_deep_tub(cls.params)
        cls.pristine_shallow = cad.build_pristine_shallow_lid(cls.params)
        cls.corrected_deep = apply_deep_mating_interface(cls.params, cls.pristine_deep)
        cls.corrected_shallow = apply_shallow_mating_interface(cls.params, cls.pristine_shallow)

    def test_opening_and_rail_contract(self) -> None:
        params = self.params
        mating = params["mating_interface"]
        self.assertEqual(mating["main_cavity"], {"width": 251.2, "depth": 144.0, "radius": 9.5, "center": [127.8, 74.2], "deep_z": [2.2, 54.65], "shallow_z": [54.85, 62.65]})
        self.assertEqual(mating["opening"], {"width": 250.0, "depth": 142.0, "radius": 8.5, "center": [127.8, 74.2], "cut_z": [52.55, 54.85]})
        self.assertEqual(mating["tongue"], {"thickness": 1.3, "z": [52.65, 54.85], "corner_tangent_relief": 1.0, "south_hinge_relief_margin": 1.0})
        self.assertEqual(mating["receiver"], {"xy_clearance": 0.25, "cut_z": [52.55, 54.65]})
        opening = opening_prism(params)
        opening_box = shape_box(opening)
        for actual, expected in zip((opening_box.xmin, opening_box.xmax, opening_box.ymin, opening_box.ymax, opening_box.zmin, opening_box.zmax), (2.8, 252.8, 3.2, 145.2, 52.55, 54.85)):
            self.assertAlmostEqual(actual, expected, places=6)
        self.assertTrue(any(abs(getattr(cast(cq.Edge, edge), "radius")() - 8.5) <= 0.05 for edge in opening.edges().vals() if cast(cq.Edge, edge).geomType() == "CIRCLE"))
        expected_rails = (
            ("mating_tongue_west", (1.50, 2.80, 12.70, 135.70, 52.65, 54.85)),
            ("mating_tongue_east", (252.80, 254.10, 12.70, 135.70, 52.65, 54.85)),
            ("mating_tongue_north", (12.30, 243.30, 145.20, 146.50, 52.65, 54.85)),
            ("mating_tongue_south_west", (12.30, 47.65, 1.90, 3.20, 52.65, 54.85)),
            ("mating_tongue_south_center", (80.15, 175.45, 1.90, 3.20, 52.65, 54.85)),
            ("mating_tongue_south_east", (207.95, 243.30, 1.90, 3.20, 52.65, 54.85)),
        )
        for (name, rail), (expected_name, expected_bounds) in zip(shallow_tongue_components(params), expected_rails):
            self.assertEqual(name, expected_name)
            box = shape_box(rail)
            for actual, expected in zip((box.xmin, box.xmax, box.ymin, box.ymax, box.zmin, box.zmax), expected_bounds):
                self.assertAlmostEqual(actual, expected, places=6)
        for (name, receiver), (_, rail) in zip(deep_receiver_components(params), shallow_tongue_components(params)):
            receiver_box = shape_box(receiver)
            rail_box = shape_box(rail)
            for actual, expected in zip((receiver_box.xmin, receiver_box.xmax, receiver_box.ymin, receiver_box.ymax), (rail_box.xmin - 0.25, rail_box.xmax + 0.25, rail_box.ymin - 0.25, rail_box.ymax + 0.25)):
                self.assertAlmostEqual(actual, expected, places=6)
            self.assertAlmostEqual(receiver_box.zmin, 52.55, places=6)
            self.assertAlmostEqual(receiver_box.zmax, 54.65, places=6)

    def test_corrected_opening_and_nominal_shifted_fit(self) -> None:
        params = self.params
        full_opening = opening_prism(params)
        self.assertLessEqual(intersection_volume(full_opening, self.corrected_shallow), VOLUME_TOLERANCE)
        for index in range(46):
            z = 52.55 + 0.05 * index
            self.assertLessEqual(intersection_volume(opening_prism(params, z, z + 0.01), self.corrected_shallow), VOLUME_TOLERANCE, f"Z{z:.2f}")
        nominal = params["device"]["envelope"]
        for shift_x, shift_y in ((0.0, 0.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0)):
            candidate = fit_candidate(params, nominal, shift_x, shift_y)
            self.assertLessEqual(intersection_volume(candidate, self.corrected_shallow), VOLUME_TOLERANCE, f"shift {shift_x},{shift_y}")
        self.assertGreater(intersection_volume(fit_candidate(params, nominal, 0.0, 0.0), self.pristine_shallow), 0.0)

    def test_original_diagonal_clash_is_reproduced_and_derived(self) -> None:
        clash, penetration = original_diagonal_clash(self.params, self.pristine_shallow)
        self.assertGreaterEqual(clash, 3.08)
        self.assertLessEqual(clash, 3.19)
        self.assertAlmostEqual(clash, 3.135323, places=6)
        self.assertAlmostEqual(penetration, 0.417, delta=0.01)

    def test_upstream_hinge_profiles_are_preserved_in_both_directions(self) -> None:
        params = self.params
        source = cad.build_source_box(params)
        pristine = {"deep": normalize_source_half(source.top(), params), "shallow": normalize_source_half(source.bottom(), params)}
        corrected = {"deep": apply_deep_mating_interface(params, pristine["deep"]), "shallow": apply_shallow_mating_interface(params, pristine["shallow"])}
        for center in hinge_centers(params):
            for variant in ("deep", "shallow"):
                differences = hinge_profile_symmetric_difference(pristine[variant], corrected[variant], transformed_hinge_profile(params, center, variant))
                self.assertLessEqual(differences[0], VOLUME_TOLERANCE)
                self.assertLessEqual(differences[1], VOLUME_TOLERANCE)

    def test_receiver_lands_corner_depth_and_outer_bounds_are_stable(self) -> None:
        lands = receiver_exterior_lands(self.params)
        self.assertEqual([name for name, _ in lands], ["mating_receiver_west", "mating_receiver_east", "mating_receiver_north", "mating_receiver_south_west", "mating_receiver_south_center", "mating_receiver_south_east"])
        for (_, land), expected in zip(lands, (1.25, 1.25, 1.65, 1.65, 1.65, 1.65)):
            self.assertAlmostEqual(land, expected, places=6)
        self.assertGreaterEqual(min(land for _, land in lands), 1.20)
        self.assertGreaterEqual(minimum_normal_corner_wall(self.params), 1.2)
        self.assertAlmostEqual(minimum_normal_corner_wall(self.params), 1.389444, places=5)
        self.assertLessEqual(corrected_outer_bbox_delta(self.pristine_deep, self.corrected_deep), 0.05)
        self.assertLessEqual(corrected_outer_bbox_delta(self.pristine_shallow, self.corrected_shallow), 0.05)

    def test_full_height_volume_and_attachment_metrics(self) -> None:
        shallow_cut = volume(self.pristine_shallow.cut(self.corrected_shallow))
        shallow_added = volume(self.corrected_shallow.cut(self.pristine_shallow))
        deep_cut = volume(self.pristine_deep.cut(self.corrected_deep))
        self.assertAlmostEqual(shallow_cut, 5272.714443, delta=0.05)
        self.assertAlmostEqual(shallow_added, 1017.110169, delta=0.05)
        self.assertAlmostEqual(volume(self.corrected_shallow), 98854.965033573, places=6)
        self.assertAlmostEqual(deep_cut, 1808.397280, delta=0.05)
        self.assertGreaterEqual(min(intersection_volume(rail, self.pristine_shallow) for _, rail in shallow_tongue_components(self.params)), 58.3275 - 1.0e-6)

    def test_hinge_gap_bar_relief_contract(self) -> None:
        reliefs = shallow_hinge_gap_bar_relief_components(self.params)
        self.assertEqual([name for name, _ in reliefs], ["shallow_hinge_gap_bar_relief_west", "shallow_hinge_gap_bar_relief_east"])
        for (name, relief), expected_x in zip(reliefs, ((47.65, 80.15), (175.45, 207.95))):
            box = shape_box(relief)
            self.assertAlmostEqual(volume(relief), 71.5, places=6)
            self.assertAlmostEqual(box.xmin, expected_x[0], places=6)
            self.assertAlmostEqual(box.xmax, expected_x[1], places=6)
            self.assertAlmostEqual(box.ymin, 2.2, places=6)
            self.assertAlmostEqual(box.ymax, 3.2, places=6)
            self.assertAlmostEqual(box.zmin, 52.65, places=6)
            self.assertAlmostEqual(box.zmax, 54.85, places=6)
            self.assertAlmostEqual(volume(self.pristine_shallow.intersect(relief)), 53.024822397, places=6)
            self.assertLessEqual(intersection_volume(self.corrected_shallow, relief), VOLUME_TOLERANCE)
        self.assertAlmostEqual(volume(self.pristine_shallow.cut(self.corrected_shallow)), 5272.714443132, places=6)

    def test_validator_rejects_either_missing_hinge_gap_bar_relief(self) -> None:
        for omitted_name, _ in shallow_hinge_gap_bar_relief_components(self.params):
            broken = self.pristine_shallow.cut(opening_prism(self.params))
            for _, relief in lip_corner_relief_components(self.params):
                broken = broken.cut(relief)
            for name, relief in shallow_hinge_gap_bar_relief_components(self.params):
                if name != omitted_name:
                    broken = broken.cut(relief)
            broken = broken.cut(main_cavity_prism(self.params, "shallow"))
            for _, fill in corner_fill_components(self.params, "shallow"):
                broken = broken.union(fill)
            for _, rail in shallow_tongue_components(self.params):
                broken = broken.union(rail)
            with self.assertRaises(ValueError):
                validate_mating_interface(self.params, self.pristine_deep, self.pristine_shallow, self.corrected_deep, broken.clean())

    def test_validator_rejects_deep_half_without_corner_fills(self) -> None:
        broken = self.pristine_deep.cut(main_cavity_prism(self.params, "deep"))
        for _, receiver in deep_receiver_components(self.params):
            broken = broken.cut(receiver).clean()
        with self.assertRaises(ValueError):
            validate_mating_interface(self.params, self.pristine_deep, self.pristine_shallow, broken, self.corrected_shallow)

if __name__ == "__main__":
    unittest.main()
