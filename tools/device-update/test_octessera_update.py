#!/usr/bin/env python3
import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
UPDATER = HERE / "octessera-update"
sys.path.insert(0, str(HERE))
from package_device_bundle import device_asset_names, package_bundle  # noqa: E402

RASPBERRY = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
FAKE_SYSTEMCTL = """#!/bin/sh
echo "$*" >> "$FAKE_LOG"
case "$1" in
  restart) if grep -q healthy "$OCTESSERA_UPDATE_ROOT/current/octessera-pi"; then echo ready > "$OCTESSERA_CANDIDATE_HEALTH_PATH"; fi ;;
  is-active) [ -f "$OCTESSERA_CANDIDATE_HEALTH_PATH" ] || exit 3 ;;
esac
exit 0
"""


class UpdaterTests(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="octessera-update-test-"))
        self.root = self.work / "opt"
        self.downloads = self.work / "downloads"
        self.log = self.work / "systemctl.log"
        self.install_release("1.0.0", b"healthy 1.0.0")
        os.symlink(self.root / "releases/1.0.0", self.root / "current")
        (self.work / "repo").mkdir()
        for name in ("LICENSE", "NOTICE"):
            (self.work / "repo" / name).write_text(f"{name} fixture\n", encoding="utf-8")
        (self.work / "board-profile.env").write_text(f"OCTESSERA_BOARD_PROFILE_ID={RASPBERRY}\n", encoding="utf-8")
        systemctl = self.work / "systemctl"
        systemctl.write_text(FAKE_SYSTEMCTL, encoding="utf-8")
        systemctl.chmod(0o755)
        self.releases = []
        self.env = {
            **os.environ,
            "OCTESSERA_UPDATE_ROOT": str(self.root),
            "OCTESSERA_UPDATE_API": (self.work / "releases.json").as_uri(),
            "OCTESSERA_UPDATE_DOWNLOADS": self.downloads.as_uri(),
            "OCTESSERA_UPDATE_PROFILE_FILE": str(self.work / "board-profile.env"),
            "OCTESSERA_UPDATE_SYSTEMCTL": str(systemctl),
            "OCTESSERA_UPDATE_DEV_DROP_IN": str(self.work / "90-octessera-dev.conf"),
            "OCTESSERA_CANDIDATE_HEALTH_PATH": str(self.work / "candidate-ready.json"),
            "OCTESSERA_UPDATE_READY_SECONDS": "2",
            "OCTESSERA_UPDATE_SETTLE_SECONDS": "0.1",
            "FAKE_LOG": str(self.log),
        }

    def tearDown(self):
        shutil.rmtree(self.work, ignore_errors=True)

    def install_release(self, version, binary):
        release = self.root / "releases" / version
        release.mkdir(parents=True)
        (release / "octessera-pi").write_bytes(binary)
        (release / "octessera-pi").chmod(0o755)

    def publish(self, version, profile=RASPBERRY, binary=None, prerelease=False, draft=False):
        runtime = self.work / f"runtime-{profile}-{version}"
        runtime.mkdir()
        binary = binary or f"healthy {version}".encode("ascii")
        (runtime / "octessera-pi").write_bytes(binary)
        digest = hashlib.sha256(binary).hexdigest()
        (runtime / "octessera-runtime.json").write_text(json.dumps({"profile": profile, "version": version, "binary_sha256": digest}), encoding="utf-8")
        (runtime / "SHA256SUMS").write_text(f"{digest}  octessera-pi\n", encoding="ascii")
        package_bundle(runtime, self.downloads / f"v{version}", self.work / "repo", profile, f"v{version}", version)
        names = device_asset_names(profile, version)
        self.releases.append({"tag_name": f"v{version}", "draft": draft, "prerelease": prerelease, "assets": [{"name": name} for name in names]})
        (self.work / "releases.json").write_text(json.dumps(self.releases), encoding="utf-8")
        return self.downloads / f"v{version}" / names[0], self.downloads / f"v{version}" / names[1]

    def invoke(self, *args, stdin=None):
        return subprocess.run([sys.executable, str(UPDATER), *args], env=self.env, input=stdin, capture_output=True, timeout=60)

    def calls(self):
        return self.log.read_text(encoding="utf-8").splitlines() if self.log.exists() else []

    def link(self, name):
        return Path(os.readlink(self.root / name)).name

    def test_check_picks_the_newest_release_for_this_board(self):
        self.publish("1.0.5")
        self.publish("1.1.0")
        self.publish("1.2.0", prerelease=True)
        self.publish("1.3.0", profile=ORANGE)
        self.publish("2.0.0", draft=True)
        result = self.invoke("check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.decode().strip(), "v1.1.0 available (installed v1.0.0)")
        (self.work / "board-profile.env").write_text(f"OCTESSERA_BOARD_PROFILE_ID={ORANGE}\n", encoding="utf-8")
        self.assertEqual(self.invoke("check").stdout.decode().strip(), "v1.3.0 available (installed v1.0.0)")

    def test_apply_stages_the_verified_release_and_starts_the_switch(self):
        self.publish("1.1.0")
        result = self.invoke("apply")
        self.assertEqual(result.returncode, 0, result.stderr)
        release = self.root / "releases/1.1.0"
        self.assertEqual(sorted(path.name for path in release.iterdir()), ["LICENSE", "NOTICE", "SHA256SUMS", "octessera-pi", "octessera-runtime.json"])
        self.assertTrue(os.access(release / "octessera-pi", os.X_OK))
        self.assertEqual(self.calls(), ["start --no-block octessera-update-switch@1.1.0.service"])
        self.assertEqual(self.link("current"), "1.0.0")
        self.assertEqual(self.invoke("apply").returncode, 0)

    def test_checksum_mismatch_aborts_without_changes(self):
        _, sums = self.publish("1.1.0")
        sums.write_text(f"{'0' * 64}  {device_asset_names(RASPBERRY, '1.1.0')[0]}\n", encoding="ascii")
        result = self.invoke("apply")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"Checksum does not match", result.stderr)
        self.assertFalse((self.root / "releases/1.1.0").exists())
        self.assertEqual(self.calls(), [])
        self.assertEqual(self.link("current"), "1.0.0")

    def test_unexpected_archive_entry_is_rejected(self):
        archive, sums = self.publish("1.1.0")
        with zipfile.ZipFile(archive, "a") as target:
            target.writestr("../evil.sh", b"#!/bin/sh\n")
        sums.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", encoding="ascii")
        result = self.invoke("apply")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"unexpected contents", result.stderr)
        self.assertEqual(sorted(path.name for path in (self.root / "releases").iterdir()), ["1.0.0"])
        self.assertFalse((self.work / "evil.sh").exists())

    def test_switch_commits_a_healthy_release_and_prunes_old_ones(self):
        self.install_release("0.9.0", b"healthy 0.9.0")
        self.install_release("1.1.0", b"healthy 1.1.0")
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.link("current"), self.link("previous")), ("1.1.0", "1.0.0"))
        self.assertEqual(sorted(path.name for path in (self.root / "releases").iterdir()), ["1.0.0", "1.1.0"])
        self.assertIn("restart octessera.service", self.calls())

    def test_switch_goes_back_when_the_release_does_not_start(self):
        self.install_release("1.1.0", b"broken 1.1.0")
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"did not start; went back to v1.0.0", result.stderr)
        self.assertEqual(self.link("current"), "1.0.0")
        self.assertFalse((self.root / "previous").is_symlink())
        self.assertEqual(self.calls().count("restart octessera.service"), 2)

    def test_rollback_needs_a_previous_release_and_uses_the_switch(self):
        result = self.invoke("rollback")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"No previous release", result.stderr)
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        result = self.invoke("rollback")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls()[-1], "start --no-block octessera-update-switch@1.0.0.service")
        self.assertEqual(self.invoke("switch", "1.0.0").returncode, 0)
        self.assertEqual((self.link("current"), self.link("previous")), ("1.0.0", "1.1.0"))

    def test_concurrent_update_is_refused(self):
        self.publish("1.1.0")
        with (self.root / ".update.lock").open("a") as handle:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
            result = self.invoke("apply")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"Another update is running", result.stderr)

    def test_development_drop_in_blocks_apply_and_rollback(self):
        self.publish("1.1.0")
        (self.work / "90-octessera-dev.conf").write_text("[Service]\n", encoding="utf-8")
        for action in ("apply", "rollback"):
            result = self.invoke(action)
            self.assertEqual(result.returncode, 1)
            self.assertIn(b"development build is running", result.stderr)
        self.assertEqual(self.calls(), [])

    def test_serve_answers_one_socket_request(self):
        self.publish("1.1.0")
        self.assertEqual(self.invoke("serve", stdin=b"check\n").stdout, b"ok\nv1.1.0 available (installed v1.0.0)")
        self.assertEqual(self.invoke("serve", stdin=b"switch 1.1.0\n").stdout, b"error\nInvalid update request")

    def test_packager_rejects_an_incomplete_runtime_bundle(self):
        runtime = self.work / "incomplete"
        runtime.mkdir()
        (runtime / "octessera-pi").write_bytes(b"runtime")
        with self.assertRaises(ValueError):
            package_bundle(runtime, self.work / "out", self.work / "repo", RASPBERRY, "v1.1.0", "1.1.0")


if __name__ == "__main__":
    unittest.main()
