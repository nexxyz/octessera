from __future__ import annotations

import unittest
from unittest.mock import patch
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from . import protective_case_device_orientation as orientation
    from .protective_case_device_fit import build_face_down_reference_top
    from .protective_case_branding_validation import symmetric_difference_volume
    from .protective_case_device_orientation import (
        GUIDE_CENTER,
        GUIDE_EXPECTED_BBOX,
        GUIDE_SCALE,
        GUIDE_STROKE,
        GUIDE_Z_MAX,
        GUIDE_Z_MIN,
        TURNAROUND_ARROW_FLAT_TIP,
        TURNAROUND_ARROW_LENGTH,
        TURNAROUND_ARROW_OVERLAP,
        TURNAROUND_ARROW_WIDTH,
        TURNAROUND_ARCS,
        TURNAROUND_CENTER,
        TURNAROUND_CENTERLINE_RADIUS,
        TURNAROUND_EXPECTED_BBOX,
        TURNAROUND_FLOOR_ATTACHMENT_VOLUME,
        TURNAROUND_SOURCE_CENTER,
        device_orientation_guide_components,
        device_orientation_guide_inlay_components,
        device_orientation_turnaround_arrow_components,
        orientation_guide_layout,
        validate_device_orientation,
    )
    from .protective_case_geometry import load_parameters
except ImportError:
    import generate_protective_case_cadquery as cad
    import protective_case_device_orientation as orientation
    from protective_case_device_fit import build_face_down_reference_top
    from protective_case_branding_validation import symmetric_difference_volume
    from protective_case_device_orientation import GUIDE_CENTER, GUIDE_EXPECTED_BBOX, GUIDE_SCALE, GUIDE_STROKE, GUIDE_Z_MAX, GUIDE_Z_MIN, TURNAROUND_ARROW_FLAT_TIP, TURNAROUND_ARROW_LENGTH, TURNAROUND_ARROW_OVERLAP, TURNAROUND_ARROW_WIDTH, TURNAROUND_ARCS, TURNAROUND_CENTER, TURNAROUND_CENTERLINE_RADIUS, TURNAROUND_EXPECTED_BBOX, TURNAROUND_FLOOR_ATTACHMENT_VOLUME, TURNAROUND_SOURCE_CENTER, device_orientation_guide_components, device_orientation_guide_inlay_components, device_orientation_turnaround_arrow_components, orientation_guide_layout, validate_device_orientation
    from protective_case_geometry import load_parameters


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


class ProtectiveCaseDeviceOrientationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.device = build_face_down_reference_top(cls.params, "raspberry-pi-zero-2w")
        cls.base_tub = cad.build_deep_tub_base(cls.params)
        cls.final_tub = cad.build_deep_tub(cls.params)

    def test_layout_uses_exact_face_down_device_geometry(self) -> None:
        layout = orientation_guide_layout(self.params)
        self.assertEqual(layout.center, GUIDE_CENTER)
        self.assertEqual(layout.scale, GUIDE_SCALE)
        self.assertEqual((layout.outer.width, layout.outer.height, layout.outer.radius), (186.0, 105.0, 6.0))
        self.assertAlmostEqual(layout.screen.center[0], 166.92375)
        self.assertAlmostEqual(layout.screen.center[1], 104.725)
        self.assertEqual((layout.screen.width, layout.screen.height, layout.screen.radius), (24.75, 24.75, 1.125))
        for actual, expected in zip(layout.encoders, (((202.3875, 89.0125), 9.0), ((179.6625, 74.9125), 6.75), ((192.4125, 57.5875), 6.75), ((166.9125, 57.5875), 6.75))):
            self.assertAlmostEqual(actual[0][0], expected[0][0])
            self.assertAlmostEqual(actual[0][1], expected[0][1])
            self.assertEqual(actual[1], expected[1])
        self.assertGreater(layout.screen.center[1], layout.center[1])
        self.assertEqual(len(layout.neokeys), 4)
        for key in layout.neokeys:
            self.assertAlmostEqual(key.width, 15.3)
            self.assertAlmostEqual(key.height, 15.3)
            self.assertAlmostEqual(key.radius, 1.5)
        self.assertEqual(len(layout.trellis), 64)
        self.assertEqual(layout.trellis[0][:2], (0, 0))
        self.assertAlmostEqual(layout.trellis[0][2][0], 126.4875)
        self.assertAlmostEqual(layout.trellis[0][2][1], 34.825)
        self.assertEqual(layout.trellis[-1][:2], (7, 7))
        self.assertAlmostEqual(layout.trellis[-1][2][0], 47.7375)
        self.assertAlmostEqual(layout.trellis[-1][2][1], 113.575)

    def test_guide_components_and_final_tub_validation(self) -> None:
        components = device_orientation_guide_components(self.params)
        self.assertEqual(len(components), 76)
        self.assertTrue(all(len(component.solids().vals()) == 1 for _, component in components))
        component_boxes = [shape_box(component) for _, component in components]
        for actual, expected in zip((min(box.xmin for box in component_boxes), max(box.xmax for box in component_boxes), min(box.ymin for box in component_boxes), max(box.ymax for box in component_boxes)), GUIDE_EXPECTED_BBOX):
            self.assertAlmostEqual(actual, expected, places=9)
        self.assertEqual(TURNAROUND_EXPECTED_BBOX, (224.109991243, 239.890008757, 66.7, 81.7))
        self.assertEqual(GUIDE_EXPECTED_BBOX, (34.8, 239.890008757, 21.7, 126.7))
        trellis_boxes = [shape_box(component) for name, component in components if "trellis_cell" in name]
        self.assertEqual(len(trellis_boxes), 64)
        for actual, expected in zip((min(box.xmin for box in trellis_boxes), max(box.xmax for box in trellis_boxes), min(box.ymin for box in trellis_boxes), max(box.ymax for box in trellis_boxes)), (43.5375, 130.6875, 30.625, 117.775)):
            self.assertAlmostEqual(actual, expected)
        raised = device_orientation_guide_components(self.params)
        inlay = device_orientation_guide_inlay_components(self.params)
        self.assertEqual([name for name, _ in raised], [name for name, _ in inlay])
        for (_, raised_part), (_, inlay_part) in zip(raised, inlay):
            raised_box = shape_box(raised_part)
            inlay_box = shape_box(inlay_part)
            for axis in ("xmin", "xmax", "ymin", "ymax"):
                self.assertAlmostEqual(getattr(raised_box, axis), getattr(inlay_box, axis))
            self.assertAlmostEqual(inlay_box.zmin, 1.8)
            self.assertAlmostEqual(inlay_box.zmax, 2.2)
        common_layer = cq.Workplane("XY").box(255.6, 148.4, 0.02, centered=(False, False, False)).translate((0.0, 0.0, 2.19))
        for (_, raised_part), (_, inlay_part) in zip(raised, inlay):
            self.assertLessEqual(symmetric_difference_volume(raised_part.intersect(common_layer), inlay_part.translate((0.0, 0.0, 0.38)).intersect(common_layer)), 1.0e-7)
        self.assertEqual(GUIDE_STROKE, 0.8)
        self.assertEqual((GUIDE_Z_MIN, GUIDE_Z_MAX), (2.18, 2.60))
        self.assertEqual(TURNAROUND_CENTER, TURNAROUND_SOURCE_CENTER)
        self.assertEqual(TURNAROUND_SOURCE_CENTER, (232.0, 74.2))
        self.assertEqual(TURNAROUND_CENTERLINE_RADIUS, 7.0)
        self.assertEqual(TURNAROUND_ARCS, ((20.0, 155.0), (200.0, 335.0)))
        self.assertEqual((TURNAROUND_ARROW_LENGTH, TURNAROUND_ARROW_WIDTH, TURNAROUND_ARROW_FLAT_TIP, TURNAROUND_ARROW_OVERLAP), (3.2, 3.2, 0.8, 0.4))
        arrows = device_orientation_turnaround_arrow_components(self.params)
        self.assertEqual(len(arrows), 2)
        self.assertEqual([name for name, _ in arrows], ["device_orientation_turnaround_arrow_1", "device_orientation_turnaround_arrow_2"])
        self.assertLessEqual(sum(s.Volume() for s in cast(list[cq.Shape], arrows[0][1].intersect(arrows[1][1]).solids().vals())), 1.0e-7)
        arrow_boxes = [shape_box(component) for _, component in arrows]
        self.assertAlmostEqual(min(box.xmin for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[0], places=9)
        self.assertAlmostEqual(max(box.xmax for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[1], places=9)
        self.assertAlmostEqual(min(box.ymin for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[2], places=9)
        self.assertAlmostEqual(max(box.ymax for box in arrow_boxes), TURNAROUND_EXPECTED_BBOX[3], places=9)
        outer_box = shape_box(components[0][1])
        self.assertAlmostEqual(min(box.xmin for box in arrow_boxes) - outer_box.xmax, 3.309991243, places=8)
        self.assertTrue(min(box.xmin for box in arrow_boxes) > outer_box.xmax)
        floor = cq.Workplane("XY").box(255.6, 148.4, 0.02, centered=(False, False, False)).translate((0.0, 0.0, 2.18))
        for _, arrow in arrows:
            self.assertAlmostEqual(sum(s.Volume() for s in cast(list[cq.Shape], arrow.intersect(floor).solids().vals())), TURNAROUND_FLOOR_ATTACHMENT_VOLUME, places=6)
        validate_device_orientation(self.params, self.final_tub, self.device, self.base_tub)

    def test_screen_and_encoders_own_the_control_side_turnaround_position(self) -> None:
        layout = orientation_guide_layout(self.params)
        self.assertGreater(layout.screen.center[0], GUIDE_CENTER[0])
        self.assertTrue(all(center[0] > GUIDE_CENTER[0] for center, _ in layout.encoders))

    def test_west_arrow_mutation_is_rejected_as_opposite_control_side(self) -> None:
        west_center = orientation.face_down_reference_xy(TURNAROUND_SOURCE_CENTER, self.params)
        with patch.object(orientation, "TURNAROUND_CENTER", west_center):
            with self.assertRaisesRegex(ValueError, "screen/encoder control side"):
                orientation.validate_device_orientation(self.params, self.final_tub, self.device, self.base_tub)

    def test_turnaround_move_preserves_body_bbox_and_first_layer(self) -> None:
        west_center = orientation.face_down_reference_xy(TURNAROUND_SOURCE_CENTER, self.params)
        with patch.object(orientation, "TURNAROUND_CENTER", west_center):
            previous_tub = cad.build_deep_tub(self.params)
            previous_arrows = orientation.device_orientation_turnaround_arrow_components(self.params)
        current_box = shape_box(self.final_tub)
        previous_box = shape_box(previous_tub)
        for axis in ("xmin", "xmax", "ymin", "ymax", "zmin", "zmax"):
            self.assertAlmostEqual(getattr(current_box, axis), getattr(previous_box, axis), places=6)
        first_layer = cq.Workplane("XY").box(255.6, 148.4, 0.21, centered=(False, False, False)).translate((0.0, 0.0, -0.01))
        current_volume = sum(s.Volume() for s in cast(list[cq.Shape], self.final_tub.intersect(first_layer).solids().vals()))
        previous_volume = sum(s.Volume() for s in cast(list[cq.Shape], previous_tub.intersect(first_layer).solids().vals()))
        self.assertAlmostEqual(current_volume, previous_volume, places=6)
        floor = cq.Workplane("XY").box(255.6, 148.4, 0.02, centered=(False, False, False)).translate((0.0, 0.0, 2.18))
        current_arrows = device_orientation_turnaround_arrow_components(self.params)
        for (_, current), (_, previous) in zip(current_arrows, previous_arrows):
            current_attachment = sum(s.Volume() for s in cast(list[cq.Shape], current.intersect(floor).solids().vals()))
            previous_attachment = sum(s.Volume() for s in cast(list[cq.Shape], previous.intersect(floor).solids().vals()))
            self.assertAlmostEqual(current_attachment, previous_attachment, places=9)

    def test_missing_guide_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing the orientation guide"):
            validate_device_orientation(self.params, self.base_tub, self.device)


if __name__ == "__main__":
    unittest.main()
