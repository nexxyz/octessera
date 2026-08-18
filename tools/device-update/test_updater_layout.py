#!/usr/bin/env python3
import unittest
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[2]


class UpdaterLayoutTests(unittest.TestCase):
    def test_installed_units_use_managed_runtime_and_boot_recovery(self):
        service = (
            REPOSITORY
            / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service"
        )
        self.assertIn(
            "ExecStart=/usr/local/bin/octessera-pi", service.read_text(encoding="utf-8")
        )
        for sudoers in (
            REPOSITORY
            / "tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-update",
            REPOSITORY / "userpatches/overlay/etc/sudoers.d/octessera-update",
        ):
            text = sudoers.read_text(encoding="utf-8")
            self.assertNotIn("octessera-update-guard", text)
            self.assertNotIn("octessera-update-recovery", text)
        for recovery_unit in (
            REPOSITORY
            / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-update-recovery.service",
            REPOSITORY
            / "userpatches/overlay/etc/systemd/system/octessera-update-recovery.service",
        ):
            text = recovery_unit.read_text(encoding="utf-8")
            self.assertNotIn("ConditionPathExists=", text)
            self.assertIn("RemainAfterExit=yes", text)

    def test_orange_production_service_waits_for_recovery(self):
        service = (
            REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera.service"
        )
        text = service.read_text(encoding="utf-8")
        self.assertIn("User=octessera-runtime", text)
        self.assertIn("Group=octessera-runtime", text)
        self.assertIn("ExecStart=/usr/local/bin/octessera-pi", text)
        self.assertIn("LimitRTPRIO=70", text)
        self.assertIn("Requires=octessera-update-recovery.service", text)
        self.assertNotIn("octessera-update-guard.service", text)
        customize = (REPOSITORY / "userpatches/customize-image.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "install_overlay_file etc/systemd/system/octessera.service", customize
        )
        self.assertIn("systemctl enable octessera.service", customize)

    def test_orange_runtime_uses_the_narrow_update_socket(self):
        socket = (
            REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-update.socket"
        ).read_text(encoding="utf-8")
        service = (
            REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-update@.service"
        ).read_text(encoding="utf-8")
        for line in (
            "ListenStream=/run/octessera-update/update.sock",
            "SocketMode=0660",
            "SocketUser=root",
            "SocketGroup=octessera-runtime",
            "DirectoryMode=0755",
            "Accept=yes",
        ):
            self.assertIn(line, socket)
        for line in (
            "User=root",
            "Group=root",
            "StandardInput=socket",
            "StandardOutput=socket",
            "ExecStart=/usr/local/sbin/octessera-update-broker",
        ):
            self.assertIn(line, service)
        self.assertNotIn("sudo", service)
        self.assertNotIn("octessera-runtime", service)
        customize = (REPOSITORY / "userpatches/customize-image.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("install_overlay_file usr/local/sbin/octessera-update-broker", customize)
        self.assertIn("install_overlay_file etc/systemd/system/octessera-update.socket", customize)
        self.assertIn("systemctl enable octessera-update.socket", customize)
        sudoers = (
            REPOSITORY / "userpatches/overlay/etc/sudoers.d/octessera-update"
        ).read_text(encoding="utf-8")
        self.assertNotIn("octessera-runtime", sudoers)
        self.assertNotIn("ALL=(ALL)", sudoers)

    def test_raspberry_release_stages_canonical_updater_modules(self):
        workflow = (
            REPOSITORY / ".github/workflows/release-board-artifacts.yml"
        ).read_text(encoding="utf-8")
        for name in (
            "updater_protocol.py",
            "updater_state.py",
            "updater_assets.py",
            "updater_guard.py",
            "updater_cli.py",
            "updater_profiles.py",
        ):
            self.assertIn(name, workflow)
        self.assertIn(
            '"tools/device-update/$updater_file" "$stage_root/usr/local/lib/octessera/$updater_file"',
            workflow,
        )
        stage = (
            REPOSITORY / "tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
        ).read_text(encoding="utf-8")
        self.assertIn("updater_profiles.py", stage)
        self.assertNotIn("zip_basename=", workflow)
        self.assertEqual(
            workflow.count("tools/device-update/package_device_bundle.py"), 2
        )
        self.assertIn("--board-profile raspberry-pi-zero-2w", workflow)
        self.assertIn("--board-profile orange-pi-zero-2w", workflow)
        self.assertNotIn("zip -9", workflow)
        self.assertNotIn("legal/notices.zip", workflow)
        self.assertIn(
            'asset="release-assets/octessera-${{ inputs.version }}-raspberry-pi-zero-2w.img.zip"',
            workflow,
        )


if __name__ == "__main__":
    unittest.main()
