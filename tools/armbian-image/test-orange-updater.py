#!/usr/bin/env python3
import json
import os
import shutil
import tempfile
from pathlib import Path

from verify_runtime_account import require_production_updater


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = json.loads((ROOT / "resources/image-construction/boot-layers/orange-pi-zero-2w.json").read_text(encoding="utf-8"))
VERSION = "1.2.3"
MANIFEST = {
    "schema_version": 2,
    "updater_protocol": 2,
    "candidate_health_protocol": 1,
    "updater_supported": True,
    "distribution": "runtime-updater",
    "tag": f"v{VERSION}",
    "version": VERSION,
    "board_profile": "orange-pi-zero-2w",
    "arch": "aarch64-unknown-linux-gnu",
    "binary": "octessera-pi",
    "platforms": ["orange-pi-zero-2w", "linux-aarch64-device"],
}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def stage_fixture(root: Path) -> None:
    release = root / f"opt/octessera/releases/{VERSION}"
    release.mkdir(parents=True)
    (release / "update-manifest.json").write_text(json.dumps(MANIFEST) + "\n", encoding="utf-8")
    state = {
        "schema_version": 2,
        "phase": "committed",
        "current": VERSION,
        "previous": None,
        "updated_at": "1970-01-01T00:00:00Z",
        "release": MANIFEST,
        "asset": None,
    }
    (root / "opt/octessera").mkdir(parents=True, exist_ok=True)
    (root / "opt/octessera/update-state.json").write_text(json.dumps(state) + "\n", encoding="utf-8")
    (root / "etc/systemd/system/multi-user.target.wants").mkdir(parents=True, exist_ok=True)
    (root / "etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service").symlink_to("../octessera-update-recovery.service")
    (root / "etc/systemd/system/sockets.target.wants").mkdir(parents=True, exist_ok=True)
    (root / "etc/systemd/system/sockets.target.wants/octessera-update.socket").symlink_to("../octessera-update.socket")
    for source_relative, installed_relative in (
        ("tools/device-update/updater_protocol.py", "usr/local/lib/octessera/updater_protocol.py"),
        ("tools/device-update/updater_contract.py", "usr/local/lib/octessera/updater_contract.py"),
        ("tools/device-update/updater_state.py", "usr/local/lib/octessera/updater_state.py"),
        ("tools/device-update/updater_assets.py", "usr/local/lib/octessera/updater_assets.py"),
        ("tools/device-update/updater_guard.py", "usr/local/lib/octessera/updater_guard.py"),
        ("tools/device-update/updater_cli.py", "usr/local/lib/octessera/updater_cli.py"),
        ("tools/device-update/updater_profiles.py", "usr/local/lib/octessera/updater_profiles.py"),
        ("tools/device-update/octessera-update-broker", "usr/local/sbin/octessera-update-broker"),
        ("userpatches/overlay/usr/local/sbin/octessera-update", "usr/local/sbin/octessera-update"),
        ("userpatches/overlay/usr/local/sbin/octessera-update-guard", "usr/local/sbin/octessera-update-guard"),
        ("userpatches/overlay/usr/local/sbin/octessera-update-recovery", "usr/local/sbin/octessera-update-recovery"),
        ("userpatches/overlay/etc/systemd/system/octessera-update-guard.service", "etc/systemd/system/octessera-update-guard.service"),
        ("userpatches/overlay/etc/systemd/system/octessera-update-recovery.service", "etc/systemd/system/octessera-update-recovery.service"),
        ("userpatches/overlay/etc/systemd/system/octessera-update.socket", "etc/systemd/system/octessera-update.socket"),
        ("userpatches/overlay/etc/systemd/system/octessera-update@.service", "etc/systemd/system/octessera-update@.service"),
        ("userpatches/overlay/etc/sudoers.d/octessera-update", "etc/sudoers.d/octessera-update"),
    ):
        destination = root / installed_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / source_relative, destination)
        os.chmod(destination, 0o755 if destination_relative_mode(installed_relative) else 0o644)
        os.chown(destination, 0, 0)  # type: ignore[attr-defined]
    os.chmod(root / "etc/sudoers.d/octessera-update", 0o440)
    os.chmod(release / "update-manifest.json", 0o444)
    os.chown(release / "update-manifest.json", 0, 0)  # type: ignore[attr-defined]
    os.chmod(root / "opt/octessera/update-state.json", 0o644)
    os.chown(root / "opt/octessera/update-state.json", 0, 0)  # type: ignore[attr-defined]


def destination_relative_mode(path: str) -> bool:
    return path.startswith("usr/local/sbin/")


def main() -> None:
    if os.name == "nt" or not hasattr(os, "geteuid") or os.geteuid() != 0:
        print("Orange updater image tests skipped outside root POSIX execution")
        return
    with tempfile.TemporaryDirectory(prefix="octessera-orange-updater-") as temporary:
        root = Path(temporary)
        stage_fixture(root)
        require_production_updater(root, CONTRACT, ROOT, VERSION, require)
        (root / "opt/octessera/update-state.json").unlink()
        try:
            require_production_updater(root, CONTRACT, ROOT, VERSION, require)
        except (AssertionError, FileNotFoundError):
            pass
        else:
            raise AssertionError("Orange updater verifier accepted a missing committed state file")
    print("Orange updater staging, manifest, state, ownership, and source identity tests passed")


if __name__ == "__main__":
    main()
