#!/usr/bin/env python3
import unittest
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[2]


class UpdaterLayoutTests(unittest.TestCase):
    def test_installed_units_use_managed_runtime_and_boot_recovery(self):
        service = REPOSITORY / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service"
        self.assertIn("ExecStart=/usr/local/bin/octessera-pi", service.read_text(encoding="utf-8"))
        for sudoers in (
            REPOSITORY / "tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-update",
            REPOSITORY / "userpatches/overlay/etc/sudoers.d/octessera-update",
        ):
            text = sudoers.read_text(encoding="utf-8")
            self.assertNotIn("octessera-update-guard", text)
            self.assertNotIn("octessera-update-recovery", text)
        for recovery_unit in (
            REPOSITORY / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-update-recovery.service",
            REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-update-recovery.service",
        ):
            text = recovery_unit.read_text(encoding="utf-8")
            self.assertNotIn("ConditionPathExists=", text)
            self.assertIn("RemainAfterExit=yes", text)

    def test_production_service_is_installed_and_enabled_without_updater_claims(self):
        service = REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera.service"
        text = service.read_text(encoding="utf-8")
        self.assertIn("User=octessera-runtime", text)
        self.assertIn("Group=octessera-runtime", text)
        self.assertIn("ExecStart=/usr/local/bin/octessera-pi", text)
        self.assertIn("LimitRTPRIO=70", text)
        self.assertNotIn("octessera-update", text)
        customize = (REPOSITORY / "userpatches/customize-image.sh").read_text(encoding="utf-8")
        self.assertIn("install_overlay_file etc/systemd/system/octessera.service", customize)
        self.assertIn("systemctl enable octessera.service", customize)

    def test_raspberry_release_stages_canonical_updater_modules(self):
        workflow = (REPOSITORY / ".github/workflows/release-board-artifacts.yml").read_text(encoding="utf-8")
        for name in ("updater_protocol.py", "updater_state.py", "updater_assets.py", "updater_guard.py", "updater_cli.py"):
            self.assertIn(name, workflow)
        self.assertIn('"tools/device-update/$updater_file" "$stage_root/usr/local/lib/octessera/$updater_file"', workflow)


if __name__ == "__main__":
    unittest.main()
