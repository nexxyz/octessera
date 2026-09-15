from __future__ import annotations

import ast
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent
REPO_ROOT = ROOT.parents[1]

DOMAIN_MODULE_RESPONSIBILITIES: dict[str, tuple[str, ...]] = {
    "generate_two_level_enclosure_cadquery.py": ("build_model", "main"),
    "top_wave_geometry.py": ("shoulder_loft", "west_wave_wall", "add_guidance_slots"),
    "top_faceplate_features.py": ("add_cutouts", "add_neokey_cutouts"),
    "top_body_assembly.py": ("rounded_plate", "build_body_model"),
    "top_branding_variants.py": ("orange_pi_top_params", "build_branding_marking"),
    "top_enclosure_export.py": ("build_branded_export_model", "export_top_variant"),
    "top_wall_port_geometry.py": ("wall_port_z_bounds", "rounded_wall_port_profile_points"),
    "top_wall_port_indent_geometry.py": (
        "make_left_wall_indent",
        "make_south_wall_indent",
        "make_north_wall_indent",
    ),
    "top_wall_port_recess_geometry.py": (
        "make_left_wall_face_recess",
        "make_south_wall_face_recess",
        "make_north_wall_face_recess",
    ),
    "top_wall_port_cutouts.py": ("add_top_wall_port_cutouts",),
    "protective_case_device_fit.py": ("build_canonical_face_down_device", "validate_actual_device_fit"),
    "protective_case_device_orientation.py": ("device_orientation_guide_components", "device_orientation_guide_inlay_components", "device_orientation_turnaround_arrow_components", "fused_device_orientation_guide_inlay", "add_device_orientation_guide", "validate_device_orientation"),
    "protective_case_bottom_latches.py": ("transformed_clip_profile", "latch_profile_components", "latch_bridge_components", "bottom_latch_components", "add_bottom_latches", "validate_source_box_clip_ownership", "validate_bottom_latches"),
}


def source(name: str) -> str:
    return (ROOT / name).read_text(encoding="utf-8")


def top_level_function_names(text: str) -> set[str]:
    tree = ast.parse(text)
    return {
        node.name
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }


