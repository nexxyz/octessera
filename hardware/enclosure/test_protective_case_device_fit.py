from __future__ import annotations

import copy
import unittest
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .protective_case_corner_restraints import corner_restraint_components, validate_corner_restraints
    from .protective_case_device_fit import (
        EXPECTED_CONTACTS,
        _insertion_sweep_offsets,
        build_face_down_reference_top,
        support_contacts,
        validate_reference_top_fit,
    )
    from .protective_case_geometry import (
        dimensions,
        face_down_reference_xy,
        face_down_reference_feature_xy,
        load_parameters,
        load_source_parameters,
        local_to_case,
    )
    from .protective_case_mating_interface import apply_shallow_mating_interface
    from .protective_case_keepouts import control_contact_keepouts
    from .validate_protective_case import intersection_volume, prism, upstream_outer_envelope
except ImportError:
    import generate_protective_case_cadquery as cad
    from protective_case_corner_restraints import corner_restraint_components, validate_corner_restraints
    from protective_case_device_fit import EXPECTED_CONTACTS, _insertion_sweep_offsets, build_face_down_reference_top, support_contacts, validate_reference_top_fit
    from protective_case_geometry import dimensions, face_down_reference_feature_xy, face_down_reference_xy, load_parameters, load_source_parameters, local_to_case
    from protective_case_mating_interface import apply_shallow_mating_interface
    from protective_case_keepouts import control_contact_keepouts
    from validate_protective_case import intersection_volume, prism, upstream_outer_envelope


class ProtectiveCaseDeviceFitTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.reference_tops = {
            variant: build_face_down_reference_top(cls.params, variant)
            for variant in cls.params["device"]["reference_top_variants"]
        }
        cls.reference_top = cls.reference_tops["raspberry-pi-zero-2w"]

    def test_face_down_feature_transform_uses_rotated_case_xy_owner(self) -> None:
        source = load_source_parameters()
        point = source["features_local"]["oled_screen_center"]
        self.assertEqual(local_to_case(source, point), (71.33500000000001, 111.0))
        self.assertEqual(
            self.params["device"]["face_down_transform"],
            {"rotate_y_degrees": 180.0, "translate": [250.8, 4.2, 42.0]},
        )
        self.assertNotIn("measured_origin", self.params["device"])
        self.assertAlmostEqual(face_down_reference_feature_xy(source, point, self.params, -0.5, -0.3)[0], 179.965)
        self.assertAlmostEqual(face_down_reference_feature_xy(source, point, self.params, -0.5, -0.3)[1], 114.9)
        key = source["features_local"]["neokey_key_centers"][0]
        self.assertAlmostEqual(face_down_reference_feature_xy(source, key, self.params, -0.25, -1.0)[0], 147.5)
        self.assertAlmostEqual(face_down_reference_feature_xy(source, key, self.params, -0.25, -1.0)[1], 78.075)
        self.assertAlmostEqual(face_down_reference_xy((70.835, 110.7), self.params)[0], 179.965)
        self.assertAlmostEqual(face_down_reference_xy((70.835, 110.7), self.params)[1], 114.9)

    def test_both_reference_top_variants_have_expected_planar_contacts(self) -> None:
        self.assertEqual(tuple(self.reference_tops), ("raspberry-pi-zero-2w", "orange-pi-zero-2w"))
        for variant, reference_top in self.reference_tops.items():
            with self.subTest(variant=variant):
                contacts = support_contacts(reference_top, self.params)
                self.assertEqual([contact.name for contact in contacts], ["NW", "SW", "NE", "SE"])
                for contact in contacts:
                    expected_area, expected_bbox, expected_z = EXPECTED_CONTACTS[contact.name]
                    self.assertAlmostEqual(contact.area, expected_area, places=3)
                    for actual, expected in zip(contact.bbox, expected_bbox):
                        self.assertAlmostEqual(actual, expected, places=3)
                    self.assertAlmostEqual(contact.z, expected_z, places=3)

    def test_reference_top_validator_qualifies_final_fit(self) -> None:
        validate_reference_top_fit(self.params)

    def test_insertion_sweep_covers_full_configured_withdrawal(self) -> None:
        offsets = _insertion_sweep_offsets(self.params)
        self.assertEqual(len(offsets), 49)
        self.assertAlmostEqual(offsets[0], 47.35, places=6)
        self.assertAlmostEqual(offsets[-1], 0.0, places=6)
        self.assertGreater(max(offsets), 32.65)
        truncated = copy.deepcopy(self.params)
        truncated["insertion"]["descent_sample_z"].pop()
        with self.assertRaisesRegex(ValueError, "49 configured stations"):
            _insertion_sweep_offsets(truncated)

    def test_reversed_shelf_elevations_report_penetration_and_gap(self) -> None:
        wrong = copy.deepcopy(self.params)
        wrong_records = {record["name"]: record for record in wrong["corner_restraints"]["records"]}
        wrong_records["NW"]["nominal_shelf_base_z"], wrong_records["NW"]["contact_z"] = 28.5, 30.0
        wrong_records["NE"]["nominal_shelf_base_z"], wrong_records["NE"]["contact_z"] = 23.5, 25.0
        with self.assertRaisesRegex(ValueError, r"NW penetration=5\.000mm.*NE gap=5\.000mm"):
            validate_reference_top_fit(wrong)

    def test_unrotated_support_polygons_hit_rotated_trellis_keepout(self) -> None:
        wrong = copy.deepcopy(self.params)
        records = {record["name"]: record for record in wrong["corner_restraints"]["records"]}
        records["NW"].update(
            {
                "shelf_polygon": [[1.62, 139.22], [1.62, 130.00], [20.10, 130.00], [20.10, 146.78], [9.18, 146.78]],
                "shelf_tabs": {"west": [1.62, 2.80, 130.00, 139.22], "north": [9.18, 20.10, 145.60, 146.78]},
                "support_lower_polygon": [[2.02, 139.32], [2.02, 136.40], [11.00, 136.40], [12.00, 137.40], [12.00, 146.38], [9.08, 146.38]],
                "support_upper_polygon": [[1.62, 139.22], [1.62, 130.00], [19.10, 130.00], [20.10, 131.00], [20.10, 146.78], [9.18, 146.78]],
                "device_corner": [20.10, 130.00],
            }
        )
        records["SW"].update(
            {
                "shelf_polygon": [[1.62, 9.18], [1.62, 18.40], [20.10, 18.40], [20.10, 1.62], [9.18, 1.62]],
                "shelf_tabs": {"west": [1.62, 2.80, 9.18, 18.40], "south": [9.18, 20.10, 1.62, 2.80]},
                "support_lower_polygon": [[2.02, 9.08], [2.02, 12.00], [11.00, 12.00], [12.00, 11.00], [12.00, 2.02], [9.08, 2.02]],
                "support_upper_polygon": [[1.62, 9.18], [1.62, 18.40], [19.10, 18.40], [20.10, 17.40], [20.10, 1.62], [9.18, 1.62]],
                "device_corner": [20.10, 18.40],
            }
        )
        keepout = next(keepout for keepout in control_contact_keepouts(self.params) if keepout.name == "NeoTrellis opening field")
        probe = prism(keepout.bounds[0], keepout.bounds[1], 0.0, keepout.bounds[2] - keepout.bounds[0], keepout.bounds[3] - keepout.bounds[1], dimensions(self.params).height)
        wrong_components = dict(corner_restraint_components(wrong))
        self.assertGreater(intersection_volume(wrong_components["corner_restraint_nw"], probe), 0.0)
        self.assertGreater(intersection_volume(wrong_components["corner_restraint_sw"], probe), 0.0)
        with self.assertRaisesRegex(ValueError, "corner_restraint_nw intersects NeoTrellis opening field"):
            validate_corner_restraints(wrong, cad.build_pristine_deep_tub(wrong), upstream_outer_envelope(wrong), cad.build_deep_tub(wrong))

    def test_shallow_lid_has_no_collision_and_cad_derived_clearance(self) -> None:
        shallow = apply_shallow_mating_interface(self.params, cad.build_pristine_shallow_lid(self.params))
        for variant, reference_top in self.reference_tops.items():
            with self.subTest(variant=variant):
                intersection = reference_top.intersect(shallow)
                self.assertEqual(sum(s.Volume() for s in cast(list[cq.Shape], intersection.solids().vals())), 0.0)
                self.assertAlmostEqual(cast(cq.Shape, reference_top.val()).distance(cast(cq.Shape, shallow.val())), 1.192686, places=5)


if __name__ == "__main__":
    unittest.main()
