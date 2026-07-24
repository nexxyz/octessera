#!/usr/bin/env python3
import unittest
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[2]


class UpdaterLayoutTests(unittest.TestCase):
    def test_installed_units_use_managed_runtime_and_boot_recovery(self):
        for service in (
            REPOSITORY / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service",
            REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera.service",
        ):
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


if __name__ == "__main__":
    unittest.main()
