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
from package_device_bundle import IMAGE_TREES, STAGED_FILES, device_asset_names, package_bundle  # noqa: E402

REPOSITORY = HERE.parents[1]
RASPBERRY = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
STAGED_SYSTEM_FILES = {
    "usr/local/sbin/octessera-update": ("staged 1.1.0", 0o755),
    "usr/local/sbin/octessera-sd-card": ("staged 1.1.0", 0o755),
    "usr/local/lib/octessera/octessera-sd-card-lib.sh": ("staged 1.1.0", 0o644),
}
FAKE_SYSTEMCTL = """#!/bin/sh
echo "$*" >> "$FAKE_LOG"
case "$1" in
  stop) if grep -q stuck "$OCTESSERA_UPDATE_ROOT/current/octessera-pi"; then exec sleep 6; fi
    sleep "${FAKE_STOP_SLEEP:-0}" ;;
  restart) if grep -q hangs "$OCTESSERA_UPDATE_ROOT/current/octessera-pi"; then sleep 5; fi
    if grep -q scribbles "$OCTESSERA_UPDATE_ROOT/current/octessera-pi"; then echo scribbled > "$FAKE_STORE/system.json"; fi
    if grep -q healthy "$OCTESSERA_UPDATE_ROOT/current/octessera-pi"; then echo ready > "$OCTESSERA_CANDIDATE_HEALTH_PATH"; fi ;;
  is-active) [ -f "$OCTESSERA_CANDIDATE_HEALTH_PATH" ] || exit 3 ;;
  show) echo "OCTESSERA_EXPECTED_BOARD_PROFILE=fixture OCTESSERA_PI_STORE_DIR=$FAKE_STORE" ;;
esac
exit 0
"""


