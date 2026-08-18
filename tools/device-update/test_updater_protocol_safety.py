#!/usr/bin/env python3
import importlib
import json
import os
import sys
from pathlib import Path
from unittest.mock import patch


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
_updater_protocol = importlib.import_module("test_updater_protocol")
PROFILE = _updater_protocol.PROFILE
UpdaterProtocolFixture = _updater_protocol.UpdaterProtocolFixture


class UpdaterProtocolSafetyTests(UpdaterProtocolFixture):
    def test_readiness_timeout_restores_fallback(self):
        self.invoke("apply", "v1.0.1")
        result = self.guard("timeout")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "update-transaction.json").exists())

    def test_restart_failure_stops_and_verifies_fallback(self):
        self.invoke("apply", "v1.0.1")
        result = self.guard("restartfail")
        self.assertNotEqual(result.returncode, 0)
        log = (self.work / "systemctl.log").read_text(encoding="utf-8")
        self.assertIn("stop octessera.service", log)
        self.assertIn("start octessera.service", log)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")

    def test_partial_restart_failure_stops_candidate_after_state_change(self):
        self.invoke("apply", "v1.0.1")
        result = self.guard("partialrestart")
        self.assertNotEqual(result.returncode, 0)
        log = (self.work / "systemctl.log").read_text(encoding="utf-8")
        self.assertIn("restart octessera.service", log)
        self.assertIn("show octessera.service", log)
        self.assertIn("stop octessera.service", log)
        self.assertIn("start octessera.service", log)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")

    def test_failed_candidate_stop_and_fallback_start_are_verified(self):
        self.invoke("apply", "v1.0.1")
        result = self.guard("timeout")
        self.assertNotEqual(result.returncode, 0)
        log = (self.work / "systemctl.log").read_text(encoding="utf-8")
        self.assertIn("stop octessera.service", log)
        self.assertIn("start octessera.service", log)

    def test_pid_mismatch_and_restart_failure_restore(self):
        for mode in ("pid", "restartfail"):
            with self.subTest(mode=mode):
                self.invoke("apply", "v1.0.1")
                result = self.guard(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual((self.root / "current").resolve().name, "1.0.0")
                self.assertFalse((self.root / "update-transaction.json").exists())

    def test_nrestarts_change_is_detected_as_a_failed_activation(self):
        self.invoke("apply", "v1.0.1")
        result = self.guard("nrestarts")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")

    def test_crash_boundary_recovery_never_rolls_forward(self):
        self.invoke("apply", "v1.0.1")
        result = self.invoke(
            "recover", "--boot", env={"SYSTEMCTL_MODE": "bootinactive"}
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "update-transaction.json").exists())
        log = (self.work / "systemctl.log").read_text(encoding="utf-8")
        self.assertNotIn("stop octessera.service", log)
        self.assertNotIn("start octessera.service", log)

    def test_successful_malformed_transaction_restoration_returns_success(self):
        (self.root / "update-transaction.json").write_text(
            json.dumps({"schema_version": 2, "phase": "validating"}), encoding="utf-8"
        )
        result = self.invoke("recover", check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "update-transaction.json").exists())

    def test_guard_requires_active_recovery(self):
        self.invoke("apply", "v1.0.1")
        result = self.guard("recoveryinactive")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((self.root / "update-transaction.json").exists())

    def test_unsafe_payload_is_rejected_without_switch(self):
        self.make_unsafe_release("1.0.2")
        result = self.invoke("apply", "v1.0.2", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "releases" / "1.0.2").exists())

    def test_bad_manifest_and_checksum_are_rejected_without_switch(self):
        self.make_release("1.0.2")
        (self.fixtures / f"SHA256SUMS-{PROFILE}-device.txt").write_text(
            f"{'0' * 64}  octessera-1.0.2-{PROFILE}-device-aarch64.zip\n",
            encoding="utf-8",
        )
        result = self.invoke("apply", "v1.0.2", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "releases" / "1.0.2").exists())
        self.make_bad_manifest_release("1.0.3")
        result = self.invoke("apply", "v1.0.3", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "releases" / "1.0.3").exists())

    def test_legacy_state_is_migrated_backwards_only(self):
        candidate = self.root / "releases" / "1.0.1"
        (self.root / "current").unlink()
        (self.root / "current").symlink_to(candidate, target_is_directory=True)
        (self.root / "update-state.json").write_text(
            json.dumps({"current": "1.0.0", "previous": "0.9.0", "next": "1.0.1"}),
            encoding="utf-8",
        )
        (self.root / "update-state.json.next").write_text("{}", encoding="utf-8")
        result = self.invoke("recover")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertEqual(
            json.loads((self.root / "update-state.json").read_text())["schema_version"],
            2,
        )

    def test_legacy_installed_release_is_bootstrapped_before_online_apply(self):
        manifest_path = self.root / "releases/1.0.0/update-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest.pop("board_profile")
        manifest["schema_version"] = 1
        manifest.pop("updater_protocol")
        manifest.pop("candidate_health_protocol")
        manifest["platforms"] = ["linux-aarch64-device"]
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        sentinel = self.work / "curl-called"
        result = self.invoke("bootstrap", check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        migrated = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(
            (
                migrated["schema_version"],
                migrated["updater_protocol"],
                migrated["candidate_health_protocol"],
                migrated["board_profile"],
            ),
            (2, 2, 1, PROFILE),
        )
        self.assertIn(PROFILE, migrated["platforms"])
        result = self.invoke(
            "apply", "v1.0.1", check=False, env={"CURL_SENTINEL": str(sentinel)}
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(sentinel.exists())

    def test_check_does_not_repair_or_write_live_state(self):
        before = {
            str(path.relative_to(self.root)): path.read_bytes()
            if path.is_file()
            else os.readlink(path)
            if path.is_symlink()
            else None
            for path in self.root.rglob("*")
        }
        result = self.invoke("check", "v1.0.1")
        self.assertEqual(result.returncode, 0, result.stderr)
        after = {
            str(path.relative_to(self.root)): path.read_bytes()
            if path.is_file()
            else os.readlink(path)
            if path.is_symlink()
            else None
            for path in self.root.rglob("*")
        }
        self.assertEqual(before, after)

    def test_orange_profile_uses_explicit_updater_asset_contract(self):
        from updater_profiles import updater_asset_names
        from updater_protocol import Updater

        (self.root / "etc/octessera/board-profile.env").write_text(
            "OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\n", encoding="utf-8"
        )
        with patch.dict(
            os.environ,
            {
                "OCTESSERA_UPDATE_BOARD_PROFILE": "orange-pi-zero-2w",
                "OCTESSERA_UPDATE_ROOT": str(self.root),
                "OCTESSERA_UPDATE_TEST_MODE": "1",
            },
        ):
            updater = Updater()
            self.assertEqual(
                updater.asset_names("1.0.1"),
                updater_asset_names("orange-pi-zero-2w", "1.0.1"),
            )
            self.assertEqual(
                updater.asset_names("1.0.1")[0],
                "octessera-1.0.1-orange-pi-zero-2w-runtime-updater-aarch64.zip",
            )

    def test_absent_profile_fails_closed(self):
        (self.root / "etc/octessera/board-profile.env").unlink()
        result = self.invoke(
            "check", "v1.0.1", check=False, env={"OCTESSERA_UPDATE_BOARD_PROFILE": ""}
        )
        self.assertNotEqual(result.returncode, 0)

    def test_public_lock_timeout_is_bounded(self):
        if os.name == "nt":
            self.skipTest("fcntl locking is Unix-only")
        import fcntl

        with open(self.work / "lock", "w", encoding="utf-8") as handle:
            os.chmod(self.work / "lock", 0o600)
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
            result = self.invoke(
                "apply",
                "v1.0.1",
                check=False,
                env={"OCTESSERA_UPDATE_LOCK_TIMEOUT": "0.05"},
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("lock", result.stderr.lower())

    def test_lock_rejects_unsafe_existing_file(self):
        lock = self.work / "lock"
        lock.write_text("", encoding="utf-8")
        if os.name != "nt":
            lock.chmod(0o644)
        result = self.invoke("check", "v1.0.1", check=False)
        if os.name == "nt":
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)

    def test_nondefault_service_is_rejected(self):
        result = self.invoke(
            "check",
            "v1.0.1",
            check=False,
            env={
                "OCTESSERA_UPDATE_TEST_MODE": "",
                "CURL_SENTINEL": str(self.work / "curl-called"),
            },
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.work / "curl-called").exists())

    def test_apply_rejects_direct_execstart_before_network(self):
        sentinel = self.work / "curl-called"
        (self.work / "octessera.service").write_text(
            "[Service]\nExecStart=/home/pi/dev/octessera-pi\n", encoding="utf-8"
        )
        result = self.invoke(
            "apply", "v1.0.1", check=False, env={"CURL_SENTINEL": str(sentinel)}
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(sentinel.exists())

    def test_scheduling_failure_restores_fallback_and_is_nonzero(self):
        result = self.invoke(
            "apply", "v1.0.1", check=False, env={"SYSTEMCTL_MODE": "schedulefail"}
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "update-transaction.json").exists())
