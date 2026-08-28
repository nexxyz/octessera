from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path
from typing import Callable

from tools.release.assemble_release_assets import assemble_release_assets
from tools.release.board_image_release import ORANGE, RPI, RESPIN_FEATURE_COMMANDS, ReleaseArtifactError, verify_and_stage_board_images
from tools.release.board_image_release_test_support import (
    ROOT,
    SOURCE_SHA,
    VERSION,
    fixture,
    full_fixture,
    refresh_checksum,
    write_json,
)


class QualifiedBoardImageTests(unittest.TestCase):
    def test_feature_commands_are_bound_to_reusable_workflow_commands(self) -> None:
        workflow = (ROOT / ".github/workflows/respin-board-image.yml").read_text(encoding="utf-8")
        for command in RESPIN_FEATURE_COMMANDS.values():
            self.assertIn(f"feature_command='{command}'", workflow)

    def _run_fixture(self, mutation: Callable[[Path, Path], None] | None = None, source_sha: str = SOURCE_SHA, version: str = VERSION) -> tuple[Path, Path]:
        temporary = tempfile.TemporaryDirectory(dir=ROOT)
        self.addCleanup(temporary.cleanup)
        work = Path(temporary.name)
        gathered, rpi_runtime, orange_runtime, release, evidence = fixture(work)
        if mutation is not None:
            mutation(gathered / f"octessera-{RPI}-image-release-assets", gathered / f"octessera-{ORANGE}-image-release-assets")
        verify_and_stage_board_images(ROOT, gathered, release, evidence, rpi_runtime, orange_runtime, version, source_sha, "qualified-respin")
        return release, evidence

    def test_valid_raspberry_and_orange_setup_respins_are_normalized_with_evidence(self) -> None:
        release, evidence = self._run_fixture()
        self.assertTrue((release / f"octessera-{VERSION}-raspberry-pi-zero-2w.img.zip").is_file())
        self.assertTrue((release / f"octessera-{VERSION}-raspberry-pi-zero-2w.rpi-imager-manifest").is_file())
        self.assertTrue((release / f"octessera-{VERSION}-orange-pi-zero-2w.img.xz").is_file())
        self.assertEqual(
            sorted(path.name for path in release.iterdir()),
            sorted(
                [
                    f"octessera-{VERSION}-raspberry-pi-zero-2w.img.zip",
                    f"octessera-{VERSION}-raspberry-pi-zero-2w.rpi-imager-manifest",
                    f"octessera-{VERSION}-orange-pi-zero-2w.img.xz",
                ]
            ),
        )

    def test_full_qualified_assembly_keeps_exact_fourteen_root_assets(self) -> None:
        temporary = tempfile.TemporaryDirectory(dir=ROOT)
        self.addCleanup(temporary.cleanup)
        work = Path(temporary.name)
        gathered, rpi_runtime, orange_runtime, release, evidence = full_fixture(work)
        shutil.rmtree(evidence)
        assemble_release_assets(
            ROOT,
            gathered,
            rpi_runtime,
            orange_runtime,
            release,
            evidence,
            VERSION,
            SOURCE_SHA,
            "qualified-respin",
        )
        prefix = f"octessera-{VERSION}"
        expected = sorted(
            [
                f"{prefix}-windows-installer.exe",
                f"{prefix}-windows-portable.zip",
                f"{prefix}-ubuntu-amd64.deb",
                f"{prefix}-ubuntu-x86_64.AppImage",
                f"{prefix}-raspberry-pi-zero-2w.img.zip",
                f"{prefix}-raspberry-pi-zero-2w.rpi-imager-manifest",
                f"{prefix}-raspberry-pi-zero-2w-device-aarch64.zip",
                f"SHA256SUMS-raspberry-pi-zero-2w-device.txt",
                f"{prefix}-orange-pi-zero-2w.img.xz",
                f"{prefix}-orange-pi-zero-2w-standalone-manual-aarch64.zip",
                f"{prefix}-orange-pi-zero-2w-runtime-updater-aarch64.zip",
                f"SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt",
                f"{prefix}-release-evidence.zip",
                "SHA256SUMS.txt",
            ]
        )
        self.assertEqual(sorted(path.name for path in release.iterdir()), expected)
        self.assertEqual(
            sorted(path.name for path in (evidence / "raspberry/image").iterdir()),
            sorted(
                [
                    f"SHA256SUMS-{RPI}.txt",
                    f"octessera-{VERSION}-{RPI}-derived-setup-respin.zip.provenance.json",
                    "raspberry-sanitized-image-proof.txt",
                    f"octessera-{VERSION}-raspberry-pi-zero-2w.rpi-imager-manifest",
                    "requested-build.json",
                    "setup-layer-proof.json",
                    "setup-post-proof.json",
                    "v0.7.5-trust-manifest.json",
                ]
            ),
        )
        self.assertEqual(
            sorted(path.name for path in (evidence / "orange/image").iterdir()),
            sorted(
                [
                    f"SHA256SUMS-{ORANGE}.txt",
                    f"octessera-{VERSION}-{ORANGE}-derived-setup-respin.img.xz.provenance.json",
                    f"octessera-{VERSION}-orange-pi-zero-2w.img.xz.sha256",
                    "orange-image-proof.json",
                    "requested-build.json",
                    "setup-layer-proof.json",
                    "setup-post-proof.json",
                    "v0.7.5-trust-manifest.json",
                ]
            ),
        )

    def test_wrong_source_version_and_board_are_rejected(self) -> None:
        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(source_sha="b" * 40)
        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(version="0.7.7")

        def wrong_board(rpi: Path, _orange: Path) -> None:
            requested = json.loads((rpi / "requested-build.json").read_text(encoding="utf-8"))
            requested["source"]["board"] = ORANGE
            write_json(rpi / "requested-build.json", requested)
            refresh_checksum(rpi, RPI)

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(wrong_board)

    def test_trust_parent_and_companion_identity_are_rejected(self) -> None:
        def wrong_parent(_rpi: Path, orange: Path) -> None:
            record = json.loads((orange / "setup-post-proof.json").read_text(encoding="utf-8"))
            record["parent"]["context"]["asset"]["sha256"] = "0" * 64
            write_json(orange / "setup-post-proof.json", record)
            refresh_checksum(orange, ORANGE)

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(wrong_parent)

        def wrong_companion(_rpi: Path, orange: Path) -> None:
            record = json.loads((orange / "setup-post-proof.json").read_text(encoding="utf-8"))
            record["companions"][0]["sha256"] = "0" * 64
            write_json(orange / "setup-post-proof.json", record)
            refresh_checksum(orange, ORANGE)

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(wrong_companion)

    def test_missing_extra_and_bad_checksum_handoff_files_are_rejected(self) -> None:
        def missing(rpi: Path, _orange: Path) -> None:
            (rpi / "setup-layer-proof.json").unlink()

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(missing)

        def extra(_rpi: Path, orange: Path) -> None:
            (orange / "unexpected.txt").write_text("unexpected", encoding="utf-8")

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(extra)

        def bad_checksum(rpi: Path, _orange: Path) -> None:
            checksum = rpi / f"SHA256SUMS-{RPI}.txt"
            lines = checksum.read_text(encoding="utf-8").splitlines()
            checksum.write_text("0" * 64 + lines[0][64:] + "\n" + "\n".join(lines[1:]) + "\n", encoding="utf-8")

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(bad_checksum)

    def test_non_blacklisted_wrong_requested_command_is_rejected(self) -> None:
        def wrong_command(rpi: Path, _orange: Path) -> None:
            requested = json.loads((rpi / "requested-build.json").read_text(encoding="utf-8"))
            requested["source"]["feature_command"] = "cross build --release -p octessera-pi --features hardware-raspberry-pi-zero-2w"
            write_json(rpi / "requested-build.json", requested)
            refresh_checksum(rpi, RPI)

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(wrong_command)

    def test_constructor_shaped_requested_build_is_rejected(self) -> None:
        def constructor_claim(rpi: Path, _orange: Path) -> None:
            requested = json.loads((rpi / "requested-build.json").read_text(encoding="utf-8"))
            requested["source"]["feature_command"] = "bash tools/pi-kernel/build-rpi-kernel.sh"
            write_json(rpi / "requested-build.json", requested)
            refresh_checksum(rpi, RPI)

        with self.assertRaises(ReleaseArtifactError):
            self._run_fixture(constructor_claim)


if __name__ == "__main__":
    unittest.main()
