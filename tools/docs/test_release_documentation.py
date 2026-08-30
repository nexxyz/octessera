from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
USERDOCS = ROOT / "userdocs"


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


class ReleaseDocumentationTests(unittest.TestCase):
    def test_support_matrix_has_required_platforms_and_evidence_columns(self) -> None:
        text = read("userdocs/release-support.md")
        for platform in ("Desktop", "Raspberry Pi Zero 2 W", "Orange Pi Zero 2W"):
            self.assertIn(platform, text)
        for column in (
            "Asset type and intended use",
            "Source/build evidence",
            "Manual FAT status",
            "Known limitations",
        ):
            self.assertIn(column, text)
        self.assertIn("exact release asset", text)
        self.assertIn("FAT-passed", text)
        self.assertIn("UNQUALIFIED", text)
        self.assertIn("No open FAT result is closed", text)

    def test_owner_checklist_covers_publish_inputs_and_gates(self) -> None:
        text = read("userdocs/release-support.md").lower()
        required = (
            "version",
            "tag",
            "manifest",
            "source sha",
            "clean",
            "ci",
            "populated draft",
            "unpublished",
            "fourteen custom",
            "exact names",
            "checksum",
            "zip",
            "legal notice",
            "320-file",
            "default patch",
            "desktop package launch/fat",
            "raspberry fat",
            "orange fat",
            "source duties",
            "runtime-only respin",
            "explicitly publish",
        )
        for phrase in required:
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, text)

    def test_support_page_records_current_parent_and_open_release_boundaries(self) -> None:
        text = " ".join(read("userdocs/release-support.md").split())
        for phrase in (
            "v0.7.5 release remains immutable historical material",
            "exact Orange 0.8.1 constructor parent passed its bounded physical promotion scope",
            "manual current-parent runtime respin path exists",
            "no real runtime respin run is evidenced yet",
            "Official v0.8.1 publication",
            "remaining Orange hardware gates",
            "Raspberry physical qualification remains open, and a Raspberry current-parent respin is unavailable",
            "CONSTRUCTOR PARENT PROMOTION PASSED / OFFICIAL PUBLICATION AND FULL ORANGE HARDWARE OPEN",
            "do not claim full platform qualification",
            "record the bounded Orange physical result",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, text)
        for stale in ("trusted-v0.7.5", "trusted-parent machinery", "frozen legacy recovery"):
            with self.subTest(stale=stale):
                self.assertNotIn(stale, text)

    def test_usb_policy_is_explicit_and_public_docs_do_not_overclaim(self) -> None:
        support = read("userdocs/release-support.md")
        safety = read("userdocs/hardware/safety-and-power.md")
        bringup = read("hardware/docs/orange-pi-armbian-bringup.md")
        combined = "\n".join((support, safety, bringup))
        self.assertIn("Linux Foundation VID/PID", combined)
        self.assertIn("local-validation", combined)
        self.assertIn("not a public product identity", combined)
        self.assertIn("defaults remain disabled", combined.lower())
        self.assertIn("no-backfeed", combined)

        public_docs = "\n".join(path.read_text(encoding="utf-8") for path in USERDOCS.rglob("*.md"))
        for forbidden in (
            "USB Audio is the fixed",
            "USB MIDI is the fixed",
            "USB Audio/MIDI support is supported",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, public_docs)

    def test_sample_docs_name_the_full_library_on_all_three_paths(self) -> None:
        docs = (
            "userdocs/README.md",
            "userdocs/desktop-simulator.md",
            "userdocs/hardware/raspberry-pi-first-boot.md",
            "userdocs/hardware/orange-pi-first-boot.md",
        )
        for relative in docs:
            with self.subTest(relative=relative):
                text = read(relative).lower()
                self.assertIn("320-file", text)
        readme = read("userdocs/README.md").lower()
        self.assertIn("both production images", readme)
        self.assertIn("add your own samples", readme)

    def test_release_process_points_to_populated_draft_handoff(self) -> None:
        workflow_docs = read("docs/development-workflows.md")
        artifacts_docs = read("release-artifacts/README.md")
        self.assertIn("populated draft", workflow_docs)
        self.assertIn("human explicitly makes that decision", workflow_docs)
        self.assertIn("release-support.md", workflow_docs)
        self.assertIn("populated draft", artifacts_docs)
        self.assertIn("human must explicitly publish", artifacts_docs)


if __name__ == "__main__":
    unittest.main()
