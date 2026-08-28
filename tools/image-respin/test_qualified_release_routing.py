from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BOARD_WORKFLOW = ROOT / ".github" / "workflows" / "release-board-artifacts.yml"
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release-artifacts.yml"
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


def _section(text: str, start: str, end: str | None = None) -> str:
    start_index = text.index(start)
    end_index = len(text) if end is None else text.index(end, start_index)
    return text[start_index:end_index]


def _job(text: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_]+:|\Z)",
        text,
    )
    if match is None:
        raise AssertionError(f"Missing workflow job: {name}")
    return match.group()


class QualifiedReleaseRoutingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.board = BOARD_WORKFLOW.read_text(encoding="utf-8")
        cls.release = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cls.ci = CI_WORKFLOW.read_text(encoding="utf-8")

    def test_board_mode_inputs_and_fail_closed_validation(self) -> None:
        call = _section(self.board, "  workflow_call:", "  workflow_dispatch:")
        self.assertEqual(
            re.findall(r"^      ([A-Za-z0-9_-]+):$", call, re.MULTILINE),
            ["tag", "version", "source_sha", "board_image_mode"],
        )
        for name in ("tag", "version", "source_sha", "board_image_mode"):
            with self.subTest(name=name):
                self.assertRegex(call, rf"      {name}:\n        required: true\n        type: string")

        dispatch = _section(self.board, "  workflow_dispatch:", "permissions:")
        self.assertRegex(
            dispatch,
            r"      board_image_mode:\n"
            r"        description: Select the full constructor or qualified respin board image path\n"
            r"        required: true\n"
            r"        default: base-refresh\n"
            r"        type: choice\n"
            r"        options:\n"
            r"          - base-refresh\n"
            r"          - qualified-respin",
        )
        self.assertIn('case "$BOARD_IMAGE_MODE" in', self.board)
        self.assertIn("base-refresh|qualified-respin", self.board)
        self.assertIn(
            '*) echo "Unsupported board image mode: $BOARD_IMAGE_MODE" >&2; exit 1 ;;',
            self.board,
        )

    def test_constructor_and_respin_paths_are_isolated(self) -> None:
        self.assertEqual(
            self.board.count("if: inputs.board_image_mode == 'base-refresh'"), 5
        )
        self.assertEqual(
            self.board.count("if: inputs.board_image_mode == 'qualified-respin'"), 2
        )
        for name in (
            "raspberry_runtime",
            "orange_runtime",
            "raspberry_kernel",
            "raspberry_image",
            "orange_image",
        ):
            with self.subTest(job=name):
                constructor = _job(self.board, name)
                self.assertIn("if: inputs.board_image_mode == 'base-refresh'", constructor)
                self.assertNotIn("qualified-respin", constructor)

        for name, board in (
            ("raspberry_respin", "raspberry-pi-zero-2w"),
            ("orange_respin", "orange-pi-zero-2w"),
        ):
            with self.subTest(job=name):
                respin = _job(self.board, name)
                self.assertIn("if: inputs.board_image_mode == 'qualified-respin'", respin)
                self.assertIn("needs: validate_board_image_mode", respin)
                self.assertIn("uses: ./.github/workflows/respin-board-image.yml", respin)
                self.assertIn(f"board: {board}", respin)
                self.assertIn("setup_layer: setup-portal", respin)
                self.assertIn("source_sha: ${{ inputs.source_sha }}", respin)
                self.assertIn("version: ${{ inputs.version }}", respin)
                self.assertIn("tag: ${{ inputs.tag }}", respin)
                for forbidden in (
                    "cross build",
                    "pi-gen",
                    "build-armbian-image",
                    "verify-sanitized-image.sh",
                    "verify-orange-image.sh",
                ):
                    with self.subTest(forbidden=forbidden):
                        self.assertNotIn(forbidden, respin)
        self.assertEqual(
            self.board.count("uses: ./.github/workflows/respin-board-image.yml"), 2
        )

    def test_runtime_ownership_and_fail_closed_device_dependencies(self) -> None:
        for name, runtime, build in (
            (
                "raspberry_runtime",
                "octessera-raspberry-runtime",
                "cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-raspberry-pi-zero-2w",
            ),
            (
                "orange_runtime",
                "octessera-orange-runtime",
                "cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w",
            ),
        ):
            with self.subTest(job=name):
                runtime_job = _job(self.board, name)
                self.assertEqual(runtime_job.count(build), 1)
                self.assertEqual(runtime_job.count(f"name: {runtime}"), 1)

        self.assertEqual(
            len(re.findall(r"^  (?:raspberry|orange)_device:$", self.board, re.MULTILINE)),
            2,
        )
        for name, runtime, respin in (
            ("raspberry_device", "raspberry_runtime", "raspberry_respin"),
            ("orange_device", "orange_runtime", "orange_respin"),
        ):
            with self.subTest(job=name):
                device = _job(self.board, name)
                self.assertIn(
                    f"needs: [validate_board_image_mode, {runtime}, {respin}]", device
                )
                self.assertIn("always()", device)
                self.assertIn(f"needs.{runtime}.result == 'success'", device)
                self.assertIn(f"needs.{respin}.result == 'success'", device)

    def test_publishing_is_base_only_and_static_gated(self) -> None:
        dispatch = _section(self.release, "  workflow_dispatch:", "concurrency:")
        self.assertEqual(
            re.findall(r"^      ([A-Za-z0-9_-]+):$", dispatch, re.MULTILINE), ["tag"]
        )
        self.assertNotIn("board_image_mode", dispatch)
        self.assertNotIn("qualified-respin", self.release)
        self.assertNotIn("inputs.board_image_mode", self.release)
        self.assertNotIn('case "$BOARD_IMAGE_MODE" in', self.release)

        board_call = _job(self.release, "board_artifacts")
        self.assertIn("tag: ${{ needs.release_info.outputs.tag }}", board_call)
        self.assertIn("version: ${{ needs.release_info.outputs.version }}", board_call)
        self.assertIn("source_sha: ${{ needs.release_info.outputs.source_sha }}", board_call)
        self.assertIn("board_image_mode: base-refresh", board_call)

        publisher = _job(self.release, "publish_release_assets")
        self.assertIn("--board-image-mode base-refresh", publisher)
        self.assertEqual(self.release.count("--board-image-mode base-refresh"), 1)
        self.assertNotIn("BOARD_IMAGE_MODE", publisher)
        self.assertIn("python3 tools/image-respin/test_qualified_release_routing.py", self.release)
        self.assertIn("python3 tools/image-respin/test_qualified_release_routing.py", self.ci)

    def test_no_fallback_cache_parent_or_publication_promotion(self) -> None:
        for name, text in (("release", self.release), ("board", self.board)):
            with self.subTest(workflow=name):
                for forbidden in (
                    "actions/cache",
                    "fallback",
                    "parent-url",
                    "parent_url",
                    "promot",
                    "gh release edit",
                    "--draft=false",
                ):
                    with self.subTest(forbidden=forbidden):
                        self.assertNotIn(forbidden, text)
                self.assertNotRegex(text, r"(?i)(latest[-_ ]+parent|parent[-_ ]+latest)")
                self.assertNotRegex(
                    text,
                    r"(?m)^      (?:parent|parent-url|parent_url|latest|cache):$",
                )


if __name__ == "__main__":
    unittest.main()
