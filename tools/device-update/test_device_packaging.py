#!/usr/bin/env python3
import hashlib
import importlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch


HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
HELPER = HERE / "package_device_bundle.py"
sys.path.insert(0, str(HERE))
_updater_protocol = importlib.import_module("updater_protocol")
UpdateError = _updater_protocol.UpdateError
Updater = _updater_protocol.Updater


VERSION = "1.2.3"
TAG = "v1.2.3"
RASPBERRY = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
EXPECTED_TIME = (1980, 1, 1, 0, 0, 0)


class DevicePackagingTests(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="octessera-device-package-test-"))
        self.repository = self.work / "repository"
        self.repository.mkdir()
        (self.repository / "LICENSE").write_bytes(b"Octessera license fixture\n")
        (self.repository / "NOTICE").write_bytes(b"Octessera notice fixture\n")

    def tearDown(self):
        shutil.rmtree(self.work, ignore_errors=True)

    def runtime_bundle(self, profile):
        runtime = self.work / f"{profile}-runtime"
        shutil.rmtree(runtime, ignore_errors=True)
        runtime.mkdir()
        binary = b"octessera runtime bytes for " + profile.encode("ascii") + b"\n"
        (runtime / "octessera-pi").write_bytes(binary)
        (runtime / "octessera-pi").chmod(0o755)
        metadata = {
            "artifact_kind": "production-runtime",
            "binary_sha256": hashlib.sha256(binary).hexdigest(),
            "name": "octessera-pi",
            "profile": profile,
            "runtime_ready": True,
            "version": VERSION,
        }
        (runtime / "octessera-runtime.json").write_text(
            json.dumps(metadata, sort_keys=True, indent=2) + "\n", encoding="utf-8"
        )
        (runtime / "SHA256SUMS").write_bytes(
            f"{metadata['binary_sha256']}  octessera-pi\n".encode("ascii")
        )
        return runtime, binary

    def package(self, profile):
        runtime, binary = self.runtime_bundle(profile)
        output = self.work / f"{profile}-release"
        result = self.invoke_packager(profile, runtime, output)
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = output / (
            f"octessera-{VERSION}-{profile}-device-aarch64.zip"
            if profile == RASPBERRY
            else f"octessera-{VERSION}-{profile}-standalone-manual-aarch64.zip"
        )
        sums = output / f"SHA256SUMS-{profile}-device.txt"
        self.assertTrue(archive.exists())
        self.assertTrue(sums.exists())
        return runtime, binary, archive, sums

    def invoke_packager(self, profile, runtime, output, tag=TAG, version=VERSION):
        return subprocess.run(
            [
                sys.executable,
                str(HELPER),
                "--runtime-bundle",
                str(runtime),
                "--output-dir",
                str(output),
                "--repository-root",
                str(self.repository),
                "--board-profile",
                profile,
                "--tag",
                tag,
                "--version",
                version,
            ],
            cwd=REPOSITORY,
            text=True,
            capture_output=True,
        )

    def assert_zip_entry(self, info, mode):
        self.assertEqual(info.date_time, EXPECTED_TIME)
        self.assertEqual((info.external_attr >> 16) & 0o170000, stat.S_IFREG)
        self.assertEqual((info.external_attr >> 16) & 0o777, mode)
        self.assertEqual(info.flag_bits & 1, 0)

    def test_raspberry_archive_is_exact_deterministic_and_updater_validated(self):
        runtime, binary, archive, sums = self.package(RASPBERRY)
        expected_names = [
            "octessera-pi",
            "octessera-device-release.json",
            "LICENSE",
            "NOTICE",
        ]
        with zipfile.ZipFile(archive) as source:
            self.assertEqual(source.namelist(), expected_names)
            for info, mode in zip(source.infolist(), (0o755, 0o644, 0o644, 0o644)):
                self.assert_zip_entry(info, mode)
            self.assertEqual(source.read("octessera-pi"), binary)
            self.assertEqual(
                source.read("octessera-pi"), (runtime / "octessera-pi").read_bytes()
            )
            self.assertEqual(
                source.read("LICENSE"), (self.repository / "LICENSE").read_bytes()
            )
            self.assertEqual(
                source.read("NOTICE"), (self.repository / "NOTICE").read_bytes()
            )
            manifest = json.loads(source.read("octessera-device-release.json"))
        self.assertEqual(
            set(manifest),
            {
                "schema_version",
                "updater_protocol",
                "candidate_health_protocol",
                "tag",
                "version",
                "board_profile",
                "arch",
                "binary",
                "platforms",
            },
        )
        self.assertEqual(manifest["board_profile"], RASPBERRY)
        self.assertEqual(manifest["updater_protocol"], 2)
        self.assertEqual(
            sums.read_text(encoding="ascii"),
            f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n",
        )
        first_archive = archive.read_bytes()
        first_sums = sums.read_bytes()
        self.package(RASPBERRY)
        self.assertEqual(archive.read_bytes(), first_archive)
        self.assertEqual(sums.read_bytes(), first_sums)

        root = self.work / "updater-root"
        (root / "releases").mkdir(parents=True, exist_ok=True)
        (root / "etc/octessera").mkdir(parents=True, exist_ok=True)
        (root / "etc/octessera/board-profile.env").write_text(
            f"OCTESSERA_BOARD_PROFILE_ID={RASPBERRY}\n", encoding="utf-8"
        )
        environment = {
            "OCTESSERA_UPDATE_ROOT": str(root),
            "OCTESSERA_UPDATE_BOARD_PROFILE": RASPBERRY,
            "OCTESSERA_UPDATE_TEST_MODE": "1",
        }
        with patch.dict(os.environ, environment):
            updater = Updater()
            extracted = root / "releases" / VERSION
            manifest = updater.extract_zip(archive, extracted, VERSION)
            self.assertEqual(updater.validate_release(extracted), manifest)
            self.assertEqual((extracted / "octessera-pi").read_bytes(), binary)

    def test_orange_archive_is_manual_six_entry_bundle(self):
        runtime, binary, archive, sums = self.package(ORANGE)
        expected_names = [
            "octessera-pi",
            "octessera-runtime.json",
            "SHA256SUMS",
            "octessera-device-release.json",
            "LICENSE",
            "NOTICE",
        ]
        with zipfile.ZipFile(archive) as source:
            self.assertEqual(source.namelist(), expected_names)
            for info in source.infolist():
                self.assert_zip_entry(
                    info, 0o755 if info.filename == "octessera-pi" else 0o644
                )
            self.assertEqual(source.read("octessera-pi"), binary)
            self.assertEqual(
                source.read("octessera-runtime.json"),
                (runtime / "octessera-runtime.json").read_bytes(),
            )
            self.assertEqual(
                source.read("SHA256SUMS"), (runtime / "SHA256SUMS").read_bytes()
            )
            manifest = json.loads(source.read("octessera-device-release.json"))
        self.assertFalse(manifest["updater_supported"])
        self.assertEqual(manifest["distribution"], "standalone-manual")
        self.assertNotIn("updater_protocol", manifest)
        self.assertNotIn("ota", json.dumps(manifest).lower())
        self.assertNotIn("device update", json.dumps(manifest).lower())
        self.assertEqual(
            sums.read_text(encoding="ascii"),
            f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n",
        )

    def test_runtime_inputs_and_release_identity_are_validated(self):
        runtime, _ = self.runtime_bundle(RASPBERRY)
        metadata = json.loads(
            (runtime / "octessera-runtime.json").read_text(encoding="utf-8")
        )
        metadata["version"] = "9.9.9"
        (runtime / "octessera-runtime.json").write_text(
            json.dumps(metadata), encoding="utf-8"
        )
        result = self.invoke_packager(RASPBERRY, runtime, self.work / "stale-metadata")
        self.assertNotEqual(result.returncode, 0)

        runtime, _ = self.runtime_bundle(RASPBERRY)
        (runtime / "SHA256SUMS").write_bytes(
            f"{'0' * 64}  octessera-pi\n".encode("ascii")
        )
        result = self.invoke_packager(RASPBERRY, runtime, self.work / "stale-sums")
        self.assertNotEqual(result.returncode, 0)

        runtime, _ = self.runtime_bundle(RASPBERRY)
        (runtime / "unexpected-entry").write_bytes(b"unexpected")
        result = self.invoke_packager(RASPBERRY, runtime, self.work / "extra-runtime")
        self.assertNotEqual(result.returncode, 0)

        runtime, _ = self.runtime_bundle(RASPBERRY)
        result = self.invoke_packager(
            "unknown-board", runtime, self.work / "bad-profile"
        )
        self.assertNotEqual(result.returncode, 0)
        result = self.invoke_packager(
            RASPBERRY, runtime, self.work / "bad-tag", tag="v9.9.9"
        )
        self.assertNotEqual(result.returncode, 0)

    def test_symlinked_legal_inputs_are_rejected(self):
        for name in ("LICENSE", "NOTICE"):
            with self.subTest(name=name):
                runtime, _ = self.runtime_bundle(RASPBERRY)
                target = self.work / f"{name}.target"
                target.write_bytes(b"external legal input\n")
                legal_input = self.repository / name
                legal_input.unlink()
                try:
                    legal_input.symlink_to(target)
                except OSError as exc:
                    self.skipTest(f"symlink creation unavailable: {exc}")
                result = self.invoke_packager(
                    RASPBERRY, runtime, self.work / f"symlink-{name.lower()}"
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(target.read_bytes(), b"external legal input\n")
                legal_input.unlink()
                legal_input.write_bytes(
                    b"Octessera license fixture\n"
                    if name == "LICENSE"
                    else b"Octessera notice fixture\n"
                )

    def test_preexisting_archive_and_checksum_symlinks_are_replaced_safely(self):
        runtime, _, archive, sums = self.package(RASPBERRY)
        output = archive.parent
        archive_target = self.work / "archive-target"
        sums_target = self.work / "sums-target"
        archive_target.write_bytes(b"archive target")
        sums_target.write_bytes(b"sums target")
        archive.unlink()
        sums.unlink()
        archive.symlink_to(archive_target)
        sums.symlink_to(sums_target)
        result = self.invoke_packager(RASPBERRY, runtime, output)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(archive.is_symlink())
        self.assertFalse(sums.is_symlink())
        self.assertEqual(archive_target.read_bytes(), b"archive target")
        self.assertEqual(sums_target.read_bytes(), b"sums target")

    def updater_for_asset(self):
        root = self.work / "asset-updater-root"
        (root / "releases").mkdir(parents=True, exist_ok=True)
        (root / "etc/octessera").mkdir(parents=True, exist_ok=True)
        (root / "etc/octessera/board-profile.env").write_text(
            f"OCTESSERA_BOARD_PROFILE_ID={RASPBERRY}\n", encoding="utf-8"
        )
        environment = {
            "OCTESSERA_UPDATE_ROOT": str(root),
            "OCTESSERA_UPDATE_BOARD_PROFILE": RASPBERRY,
            "OCTESSERA_UPDATE_TEST_MODE": "1",
        }
        return root, environment

    def rewrite_archive(
        self, source_path, target_path, remove=None, extra=None, mutate=None
    ):
        with zipfile.ZipFile(source_path) as source:
            entries = [
                (info, source.read(info))
                for info in source.infolist()
                if info.filename != remove
            ]
        with zipfile.ZipFile(target_path, "w") as target:
            for info, value in entries:
                if mutate and info.filename == mutate[0]:
                    value = mutate[1]
                target.writestr(info, value)
            if extra:
                target.writestr(extra, b"unsafe")

    def assert_asset_rejected(self, archive, message):
        root, environment = self.updater_for_asset()
        with patch.dict(os.environ, environment):
            updater = Updater()
            destination = root / "releases" / message
            with self.assertRaises(UpdateError):
                updater.extract_zip(archive, destination, VERSION)

    def test_actual_updater_rejects_tampered_missing_extra_and_unsafe_assets(self):
        _, _, archive, sums = self.package(RASPBERRY)
        missing = self.work / "missing.zip"
        self.rewrite_archive(archive, missing, remove="octessera-device-release.json")
        self.assert_asset_rejected(missing, "missing")
        extra = self.work / "extra.zip"
        self.rewrite_archive(archive, extra, extra="octessera-runtime.json")
        self.assert_asset_rejected(extra, "extra")
        unsafe = self.work / "unsafe.zip"
        self.rewrite_archive(archive, unsafe, extra="../escape")
        self.assert_asset_rejected(unsafe, "unsafe")

        tampered = self.work / "tampered.zip"
        self.rewrite_archive(
            archive, tampered, mutate=("octessera-pi", b"tampered runtime bytes\n")
        )
        root, environment = self.updater_for_asset()
        with patch.dict(os.environ, environment):
            updater = Updater()
            archive_name = archive.name
            sums_name = sums.name
            payload = {
                "tag_name": TAG,
                "assets": [
                    {
                        "name": archive_name,
                        "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/{TAG}/{archive_name}",
                    },
                    {
                        "name": sums_name,
                        "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/{TAG}/{sums_name}",
                    },
                ],
            }
            updater.release_json = lambda tag: payload

            def copy_asset(url, output, max_bytes):
                shutil.copyfile(sums if url.endswith(sums_name) else tampered, output)

            updater.curl = copy_asset
            with self.assertRaises(UpdateError):
                updater.download_candidate(TAG)
            self.assertFalse((root / "releases" / VERSION).exists())


if __name__ == "__main__":
    unittest.main()
