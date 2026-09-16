from __future__ import annotations

import unittest
from pathlib import Path
import tempfile
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .branding_marking_cadquery import Point, branding_marking_parts_placed, logo_marking_placed
    from .protective_case_branding import (
        build_deep_tub_debossed_branding_parts,
        build_deep_tub_flush_branding_parts,
        build_deep_tub_multicolor_branding_parts,
        build_shallow_lid_debossed_branding_parts,
        build_shallow_lid_flush_branding_parts,
        mirror_for_negative_z_exterior_view,
        rotate_lockup_group,
    )
    from .protective_case_geometry import artifact_paths, dimensions, load_parameters
    from .protective_case_device_orientation import GUIDE_EXPECTED_BBOX, device_orientation_guide_inlay_components, fused_device_orientation_guide_inlay
    from .protective_case_3mf_validation import validate_deep_tub_multicolor_package, validate_shallow_lid_multicolor_package
    from .protective_case_branding_validation import intersection_volume, symmetric_difference_volume, validate_deep_multicolor_geometry
except ImportError:
    import generate_protective_case_cadquery as cad
    from branding_marking_cadquery import Point, branding_marking_parts_placed, logo_marking_placed
    from protective_case_branding import (
        build_deep_tub_debossed_branding_parts,
        build_deep_tub_flush_branding_parts,
        build_deep_tub_multicolor_branding_parts,
        build_shallow_lid_debossed_branding_parts,
        build_shallow_lid_flush_branding_parts,
        mirror_for_negative_z_exterior_view,
        rotate_lockup_group,
    )
    from protective_case_geometry import artifact_paths, dimensions, load_parameters
    from protective_case_device_orientation import GUIDE_EXPECTED_BBOX, device_orientation_guide_inlay_components, fused_device_orientation_guide_inlay
    from protective_case_3mf_validation import validate_deep_tub_multicolor_package, validate_shallow_lid_multicolor_package
    from protective_case_branding_validation import intersection_volume, symmetric_difference_volume, validate_deep_multicolor_geometry


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


class ProtectiveCaseBrandingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.dims = dimensions(cls.params)

    def canonical_parts(self, z_min: float, height: float, mirror: bool, rotate: bool = False) -> list[tuple[str, cq.Workplane]]:
        branding = self.params["branding"]
        center_x, center_y = branding["combined_center"]
        logo_seed = logo_marking_placed(branding["logo_target_size"], Point(center_x, 0.0), z_min, height)
        logo_height = shape_box(logo_seed).ymax - shape_box(logo_seed).ymin
        total_height = logo_height + branding["vertical_gap"] + branding["wordmark_height"]
        top_y = center_y + total_height / 2.0
        parts = branding_marking_parts_placed(
            branding["logo_target_size"],
            Point(center_x, top_y - logo_height / 2.0),
            branding["wordmark_width"],
            branding["wordmark_height"],
            Point(center_x, top_y - logo_height - branding["vertical_gap"] - branding["wordmark_height"] / 2.0),
            z_min,
            height,
        )
        if rotate:
            parts = rotate_lockup_group(parts, branding["combined_center"], branding["shallow_lid_lockup_rotation_degrees"])
        if mirror:
            parts = [(name, mirror_for_negative_z_exterior_view(part, branding["combined_center"][0])) for name, part in parts]
        return parts

    def canonical_deep_logo(self, z_min: float, height: float) -> list[tuple[str, cq.Workplane]]:
        branding = self.params["branding"]
        center_x, center_y = branding["combined_center"]
        logo = logo_marking_placed(branding["deep_tub_logo_target_size"], Point(center_x, center_y), z_min, height)
        return [("octessera_logo", mirror_for_negative_z_exterior_view(logo, center_x))]

    def assert_parts_match_canonical(
        self,
        actual: list[tuple[str, cq.Workplane]],
        expected: list[tuple[str, cq.Workplane]],
        body: cq.Workplane,
    ) -> None:
        self.assertEqual([name.removeprefix("protective_case_deep_tub_").removeprefix("protective_case_shallow_lid_") for name, _ in actual], [name.removeprefix("octessera_") for name, _ in expected])
        body_box = shape_box(body)
        for (_, actual_part), (_, expected_part) in zip(actual, expected):
            actual_box = shape_box(actual_part)
            expected_box = shape_box(expected_part)
            self.assertLessEqual(symmetric_difference_volume(actual_part, expected_part), 1.0e-7)
            for axis in ("xmin", "ymin", "xmax", "ymax", "zmin", "zmax"):
                self.assertAlmostEqual(getattr(actual_box, axis), getattr(expected_box, axis), places=5)
            self.assertGreaterEqual(actual_box.xmin, body_box.xmin - 0.05)
            self.assertLessEqual(actual_box.xmax, body_box.xmax + 0.05)
            self.assertGreaterEqual(actual_box.ymin, body_box.ymin - 0.05)
            self.assertLessEqual(actual_box.ymax, body_box.ymax + 0.05)
        if len(actual) == 2:
            self.assertLessEqual(intersection_volume(actual[0][1], actual[1][1]), 1.0e-7)

    def test_deep_logo_and_shallow_lockup_match_source_geometry(self) -> None:
        params = self.params
        branding = params["branding"]
        self.assertEqual(
            branding,
            {
                "logo_target_size": 64.0,
                "wordmark_width": 100.0,
                "wordmark_height": 15.0943396226,
                "combined_center": [127.8, 74.2],
                "vertical_gap": 6.0,
                "shallow_lid_lockup_rotation_degrees": 180.0,
                "deep_tub_logo_target_size": 72.0,
                "deboss_depth": 0.4,
                "flush_depth": 0.4,
            },
        )
        deep_debossed = build_deep_tub_debossed_branding_parts(params)
        deep_flush = build_deep_tub_flush_branding_parts(params)
        shallow_debossed = build_shallow_lid_debossed_branding_parts(params)
        shallow_flush = build_shallow_lid_flush_branding_parts(params)
        deep_body = cad.build_deep_tub(params)
        shallow_body = cad.build_pristine_shallow_lid(params)
        self.assert_parts_match_canonical(deep_debossed, self.canonical_deep_logo(0.0, 0.4), deep_body)
        self.assert_parts_match_canonical(deep_flush, self.canonical_deep_logo(0.0, 0.4), deep_body)
        self.assert_parts_match_canonical(shallow_debossed, self.canonical_parts(self.dims.height - 0.4, 0.4, False, True), shallow_body)
        self.assert_parts_match_canonical(shallow_flush, self.canonical_parts(self.dims.height - 0.4, 0.4, False, True), shallow_body)
        self.assertEqual([name for name, _ in deep_debossed], ["protective_case_deep_tub_logo"])
        self.assertFalse(any("wordmark" in name for name, _ in deep_flush))
        self.assertEqual([name for name, _ in shallow_debossed], ["protective_case_shallow_lid_logo", "protective_case_shallow_lid_wordmark"])

        deep_logo_box = shape_box(deep_flush[0][1])
        self.assertAlmostEqual(deep_logo_box.xmin, 91.8, places=3)
        self.assertAlmostEqual(deep_logo_box.ymin, 48.364706, places=3)
        self.assertAlmostEqual(deep_logo_box.xmax, 163.8, places=3)
        self.assertAlmostEqual(deep_logo_box.ymax, 100.035294, places=3)
        shallow_logo_box = shape_box(shallow_flush[0][1])
        shallow_wordmark_box = shape_box(shallow_flush[1][1])
        self.assertAlmostEqual(shallow_logo_box.xmin, 95.8, places=3)
        self.assertAlmostEqual(shallow_logo_box.ymin, 40.688124, places=3)
        self.assertAlmostEqual(shallow_logo_box.xmax, 159.8, places=3)
        self.assertAlmostEqual(shallow_logo_box.ymax, 86.617536, places=3)
        self.assertAlmostEqual(shallow_wordmark_box.xmin, 77.8, places=3)
        self.assertAlmostEqual(shallow_wordmark_box.ymin, 92.617536, places=3)
        self.assertAlmostEqual(shallow_wordmark_box.xmax, 177.8, places=3)
        self.assertAlmostEqual(shallow_wordmark_box.ymax, 107.711876, places=3)
        self.assertAlmostEqual((shallow_wordmark_box.xmin + shallow_wordmark_box.xmax) / 2.0, 127.8, places=5)
        self.assertAlmostEqual((shallow_logo_box.ymin + shallow_logo_box.ymax) / 2.0, 63.65283, places=5)
        self.assertAlmostEqual((shallow_wordmark_box.ymin + shallow_wordmark_box.ymax) / 2.0, 100.164706, places=5)
        self.assertAlmostEqual(shallow_wordmark_box.ymin - shallow_logo_box.ymax, 6.0, places=5)

    def test_shallow_lockup_is_one_rigid_group_rotation(self) -> None:
        branding = self.params["branding"]
        z_min = self.dims.height - branding["flush_depth"]
        unrotated = self.canonical_parts(z_min, branding["flush_depth"], False)
        transformed = rotate_lockup_group(unrotated, branding["combined_center"], branding["shallow_lid_lockup_rotation_degrees"])
        debossed = build_shallow_lid_debossed_branding_parts(self.params)
        multicolor = build_shallow_lid_flush_branding_parts(self.params)
        for (_, actual), (_, expected), (_, source) in zip(debossed, transformed, unrotated):
            self.assertLessEqual(symmetric_difference_volume(actual, expected), 1.0e-7)
            source_box = shape_box(source)
            expected_box = shape_box(expected)
            self.assertAlmostEqual(expected_box.xmax - expected_box.xmin, source_box.xmax - source_box.xmin, places=6)
            self.assertAlmostEqual(expected_box.ymax - expected_box.ymin, source_box.ymax - source_box.ymin, places=6)
        for (_, debossed_part), (_, multicolor_part) in zip(debossed, multicolor):
            self.assertLessEqual(symmetric_difference_volume(debossed_part, multicolor_part), 1.0e-7)
        source_logo_box, source_wordmark_box = (shape_box(part) for _, part in unrotated)
        transformed_logo_box, transformed_wordmark_box = (shape_box(part) for _, part in transformed)
        source_delta = ((source_wordmark_box.xmin + source_wordmark_box.xmax - source_logo_box.xmin - source_logo_box.xmax) / 2.0, (source_wordmark_box.ymin + source_wordmark_box.ymax - source_logo_box.ymin - source_logo_box.ymax) / 2.0)
        transformed_delta = ((transformed_wordmark_box.xmin + transformed_wordmark_box.xmax - transformed_logo_box.xmin - transformed_logo_box.xmax) / 2.0, (transformed_wordmark_box.ymin + transformed_wordmark_box.ymax - transformed_logo_box.ymin - transformed_logo_box.ymax) / 2.0)
        self.assertAlmostEqual(transformed_delta[0], -source_delta[0], places=6)
        self.assertAlmostEqual(transformed_delta[1], -source_delta[1], places=6)
        self.assertAlmostEqual(source_logo_box.ymin - source_wordmark_box.ymax, 6.0, places=6)
        self.assertAlmostEqual(transformed_wordmark_box.ymin - transformed_logo_box.ymax, 6.0, places=6)

    def test_deep_multicolor_uses_flush_floor_orientation_inlay(self) -> None:
        body, parts = cad.build_multicolor_deep_tub(self.params)
        self.assertEqual([name for name, _ in parts], ["protective_case_deep_tub_logo", "protective_case_deep_tub_device_orientation_guide"])
        logo_box = shape_box(parts[0][1])
        guide_box = shape_box(parts[1][1])
        self.assertEqual(len(parts[1][1].solids().vals()), 73)
        self.assertEqual(len(device_orientation_guide_inlay_components(self.params)), 76)
        for actual, expected in zip((guide_box.xmin, guide_box.xmax, guide_box.ymin, guide_box.ymax), GUIDE_EXPECTED_BBOX):
            self.assertAlmostEqual(actual, expected, places=5)
        self.assertAlmostEqual(guide_box.zmin, 1.8, places=6)
        self.assertAlmostEqual(guide_box.zmax, 2.2, places=6)
        cutter_box = shape_box(fused_device_orientation_guide_inlay(self.params, True))
        self.assertAlmostEqual(cutter_box.zmin, 1.8, places=6)
        self.assertAlmostEqual(cutter_box.zmax, 2.22, places=6)
        self.assertAlmostEqual(guide_box.zmin - logo_box.zmax, 1.4, places=6)
        validate_deep_multicolor_geometry(self.params, body, parts)
        self.assertGreater(intersection_volume(cad.build_deep_tub(self.params), parts[1][1]), 0.0)
        self.assertFalse(any("orientation" in name for name, _ in build_shallow_lid_flush_branding_parts(self.params)))

    def test_deep_multicolor_rejects_guide_mutations(self) -> None:
        body, parts = cad.build_multicolor_deep_tub(self.params)
        with self.assertRaisesRegex(ValueError, "orientation guide geometry changed"):
            validate_deep_multicolor_geometry(self.params, body, [(parts[0][0], parts[0][1]), (parts[1][0], parts[1][1].translate((1.0, 0.0, 0.0)))])
        with self.assertRaisesRegex(ValueError, "logo and orientation guide"):
            validate_deep_multicolor_geometry(self.params, body, parts[:1])
        with self.assertRaisesRegex(ValueError, "body intersects a marking"):
            validate_deep_multicolor_geometry(self.params, cad.build_deep_tub(self.params), parts)
        with self.assertRaisesRegex(ValueError, "solid web is too thin"):
            moved_logo = parts[0][1].translate((0.0, 0.0, 0.2))
            validate_deep_multicolor_geometry(self.params, body, [(parts[0][0], moved_logo), parts[1]])
        for index in (0, 1):
            missing_arrow = parts[1][1].cut(device_orientation_guide_inlay_components(self.params)[74 + index][1])
            with self.assertRaisesRegex(ValueError, "orientation guide geometry changed"):
                validate_deep_multicolor_geometry(self.params, body, [parts[0], (parts[1][0], missing_arrow)])

    def test_protective_artifact_contract_uses_branding_names(self) -> None:
        paths = artifact_paths(self.params)
        self.assertEqual(len(paths), 8)
        self.assertEqual(
            [path.relative_to(Path(__file__).resolve().parents[2]).as_posix() for path in paths],
            [
                "hardware/enclosure/step/protective_case_deep_tub_debossed_logo.step",
                "hardware/enclosure/stl/protective_case_deep_tub_debossed_logo.stl",
                "hardware/enclosure/3mf-single-material/protective_case_deep_tub_debossed_logo.3mf",
                "hardware/enclosure/3mf-multicolor/protective_case_deep_tub_multicolor_logo.3mf",
                "hardware/enclosure/step/protective_case_shallow_lid_debossed_branding.step",
                "hardware/enclosure/stl/protective_case_shallow_lid_debossed_branding.stl",
                "hardware/enclosure/3mf-single-material/protective_case_shallow_lid_debossed_branding.3mf",
                "hardware/enclosure/3mf-multicolor/protective_case_shallow_lid_multicolor_branding.3mf",
            ],
        )
        three_mf_paths = [path for path in paths if path.suffix == ".3mf"]
        self.assertEqual(sum(path.parent.name == "3mf-single-material" for path in three_mf_paths), 2)
        self.assertEqual(sum(path.parent.name == "3mf-multicolor" for path in three_mf_paths), 2)
        debossed_paths = [path for path in three_mf_paths if "debossed" in path.name]
        self.assertEqual(len(debossed_paths), 2)
        self.assertTrue(all(path.parent.name == "3mf-single-material" for path in debossed_paths))
        self.assertFalse(any(path.parent.name == "3mf-multicolor" for path in debossed_paths))

    def test_shared_owner_transforms_preserve_multicolor_placement_and_3mf_topology(self) -> None:
        params = self.params
        with tempfile.TemporaryDirectory() as directory:
            for label, builder, exterior, validator in (
                ("deep", cad.build_multicolor_deep_tub, "negative_z", validate_deep_tub_multicolor_package),
                ("shallow", cad.build_multicolor_shallow_lid, "positive_z", validate_shallow_lid_multicolor_package),
            ):
                body, marks = builder(params)
                transform = cad.print_transform(body, exterior)
                expected_transform = (0.0, (0.0, 4.0, 0.0)) if label == "deep" else (180.0, (0.0, 151.4, 62.85))
                self.assertAlmostEqual(transform.rotate_x_degrees, expected_transform[0], places=6)
                for actual, expected in zip(transform.translation, expected_transform[1]):
                    self.assertAlmostEqual(actual, expected, places=6)
                printed_body = cad.apply_print_transform(body, transform)
                printed_marks = [cad.apply_print_transform(mark, transform) for _, mark in marks]
                if label == "shallow":
                    printed_logo_box = shape_box(printed_marks[0])
                    printed_wordmark_box = shape_box(printed_marks[1])
                    for axis, expected in (("xmin", 95.8), ("ymin", 64.7824639289), ("zmin", 0.0), ("xmax", 159.8), ("ymax", 110.7118759289), ("zmax", 0.4)):
                        self.assertAlmostEqual(getattr(printed_logo_box, axis), expected, places=5)
                    for axis, expected in (("xmin", 77.8), ("ymin", 43.6881239289), ("zmin", 0.0), ("xmax", 177.8), ("ymax", 58.7824639289), ("zmax", 0.4)):
                        self.assertAlmostEqual(getattr(printed_wordmark_box, axis), expected, places=5)
                else:
                    printed_logo_box = shape_box(printed_marks[0])
                    for axis, expected in (("xmin", 91.8), ("ymin", 52.364706), ("zmin", 0.0), ("xmax", 163.8), ("ymax", 104.035294), ("zmax", 0.4)):
                        self.assertAlmostEqual(getattr(printed_logo_box, axis), expected, places=5)
                for printed_mark in printed_marks:
                    self.assertLessEqual(intersection_volume(printed_mark, printed_body), 1.0e-7)
                expected_parts = {1: shape_box(printed_body), **{index + 2: shape_box(mark) for index, mark in enumerate(printed_marks)}}
                expected_bbox = (
                    expected_parts[1].xmin,
                    expected_parts[1].ymin,
                    expected_parts[1].zmin,
                    expected_parts[1].xmax,
                    expected_parts[1].ymax,
                    expected_parts[1].zmax,
                )
                expected_mesh_bboxes = {
                    object_id: (
                        box.xmin,
                        box.ymin,
                        box.zmin,
                        box.xmax,
                        box.ymax,
                        box.zmax,
                    )
                    for object_id, box in expected_parts.items()
                }
                path = Path(directory) / f"{label}.3mf"
                cad.write_parts_3mf(
                    path,
                    [("protective_case_" + ("deep_tub" if label == "deep" else "shallow_lid") + "_body", printed_body, 1)]
                    + [(name, printed_mark, 2) for (name, _), printed_mark in zip(marks, printed_marks)],
                )
                validator(path, expected_bbox, expected_mesh_bboxes)
                for (_, mark), printed_mark in zip(marks, printed_marks):
                    independently_normalized = cad.orient_for_print(mark, exterior)
                    self.assertGreater(symmetric_difference_volume(printed_mark, independently_normalized), 1.0e-7)


if __name__ == "__main__":
    unittest.main()