class CadSourceStructureTests(unittest.TestCase):
    def test_domain_modules_have_named_responsibilities(self) -> None:
        for module, expected_functions in DOMAIN_MODULE_RESPONSIBILITIES.items():
            with self.subTest(module=module):
                actual_functions = top_level_function_names(source(module))
                self.assertTrue(set(expected_functions).issubset(actual_functions))

    def test_generator_remains_the_preferred_export_entrypoint(self) -> None:
        text = source("generate_two_level_enclosure_cadquery.py")
        ast.parse(text)
        self.assertIn("top_enclosure_export", text)
        self.assertIn("export_top_variant", text)
        self.assertIn('if __name__ == "__main__":', text)

    def test_port_policy_uses_reusable_geometry_modules(self) -> None:
        policy = source("top_wall_port_cutouts.py")
        geometry = "\n".join(
            source(name)
            for name in (
                "top_wall_port_geometry.py",
                "top_wall_port_indent_geometry.py",
                "top_wall_port_recess_geometry.py",
            )
        )
        self.assertIn("top_wall_port_geometry", policy)
        self.assertIn("top_wall_port_indent_geometry", policy)
        self.assertIn("top_wall_port_recess_geometry", policy)
        self.assertIn('params["ports_v21"]', policy)
        self.assertNotIn('params["ports_v21"]', geometry)
        self.assertIn("OLED_SD_X0 = 58.63", policy)
        self.assertNotIn("OLED_SD_X0", geometry)

    def test_parametric_geometry_sources_preserve_invariants(self) -> None:
        wave = source("top_wave_geometry.py")
        guidance = source("wave_guidance.py")
        ports = source("top_wall_port_geometry.py")
        self.assertIn("LOW_Z = 12.0", wave)
        self.assertIn("HIGH_Z = 17.0", wave)
        self.assertIn("UNDERSIDE_Z = 9.0", wave)
        self.assertIn("HIGH_UNDERSIDE_Z = 14.0", wave)
        self.assertIn("SLOPE_PROFILE_STEPS = 12", guidance)
        self.assertIn("SOUTH_SHOULDER_SAMPLES = 36", guidance)
        self.assertIn("SOUTH_SHOULDER_PLAN_WIDTH = 8.5", guidance)
        self.assertIn("def south_edge_samples", guidance)
        self.assertIn("ROUNDED_CORNER_STEPS = 4", ports)
        self.assertIn("def rounded_wall_port_profile_points", ports)

    def test_refactored_sources_parse_without_executing_cad(self) -> None:
        for module in ROOT.glob("*.py"):
            with self.subTest(module=module.name):
                ast.parse(module.read_text(encoding="utf-8"), filename=str(module))

    def test_upstream_import_is_side_effect_free(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, "-B", "-c", "import hardware.enclosure.upstream.andy_wings_parametric_box"],
                cwd=directory,
                env={**os.environ, "PYTHONPATH": str(REPO_ROOT)},
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(tuple(Path(directory).iterdir()), ())

    def test_upstream_exposes_guarded_reusable_api(self) -> None:
        text = source("upstream/andy_wings_parametric_box.py")
        self.assertIn("def build_box(measures):", text)
        self.assertIn('if __name__ == "__main__":', text)
        self.assertIn("Clip(m.clip)", text)
        self.assertNotIn("clip=Clip", text)
        self.assertIn("m.hinge.leaf_depth", text)
        self.assertIn("if m.text:", text)
        self.assertIn("positions_x", text)

    def test_upstream_attribution_is_generic_cc_by_sa(self) -> None:
        for name in ("upstream/README.md", "upstream/andy_wings_parametric_box.py"):
            text = source(name)
            self.assertIn("CC BY-SA, version unspecified by controlling source", text)
            self.assertNotIn("CC BY-SA 4.0", text)

    def test_rejected_homemade_sources_are_absent(self) -> None:
        for suffix in ("hinge.py", "external_latches.py", "corner_blocks.py"):
            path = ROOT / f"protective_case_{suffix}"
            self.assertFalse(path.exists(), path.name)

    def test_protective_case_uses_upstream_and_new_ownership(self) -> None:
        generator = source("generate_protective_case_cadquery.py")
        geometry = source("protective_case_geometry.py")
        restraints = source("protective_case_corner_restraints.py")
        keepouts = source("protective_case_keepouts.py")
        orientation = source("protective_case_device_orientation.py")
        foam = source("protective_case_foam_landing_pads.py")
        latches = source("protective_case_bottom_latches.py")
        self.assertIn("andy_wings_parametric_box", generator)
        self.assertIn("return upstream.build_box(parametric_box_measures(params, ()))", generator)
        self.assertIn("add_corner_restraints", generator)
        self.assertIn("normalize_source_half", geometry)
        self.assertIn("both_wall_witnesses", restraints)
        self.assertIn("--check-artifacts", source("validate_protective_case.py"))
        self.assertIn("corner_restraint_components", restraints)
        self.assertIn("face_down_transport_transform", geometry)
        self.assertIn("face_down_transport_feature_xy", keepouts)
        self.assertIn("device_orientation_guide_components", orientation)
        self.assertIn("device_orientation_turnaround_arrow_components", orientation)
        self.assertIn("validate_device_orientation", orientation)
        self.assertIn("upstream.Clip", latches)
        self.assertIn("bottom_latch_components", foam)
        self.assertNotIn("upstream.Clip", foam)
        self.assertIn("bottom_latch_components", orientation)
        self.assertNotIn("upstream.Clip", orientation)
        self.assertIn('mirror("XY"', latches)
        self.assertIn("add_bottom_latches", generator)

    def test_params_have_exact_new_outputs_and_no_old_terms(self) -> None:
        params = json.loads(source("protective_case_params.json"))
        paths = [Path(path) for path in params["artifacts"]]
        names = [path.name for path in paths]
        self.assertEqual(len(names), 8)
        self.assertEqual(len(set(names)), 8)
        self.assertEqual(
            [path.as_posix() for path in paths],
            [
                "release-artifacts/enclosure/step/transport_case_deep_tub_debossed_logo.step",
                "release-artifacts/enclosure/stl/transport_case_deep_tub_debossed_logo.stl",
                "release-artifacts/enclosure/3mf/transport_case_deep_tub_debossed_logo.3mf",
                "release-artifacts/enclosure/3mf-multicolor/transport_case_deep_tub_multicolor_logo.3mf",
                "release-artifacts/enclosure/step/transport_case_shallow_lid_debossed_branding.step",
                "release-artifacts/enclosure/stl/transport_case_shallow_lid_debossed_branding.stl",
                "release-artifacts/enclosure/3mf/transport_case_shallow_lid_debossed_branding.3mf",
                "release-artifacts/enclosure/3mf-multicolor/transport_case_shallow_lid_multicolor_branding.3mf",
            ],
        )
        three_mf_paths = [path for path in paths if path.suffix == ".3mf"]
        self.assertEqual(sum(path.parent.as_posix() == "release-artifacts/enclosure/3mf" for path in three_mf_paths), 2)
        self.assertEqual(sum(path.parent.as_posix() == "release-artifacts/enclosure/3mf-multicolor" for path in three_mf_paths), 2)
        debossed_paths = [path for path in three_mf_paths if "debossed" in path.name]
        self.assertEqual(len(debossed_paths), 2)
        self.assertTrue(all(path.parent.as_posix() == "release-artifacts/enclosure/3mf" for path in debossed_paths))
        self.assertFalse(any(path.parent.as_posix() == "release-artifacts/enclosure/3mf-multicolor" for path in debossed_paths))
        self.assertTrue(all("debossed" in name or "multicolor" in name for name in names))
        legacy_branding = "emb" + "ossed"
        self.assertFalse(any("hinged" in name or "prototype" in name or legacy_branding in name for name in names))
        self.assertIn("seated_base_z", source("protective_case_params.json"))
        self.assertIn("paddle_envelope", source("protective_case_params.json"))
        restraint_text = source("protective_case_params.json")
        self.assertIn("shelf_plan_radius", restraint_text)
        self.assertIn("shelf_top_edge_radius", restraint_text)
        self.assertIn("support_lower_polygon", restraint_text)
        self.assertIn("support_upper_polygon", restraint_text)
        self.assertIn("support_lower_z", restraint_text)
        self.assertIn('"bottom_latches"', restraint_text)
        self.assertEqual(
            params["bottom_latches"],
            {
                "root_bridge": {"depth": 0.2, "host_profile_overlap": 0.1},
                "actuator": {"width": 19.4, "z": [56.95, 60.7]},
                "catch": {"width": 23.4, "z": [47.6, 51.6]},
            },
        )
        self.assertNotIn('"seam_z"', restraint_text)
        self.assertNotIn('"wall_y"', restraint_text)
        self.assertNotIn('"centers"', restraint_text)
        self.assertEqual(
            params["branding"],
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
        self.assertTrue(all("deep_tub_debossed_logo" in name or "deep_tub_multicolor_logo" in name or "shallow_lid" in name for name in names))
        self.assertTrue(all("shallow_lid_debossed_branding" in name or "shallow_lid_multicolor_branding" in name or "deep_tub" in name for name in names))
        self.assertNotIn("floor_toes", restraint_text)
        self.assertNotIn("wall_wings", restraint_text)
        self.assertNotIn("tangential_run", restraint_text)
        self.assertNotIn("floor_toe", restraint_text)
        self.assertNotIn("wall_wing", restraint_text)
        self.assertNotIn('"root"', restraint_text)

    def test_contact_pad_rounding_is_local_to_shelf(self) -> None:
        restraints = source("protective_case_corner_restraints.py")
        self.assertIn("def shelf_contact_pad", restraints)
        self.assertIn("def _rounded_shelf_plan", restraints)
        self.assertIn("def _device_facing_top_edges", restraints)
        self.assertIn(".fillet(spec[\"shelf_top_edge_radius\"])", restraints)
        self.assertIn("shelf_contact_pad(restraint, params)", restraints)
        self.assertNotIn("result.edges", restraints)

    def test_shelf_support_is_one_ruled_loft(self) -> None:
        restraints = source("protective_case_corner_restraints.py")
        geometry = source("protective_case_geometry.py")
        self.assertIn("def continuous_shelf_support", restraints)
        self.assertIn(".loft(combine=True, ruled=True)", restraints)
        self.assertNotIn("_wing_to_shelf_tab", restraints)
        self.assertNotIn("lower_corner_restraint_component", restraints)
        self.assertNotIn("floor_toes", geometry)
        self.assertNotIn("wall_wings", geometry)
        self.assertIn("return continuous_shelf_support(restraint, params).union(shelf_contact_component(restraint, params)).clean()", restraints)
        self.assertIn("continuous_shelf_support(restraint, params)", restraints)

    def test_protective_branding_uses_deep_logo_and_shallow_lockup_ownership(self) -> None:
        branding = source("protective_case_branding.py")
        self.assertIn("branding_marking_parts_placed", branding)
        self.assertIn("logo_marking_placed", branding)
        self.assertIn("def rotate_lockup_group", branding)
        self.assertIn("shallow_lid_lockup_rotation_degrees", branding)
        self.assertIn("build_deep_tub_multicolor_branding_parts", branding)
        self.assertIn("transport_case_deep_tub_device_orientation_guide", branding)
        self.assertIn("mirror_for_negative_z_exterior_view", branding)
        self.assertNotIn("device_origin", branding)
        self.assertNotIn("branding_marking_parts(", branding)
        self.assertNotIn("_center_parts", branding)
        for name in (
            "protective_case_branding.py",
            "protective_case_branding_validation.py",
            "generate_protective_case_cadquery.py",
            "protective_case_3mf_validation.py",
            "validate_protective_case.py",
        ):
            self.assertNotIn("emb" + "oss", source(name).lower(), name)
        validator = source("protective_case_3mf_validation.py")
        self.assertIn('"transport_case_deep_tub_logo", "2"', validator)
        self.assertIn('"transport_case_deep_tub_device_orientation_guide", "2"', validator)
        self.assertNotIn('"transport_case_deep_tub_wordmark", "2"', validator)
        self.assertIn('"transport_case_shallow_lid_logo", "2"', validator)
        self.assertIn('"transport_case_shallow_lid_wordmark", "2"', validator)
        self.assertIn("deep tub branding must contain logo only", source("protective_case_branding_validation.py"))
        self.assertIn("shallow lid branding must contain logo and wordmark", source("protective_case_branding_validation.py"))
        generator = source("generate_protective_case_cadquery.py")
        self.assertIn('orient_for_print(debossed_deep_tub, "negative_z")', generator)
        self.assertIn("apply_print_transform(part, deep_transform)", generator)
        self.assertIn('orient_for_print(debossed_shallow_lid, "positive_z")', generator)
        self.assertIn("apply_print_transform(part, shallow_transform)", generator)
        self.assertIn("def print_transform(", generator)
        self.assertIn("def apply_print_transform(", generator)
        self.assertIn("deep_transform = print_transform(multicolor_deep_tub, \"negative_z\")", generator)
        self.assertIn("shallow_transform = print_transform(multicolor_shallow_lid, \"positive_z\")", generator)
        package_validator = source("protective_case_3mf_validation.py")
        self.assertIn("root_components", package_validator)
        self.assertIn("expected_mesh_bboxes", package_validator)
        self.assertIn("identity", package_validator)
        self.assertIn("mesh object IDs", package_validator)

    def test_xy_retention_has_no_source_owned_hard_locator_geometry(self) -> None:
        params = json.loads(source("protective_case_params.json"))
        self.assertFalse(params["device"]["xy_retention"]["integrated_hard_locators"])
        for name in (
            "protective_case_geometry.py",
            "generate_protective_case_cadquery.py",
            "protective_case_corner_restraints.py",
            "protective_case_keepouts.py",
            "protective_case_foam_landing_pads.py",
        ):
            self.assertNotIn("locator", source(name).lower(), name)

    def test_foam_landing_pads_have_separate_owner_and_generator_order(self) -> None:
        params = json.loads(source("protective_case_params.json"))
        self.assertEqual(params["foam_landing_pads"]["center_z"], 32.0)
        self.assertEqual(params["foam_landing_pads"]["wall_overlap"], 0.4)
        self.assertEqual(params["foam_landing_pads"]["cavity_proud"], 0.4)
        self.assertEqual(set(params["foam_landing_pads"]["walls"]), {"west", "east", "south", "north"})
        module = source("protective_case_foam_landing_pads.py")
        generator = source("generate_protective_case_cadquery.py")
        restraints = source("protective_case_corner_restraints.py")
        self.assertIn("def foam_landing_pad_components", module)
        self.assertIn("def add_foam_landing_pads", module)
        self.assertIn("add_foam_landing_pads", generator)
        self.assertIn("corrected = apply_deep_mating_interface(params, build_pristine_deep_tub(params))", generator)
        self.assertIn("add_device_orientation_guide", generator)
        self.assertIn("def build_deep_tub_base", generator)
        self.assertIn("latched = add_bottom_latches(params, corrected, \"deep\")", generator)
        self.assertIn("return add_foam_landing_pads(params, add_corner_restraints(params, latched))", generator)
        self.assertIn("return add_device_orientation_guide(params, build_deep_tub_base(params))", generator)
        self.assertIn("return compose_multicolor_deep_tub(params, build_deep_tub_base)", generator)
        self.assertNotIn("foam_landing_pad", restraints)

    def test_mating_interface_has_separate_owner_and_normalized_generator_order(self) -> None:
        params = json.loads(source("protective_case_params.json"))
        self.assertEqual(
            params["mating_interface"],
            {
                "main_cavity": {"width": 251.2, "depth": 144.0, "radius": 9.5, "center": [127.8, 74.2], "deep_z": [2.2, 54.65], "shallow_z": [54.85, 62.65]},
                "opening": {"width": 250.0, "depth": 142.0, "radius": 8.5, "center": [127.8, 74.2], "cut_z": [52.55, 54.85]},
                "tongue": {"thickness": 1.3, "z": [52.65, 54.85], "corner_tangent_relief": 1.0, "south_hinge_relief_margin": 1.0},
                "receiver": {"xy_clearance": 0.25, "cut_z": [52.55, 54.65]},
                "lip_corner_relief": {"z": [52.55, 54.85]},
            },
        )
        mating = source("protective_case_mating_interface.py")
        generator = source("generate_protective_case_cadquery.py")
        validator = source("validate_protective_case.py")
        self.assertIn("def opening_prism", mating)
        self.assertIn("def mating_rails", mating)
        self.assertIn("def receiver_rails", mating)
        self.assertIn("def apply_shallow_mating_interface", mating)
        self.assertIn("def apply_deep_mating_interface", mating)
        self.assertIn("def shallow_hinge_gap_bar_relief_components", mating)
        self.assertIn("def original_diagonal_clash", mating)
        self.assertIn("def hinge_profile_symmetric_difference", mating)
        self.assertIn("def receiver_exterior_lands", mating)
        self.assertIn("def minimum_normal_corner_wall", mating)
        self.assertIn("def symmetric_difference_volume", mating)
        self.assertIn("def corrected_outer_bbox_delta", mating)
        self.assertNotIn("configured_clip_section_areas", mating)
        self.assertNotIn("clip_addition_symmetric_difference", mating)
        self.assertIn("apply_deep_mating_interface(params, build_pristine_deep_tub(params))", generator)
        self.assertIn("apply_shallow_mating_interface(params, build_pristine_shallow_lid(params))", generator)
        self.assertIn("validate_mating_interface", validator)
        self.assertIn("build_pristine_deep_tub", generator)
        self.assertIn("build_pristine_shallow_lid", generator)

    def test_no_project_hinge_or_clip_cutter(self) -> None:
        generator = source("generate_protective_case_cadquery.py")
        self.assertNotIn(".cut(", generator)
        self.assertNotIn("four", generator.lower())

    def test_protective_checked_lane_is_narrow_and_current(self) -> None:
        checked = source("generate_protective_case_artifacts_checked.ps1")
        worker = source("generate_protective_case_artifacts_worker.ps1")
        async_launcher = source("generate_protective_case_artifacts_async.ps1")
        status = source("protective_case_artifacts_async_status.ps1")
        self.assertIn("generate_protective_case_cadquery.py", checked)
        self.assertIn("validate_protective_case.py", checked)
        self.assertIn("--check-artifacts", checked)
        self.assertIn("__PROTECTIVE_CASE_VALIDATION_DONE__", checked)
        self.assertNotIn("Remove-Item", checked)
        self.assertIn("__PROTECTIVE_CASE_VALIDATION_DONE__", worker)
        self.assertIn("Start-Process", async_launcher)
        self.assertIn("pid", async_launcher)
        self.assertIn("__PROTECTIVE_CASE_ASYNC_STATUS_DONE__", status)


if __name__ == "__main__":
    unittest.main()
