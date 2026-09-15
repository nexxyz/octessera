from __future__ import annotations

import unittest
from typing import cast

import cadquery as cq

try:
    from . import generate_protective_case_cadquery as cad
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_corner_restraints import continuous_shelf_support, corner_restraint_components, outside_section_area, outside_volume, section_symmetric_difference_volume, shelf_contact_component, shelf_contact_pad, support_section_stations, wall_witness
    from .protective_case_geometry import build_device_insertion, corner_restraints, dimensions, hinge_axis, hinge_centers, load_parameters, normalize_source_half
    from .protective_case_keepouts import control_contact_keepouts
    from .protective_case_mating_interface import apply_deep_mating_interface
    from .validate_protective_case import filament_pin, intersection_volume, paddle_envelope, prism, upstream_outer_envelope
except ImportError:
    import generate_protective_case_cadquery as cad
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_corner_restraints import continuous_shelf_support, corner_restraint_components, outside_section_area, outside_volume, section_symmetric_difference_volume, shelf_contact_component, shelf_contact_pad, support_section_stations, wall_witness
    from protective_case_geometry import build_device_insertion, corner_restraints, dimensions, hinge_axis, hinge_centers, load_parameters, normalize_source_half
    from protective_case_keepouts import control_contact_keepouts
    from protective_case_mating_interface import apply_deep_mating_interface
    from validate_protective_case import filament_pin, intersection_volume, paddle_envelope, prism, upstream_outer_envelope


def shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


class ProtectiveCaseGeometryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.params = load_parameters()
        cls.dims = dimensions(cls.params)

    def test_upstream_default_regression(self) -> None:
        source_box = upstream.build_box(upstream.default_measures())
        expected = {
            "top": (15683.8187, (100.0, 67.0, 18.8146)),
            "bottom": (23552.5168, (100.0, 65.4350, 23.0)),
        }
        for name, part in (("top", source_box.top()), ("bottom", source_box.bottom())):
            self.assertTrue(cast(cq.Shape, part.val()).isValid())
            self.assertEqual(len(part.solids().vals()), 1)
            bbox = shape_box(part)
            self.assertAlmostEqual(volume(part), expected[name][0], places=3)
            self.assertAlmostEqual(bbox.xmax - bbox.xmin, expected[name][1][0], places=3)
            self.assertAlmostEqual(bbox.ymax - bbox.ymin, expected[name][1][1], places=3)
            self.assertAlmostEqual(bbox.zmax - bbox.zmin, expected[name][1][2], places=3)

    def test_normalized_upstream_halves_and_body_frame(self) -> None:
        params = self.params
        source_box = cad.build_source_box(params)
        deep = cad.build_pristine_deep_tub(params)
        shallow = cad.build_pristine_shallow_lid(params)
        for part in (deep, shallow):
            self.assertTrue(cast(cq.Shape, part.val()).isValid())
            self.assertEqual(len(part.solids().vals()), 1)
        for part, expected in (
            (normalize_source_half(source_box.source_top(), params), (0.0, 255.6, 0.0, 148.4, 0.0, 54.6)),
            (normalize_source_half(source_box.source_bottom(), params), (0.0, 255.6, 0.0, 148.4, 52.65, 64.85)),
            (deep, (0.0, 255.6, -4.0, 148.4, 0.0, 57.85)),
            (shallow, (0.0, 255.6, -4.0, 148.4, 51.85, 64.85)),
        ):
            bbox = shape_box(part)
            actual = (bbox.xmin, bbox.xmax, bbox.ymin, bbox.ymax, bbox.zmin, bbox.zmax)
            for measured, target in zip(actual, expected):
                self.assertAlmostEqual(measured, target, places=3)
        self.assertEqual(hinge_axis(params), (-1.0, 54.85))
        self.assertEqual(hinge_centers(params), (63.9, 191.7))

    def test_device_insertion_and_pristine_addition_contract(self) -> None:
        params = self.params
        deep = cad.build_deep_tub(params)
        pristine = cad.build_pristine_deep_tub(params)
        shallow = cad.build_pristine_shallow_lid(params)
        descent = params["insertion"]["descent_sample_z"]
        self.assertEqual(descent[0], params["insertion"]["entry_base_z"])
        self.assertEqual(descent[-1], params["insertion"]["seated_base_z"])
        self.assertTrue(all(first - second <= 1.0 + 1.0e-6 for first, second in zip(descent, descent[1:])))
        self.assertAlmostEqual(volume(pristine.cut(apply_deep_mating_interface(params, pristine))), 1808.397280, delta=0.05)
        self.assertTrue(cast(cq.Shape, deep.val()).isValid())
        self.assertEqual(len(deep.solids().vals()), 1)
        for check_fit in (False, True):
            insertion = build_device_insertion(params, check_fit)
            self.assertTrue(cast(cq.Shape, insertion.val()).isValid())
            self.assertLessEqual(intersection_volume(insertion, shallow), 1.0e-7)
            self.assertLess(shape_box(insertion).zmax, params["insertion"]["entry_base_z"])
            for base_z in params["insertion"]["descent_sample_z"]:
                insertion = build_device_insertion(params, check_fit, base_z)
                self.assertLessEqual(intersection_volume(insertion, pristine), 1.0e-7)

    def test_xy_retention_contract_and_derived_wall_clearances(self) -> None:
        device = self.params["device"]
        self.assertEqual(
            device["xy_retention"],
            {
                "method": "adhesive_foam_strips",
                "thickness_range": [1.0, 2.0],
                "nominal_thickness_by_wall": {"west": 1.5, "east": 1.5, "south": 2.0, "north": 2.0},
                "straight_inner_wall_spans": ["west", "east", "south", "north"],
                "strips_per_span": 1,
                "material": "soft_closed_cell_adhesive_foam",
                "stacking": False,
                "integrated_hard_locators": False,
            },
        )
        for fit_name, expected in (("envelope", (1.6, 2.0)), ("check_fit", (1.1, 1.5))):
            fit = device[fit_name]
            self.assertAlmostEqual((self.dims.inner_width - fit["width"]) / 2.0, expected[0], places=6)
            self.assertAlmostEqual((self.dims.inner_depth - fit["depth"]) / 2.0, expected[1], places=6)

    def test_hinge_pins_and_outward_paddle_envelopes(self) -> None:
        params = self.params
        source_box = cad.build_source_box(params)
        deep = normalize_source_half(source_box.top(), params)
        shallow = normalize_source_half(source_box.bottom(), params)
        paddle_spec = params["hinge_pins"]["paddle_envelope"]
        for center in hinge_centers(params):
            pin = filament_pin(params, center)
            self.assertLessEqual(intersection_volume(pin, deep), 1.0e-7)
            self.assertLessEqual(intersection_volume(pin, shallow), 1.0e-7)
            for side in ("west", "east"):
                paddle = paddle_envelope(params, center, side)
                bbox = shape_box(paddle)
                self.assertAlmostEqual(bbox.xmax - bbox.xmin, paddle_spec["axial_length"], places=4)
                self.assertAlmostEqual(bbox.ymax - bbox.ymin, paddle_spec["outward_depth"] + paddle_spec["inward_allowance"], places=4)
                self.assertAlmostEqual(bbox.zmax - bbox.zmin, paddle_spec["vertical_span"], places=4)
                self.assertGreater(intersection_volume(pin, paddle), 0.0)
                self.assertLessEqual(intersection_volume(paddle, deep), 1.0e-7)
                self.assertLessEqual(intersection_volume(paddle, shallow), 1.0e-7)

    def test_corner_restraints_cross_both_walls_and_avoid_keepouts(self) -> None:
        params = self.params
        pristine = cad.build_pristine_deep_tub(params)
        specs = params["corner_restraints"]
        keepouts = control_contact_keepouts(params)
        adjacent = {"NW": ("west", "north"), "SW": ("west", "south"), "NE": ("east", "north"), "SE": ("east", "south")}
        for restraint, (_, component) in zip(corner_restraints(params), corner_restraint_components(params)):
            self.assertTrue(cast(cq.Shape, component.val()).isValid())
            self.assertEqual(len(component.solids().vals()), 1)
            self.assertGreater(intersection_volume(component, pristine), 0.0)
            shelf_bounds = (min(point[0] for point in restraint.shelf_polygon), max(point[0] for point in restraint.shelf_polygon), min(point[1] for point in restraint.shelf_polygon), max(point[1] for point in restraint.shelf_polygon))
            support_points = restraint.support_lower_polygon + restraint.support_upper_polygon
            bounds = (
                (
                    min(point[0] for point in support_points),
                    max(point[0] for point in support_points),
                    min(point[1] for point in support_points),
                    max(point[1] for point in support_points),
                ),
            ) + tuple(restraint.shelf_tabs.values()) + (shelf_bounds,)
            component_box = shape_box(component)
            self.assertAlmostEqual(component_box.xmin, min(bound[0] for bound in bounds), places=3)
            self.assertAlmostEqual(component_box.xmax, max(bound[1] for bound in bounds), places=3)
            self.assertAlmostEqual(component_box.ymin, min(bound[2] for bound in bounds), places=3)
            self.assertAlmostEqual(component_box.ymax, max(bound[3] for bound in bounds), places=3)
            self.assertFalse(
                any(
                    intersection_volume(component, prism(keepout.bounds[0], keepout.bounds[1], 0.0, keepout.bounds[2] - keepout.bounds[0], keepout.bounds[3] - keepout.bounds[1], self.dims.height)) > 1.0e-7
                    for keepout in keepouts
                )
            )
            floor_join = intersection_volume(component, pristine, prism(0.0, 0.0, 2.0, self.dims.width, self.dims.depth, 0.22))
            self.assertGreater(floor_join, 0.0)
            for side in adjacent[restraint.name]:
                witness = wall_witness(params, restraint, side)
                wall_contact = intersection_volume(component, pristine, witness)
                self.assertGreaterEqual(wall_contact, specs["wall_intersection_volume_min"])
                section = component.intersect(pristine).intersect(witness).intersect(cq.Workplane("XY").box(255.6, 148.4, 0.05, centered=(False, False, False)).translate((0.0, 0.0, 3.975)))
                section_box = shape_box(section)
                contact_length = section_box.ymax - section_box.ymin if side in ("west", "east") else section_box.xmax - section_box.xmin
                self.assertGreaterEqual(contact_length, 3.10)

    def test_corner_restraints_are_inside_upstream_outer_envelope(self) -> None:
        params = self.params
        outer = upstream_outer_envelope(params)
        specs = params["corner_restraints"]
        for restraint, (_, component) in zip(corner_restraints(params), corner_restraint_components(params)):
            self.assertLessEqual(outside_volume(component, outer), 1.0e-7)
            sample_z = (
                2.01,
                2.20,
                2.40,
                3.00,
                4.00,
                4.91127,
                7.80,
                8.00,
                (specs["support_lower_z"] + restraint.nominal_shelf_base_z - specs["shelf_overlap"]) / 2.0,
                restraint.nominal_shelf_base_z,
                restraint.contact_z - 0.01,
            )
            for z in sample_z:
                self.assertLessEqual(outside_section_area(component, outer, z, self.dims.width, self.dims.depth), 1.0e-7, f"{restraint.name} at Z{z}")

    def test_continuous_shelf_supports_are_ruled_and_wall_attached(self) -> None:
        params = self.params
        spec = params["corner_restraints"]
        for restraint in corner_restraints(params):
            support = continuous_shelf_support(restraint, params)
            self.assertTrue(cast(cq.Shape, support.val()).isValid())
            self.assertEqual(len(support.solids().vals()), 1)
            self.assertEqual(len(support.shells().vals()), 1)
            self.assertEqual(len(restraint.support_lower_polygon), 6)
            self.assertEqual(len(restraint.support_upper_polygon), 6)
            support_box = shape_box(support)
            self.assertEqual(spec["shelf_overlap"], 0.10)
            self.assertAlmostEqual(support_box.zmin, spec["support_lower_z"], places=3)
            self.assertAlmostEqual(support_box.zmax, restraint.nominal_shelf_base_z, places=3)
            shelf = shelf_contact_component(restraint, params)
            shelf_box = shape_box(shelf)
            self.assertAlmostEqual(support_box.zmax - shelf_box.zmin, spec["shelf_overlap"], places=3)
            self.assertGreater(intersection_volume(support, shelf), 0.0)
            component = dict(corner_restraint_components(params))[f"corner_restraint_{restraint.name.lower()}"]
            self.assertLessEqual(volume(support.cut(component)), 1.0e-7)

    def test_no_ridge_sections_match_support_and_final_additions(self) -> None:
        params = self.params
        deep = cad.build_pristine_deep_tub(params)
        final_tub = cad.build_deep_tub(params)
        additions = final_tub.cut(deep)
        for restraint, (_, component) in zip(corner_restraints(params), corner_restraint_components(params)):
            support = continuous_shelf_support(restraint, params)
            component_box = shape_box(component)
            local_probe = prism(
                component_box.xmin - 0.01,
                component_box.ymin - 0.01,
                0.0,
                component_box.xmax - component_box.xmin + 0.02,
                component_box.ymax - component_box.ymin + 0.02,
                self.dims.height,
            )
            expected_additions = support.cut(deep)
            for z in support_section_stations(restraint, params):
                self.assertLessEqual(
                    section_symmetric_difference_volume(component, support, z, self.dims.width, self.dims.depth),
                    1.0e-7,
                    f"{restraint.name} restraint has a ridge at Z{z}",
                )
                self.assertLessEqual(
                    section_symmetric_difference_volume(
                        additions.intersect(local_probe),
                        expected_additions.intersect(local_probe),
                        z,
                        self.dims.width,
                        self.dims.depth,
                    ),
                    1.0e-7,
                    f"{restraint.name} final tub has a support-plus-strip appendage at Z{z}",
                )

    def test_contact_pads_are_rounded_with_flat_contact_planes(self) -> None:
        params = self.params
        spec = params["corner_restraints"]
        expected = {
            "NW": (131.7592637, 13.98, 12.48),
            "SW": (131.7592637, 13.98, 12.48),
            "NE": (262.1032637, 18.48, 16.78),
            "SE": (262.1032637, 18.48, 16.78),
        }
        for restraint in corner_restraints(params):
            pad = shelf_contact_pad(restraint, params)
            shelf = shelf_contact_component(restraint, params)
            self.assertTrue(cast(cq.Shape, pad.val()).isValid())
            self.assertEqual(len(pad.solids().vals()), 1)
            self.assertTrue(cast(cq.Shape, shelf.val()).isValid())
            self.assertEqual(len(shelf.solids().vals()), 1)
            pad_box = shape_box(pad)
            shelf_bounds = (min(point[0] for point in restraint.shelf_polygon), max(point[0] for point in restraint.shelf_polygon), min(point[1] for point in restraint.shelf_polygon), max(point[1] for point in restraint.shelf_polygon))
            self.assertAlmostEqual(pad_box.xmin, shelf_bounds[0], places=3)
            self.assertAlmostEqual(pad_box.xmax, shelf_bounds[1], places=3)
            self.assertAlmostEqual(pad_box.ymin, shelf_bounds[2], places=3)
            self.assertAlmostEqual(pad_box.ymax, shelf_bounds[3], places=3)
            self.assertAlmostEqual(pad_box.zmax, restraint.contact_z, places=3)
            top = cast(cq.Shape, shelf.faces(">Z").val())
            self.assertEqual(top.geomType(), "PLANE")
            area, width, depth = expected[restraint.name]
            top_box = top.BoundingBox()
            self.assertAlmostEqual(top.Area(), area, places=3)
            self.assertAlmostEqual(top_box.xmax - top_box.xmin, width, places=3)
            self.assertAlmostEqual(top_box.ymax - top_box.ymin, depth, places=3)
            edge_details = [(getattr(edge, "radius")(), cast(cq.Shape, edge).BoundingBox()) for edge in cast(list[cq.Shape], pad.edges().vals()) if edge.geomType() == "CIRCLE"]
            self.assertEqual(sum(abs(radius - spec["shelf_plan_radius"]) <= 0.05 for radius, _ in edge_details), 2)
            self.assertEqual(sum(abs(radius - spec["shelf_top_edge_radius"]) <= 0.05 for radius, _ in edge_details), 4)
            self.assertEqual(sum(abs(radius - spec["shelf_plan_radius"]) <= 0.05 and abs(edge_box.zmin - pad_box.zmin) <= 0.05 for radius, edge_box in edge_details), 1)
            self.assertEqual(sum(abs(radius - spec["shelf_plan_radius"]) <= 0.05 and abs(edge_box.zmin - (restraint.contact_z - spec["shelf_top_edge_radius"])) <= 0.05 for radius, edge_box in edge_details), 1)
            self.assertEqual(sum(abs(radius - spec["shelf_top_edge_radius"]) <= 0.05 and abs(edge_box.zmin - (restraint.contact_z - spec["shelf_top_edge_radius"])) <= 0.05 and abs(edge_box.zmax - restraint.contact_z) <= 0.05 for radius, edge_box in edge_details), 4)

if __name__ == "__main__":
    unittest.main()
