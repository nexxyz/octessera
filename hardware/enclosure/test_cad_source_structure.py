from __future__ import annotations

import ast
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent

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
        modules = ROOT.glob("*.py")
        for module in modules:
            with self.subTest(module=module.name):
                ast.parse(module.read_text(encoding="utf-8"), filename=str(module))


if __name__ == "__main__":
    unittest.main()