class UpdaterTests(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="octessera-update-test-"))
        self.root = self.work / "opt"
        self.downloads = self.work / "downloads"
        self.log = self.work / "systemctl.log"
        self.store = self.work / "presets"
        self.store.mkdir()
        self.write_store("1.0.0 settings")
        self.install_release("1.0.0", b"healthy 1.0.0")
        os.symlink(self.root / "releases/1.0.0", self.root / "current")
        (self.work / "repo").mkdir()
        for name in ("LICENSE", "NOTICE"):
            (self.work / "repo" / name).write_text(f"{name} fixture\n", encoding="utf-8")
        for tree in IMAGE_TREES.values():
            self.write_image_file(tree, "etc/systemd/system/octessera.service", "unit 1.1.0")
            self.write_image_file(tree, "etc/sudoers.d/octessera-update", "image only")
        self.write_staged(RASPBERRY)
        self.system = self.work / "system"
        (self.system / "etc/systemd/system").mkdir(parents=True)
        (self.system / "etc/systemd/system/octessera.service").write_text("unit 1.0.0", encoding="utf-8")
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
            "OCTESSERA_UPDATE_SYSTEM_ROOT": str(self.system),
            "OCTESSERA_UPDATE_DEV_DROP_IN": str(self.work / "90-octessera-dev.conf"),
            "OCTESSERA_CANDIDATE_HEALTH_PATH": str(self.work / "candidate-ready.json"),
            "OCTESSERA_UPDATE_READY_SECONDS": "2",
            "OCTESSERA_UPDATE_SETTLE_SECONDS": "0.1",
            "OCTESSERA_UPDATE_SYSTEMCTL_SECONDS": "2",
            "OCTESSERA_UPDATE_STOP_SECONDS": "4",
            "FAKE_LOG": str(self.log),
            "FAKE_STORE": str(self.store),
        }

    def tearDown(self):
        shutil.rmtree(self.work, ignore_errors=True)

    def install_release(self, version, binary):
        release = self.root / "releases" / version
        release.mkdir(parents=True)
        (release / "octessera-pi").write_bytes(binary)
        (release / "octessera-pi").chmod(0o755)

    def write_image_file(self, tree, relative, text):
        path = self.work / "repo" / tree / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def write_staged(self, profile):
        for source in STAGED_FILES[profile].values():
            self.write_image_file("", source, "staged 1.1.0")

    def system_files(self):
        return {path.relative_to(self.system).as_posix(): (path.read_text(encoding="utf-8"), path.stat().st_mode & 0o777) for path in self.system.rglob("*") if path.is_file()}

    def write_store(self, text):
        for name in ("system.json", "default.patch.json"):
            (self.store / name).write_text(f"{name} {text}", encoding="utf-8")

    def stored(self, directory=None):
        directory = directory or self.store
        return {name: (directory / name).read_text(encoding="utf-8") for name in ("system.json", "default.patch.json")}

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
        self.write_staged(ORANGE)
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
        self.assertEqual(sorted(path.name for path in release.iterdir()), ["LICENSE", "NOTICE", "SHA256SUMS", "octessera-pi", "octessera-runtime.json", "root"])
        self.assertIn("etc/systemd/system/octessera.service", [path.relative_to(release / "root").as_posix() for path in (release / "root").rglob("*")])
        self.assertFalse((release / "root/etc/sudoers.d").exists())
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

    def test_unsafe_root_entry_is_rejected(self):
        archive, sums = self.publish("1.1.0")
        with zipfile.ZipFile(archive, "a") as target:
            target.writestr("root/etc/../../evil.sh", b"#!/bin/sh\n")
        sums.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", encoding="ascii")
        result = self.invoke("apply")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"unexpected contents", result.stderr)
        self.assertEqual(sorted(path.name for path in (self.root / "releases").iterdir()), ["1.0.0"])

    def test_update_installs_the_system_files_and_keeps_the_replaced_ones(self):
        self.write_image_file(IMAGE_TREES[RASPBERRY], "usr/local/sbin/octessera-show-hold-apply", "script 1.1.0")
        self.publish("1.1.0")
        self.assertEqual(self.invoke("apply").returncode, 0)
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.system_files(), {
            **STAGED_SYSTEM_FILES,
            "etc/systemd/system/octessera.service": ("unit 1.1.0", 0o644),
            "usr/local/sbin/octessera-show-hold-apply": ("script 1.1.0", 0o755),
        })
        old = self.root / "releases/1.0.0"
        self.assertEqual((old / "root-replaced/etc/systemd/system/octessera.service").read_text(encoding="utf-8"), "unit 1.0.0")
        self.assertEqual((old / "root-added").read_text(encoding="utf-8").split(), ["usr/local/lib/octessera/octessera-sd-card-lib.sh", "usr/local/sbin/octessera-sd-card", "usr/local/sbin/octessera-show-hold-apply", "usr/local/sbin/octessera-update"])
        calls = self.calls()
        self.assertLess(calls.index("daemon-reload"), calls.index("restart --no-block octessera.service"))

    def test_failed_update_puts_the_old_system_files_back(self):
        self.write_image_file(IMAGE_TREES[RASPBERRY], "usr/local/sbin/octessera-show-hold-apply", "script 1.1.0")
        self.publish("1.1.0", binary=b"broken 1.1.0")
        self.assertEqual(self.invoke("apply").returncode, 0)
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.system_files(), {"etc/systemd/system/octessera.service": ("unit 1.0.0", 0o644)})
        self.assertFalse((self.root / "releases/1.0.0/root-added").exists())
        self.assertEqual(self.calls().count("daemon-reload"), 2)

    def test_rollback_puts_the_old_system_files_back_and_forward_installs_them_again(self):
        self.write_image_file(IMAGE_TREES[RASPBERRY], "usr/local/sbin/octessera-show-hold-apply", "script 1.1.0")
        self.publish("1.1.0")
        self.assertEqual(self.invoke("apply").returncode, 0)
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        installed = self.system_files()
        self.assertEqual(self.invoke("rollback").returncode, 0)
        self.assertEqual(self.invoke("switch", "1.0.0").returncode, 0)
        self.assertEqual(self.system_files(), {"etc/systemd/system/octessera.service": ("unit 1.0.0", 0o644)})
        self.assertFalse((self.root / "releases/1.0.0/root-replaced").exists())
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.assertEqual(self.system_files(), installed)

    def test_failed_rollback_reinstalls_the_newer_system_files(self):
        self.publish("1.1.0")
        self.assertEqual(self.invoke("apply").returncode, 0)
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        (self.root / "releases/1.0.0/octessera-pi").write_bytes(b"broken 1.0.0")
        result = self.invoke("switch", "1.0.0")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.system_files(), {**STAGED_SYSTEM_FILES, "etc/systemd/system/octessera.service": ("unit 1.1.0", 0o644)})
        self.assertEqual((self.root / "releases/1.0.0/root-replaced/etc/systemd/system/octessera.service").read_text(encoding="utf-8"), "unit 1.0.0")

    def test_switch_commits_a_healthy_release_and_prunes_old_ones(self):
        self.install_release("0.9.0", b"healthy 0.9.0")
        self.install_release("1.1.0", b"healthy 1.1.0")
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.link("current"), self.link("previous")), ("1.1.0", "1.0.0"))
        self.assertEqual(sorted(path.name for path in (self.root / "releases").iterdir()), ["1.0.0", "1.1.0"])
        self.assertIn("restart --no-block octessera.service", self.calls())

    def test_switch_goes_back_when_the_release_does_not_start(self):
        self.install_release("1.1.0", b"broken 1.1.0")
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"did not start; went back to v1.0.0", result.stderr)
        self.assertEqual(self.link("current"), "1.0.0")
        self.assertFalse((self.root / "previous").is_symlink())
        self.assertEqual(self.calls().count("restart --no-block octessera.service"), 2)

    def test_switch_goes_back_when_the_restart_times_out(self):
        self.install_release("1.1.0", b"hangs 1.1.0")
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"did not start; went back to v1.0.0", result.stderr)
        self.assertEqual(self.link("current"), "1.0.0")
        self.assertFalse((self.root / "previous").is_symlink())
        self.assertEqual(self.calls().count("restart --no-block octessera.service"), 2)

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

    def test_first_update_snapshots_the_old_store_and_keeps_the_current_one(self):
        (self.store / "system.json").chmod(0o640)
        (self.store / "presets.json").write_text("presets", encoding="utf-8")
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        snapshot = self.root / "releases/1.0.0/store-snapshot"
        self.assertEqual(self.stored(snapshot), self.stored())
        self.assertEqual(self.stored()["system.json"], "system.json 1.0.0 settings")
        self.assertEqual((snapshot / "system.json").stat().st_mode & 0o777, 0o640)
        self.assertFalse((snapshot / "presets.json").exists())

    def test_going_forward_right_after_a_rollback_brings_back_the_newer_settings(self):
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.write_store("1.1.0 settings")
        self.assertEqual(self.invoke("switch", "1.0.0").returncode, 0)
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.assertEqual(self.stored()["system.json"], "system.json 1.1.0 settings")

    def test_going_forward_after_playing_on_the_rollback_keeps_that_work(self):
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.write_store("1.1.0 settings")
        self.assertEqual(self.invoke("switch", "1.0.0").returncode, 0)
        self.write_store("weeks on 1.0.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.assertEqual(self.stored()["system.json"], "system.json weeks on 1.0.0")
        self.assertEqual(
            self.stored(self.root / "releases/1.1.0/store-snapshot")["system.json"],
            "system.json 1.1.0 settings",
        )

    def test_going_back_restores_the_older_store_snapshot(self):
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.write_store("1.1.0 settings")
        result = self.invoke("switch", "1.0.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.stored()["default.patch.json"], "default.patch.json 1.0.0 settings")
        self.assertEqual(self.stored(self.root / "releases/1.1.0/store-snapshot")["system.json"], "system.json 1.1.0 settings")

    def test_failed_going_back_puts_the_newer_store_back(self):
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.write_store("1.1.0 settings")
        (self.root / "releases/1.0.0/octessera-pi").write_bytes(b"broken 1.0.0")
        result = self.invoke("switch", "1.0.0")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"did not start; went back to v1.1.0", result.stderr)
        self.assertEqual(self.link("current"), "1.1.0")
        self.assertEqual(set(self.stored().values()), {"system.json 1.1.0 settings", "default.patch.json 1.1.0 settings"})

    def test_menu_update_rollback_and_forward_keep_each_versions_settings(self):
        self.publish("1.1.0")
        self.assertEqual(self.invoke("apply").returncode, 0)
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.write_store("1.1.0 settings")
        self.assertEqual(self.invoke("rollback").returncode, 0)
        self.assertEqual(self.invoke("switch", "1.0.0").returncode, 0)
        self.assertEqual(self.stored()["system.json"], "system.json 1.0.0 settings")
        shutil.rmtree(self.downloads)
        result = self.invoke("apply")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls()[-1], "start --no-block octessera-update-switch@1.1.0.service")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        self.assertEqual(self.stored()["system.json"], "system.json 1.1.0 settings")

    def test_the_runtime_is_stopped_before_its_settings_are_copied(self):
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.assertEqual(self.invoke("switch", "1.1.0").returncode, 0)
        calls = self.calls()
        stop = calls.index("stop octessera.service")
        self.assertLess(stop, calls.index("restart --no-block octessera.service"))

    def test_a_slow_stop_gets_longer_than_other_systemctl_calls(self):
        self.install_release("1.1.0", b"healthy 1.1.0")
        self.env["FAKE_STOP_SLEEP"] = "3"
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.link("current"), "1.1.0")

    def test_failed_update_goes_back_even_when_the_new_version_will_not_stop(self):
        self.install_release("1.1.0", b"stuck 1.1.0")
        before = self.stored()
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"did not start; went back to v1.0.0", result.stderr)
        self.assertEqual(self.link("current"), "1.0.0")
        self.assertFalse((self.root / "previous").is_symlink())
        self.assertEqual(self.stored(), before)
        self.assertEqual(self.calls()[-1], "restart --no-block octessera.service")

    def test_failed_update_undoes_what_the_new_version_wrote(self):
        self.install_release("1.1.0", b"scribbles 1.1.0")
        before = self.stored()
        result = self.invoke("switch", "1.1.0")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.link("current"), "1.0.0")
        self.assertEqual(self.stored(), before)

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

    def test_packager_carries_only_octessera_files_from_each_image_tree(self):
        runtime = self.work / "runtime"
        runtime.mkdir()
        for name in ("octessera-pi", "octessera-runtime.json", "SHA256SUMS"):
            (runtime / name).write_bytes(b"fixture")
        modes = {}
        for profile in (RASPBERRY, ORANGE):
            with zipfile.ZipFile(package_bundle(runtime, self.work / profile, REPOSITORY, profile, "v1.1.0", "1.1.0")) as archive:
                modes[profile] = {info.filename[5:]: (info.external_attr >> 16) & 0o777 for info in archive.infolist() if info.filename.startswith("root/")}
        for profile in (RASPBERRY, ORANGE):
            self.assertEqual(modes[profile]["etc/systemd/system/octessera.service"], 0o644)
            self.assertEqual(modes[profile]["usr/lib/systemd/system-shutdown/octessera-oled-off"], 0o755)
            self.assertEqual(modes[profile]["usr/local/sbin/octessera-show-hold-apply"], 0o755)
            self.assertEqual(modes[profile]["usr/local/lib/octessera/setup_http.py"], 0o644)
            for path in modes[profile]:
                self.assertIn("octessera", path)
                self.assertFalse(path.startswith(("etc/sudoers.d/", "etc/initramfs-tools/", "etc/octessera/", "usr/local/share/octessera/")), path)
        self.assertEqual(modes[RASPBERRY]["etc/systemd/system/octessera.service.d/audio-realtime.conf"], 0o644)
        self.assertEqual(modes[ORANGE]["usr/local/sbin/octessera-orange-oled-handoff.py"], 0o644)
        self.assertEqual(modes[ORANGE]["usr/local/sbin/octessera-orange-storage"], 0o755)
        self.assertNotIn("usr/local/lib/octessera/setup-image-layer.sh", modes[ORANGE])
        self.assertEqual(modes[RASPBERRY]["usr/local/sbin/octessera-sd-card"], 0o755)
        self.assertEqual(modes[ORANGE]["usr/local/sbin/octessera-update"], 0o755)
        self.assertEqual(modes[ORANGE]["usr/local/lib/octessera/device_config.py"], 0o644)

    def test_packager_rejects_an_incomplete_runtime_bundle(self):
        runtime = self.work / "incomplete"
        runtime.mkdir()
        (runtime / "octessera-pi").write_bytes(b"runtime")
        with self.assertRaises(ValueError):
            package_bundle(runtime, self.work / "out", self.work / "repo", RASPBERRY, "v1.1.0", "1.1.0")


if __name__ == "__main__":
    unittest.main()
