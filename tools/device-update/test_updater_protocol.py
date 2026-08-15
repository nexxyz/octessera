#!/usr/bin/env python3
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
MODULE = HERE / "updater_cli.py"
PACKAGER = HERE / "package_device_bundle.py"
REPOSITORY = HERE.parents[1]
PROFILE = "raspberry-pi-zero-2w"


class UpdaterProtocolFixture(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="octessera-updater-test-"))
        self.root = self.work / "root"
        self.root.mkdir()
        (self.root / "releases").mkdir()
        (self.root / "etc/octessera").mkdir(parents=True)
        (self.root / "etc/octessera/board-profile.env").write_text(
            f"OCTESSERA_BOARD_PROFILE_ID={PROFILE}\n", encoding="utf-8"
        )
        self.proc = self.work / "proc"
        (self.proc / "4242").mkdir(parents=True)
        (self.proc / "4343").mkdir()
        self.fixtures = self.work / "fixtures"
        self.fixtures.mkdir()
        (self.fixtures / "LICENSE").write_bytes(b"Octessera license fixture\n")
        (self.fixtures / "NOTICE").write_bytes(b"Octessera notice fixture\n")
        self.bin = self.work / "bin"
        self.bin.mkdir()
        curl_impl = self.bin / "curl.py"
        systemctl_impl = self.bin / "systemctl.py"
        self.curl = self.bin / ("curl.cmd" if os.name == "nt" else "curl.py")
        self.systemctl = self.bin / (
            "systemctl.cmd" if os.name == "nt" else "systemctl.py"
        )
        self.write_executable(
            curl_impl,
            """#!/usr/bin/env python3
import os, shutil, sys
args = sys.argv[1:]
url = next(value for value in args if value.startswith('http'))
out = args[args.index('--output') + 1]
if os.environ.get('CURL_SENTINEL'):
    open(os.environ['CURL_SENTINEL'], 'a', encoding='utf-8').close()
name = 'release.json' if 'api.github.com' in url else url.rsplit('/', 1)[-1]
shutil.copyfile(os.path.join(os.environ['FIXTURES'], name), out)
""",
        )
        self.write_executable(
            systemctl_impl,
            """#!/usr/bin/env python3
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
""",
        )
        if os.name == "nt":
            self.curl.write_text(
                f'@echo off\n"{sys.executable}" "%~dp0curl.py" %*\n', encoding="utf-8"
            )
            self.systemctl.write_text(
                f'@echo off\n"{sys.executable}" "%~dp0systemctl.py" %*\n',
                encoding="utf-8",
            )
        self.env = os.environ.copy()
        self.env.update(
            {
                "PATH": str(self.bin) + os.pathsep + self.env.get("PATH", ""),
                "FIXTURES": str(self.fixtures),
                "OCTESSERA_UPDATE_ROOT": str(self.root),
                "ROOT": str(self.root),
                "OCTESSERA_UPDATE_BIN_LINK": str(self.work / "octessera-pi"),
                "OCTESSERA_UPDATE_LOCK": str(self.work / "lock"),
                "OCTESSERA_UPDATE_SERVICE": str(self.work / "octessera.service"),
                "OCTESSERA_UPDATE_SYSTEMCTL": str(self.systemctl),
                "OCTESSERA_UPDATE_CURL": str(self.curl),
                "OCTESSERA_CANDIDATE_HEALTH_PATH": str(
                    self.work / "candidate-ready.json"
                ),
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
            }
        )
        (self.work / "octessera.service").write_text(
            "[Service]\nExecStart=" + str(self.work / "octessera-pi") + "\n",
            encoding="utf-8",
        )
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
        runtime = self.fixtures / f"runtime-{release_version}"
        runtime.mkdir()
        binary = b"#!/bin/sh\nexit 0\n"
        (runtime / "octessera-pi").write_bytes(binary)
        (runtime / "octessera-pi").chmod(0o755)
        binary_digest = hashlib.sha256(binary).hexdigest()
        metadata = {
            "artifact_kind": "production-runtime",
            "binary_sha256": binary_digest,
            "name": "octessera-pi",
            "profile": PROFILE,
            "runtime_ready": True,
            "version": release_version,
        }
        (runtime / "octessera-runtime.json").write_text(
            json.dumps(metadata, sort_keys=True, indent=2) + "\n", encoding="utf-8"
        )
        (runtime / "SHA256SUMS").write_bytes(
            f"{binary_digest}  octessera-pi\n".encode("ascii")
        )
        command = [
            sys.executable,
            str(PACKAGER),
            "--runtime-bundle",
            str(runtime),
            "--output-dir",
            str(self.fixtures),
            "--repository-root",
            str(self.fixtures),
            "--board-profile",
            PROFILE,
            "--tag",
            "v" + release_version,
            "--version",
            release_version,
        ]
        result = subprocess.run(command, cwd=REPOSITORY, text=True, capture_output=True)
        if result.returncode != 0:
            raise AssertionError(result.stderr)
        assets = [
            {
                "name": name,
                "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{name}",
            }
            for name in (archive_name, sums_name)
        ]
        (self.fixtures / "release.json").write_text(
            json.dumps({"tag_name": "v" + release_version, "assets": assets}),
            encoding="utf-8",
        )

    def make_unsafe_release(self, release_version):
        archive_name = f"octessera-{release_version}-{PROFILE}-device-aarch64.zip"
        sums_name = f"SHA256SUMS-{PROFILE}-device.txt"
        archive = self.fixtures / archive_name
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("../escape", b"bad")
            output.writestr(
                "octessera-device-release.json",
                json.dumps(self.manifest(release_version)),
            )
            output.writestr("octessera-pi", b"bad")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.fixtures / sums_name).write_text(
            f"{digest}  {archive_name}\n", encoding="utf-8"
        )
        (self.fixtures / "release.json").write_text(
            json.dumps(
                {
                    "tag_name": "v" + release_version,
                    "assets": [
                        {
                            "name": archive_name,
                            "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{archive_name}",
                        },
                        {
                            "name": sums_name,
                            "browser_download_url": f"https://github.com/nexxyz/octessera/releases/download/v{release_version}/{sums_name}",
                        },
                    ],
                }
            ),
            encoding="utf-8",
        )

    def make_bad_manifest_release(self, release_version):
        self.make_release(release_version)
        archive_name = f"octessera-{release_version}-{PROFILE}-device-aarch64.zip"
        archive = self.fixtures / archive_name
        temporary = self.fixtures / "bad.zip"
        with (
            zipfile.ZipFile(archive) as source,
            zipfile.ZipFile(temporary, "w") as output,
        ):
            for info in source.infolist():
                value = source.read(info)
                if info.filename == "octessera-device-release.json":
                    bad = self.manifest(release_version)
                    bad["board_profile"] = "orange-pi-zero-2w"
                    value = json.dumps(bad).encode()
                output.writestr(info, value)
        temporary.replace(archive)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.fixtures / f"SHA256SUMS-{PROFILE}-device.txt").write_text(
            f"{digest}  {archive_name}\n", encoding="utf-8"
        )

    def install_state(self, current, previous):
        for release in (current, previous):
            directory = self.root / "releases" / release
            directory.mkdir(exist_ok=True)
            binary = directory / "octessera-pi"
            binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            binary.chmod(0o755)
            (directory / "update-manifest.json").write_text(
                json.dumps(self.manifest(release)), encoding="utf-8"
            )
        (self.root / "current").symlink_to(
            self.root / "releases" / current, target_is_directory=True
        )
        (self.work / "octessera-pi").symlink_to(self.root / "current" / "octessera-pi")
        (self.root / "update-state.json").write_text(
            json.dumps(
                {
                    "schema_version": 2,
                    "phase": "committed",
                    "current": current,
                    "previous": previous,
                }
            ),
            encoding="utf-8",
        )

    def invoke(self, *args, check=True, env=None):
        actual = self.env.copy()
        if env:
            actual.update(env)
        return subprocess.run(
            [sys.executable, str(MODULE), *args],
            env=actual,
            text=True,
            capture_output=True,
            check=check,
        )

    def guard(self, mode="normal"):
        candidate = Path(
            json.loads((self.root / "update-transaction.json").read_text())[
                "candidate"
            ]["path"]
        )
        (self.work / "service-restarted").unlink(missing_ok=True)
        (self.proc / "4242" / "exe").unlink(missing_ok=True)
        (self.proc / "4343" / "exe").unlink(missing_ok=True)
        (self.proc / "4242" / "exe").symlink_to(candidate / "octessera-pi")
        (self.proc / "4343" / "exe").symlink_to(candidate / "octessera-pi")
        if mode == "pid":
            (self.proc / "4343" / "exe").unlink()
            (self.proc / "4343" / "exe").symlink_to(
                self.root / "releases" / "1.0.0" / "octessera-pi"
            )
        return self.invoke("guard", env={"SYSTEMCTL_MODE": mode}, check=False)


