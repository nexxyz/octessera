from __future__ import annotations

import copy
import unittest
from unittest.mock import patch
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from . import protective_case_bottom_latches as latches
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_device_orientation import device_orientation_guide_components
    from .protective_case_geometry import build_device_insertion, hinge_centers, load_parameters, parametric_box_measures
    from .protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface, deep_receiver_components, opening_prism, shallow_tongue_components, transformed_hinge_profile
except ImportError:
    import generate_protective_case_cadquery as cad
    import protective_case_bottom_latches as latches
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_device_orientation import device_orientation_guide_components
    from protective_case_geometry import build_device_insertion, hinge_centers, load_parameters, parametric_box_measures
    from protective_case_mating_interface import apply_deep_mating_interface, apply_shallow_mating_interface, deep_receiver_components, opening_prism, shallow_tongue_components, transformed_hinge_profile


TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7


def volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def intersection_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return volume(first.intersect(second))


def symmetric_difference_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return volume(first.cut(second)) + volume(second.cut(first))


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


class ProtectiveCaseBottomLatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.pristine_deep = cad.build_pristine_deep_tub(cls.params)
        cls.pristine_shallow = cad.build_pristine_shallow_lid(cls.params)
        cls.corrected_deep = apply_deep_mating_interface(cls.params, cls.pristine_deep)
        cls.corrected_shallow = apply_shallow_mating_interface(cls.params, cls.pristine_shallow)

    def test_source_box_has_no_owned_clips_and_profiles_are_reversed(self) -> None:
        source_box = cad.build_source_box(self.params)
        latches.validate_source_box_clip_ownership(source_box)
        old_owned = upstream.build_box(parametric_box_measures(self.params, (-55.0, 55.0)))
        with self.assertRaises(ValueError):
            latches.validate_source_box_clip_ownership(old_owned)
        self.assertEqual(latches.latch_centers(self.params), (72.8, 182.8))
        self.assertEqual(latches.seam_z(self.params), 54.85)
        expected = {
            "shallow": ((62.8, 82.8, 148.4, 151.4, 46.035410, 60.85), 522.288830, 79.945371, 18),
            "deep": ((60.8, 84.8, 148.4, 149.834972, 47.35, 51.85), 80.593856, 107.858996, 6),
        }
        for half, (bounds, expected_volume, root_area, fillet_count) in expected.items():
            components = latches.latch_profile_components(self.params, half)
            self.assertEqual(len(components), 2)
            for index, (_, profile) in enumerate(components):
                box = shape_box(profile)
                shift = 110.0 if index else 0.0
                actual = (box.xmin, box.xmax, box.ymin, box.ymax, box.zmin, box.zmax)
                target = (bounds[0] + shift, bounds[1] + shift, bounds[2], bounds[3], bounds[4], bounds[5])
                for measured, value in zip(actual, target):
                    self.assertAlmostEqual(measured, value, delta=TOLERANCE)
                self.assertAlmostEqual(volume(profile), expected_volume, delta=TOLERANCE)
                circles = [edge for edge in cast(list[cq.Shape], profile.edges().vals()) if edge.geomType() == "CIRCLE"]
                self.assertEqual(len(circles), fillet_count)
                self.assertTrue(all(abs(getattr(edge, "radius")() - 0.3) <= TOLERANCE for edge in circles))
                root_faces = [face for face in profile.faces().vals() if abs(cast(cq.Face, face).BoundingBox().ymin - 148.4) <= 1.0e-6 and abs(cast(cq.Face, face).BoundingBox().ymax - 148.4) <= 1.0e-6]
                self.assertEqual(len(root_faces), 1)
                self.assertAlmostEqual(cast(cq.Face, root_faces[0]).Area(), root_area, delta=TOLERANCE)
                mirrored = latches.transformed_clip_profile(self.params, latches.latch_centers(self.params)[index], "actuator" if half == "shallow" else "catch", mirrored=False).mirror("XY", (0.0, 0.0, 54.85))
                self.assertLessEqual(symmetric_difference_volume(profile, mirrored), VOLUME_TOLERANCE)

    def test_root_bridges_overlap_host_and_profile_without_visible_geometry(self) -> None:
        expected = {
            "shallow": (19.4, 56.95, 60.70, 14.55, 7.275, 79.945371),
            "deep": (23.4, 47.60, 51.60, 18.72, 9.36, 107.858996),
        }
        for half, host in (("shallow", self.corrected_shallow), ("deep", self.corrected_deep)):
            width, z_min, z_max, bridge_volume, overlap, root_area = expected[half]
            profiles = latches.latch_profile_components(self.params, half)
            bridges = latches.latch_bridge_components(self.params, half)
            self.assertEqual(len(bridges), 2)
            for (_, profile), (_, bridge) in zip(profiles, bridges):
                box = shape_box(bridge)
                self.assertAlmostEqual(box.xmax - box.xmin, width, places=6)
                self.assertAlmostEqual(box.ymin, 148.30, places=6)
                self.assertAlmostEqual(box.ymax, 148.50, places=6)
                self.assertAlmostEqual(box.zmin, z_min, places=6)
                self.assertAlmostEqual(box.zmax, z_max, places=6)
                self.assertAlmostEqual(volume(bridge), bridge_volume, places=6)
                self.assertAlmostEqual(intersection_volume(bridge, host), overlap, places=6)
                self.assertAlmostEqual(intersection_volume(bridge, profile), overlap, places=6)
                self.assertLessEqual(volume(bridge.cut(host.union(profile))), VOLUME_TOLERANCE)
                root_face = [face for face in profile.faces().vals() if abs(cast(cq.Face, face).BoundingBox().ymin - 148.4) <= 1.0e-6 and abs(cast(cq.Face, face).BoundingBox().ymax - 148.4) <= 1.0e-6]
                self.assertAlmostEqual(cast(cq.Face, root_face[0]).Area(), root_area, delta=TOLERANCE)

    def test_snap_sweep_and_collision_decomposition_contract(self) -> None:
        latches.validate_bottom_latches(self.params, self.corrected_deep, self.corrected_shallow)

    def test_mating_and_branding_keep_latches_intact(self) -> None:
        for half, corrected in (("shallow", self.corrected_shallow), ("deep", self.corrected_deep)):
            latched = latches.add_bottom_latches(self.params, corrected, half)
            for _, profile in latches.latch_profile_components(self.params, half):
                self.assertLessEqual(volume(profile.cut(latched)), VOLUME_TOLERANCE)
        branded_bodies = (
            ("deep", cad.build_debossed_deep_tub(self.params)[0]),
            ("deep", cad.build_multicolor_deep_tub(self.params)[0]),
            ("shallow", cad.build_debossed_shallow_lid(self.params)[0]),
            ("shallow", cad.build_multicolor_shallow_lid(self.params)[0]),
        )
        for half, body in branded_bodies:
            for _, profile in latches.latch_profile_components(self.params, half):
                self.assertLessEqual(volume(profile.cut(body)), VOLUME_TOLERANCE)
            printed = cad.orient_for_print(body, "negative_z" if half == "deep" else "positive_z")
            box = shape_box(printed)
            sizes = (box.xmax - box.xmin, box.ymax - box.ymin, box.zmax - box.zmin)
            expected_sizes = (255.6, 153.834972, 57.85) if half == "deep" else (255.6, 155.4, 16.814590)
            for actual, expected in zip(sizes, expected_sizes):
                self.assertAlmostEqual(actual, expected, delta=TOLERANCE)
            self.assertTrue(all(size <= 260.0 + TOLERANCE for size in sizes))

    def test_latches_clear_mating_hinges_guide_and_north_wall(self) -> None:
        opening = opening_prism(self.params)
        rails = shallow_tongue_components(self.params)
        receivers = deep_receiver_components(self.params)
        for half, mating_parts, hinge_variant in (
            ("shallow", rails, "shallow"),
            ("deep", receivers, "deep"),
        ):
            for _, component in latches.latch_profile_components(self.params, half) + latches.latch_bridge_components(self.params, half):
                self.assertLessEqual(intersection_volume(component, opening), VOLUME_TOLERANCE)
                self.assertTrue(all(intersection_volume(component, mating) <= VOLUME_TOLERANCE for _, mating in mating_parts))
                self.assertTrue(all(intersection_volume(component, transformed_hinge_profile(self.params, center, hinge_variant)) <= VOLUME_TOLERANCE for center in hinge_centers(self.params)))
        guide = device_orientation_guide_components(self.params)
        for _, catch in latches.latch_profile_components(self.params, "deep") + latches.latch_bridge_components(self.params, "deep"):
            self.assertTrue(all(intersection_volume(catch, component) <= VOLUME_TOLERANCE for _, component in guide))
        device = build_device_insertion(self.params)
        for _, catch in latches.latch_profile_components(self.params, "deep") + latches.latch_bridge_components(self.params, "deep"):
            self.assertLessEqual(intersection_volume(catch, device), VOLUME_TOLERANCE)
        for model, z_min, z_max in ((self.corrected_shallow, 56.95, 60.70), (self.corrected_deep, 47.60, 51.60)):
            wall = cq.Workplane("XY").box(215.0, 2.2, z_max - z_min, centered=(False, False, False)).translate((20.0, 146.2, z_min))
            self.assertAlmostEqual(intersection_volume(model, wall), volume(wall), places=6)

    def test_mutated_center_or_wall_plane_is_rejected(self) -> None:
        for mutation in ("center", "wall"):
            broken = copy.deepcopy(self.params)
            if mutation == "center":
                broken["parametric_box"]["clip"]["positions_x"][0] += 0.1
            else:
                broken["wall_planes"]["north_y"] += 0.1
            with self.assertRaises(ValueError):
                latches.validate_bottom_latches(broken, self.corrected_deep, self.corrected_shallow)

    def test_mutated_source_or_mirror_is_rejected(self) -> None:
        original_source = latches._source_clip_profile
        original_transform = latches.transformed_clip_profile

        def changed_source(params: dict, kind: str) -> cq.Workplane:
            return original_source(params, kind).translate((0.1, 0.0, 0.0))

        def wrong_mirror(params: dict, center: float, kind: str, mirrored: bool = True) -> cq.Workplane:
            return original_transform(params, center, kind, False if mirrored else mirrored)

        for function, replacement in ((latches._source_clip_profile, changed_source), (latches.transformed_clip_profile, wrong_mirror)):
            with patch.object(latches, function.__name__, replacement):
                with self.assertRaises(ValueError):
                    latches.validate_bottom_latches(self.params, self.corrected_deep, self.corrected_shallow)

    def test_stale_hinge_centers_are_rejected(self) -> None:
        with patch.object(latches, "hinge_centers", return_value=latches.latch_centers(self.params)):
            with self.assertRaisesRegex(ValueError, "hinge centers changed"):
                latches.validate_bottom_latches(self.params, self.corrected_deep, self.corrected_shallow)

    def test_mutated_profile_or_bridge_ownership_is_rejected(self) -> None:
        original_profiles = latches.latch_profile_components
        original_bridges = latches.latch_bridge_components

        def omitted_profile(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
            return original_profiles(params, half)[:-1]

        def omitted_bridge(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
            return original_bridges(params, half)[:-1]

        def visible_bridge(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
            return tuple((name, bridge.translate((0.0, 0.01, 0.0))) for name, bridge in original_bridges(params, half))

        for name, replacement in (("latch_profile_components", omitted_profile), ("latch_bridge_components", omitted_bridge), ("latch_bridge_components", visible_bridge)):
            with patch.object(latches, name, replacement):
                with self.assertRaises(ValueError):
                    latches.validate_bottom_latches(self.params, self.corrected_deep, self.corrected_shallow)

    def test_final_owner_sweep_rejects_shell_collision_after_two_degrees(self) -> None:
        original_add = latches.add_bottom_latches
        collider = cq.Workplane("XY").box(20.0, 20.0, 1.0, centered=(False, False, False)).translate((120.0, 20.0, 66.0))

        def add_mutated(params: dict, model: cq.Workplane, half: str) -> cq.Workplane:
            result = original_add(params, model, half)
            return result.union(collider).clean() if half == "deep" else result

        with patch.object(latches, "add_bottom_latches", add_mutated):
            with self.assertRaisesRegex(ValueError, "after 2 degrees"):
                latches._validate_sweep(self.params, self.corrected_deep, self.corrected_shallow, build_device_insertion(self.params))


if __name__ == "__main__":
    unittest.main()
