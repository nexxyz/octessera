from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "respin-board-image.yml"
sys.path.insert(0, str(ROOT / "tools" / "image-respin"))
from post_proof_record import PROOF_TEMPLATE
from requested_build_record import REQUIRED_FILES


class RespinWorkflowStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_manual_dispatch_is_orange_runtime_only(self) -> None:
        self.assertIn("  workflow_dispatch:\n", self.text)
        self.assertNotIn("workflow_call:", self.text)
        self.assertNotIn("inputs.", self.text)
        self.assertNotRegex(self.text, r"^\s+(push|pull_request|schedule):", re.MULTILINE)
        self.assertIn("BOARD: orange-pi-zero-2w", self.text)
        self.assertNotIn("raspberry-pi-zero-2w", self.text)
        self.assertNotIn("SETUP_LAYER", self.text)
        self.assertNotIn("setup-portal", self.text)
        self.assertIn("permissions:\n  actions: read\n  contents: read", self.text)
        self.assertNotIn("contents: write", self.text)

    def test_source_is_pinned_to_manual_dispatch_commit(self) -> None:
        self.assertIn("ref: ${{ github.sha }}", self.text)
        self.assertIn('test "$(git rev-parse HEAD)" = "${{ github.sha }}"', self.text)
        self.assertIn("fetch-depth: 0", self.text)
        self.assertNotIn("fetch-tags:", self.text)

    def test_orange_parent_contract_is_canonical(self) -> None:
        self.assertIn("PARENT_RECORD: resources/image-parents/orange-pi-zero-2w-current.json", self.text)
        self.assertIn("BOOT_NEUTRAL_CONTRACT: resources/image-derivations/boot-neutral/orange-pi-zero-2w-v0.8.1.json", self.text)

    def test_parent_acquisition_precedes_dependencies_and_build(self) -> None:
        acquisition = self.text.index("- name: Acquire the exact reviewed current parent")
        dependencies = self.text.index("- name: Install image respin dependencies")
        build = self.text.index("- name: Build selected runtime exactly once")
        self.assertLess(acquisition, dependencies)
        self.assertLess(acquisition, build)
        self.assertIn("actions: read", self.text)

    def test_current_parent_is_acquired_and_bound(self) -> None:
        self.assertIn("python3 tools/image-respin/current_parent.py", self.text)
        self.assertIn('--repository nexxyz/octessera', self.text)
        self.assertIn('--record "$PARENT_RECORD"', self.text)
        self.assertNotIn("PARENT_CONTEXT", self.text)
        self.assertIn('--parent-record "$PARENT_RECORD"', self.text)
        self.assertNotIn("--trust-manifest", self.text)
        self.assertNotIn("v0.7.5", self.text)
        self.assertNotIn("trusted-v0.7.5-boot-neutral", self.text)

    def test_runtime_build_and_proof_are_orange_only(self) -> None:
        command = "cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w"
        self.assertEqual(self.text.count(command), 2)
        self.assertIn("feature_command='" + command + "'", self.text)
        self.assertIn("--boot-proof-mode validated-parent", self.text)
        self.assertIn("verify-orange-image.sh", self.text)
        self.assertIn("orange-image=orange-production", self.text)
        self.assertEqual(set(PROOF_TEMPLATE), {"orange-image"})
        self.assertNotIn("verify-sanitized-image.sh", self.text)
        self.assertNotIn("RPI_PROOF_IMAGE", self.text)

    def test_record_inputs_match_record_contract(self) -> None:
        match = re.search(
            r"(?ms)^\s+- name: Record requested build identity\s*\n(?P<body>.*?)(?=^\s+- name: |\Z)",
            self.text,
        )
        self.assertIsNotNone(match)
        assert match is not None
        body = match.group("body")
        input_files = re.findall(r"^\s+--input-file\s+(?P<path>[^\s\\]+)\s*\\\s*$", body, re.MULTILINE)
        self.assertEqual(input_files, list(REQUIRED_FILES))
        loop = re.findall(r"^\s+for path in (?P<paths>tools/image-respin/[^;\n]+); do\s*$", body, re.MULTILINE)
        self.assertEqual(loop, [])

    def test_setup_layer_is_not_exposed(self) -> None:
        self.assertNotIn("setup-portal", self.text)
        self.assertNotIn("disk_setup_respin.py", self.text)
        self.assertNotIn("setup-post-proof", self.text)
        self.assertNotIn("setup-layer-proof", self.text)

    def test_no_release_inputs_or_handoffs(self) -> None:
        for value in ("REQUESTED_", "RELEASE_", "workflow_call", "inputs.", "tag:", "--tag", "latest", "override", "fallback", "canonical Orange", "octessera-orange-runtime", "octessera-orange-image-release-assets", "gh release"):
            self.assertNotIn(value, self.text)
        self.assertIn("uses: actions/upload-artifact@v7", self.text)
        self.assertEqual(self.text.count("uses: actions/upload-artifact@v7"), 1)
        self.assertIn("name: octessera-orange-pi-zero-2w-derived-runtime-respin", self.text)
        for value in ("actions/cache", "cache:", "--clobber"):
            self.assertNotIn(value, self.text)
        self.assertIn("if-no-files-found: error", self.text)
        self.assertIn("retention-days: 7", self.text)


if __name__ == "__main__":
    unittest.main()