class UpdaterProtocolTests(UpdaterProtocolFixture):
    def test_normal_commit(self):
        self.invoke("apply", "v1.0.1")
        transaction = json.loads(
            (self.root / "update-transaction.json").read_text(encoding="utf-8")
        )
        self.assertEqual(
            (
                transaction["schema_version"],
                transaction["candidate_source"],
                transaction["candidate_health_protocol"],
                transaction["activation_attempted"],
            ),
            (2, "downloaded", 1, False),
        )
        result = self.guard()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "current").resolve().name, "1.0.1")
        state = json.loads((self.root / "update-state.json").read_text())
        self.assertEqual(
            (state["current"], state["previous"], state["schema_version"]),
            ("1.0.1", "1.0.0", 2),
        )

    def test_device_archive_root_inventory_and_extraction(self):
        archive = (
            self.fixtures / "octessera-1.0.1-raspberry-pi-zero-2w-device-aarch64.zip"
        )
        with zipfile.ZipFile(archive) as source:
            self.assertEqual(
                set(source.namelist()),
                {"octessera-pi", "octessera-device-release.json", "LICENSE", "NOTICE"},
            )
            self.assertEqual(source.read("LICENSE"), b"Octessera license fixture\n")
            self.assertEqual(source.read("NOTICE"), b"Octessera notice fixture\n")
        self.invoke("apply", "v1.0.1")
        extracted = self.root / "releases" / "1.0.1"
        self.assertEqual(
            {path.name for path in extracted.iterdir()},
            {
                "octessera-pi",
                "update-manifest.json",
                "update-asset.json",
                "LICENSE",
                "NOTICE",
            },
        )
        self.assertEqual(
            (extracted / "LICENSE").read_bytes(), b"Octessera license fixture\n"
        )
        self.assertEqual(
            (extracted / "NOTICE").read_bytes(), b"Octessera notice fixture\n"
        )

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


if __name__ == "__main__":
    unittest.main()
