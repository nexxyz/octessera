from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "respin-board-image.yml"
sys.path.insert(0, str(ROOT / "tools" / "image-respin"))
from post_proof_record import PROOF_TEMPLATE


class RespinWorkflowStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.text = WORKFLOW.read_text(encoding="utf-8")
        cls.ci_text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")

    def test_manual_only_board_choice_and_read_permission(self) -> None:
        self.assertIn("on:\n  workflow_dispatch:", self.text)
        self.assertNotRegex(self.text, r"^\s+(push|pull_request|schedule):", re.MULTILINE)
        self.assertRegex(
            self.text,
            r"options:\n\s+- raspberry-pi-zero-2w\n\s+- orange-pi-zero-2w",
        )
        self.assertIn("permissions:\n  contents: read", self.text)
        self.assertNotIn("contents: write", self.text)

    def test_source_trust_and_release_checks_are_exact(self) -> None:
        self.assertGreaterEqual(self.text.count("${{ github.sha }}"), 2)
        self.assertIn('test "$(git rev-parse HEAD)" = "${{ github.sha }}"', self.text)
        self.assertIn('with open("package.json", encoding="utf-8")', self.text)
        self.assertIn('with open("apps/pi-zero/Cargo.toml", "rb")', self.text)
        self.assertIn('with open("Cargo.toml", "rb")', self.text)
        self.assertIn('"apps/pi-zero" not in workspace["members"]', self.text)
        self.assertIn("Canonical versions disagree", self.text)
        self.assertNotIn("manifest_tag", self.text)
        self.assertNotIn('= "v$version"', self.text)
        self.assertEqual(len(re.findall(r"^\s+cross build --release --locked .*", self.text, re.MULTILINE)), 2)
        self.assertIn("git diff --exit-code", self.text)
        self.assertIn('git status --porcelain --untracked-files=no', self.text)
        self.assertIn("docker run --rm octessera-pi-cross rustc -vV", self.text)
        self.assertIn("docker run --rm octessera-pi-cross cargo --version", self.text)
        self.assertIn("--container-rustc-version-file", self.text)
        self.assertIn("--container-cargo-version-file", self.text)
        self.assertIn("resources/image-parents/v0.7.5-trust-manifest.json", self.text)
        self.assertIn("--validate-manifest", self.text)
        self.assertIn('gh api "repos/nexxyz/octessera/releases/tags/v0.7.5"', self.text)
        self.assertIn("jq -e '.draft == false and .prerelease == false'", self.text)
        self.assertIn('repos/nexxyz/octessera/git/ref/tags/v0.7.5', self.text)
        self.assertIn('repos/nexxyz/octessera/git/tags/$tag_object', self.text)
        self.assertIn("--release-json", self.text)
        self.assertIn("--print-board-assets --board", self.text)

    def test_no_cache_or_release_mutation_and_exact_downloads(self) -> None:
        forbidden = (
            "actions/cache",
            "cache:",
            "gh api --output",
            "gh release upload",
            "gh release edit",
            "gh release create",
            "gh release delete",
            "upload-release-asset",
            "--pattern",
            "--clobber",
        )
        for value in forbidden:
            with self.subTest(value=value):
                self.assertNotIn(value, self.text)
        self.assertNotRegex(self.text, r"gh api[^\n]*--output")
        self.assertIn('temporary="$(mktemp "$PARENT_ASSETS/.octessera-asset.XXXXXX")"', self.text)
        self.assertIn('mv -n -- "$temporary" "$destination"', self.text)
        self.assertNotRegex(self.text, r"gh (?:api|release)[^\n]*latest")
        self.assertIn("releases/assets/$asset_id", self.text)
        self.assertIn("if-no-files-found: error", self.text)
        self.assertIn("retention-days: 7", self.text)

    def test_parent_version_is_independent_from_future_source_version(self) -> None:
        future_source = {
            "package_json": "0.7.6",
            "pi_cargo": "0.7.6",
            "workspace_cargo": "0.7.6",
            "parent_tag": "v0.7.5",
        }
        self.assertEqual(
            len({future_source["package_json"], future_source["pi_cargo"], future_source["workspace_cargo"]}),
            1,
        )
        self.assertNotEqual(future_source["workspace_cargo"], future_source["parent_tag"].removeprefix("v"))
        self.assertIn("--version \"$OCTESSERA_VERSION\"", self.text)
        self.assertIn("octessera-$OCTESSERA_VERSION-$BOARD-derived-runtime-respin", self.text)
        self.assertNotIn("Respine", (ROOT / "tools" / "image-respin" / "disk_respin.py").read_text())

    def test_runtime_build_respin_naming_and_board_proofs_are_present(self) -> None:
        self.assertIn("timeout-minutes: 120", self.text)
        self.assertIn("uses: actions/checkout@v6", self.text)
        self.assertIn("uses: actions/upload-artifact@v7", self.text)
        self.assertNotIn("actions/checkout@v4", self.text)
        self.assertNotIn("actions/upload-artifact@v4", self.text)
        self.assertIn("apt-get install -y --no-install-recommends cpio", self.text)
        self.assertIn("zstd", self.text)
        self.assertIn("--features hardware-raspberry-pi-zero-2w", self.text)
        self.assertIn("--features hardware-orange-pi-zero-2w", self.text)
        self.assertIn("runtime_bundle.py", self.text)
        self.assertIn("--output \"$RUNTIME_BUNDLE\"", self.text)
        self.assertIn("sudo python3 tools/image-respin/disk_respin.py", self.text)
        self.assertIn("derived-runtime-respin", self.text)
        self.assertIn("verify-orange-image.sh", self.text)
        self.assertIn("--boot-proof-mode trusted-v0.7.5-boot-neutral", self.text)
        self.assertIn("--boot-neutral-contract", self.text)
        self.assertIn("--parent-image", self.text)
        self.assertIn("--derivation-kind", self.text)
        self.assertIn("--respin-provenance", self.text)
        self.assertIn("ORANGE_PARENT_IMAGE: parent-assets/octessera-0.7.5-orange-pi-zero-2w.img.xz", self.text)
        self.assertIn("verify-sanitized-image.sh", self.text)
        self.assertIn("verify-rpi-kernel-image.sh", self.text)
        self.assertIn("--manifest tools/kernel-patches/orange-midi-interface-manifest.json", self.text)
        self.assertIn('sums_name="SHA256SUMS-$BOARD.txt"', self.text)
        steps = (
            "Record requested build identity",
            "Respin the trusted parent image",
            "Re-run exact Orange production proof",
            "Generate and validate post-proof record",
            "Generate and validate board-qualified respin checksums",
            "Upload derived respin artifact only",
        )
        positions = [self.text.index(step) for step in steps]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("workflow_records.py requested", self.text)
        self.assertIn("workflow_records.py post-proof", self.text)
        self.assertIn("requested-build.json post-proof.json", self.text)
        self.assertIn('--root "$GITHUB_WORKSPACE"', self.text)
        self.assertIn('--artifact "$RESPIN_ARTIFACT"', self.text)
        self.assertIn('output="$RESPIN_OUTPUT/octessera-$OCTESSERA_VERSION-$BOARD-derived-runtime-respin$suffix"', self.text)
        self.assertNotIn('output="$GITHUB_WORKSPACE/$RESPIN_OUTPUT', self.text)
        self.assertIn("--proof-template", self.text)
        self.assertIn('orange-image=orange-production', self.text)
        self.assertIn('raspberry-kernel=raspberry-kernel', self.text)
        self.assertNotIn("RPI_PROOF_IMAGE", self.text)
        self.assertNotIn("proof_command_args", self.text)
        for label, (_, command, _) in PROOF_TEMPLATE.items():
            self.assertIn("{artifact}" if label != "raspberry-kernel" else "{extracted_image}", command)
            self.assertNotIn("GITHUB_WORKSPACE", command)
        self.assertIn("{extracted_image}", PROOF_TEMPLATE["raspberry-kernel"][1])
        for name in ("install-rpi-kernel.py", "rpi_kernel_contract.py", "rpi_kernel_image.py", "raspi_firmware_hook_mask.py"):
            self.assertIn(f"stage3-octessera-kernel/files/root/usr/local/lib/octessera/{name}", "\n".join(__import__("post_proof_record").RPI_TOOLS))
        self.assertIn("tools/pi-image/rpi_initramfs_proof.py", "\n".join(__import__("post_proof_record").RPI_TOOLS))
        self.assertIn("resources/image-construction/boot-layers/raspberry-pi-zero-2w.json", "\n".join(__import__("post_proof_record").RPI_TOOLS))

    def test_setup_layer_is_opt_in_and_separate_from_runtime_only(self) -> None:
        self.assertIn("setup_layer:", self.text)
        self.assertIn("default: runtime-only", self.text)
        self.assertIn("- setup-portal", self.text)
        self.assertIn("resources/image-mutations/$BOARD-setup.json", self.text)
        self.assertIn("disk_setup_respin.py", self.text)
        self.assertIn("setup-post-proof", self.text)
        self.assertIn("derived-setup-respin", self.text)
        self.assertIn("verify-sanitized-image.sh --setup-layer", self.text)
        self.assertIn("--strict-setup-source-tracking", self.text)
        self.assertIn('--production-proof "raspberry-sanitized=', self.text)
        self.assertIn('--production-proof "raspberry-kernel=', self.text)
        self.assertIn("raspberry-pi-zero-2w) proof_names=(setup-layer-proof.json raspberry-sanitized-image-proof.txt raspberry-kernel-image-proof.txt)", self.text)

    def test_ci_keeps_privileged_disk_tests_separate_from_nonroot_checks(self) -> None:
        self.assertIn("apt-get install -y --no-install-recommends cpio", self.ci_text)
        self.assertIn("shellcheck", self.ci_text)
        self.assertIn("zstd", self.ci_text)
        self.assertIn("go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.7", self.ci_text)
        self.assertIn('actionlint" -shellcheck shellcheck .github/workflows/respin-board-image.yml .github/workflows/ci.yml', self.ci_text)
        self.assertIn('CI: "true"', self.ci_text)
        self.assertNotIn("! grep", self.ci_text)
        self.assertIn("test_disk_*.py", self.ci_text)
        self.assertIn("test_trust_manifest.py", self.ci_text)
        self.assertIn("test_runtime_contract.py", self.ci_text)
        self.assertIn("test_workflow_records.py", self.ci_text)
        self.assertIn("test_setup_workflow_records.py", self.ci_text)
        action_text = (ROOT / ".github" / "actions" / "build-armbian-image" / "action.yml").read_text(encoding="utf-8")
        self.assertIn("boot_proof_mode:", action_text)
        self.assertIn("construction_contract:", action_text)
        self.assertIn("--boot-proof-mode", action_text)
        self.assertIn("--construction-contract", action_text)
        self.assertIn('--manifest "$custom_root/tools/kernel-patches/orange-midi-interface-manifest.json"', action_text)
        self.assertIn("--manifest tools/kernel-patches/orange-midi-interface-manifest.json", (ROOT / ".github" / "workflows" / "release-artifacts.yml").read_text(encoding="utf-8"))
        self.assertIn("--manifest tools/kernel-patches/orange-midi-interface-manifest.json", (ROOT / ".github" / "workflows" / "release-board-artifacts.yml").read_text(encoding="utf-8"))
        self.assertNotIn("test-trust-manifest.py", self.ci_text)
        self.assertNotIn("discover -s tools/image-respin -p 'test_*.py'", self.ci_text)


if __name__ == "__main__":
    unittest.main()
