#!/usr/bin/env python3
import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
MODULE = HERE / "updater_cli.py"
PROFILE = "raspberry-pi-zero-2w"


class UpdaterProtocolTests(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="octessera-updater-test-"))
        self.root = self.work / "root"
        self.root.mkdir()
        (self.root / "releases").mkdir()
        (self.root / "etc/octessera").mkdir(parents=True)
        (self.root / "etc/octessera/board-profile.env").write_text(f"OCTESSERA_BOARD_PROFILE_ID={PROFILE}\n", encoding="utf-8")
        self.proc = self.work / "proc"
        (self.proc / "4242").mkdir(parents=True)
        (self.proc / "4343").mkdir()
        self.fixtures = self.work / "fixtures"
        self.fixtures.mkdir()
        self.bin = self.work / "bin"
        self.bin.mkdir()
        curl_impl = self.bin / "curl.py"
        systemctl_impl = self.bin / "systemctl.py"
        self.curl = self.bin / ("curl.cmd" if os.name == "nt" else "curl.py")
        self.systemctl = self.bin / ("systemctl.cmd" if os.name == "nt" else "systemctl.py")
        self.write_executable(curl_impl, """#!/usr/bin/env python3
import os, shutil, sys
args = sys.argv[1:]
url = next(value for value in args if value.startswith('http'))
out = args[args.index('--output') + 1]
if os.environ.get('CURL_SENTINEL'):
    open(os.environ['CURL_SENTINEL'], 'a', encoding='utf-8').close()
name = 'release.json' if 'api.github.com' in url else url.rsplit('/', 1)[-1]
shutil.copyfile(os.path.join(os.environ['FIXTURES'], name), out)
""")
        self.write_executable(systemctl_impl, """#!/usr/bin/env python3
import json, os, sys, time
args = sys.argv[1:]
mode = os.environ.get('SYSTEMCTL_MODE', 'normal')
with open(os.environ['SYSTEMCTL_LOG'], 'a', encoding='utf-8') as log:
    log.write(' '.join(args) + '\\n')
if args and args[0] == 'start' and mode == 'schedulefail':
    raise SystemExit(1)
if args and args[0] == 'stop' and args[1] == 'octessera.service':
    open(os.environ['SERVICE_STATE'], 'w', encoding='utf-8').close()
if args and args[0] == 'start' and args[1] == 'octessera.service':
    try:
        os.unlink(os.environ['SERVICE_STATE'])
    except FileNotFoundError:
        pass
    for proc_exe in (os.path.join(os.environ['PROC'], '4242', 'exe'), os.path.join(os.environ['PROC'], '4343', 'exe')):
        try:
            os.unlink(proc_exe)
        except FileNotFoundError:
            pass
        os.symlink(os.path.realpath(os.path.join(os.environ['ROOT'], 'current', 'octessera-pi')), proc_exe)
if args and args[0] == 'restart':
    if mode in ('normal', 'pid', 'partialrestart', 'nrestarts', 'legacy'):
        open(os.environ['RESTARTED'], 'w', encoding='utf-8').close()
    if mode in ('normal', 'pid', 'partialrestart'):
        tx = json.load(open(os.environ['TX'], encoding='utf-8'))
        marker = {
            'schema_version': 1,
            'pid': 4343,
            'systemd_invocation_id': 'inv-2',
            'package_version': tx['candidate']['version'],
            'board_profile': os.environ['OCTESSERA_UPDATE_BOARD_PROFILE'],
            'ready_at_unix_ms': int(time.time() * 1000),
        }
        with open(os.environ['HEALTH'], 'w', encoding='utf-8') as handle:
            json.dump(marker, handle)
    if mode in ('restartfail', 'partialrestart'):
        raise SystemExit(1)
    if mode == 'nrestarts':
        tx = json.load(open(os.environ['TX'], encoding='utf-8'))
        marker = {
            'schema_version': 1,
            'pid': 4343,
            'systemd_invocation_id': 'inv-2',
            'package_version': tx['candidate']['version'],
            'board_profile': os.environ['OCTESSERA_UPDATE_BOARD_PROFILE'],
            'ready_at_unix_ms': int(time.time() * 1000),
        }
        with open(os.environ['HEALTH'], 'w', encoding='utf-8') as handle:
            json.dump(marker, handle)
if args and args[0] == 'show':
    unit = args[1]
    if unit == 'octessera-update-guard.service':
        print('ActiveState=active')
        print('SubState=running')
    elif unit == 'octessera-update-recovery.service':
        print('ActiveState=' + ('inactive' if mode == 'recoveryinactive' else 'active'))
        print('SubState=' + ('dead' if mode == 'recoveryinactive' else 'exited'))
    else:
        stopped = mode == 'bootinactive' or os.path.exists(os.environ['SERVICE_STATE'])
        restarted = os.path.exists(os.environ['RESTARTED'])
        pid = '0' if stopped else ('4343' if restarted else '4242')
        invocation = '' if stopped else ('inv-2' if restarted else 'inv-1')
        print('MainPID=' + pid)
        print('InvocationID=' + invocation)
        print('NRestarts=' + ('1' if mode == 'nrestarts' and os.path.exists(os.environ['RESTARTED']) else '0'))
        print('ActiveState=' + ('inactive' if stopped else 'active'))
        print('SubState=' + ('dead' if stopped else 'running'))
""")
        if os.name == "nt":
            self.curl.write_text(f'@echo off\n"{sys.executable}" "%~dp0curl.py" %*\n', encoding="utf-8")
            self.systemctl.write_text(f'@echo off\n"{sys.executable}" "%~dp0systemctl.py" %*\n', encoding="utf-8")
        self.env = os.environ.copy()
        self.env.update({
            "PATH": str(self.bin) + os.pathsep + self.env.get("PATH", ""),
            "FIXTURES": str(self.fixtures),
            "OCTESSERA_UPDATE_ROOT": str(self.root),
            "ROOT": str(self.root),
            "OCTESSERA_UPDATE_BIN_LINK": str(self.work / "octessera-pi"),
            "OCTESSERA_UPDATE_LOCK": str(self.work / "lock"),
            "OCTESSERA_UPDATE_SERVICE": str(self.work / "octessera.service"),
            "OCTESSERA_UPDATE_SYSTEMCTL": str(self.systemctl),
            "OCTESSERA_UPDATE_CURL": str(self.curl),
            "OCTESSERA_CANDIDATE_HEALTH_PATH": str(self.work / "candidate-ready.json"),
            "OCTESSERA_UPDATE_BOARD_PROFILE": PROFILE,
            "OCTESSERA_UPDATE_TEST_MODE": "1",
            "OCTESSERA_UPDATE_MODULE": str(MODULE),
            "TX": str(self.root / "update-transaction.json"),
            "HEALTH": str(self.work / "candidate-ready.json"),
            "SERVICE_STATE": str(self.work / "service-stopped"),
            "RESTARTED": str(self.work / "service-restarted"),
            "SYSTEMCTL_LOG": str(self.work / "systemctl.log"),
            "OCTESSERA_UPDATE_READINESS_TIMEOUT": "0.4",
            "OCTESSERA_UPDATE_STABILITY_WINDOW": "0.05",
            "OCTESSERA_UPDATE_POLL_SECONDS": "0.01",
            "OCTESSERA_UPDATE_PROC_ROOT": str(self.proc),
            "PROC": str(self.proc),
        })
        (self.work / "octessera.service").write_text("[Service]\nExecStart=" + str(self.work / "octessera-pi") + "\n", encoding="utf-8")
        (self.work / "octessera.service").chmod(0o644)
        self.make_release("0.9.0")
        self.make_release("1.0.0")
        self.make_release("1.0.1")
        self.install_state("1.0.0", "0.9.0")

    def tearDown(self):
        for path in self.work.rglob("*"):
            if path.is_file() and not path.is_symlink():
                path.chmod(0o666)
            elif path.is_dir() and not path.is_symlink():
                path.chmod(0o777)
        for path in sorted(self.work.rglob("*"), reverse=True):
            if path.is_symlink() or path.is_file():
                path.unlink(missing_ok=True)
            elif path.is_dir():
                path.rmdir()
        self.work.rmdir()

    @staticmethod
    def write_executable(path, content):
        path.write_text(content, encoding="utf-8")
        path.chmod(0o755)

    def manifest(self, release_version):
        return {
            "schema_version": 2,
            "updater_protocol": 2,
            "candidate_health_protocol": 1,
            "tag": "v" + release_version,
            "version": release_version,
            "board_profile": PROFILE,
            "arch": "aarch64-unknown-linux-gnu",
            "binary": "octessera-pi",
            "platforms": [PROFILE, "linux-aarch64-device"],
        }

    def make_release(self, release_version):
        archive_name = f"octessera-{release_version}-{PROFILE}-device-aarch64.zip"
        sums_name = f"SHA256SUMS-{PROFILE}-device.txt"
        binary = b"#!/bin/sh\nexit 0\n"
        manifest = json.dumps(self.manifest(release_version)).encode()
        archive = self.fixtures / archive_name
        with zipfile.ZipFile(archive, "w") as output:
            binary_info = zipfile.ZipInfo("octessera-pi")
            binary_info.external_attr = (stat.S_IFREG | 0o755) << 16
            output.writestr(binary_info, binary)
            output.writestr("octessera-device-release.json", manifest)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.fixtures / sums_name).write_text(f"{digest}  {archive_name}\n", encoding="utf-8")
        (self.fixtures / "release.json").write_text(json.dumps({
            "tag_name": "v" + release_version,
            "assets": [
                {"name": archive_name, "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{archive_name}"},
                {"name": sums_name, "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{sums_name}"},
            ],
        }), encoding="utf-8")

    def make_unsafe_release(self, release_version):
        archive_name = f"octessera-{release_version}-{PROFILE}-device-aarch64.zip"
        sums_name = f"SHA256SUMS-{PROFILE}-device.txt"
        archive = self.fixtures / archive_name
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("../escape", b"bad")
            output.writestr("octessera-device-release.json", json.dumps(self.manifest(release_version)))
            output.writestr("octessera-pi", b"bad")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.fixtures / sums_name).write_text(f"{digest}  {archive_name}\n", encoding="utf-8")
        (self.fixtures / "release.json").write_text(json.dumps({
            "tag_name": "v" + release_version,
            "assets": [
                {"name": archive_name, "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{archive_name}"},
                {"name": sums_name, "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{sums_name}"},
            ],
        }), encoding="utf-8")

    def make_bad_manifest_release(self, release_version):
        self.make_release(release_version)
        archive_name = f"octessera-{release_version}-{PROFILE}-device-aarch64.zip"
        archive = self.fixtures / archive_name
        temporary = self.fixtures / "bad.zip"
        with zipfile.ZipFile(archive) as source, zipfile.ZipFile(temporary, "w") as output:
            for info in source.infolist():
                value = source.read(info)
                if info.filename == "octessera-device-release.json":
                    bad = self.manifest(release_version)
                    bad["board_profile"] = "orange-pi-zero-2w"
                    value = json.dumps(bad).encode()
                output.writestr(info, value)
        temporary.replace(archive)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.fixtures / f"SHA256SUMS-{PROFILE}-device.txt").write_text(f"{digest}  {archive_name}\n", encoding="utf-8")

    def install_state(self, current, previous):
        for release in (current, previous):
            directory = self.root / "releases" / release
            directory.mkdir(exist_ok=True)
            binary = directory / "octessera-pi"
            binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            binary.chmod(0o755)
            (directory / "update-manifest.json").write_text(json.dumps(self.manifest(release)), encoding="utf-8")
        (self.root / "current").symlink_to(self.root / "releases" / current, target_is_directory=True)
        (self.work / "octessera-pi").symlink_to(self.root / "current" / "octessera-pi")
        (self.root / "update-state.json").write_text(json.dumps({
            "schema_version": 2, "phase": "committed", "current": current, "previous": previous,
        }), encoding="utf-8")

    def invoke(self, *args, check=True, env=None):
        actual = self.env.copy()
        if env:
            actual.update(env)
        return subprocess.run([sys.executable, str(MODULE), *args], env=actual, text=True, capture_output=True, check=check)

    def guard(self, mode="normal"):
        candidate = Path(json.loads((self.root / "update-transaction.json").read_text())["candidate"]["path"])
        (self.work / "service-restarted").unlink(missing_ok=True)
        (self.proc / "4242" / "exe").unlink(missing_ok=True)
        (self.proc / "4343" / "exe").unlink(missing_ok=True)
        (self.proc / "4242" / "exe").symlink_to(candidate / "octessera-pi")
        (self.proc / "4343" / "exe").symlink_to(candidate / "octessera-pi")
        if mode == "pid":
            (self.proc / "4343" / "exe").unlink()
            (self.proc / "4343" / "exe").symlink_to(self.root / "releases" / "1.0.0" / "octessera-pi")
        return self.invoke("guard", env={"SYSTEMCTL_MODE": mode}, check=False)

    def test_normal_commit(self):
        self.invoke("apply", "v1.0.1")
        transaction = json.loads((self.root / "update-transaction.json").read_text(encoding="utf-8"))
        self.assertEqual((transaction["schema_version"], transaction["candidate_source"], transaction["candidate_health_protocol"], transaction["activation_attempted"]), (2, "downloaded", 1, False))
        result = self.guard()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.1")
        state = json.loads((self.root / "update-state.json").read_text())
        self.assertEqual((state["current"], state["previous"], state["schema_version"]), ("1.0.1", "1.0.0", 2))

    def test_rollback_uses_the_same_guarded_transition(self):
        self.invoke("apply", "v1.0.1")
        self.assertEqual(self.guard().returncode, 0)
        self.invoke("rollback")
        result = self.guard()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        state = json.loads((self.root / "update-state.json").read_text())
        self.assertEqual(state["previous"], "1.0.1")

    def test_legacy_manual_rollback_does_not_require_health_marker(self):
        self.invoke("apply", "v1.0.1")
        self.assertEqual(self.guard().returncode, 0)
        self.invoke("rollback")
        result = self.guard("legacy")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")

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
        result = self.invoke("recover", "--boot", env={"SYSTEMCTL_MODE": "bootinactive"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "update-transaction.json").exists())
        log = (self.work / "systemctl.log").read_text(encoding="utf-8")
        self.assertNotIn("stop octessera.service", log)
        self.assertNotIn("start octessera.service", log)

    def test_successful_malformed_transaction_restoration_returns_success(self):
        (self.root / "update-transaction.json").write_text(json.dumps({"schema_version": 2, "phase": "validating"}), encoding="utf-8")
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
        (self.fixtures / f"SHA256SUMS-{PROFILE}-device.txt").write_text(f"{'0' * 64}  octessera-1.0.2-{PROFILE}-device-aarch64.zip\n", encoding="utf-8")
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
        (self.root / "update-state.json").write_text(json.dumps({"current": "1.0.0", "previous": "0.9.0", "next": "1.0.1"}), encoding="utf-8")
        (self.root / "update-state.json.next").write_text("{}", encoding="utf-8")
        result = self.invoke("recover")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertEqual(json.loads((self.root / "update-state.json").read_text())["schema_version"], 2)

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
        self.assertEqual((migrated["schema_version"], migrated["updater_protocol"], migrated["candidate_health_protocol"], migrated["board_profile"]), (2, 2, 1, PROFILE))
        self.assertIn(PROFILE, migrated["platforms"])
        result = self.invoke("apply", "v1.0.1", check=False, env={"CURL_SENTINEL": str(sentinel)})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(sentinel.exists())

    def test_check_does_not_repair_or_write_live_state(self):
        before = {str(path.relative_to(self.root)): path.read_bytes() if path.is_file() else os.readlink(path) if path.is_symlink() else None for path in self.root.rglob("*")}
        result = self.invoke("check", "v1.0.1")
        self.assertEqual(result.returncode, 0, result.stderr)
        after = {str(path.relative_to(self.root)): path.read_bytes() if path.is_file() else os.readlink(path) if path.is_symlink() else None for path in self.root.rglob("*")}
        self.assertEqual(before, after)

    def test_orange_check_and_apply_reject_before_network(self):
        sentinel = self.work / "curl-called"
        (self.root / "etc/octessera/board-profile.env").write_text("OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\n", encoding="utf-8")
        result = self.invoke("check", "v1.0.1", check=False, env={"OCTESSERA_UPDATE_BOARD_PROFILE": "orange-pi-zero-2w", "CURL_SENTINEL": str(sentinel)})
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(sentinel.exists())
        result = self.invoke("apply", "v1.0.1", check=False, env={"OCTESSERA_UPDATE_BOARD_PROFILE": "orange-pi-zero-2w", "CURL_SENTINEL": str(sentinel)})
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(sentinel.exists())

    def test_absent_profile_fails_closed(self):
        (self.root / "etc/octessera/board-profile.env").unlink()
        result = self.invoke("check", "v1.0.1", check=False, env={"OCTESSERA_UPDATE_BOARD_PROFILE": ""})
        self.assertNotEqual(result.returncode, 0)

    def test_public_lock_timeout_is_bounded(self):
        if os.name == "nt":
            self.skipTest("fcntl locking is Unix-only")
        import fcntl
        with open(self.work / "lock", "w", encoding="utf-8") as handle:
            os.chmod(self.work / "lock", 0o600)
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
            result = self.invoke("apply", "v1.0.1", check=False, env={"OCTESSERA_UPDATE_LOCK_TIMEOUT": "0.05"})
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
        result = self.invoke("check", "v1.0.1", check=False, env={"OCTESSERA_UPDATE_TEST_MODE": "", "CURL_SENTINEL": str(self.work / "curl-called")})
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.work / "curl-called").exists())

    def test_apply_rejects_direct_execstart_before_network(self):
        sentinel = self.work / "curl-called"
        (self.work / "octessera.service").write_text("[Service]\nExecStart=/home/pi/dev/octessera-pi\n", encoding="utf-8")
        result = self.invoke("apply", "v1.0.1", check=False, env={"CURL_SENTINEL": str(sentinel)})
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(sentinel.exists())

    def test_scheduling_failure_restores_fallback_and_is_nonzero(self):
        result = self.invoke("apply", "v1.0.1", check=False, env={"SYSTEMCTL_MODE": "schedulefail"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.root / "current").resolve().name, "1.0.0")
        self.assertFalse((self.root / "update-transaction.json").exists())

if __name__ == "__main__":
    unittest.main()
