#!/usr/bin/env python3
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path

try:
    import grp
    import pwd
except ImportError:
    print("Orange runtime identity tests skipped without POSIX account modules")
    raise SystemExit(0)


ROOT = Path(__file__).resolve().parents[2]
LOGO = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
HANDOFF = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
RUNTIME_USER = "octessera-runtime"
HARDWARE_GROUPS = ("audio", "i2c", "spi", "gpio")


def load_module(name, source):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(source)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(command):
    subprocess.run(command, check=True, capture_output=True, text=True)


def ensure_runtime_account():
    created_user = False
    created_groups = []
    account = None
    try:
        account = pwd.getpwnam(RUNTIME_USER)
    except KeyError:
        runtime_uid = next(uid for uid in (990, 1990, 2990) if _uid_available(uid))
        try:
            grp.getgrnam(RUNTIME_USER)
        except KeyError:
            run(["groupadd", "--system", "--gid", str(runtime_uid), RUNTIME_USER])
            created_groups.append(RUNTIME_USER)
        run(["useradd", "--system", "--uid", str(runtime_uid), "--gid", RUNTIME_USER, "--no-create-home", "--shell", "/usr/sbin/nologin", RUNTIME_USER])
        created_user = True
        account = pwd.getpwnam(RUNTIME_USER)
    original_groups = [group.gr_name for group in grp.getgrall() if RUNTIME_USER in group.gr_mem]
    for group_name in HARDWARE_GROUPS:
        try:
            grp.getgrnam(group_name)
        except KeyError:
            run(["groupadd", "--system", group_name])
            created_groups.append(group_name)
        if group_name not in original_groups:
            run(["usermod", "-a", "-G", group_name, RUNTIME_USER])
    account = pwd.getpwnam(RUNTIME_USER)

    def cleanup():
        if created_user:
            subprocess.run(["userdel", RUNTIME_USER], check=False, capture_output=True)
        elif original_groups != [group.gr_name for group in grp.getgrall() if RUNTIME_USER in group.gr_mem]:
            supplementary = ",".join(original_groups)
            subprocess.run(["usermod", "-G", supplementary, RUNTIME_USER], check=False, capture_output=True)
        for group_name in reversed(created_groups):
            subprocess.run(["groupdel", group_name], check=False, capture_output=True)

    return account, cleanup


def _uid_available(uid):
    try:
        pwd.getpwuid(uid)
    except KeyError:
        return True
    return False


CHILD = r'''
import importlib.util
import json
import os
import sys
from importlib.machinery import SourceFileLoader

spec = importlib.util.spec_from_loader("orange_logo_child", SourceFileLoader("orange_logo_child", sys.argv[1]))
assert spec is not None and spec.loader is not None
logo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logo)
logo.drop_to_runtime()
handoff = logo._handoff_module
handoff.HANDOFF_ROOT = sys.argv[2]
owner = handoff.Handoff.open(True)
owner.start()
print(json.dumps({"uid": os.getuid(), "gid": os.getgid(), "groups": os.getgroups()}))
owner.close()
'''


def child(root):
    return subprocess.run([sys.executable, "-c", CHILD, str(LOGO), str(root)], capture_output=True, text=True)


if not hasattr(os, "geteuid") or os.geteuid() != 0:
    print("Orange runtime identity tests skipped outside WSL root")
    raise SystemExit(0)

account, cleanup_account = ensure_runtime_account()
try:
    handoff = load_module("orange_handoff_identity", HANDOFF)
    real_getpwnam = handoff.pwd.getpwnam
    handoff.pwd.getpwnam = lambda name: (_ for _ in ()).throw(KeyError(name))
    try:
        try:
            handoff.runtime_identity()
        except RuntimeError:
            pass
        else:
            raise AssertionError("missing runtime account was accepted")
    finally:
        handoff.pwd.getpwnam = real_getpwnam

    with tempfile.TemporaryDirectory() as directory:
        os.chmod(directory, 0o755)
        owned = Path(directory) / "owned"
        owned.mkdir(mode=0o750)
        os.chmod(owned, 0o750)
        os.chown(owned, account.pw_uid, account.pw_gid)
        result = child(owned)
        assert result.returncode == 0, result.stderr
        identity = json.loads(result.stdout)
        assert identity["uid"] == account.pw_uid and identity["gid"] == account.pw_gid
        expected_groups = {grp.getgrnam(name).gr_gid for name in HARDWARE_GROUPS}
        assert expected_groups.issubset(set(identity["groups"]))
        assert (owned / "oled.lock").stat().st_uid == account.pw_uid
        assert (owned / "status.json").stat().st_gid == account.pw_gid
        assert not (owned / "stop.request").exists()

        wrong_owner = Path(directory) / "wrong-owner"
        wrong_owner.mkdir(mode=0o750)
        os.chmod(wrong_owner, 0o750)
        assert child(wrong_owner).returncode != 0

    wrong_identity = subprocess.run(
        [sys.executable, "-c", "import importlib.util, os, sys; from importlib.machinery import SourceFileLoader; s=importlib.util.spec_from_loader('x', SourceFileLoader('x', sys.argv[1])); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); os.setgid(65534); os.setuid(65534); m.drop_to_runtime()", str(LOGO)],
        capture_output=True,
        text=True,
    )
    assert wrong_identity.returncode != 0
finally:
    cleanup_account()

print("Orange runtime identity, ownership, supplementary-group, wrong-identity, and missing-account tests passed")
