from __future__ import annotations

import json
import os
import stat
import tempfile
import unittest
from pathlib import Path

try:
    from .respin import RespinError, boot_hashes, install_runtime, sha256_file, update_manifest
except ImportError:
    from respin import RespinError, boot_hashes, install_runtime, sha256_file, update_manifest

METADATA = (
    "OCTESSERA_IMAGE_KIND=armbian\n"
    "OCTESSERA_RUNTIME_VERSION=0.8.1\n"
    "OCTESSERA_RUNTIME_BINARY_SHA256=old\n"
    "OCTESSERA_RUNTIME_MANIFEST_SHA256=old\n"
    "OCTESSERA_RUNTIME_METADATA_SHA256=old\n"
    "OCTESSERA_PI_DEFAULT_SHA256=keep\n"
)


def parent_root(base: Path) -> Path:
    root = base / "root"
    prior = root / "opt/octessera/releases/0.8.1"
    prior.mkdir(parents=True)
    (prior / "octessera-pi").write_bytes(b"old")
    (root / "opt/octessera/current").symlink_to("/opt/octessera/releases/0.8.1")
    (root / "usr/local/bin").mkdir(parents=True)
    (root / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
    state = {"schema_version": 2, "phase": "committed", "current": "0.8.1", "previous": None, "asset": None, "release": update_manifest("0.8.1"), "updated_at": "2026-09-01T00:00:00Z"}
    (root / "opt/octessera/update-state.json").write_text(json.dumps(state), encoding="utf-8")
    (root / "etc/octessera").mkdir(parents=True)
    (root / "etc/octessera/build-metadata.env").write_text(METADATA, encoding="utf-8")
    (root / "boot").mkdir()
    (root / "boot/Image").write_bytes(b"kernel")
    return root


def bundle(base: Path) -> Path:
    path = base / "bundle"
    path.mkdir()
    for name in ("octessera-pi", "octessera-runtime.json", "SHA256SUMS"):
        (path / name).write_bytes(name.encode())
    return path


@unittest.skipUnless(hasattr(os, "geteuid") and os.geteuid() == 0, "requires root for ownership")
class InstallRuntimeTests(unittest.TestCase):
    def test_replaces_release_links_state_and_runtime_metadata_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = parent_root(base)
            boot_before = boot_hashes(root)
            self.assertEqual(install_runtime(root, bundle(base), "0.8.7"), "0.8.1")
            release = root / "opt/octessera/releases/0.8.7"
            self.assertFalse((root / "opt/octessera/releases/0.8.1").exists())
            self.assertEqual(sorted(path.name for path in release.iterdir()), ["SHA256SUMS", "octessera-pi", "octessera-runtime.json", "update-manifest.json"])
            self.assertEqual(stat.S_IMODE(release.stat().st_mode), 0o555)
            self.assertEqual(stat.S_IMODE((release / "octessera-pi").stat().st_mode), 0o555)
            self.assertEqual(json.loads((release / "update-manifest.json").read_text()), update_manifest("0.8.7"))
            self.assertEqual(os.readlink(root / "opt/octessera/current"), "/opt/octessera/releases/0.8.7")
            self.assertEqual(os.readlink(root / "usr/local/bin/octessera-pi"), "/opt/octessera/current/octessera-pi")
            state = json.loads((root / "opt/octessera/update-state.json").read_text())
            self.assertEqual((state["current"], state["previous"], state["updated_at"]), ("0.8.7", None, "2026-09-01T00:00:00Z"))
            metadata = (root / "etc/octessera/build-metadata.env").read_text()
            self.assertIn("OCTESSERA_RUNTIME_VERSION=0.8.7\n", metadata)
            self.assertIn(f"OCTESSERA_RUNTIME_BINARY_SHA256={sha256_file(release / 'octessera-pi')}\n", metadata)
            self.assertIn("OCTESSERA_PI_DEFAULT_SHA256=keep\n", metadata)
            self.assertEqual(boot_hashes(root), boot_before)

    def test_rejects_uncommitted_parent_state(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = parent_root(base)
            state_path = root / "opt/octessera/update-state.json"
            state = json.loads(state_path.read_text())
            state["phase"] = "pending"
            state_path.write_text(json.dumps(state))
            with self.assertRaisesRegex(RespinError, "committed"):
                install_runtime(root, bundle(base), "0.8.7")


if __name__ == "__main__":
    unittest.main()
