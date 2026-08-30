from __future__ import annotations

import hashlib
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parent))

from inventory import build_inventory, inventory_digest
from runtime_mutation import MutationError, mutate_runtime
from boot_neutral import load_policy


RPI = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"


def _owner(path: Path, uid: int = 0, gid: int = 0) -> None:
    if hasattr(os, "chown") and os.name != "nt" and os.geteuid() == 0:
        os.chown(path, uid, gid, follow_symlinks=False)


def _write(path: Path, data: bytes | str, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data if isinstance(data, bytes) else data.encode())
    os.chmod(path, mode)
    _owner(path)


def _mkdir(path: Path, mode: int = 0o755) -> None:
    path.mkdir(parents=True, exist_ok=True)
    os.chmod(path, mode)
    _owner(path)


def _manifest(board: str, version: str) -> dict[str, Any]:
    return {"schema_version": 2, "updater_protocol": 2, "candidate_health_protocol": 1, "tag": f"v{version}", "version": version, "board_profile": board, "arch": "aarch64-unknown-linux-gnu", "binary": "octessera-pi", "platforms": [board, "linux-aarch64-device"]}


def _bundle(work: Path, board: str, version: str, binary: bytes = b"new-runtime") -> Path:
    bundle = work / f"bundle-{board}"
    _mkdir(bundle)
    digest = hashlib.sha256(binary).hexdigest()
    _write(bundle / "octessera-pi", binary, 0o755)
    metadata = {"artifact_kind": "production-runtime", "binary_sha256": digest, "name": "octessera-pi", "profile": board, "runtime_ready": True, "version": version}
    _write(bundle / "octessera-runtime.json", json.dumps(metadata, sort_keys=True, indent=2) + "\n", 0o644)
    _write(bundle / "SHA256SUMS", f"{digest}  octessera-pi\n", 0o644)
    return bundle


def _parent_context(board: str) -> dict[str, Any]:
    return {"schema": "octessera.image-current-parent/v1", "repository": "nexxyz/octessera", "board_profile": board, "version": "9.9.9", "constructor": {"run_id": 42, "source_sha": "a" * 40}, "artifact": {"id": 43, "name": "test-parent-assets", "size": 44, "digest": "sha256:" + "b" * 64, "expires_at": "2099-01-01T00:00:00Z", "entries": []}, "image": {"name": "octessera-9.9.9-orange-pi-zero-2w.img.xz", "size": 123, "sha256": "b" * 64}, "record": {"path": "resources/image-parents/orange-pi-zero-2w-current.json", "sha256": "c" * 64, "size": 1}}


def _fixture(work: Path, board: str, prior: str = "1.0.0") -> tuple[Path, Path]:
    root = work / f"root-{board}"
    for relative in ("opt/octessera/releases", "usr/local/bin", "etc", "usr/share/doc", "usr/share/common-licenses"):
        _mkdir(root / relative)
    _write(root / "etc/keep", b"untouched", 0o644)
    _write(root / "usr/share/doc/base-files/copyright", b"vendor copyright\n", 0o644)
    _write(root / "usr/share/common-licenses/GPL-3", b"vendor GPL\n", 0o644)
    release = root / "opt/octessera/releases" / prior
    _mkdir(release)
    old = b"old-runtime"
    _write(release / "octessera-pi", old, 0o555 if board == ORANGE else 0o755)
    if board == RPI:
        _write(release / "update-manifest.json", json.dumps(_manifest(board, prior), sort_keys=True, indent=2) + "\n", 0o644)
        state = {"schema_version": 2, "phase": "committed", "current": prior, "previous": None, "next": None, "updated_at": "1970-01-01T00:00:00Z", "release": _manifest(board, prior), "asset": None}
        _write(root / "opt/octessera/update-state.json", json.dumps(state, sort_keys=True, indent=2) + "\n", 0o644)
    else:
        digest = hashlib.sha256(old).hexdigest()
        metadata = {"artifact_kind": "production-runtime", "binary_sha256": digest, "name": "octessera-pi", "profile": board, "runtime_ready": True, "version": prior}
        runtime_metadata = json.dumps(metadata, sort_keys=True, indent=2) + "\n"
        runtime_sums = f"{digest}  octessera-pi\n"
        _write(release / "octessera-runtime.json", runtime_metadata, 0o444)
        _write(release / "SHA256SUMS", runtime_sums, 0o444)
        metadata_hash = hashlib.sha256(runtime_metadata.encode()).hexdigest()
        sums_hash = hashlib.sha256(runtime_sums.encode()).hexdigest()
        build_metadata = f"OCTESSERA_IMAGE_KIND=armbian\nOCTESSERA_IMAGE_MODE=production\nOCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\nOCTESSERA_IMAGE_BUILT_AT=2025-01-01T00:00:00Z\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=true\nOCTESSERA_IMAGE_CONTRACT_SHA256={'a' * 64}\nOCTESSERA_RUNTIME_VERSION={prior}\nOCTESSERA_RUNTIME_BINARY_SHA256={digest}\nOCTESSERA_RUNTIME_MANIFEST_SHA256={sums_hash}\nOCTESSERA_RUNTIME_METADATA_SHA256={metadata_hash}\nOCTESSERA_SPI1_CS0_DTS_SHA256={'b' * 64}\nOCTESSERA_SPI1_CS0_DTBO_SHA256={'c' * 64}\nOCTESSERA_INPUT_ROUTING_DTS_SHA256={'d' * 64}\nOCTESSERA_INPUT_ROUTING_DTBO_SHA256={'e' * 64}\nOCTESSERA_PI_DEFAULT_SHA256={'f' * 64}\nOCTESSERA_SAMPLES_MANIFEST_SHA256={'0' * 64}\n"
        _write(root / "etc/octessera/build-metadata.env", build_metadata, 0o664)
        policy = load_policy(Path(__file__).resolve().parents[2])
        _write(root / "boot/Image", b"kernel")
        _write(root / "boot/uInitrd", b"initramfs")
        _write(root / "boot/armbianEnv.txt", "verbosity=1\nfdtfile=sun50i-h618-orangepi-zero2w.dtb\n")
        _write(root / f"boot/initrd.img-{policy.contract['selected_boot']['kernel_release']}", b"initramfs")
        _write(root / f"boot/config-{policy.contract['selected_boot']['kernel_release']}", b"config")
        _write(root / f"usr/lib/linux-image-{policy.contract['selected_boot']['kernel_release']}/Image", b"kernel")
        _write(root / f"usr/lib/linux-image-{policy.contract['selected_boot']['kernel_release']}/allwinner/{policy.contract['selected_boot']['dtb_name']}", b"dtb")
        _write(root / f"usr/lib/modules/{policy.contract['selected_boot']['kernel_release']}/modules.dep", b"modules")
        _write(root / "usr/lib/systemd/system-sleep/octessera-orange-oled", b"sleep-hook")
        (root / "lib").symlink_to("usr/lib")
        _write(root / "etc/initramfs-tools/scripts/init-bottom/octessera-orange-boot-splash", b"initramfs-hook")
        _write(root / "etc/udev/rules.d/70-octessera-orange-runtime.rules", b"udev-rule")
        for relative in policy.contract["protected_paths"]:
            if relative == "etc/systemd/system/multi-user.target.wants/octessera.service":
                continue
            path = root / relative
            if path.exists() or path.is_symlink():
                continue
            _write(path, "[Service]\n" if relative.endswith(".service") else b"protected")
        link = root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service"
        if link.exists() or link.is_symlink():
            link.unlink()
        link.symlink_to("../octessera-orange-boot-splash.service")
        runtime_link = root / "etc/systemd/system/multi-user.target.wants/octessera.service"
        runtime_link.parent.mkdir(parents=True, exist_ok=True)
        runtime_link.symlink_to("../octessera.service")
    os.chmod(release, 0o555 if board == ORANGE else 0o755)
    (root / "opt/octessera/current").symlink_to(f"/opt/octessera/releases/{prior}")
    (root / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
    _owner(root / "opt/octessera/current")
    _owner(root / "usr/local/bin/octessera-pi")
    return root, _bundle(work, board, "2.0.0")


class RuntimeMutationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            probe = Path(temporary) / "root"
            probe.mkdir()
            try:
                (probe / "link").symlink_to("target")
            except OSError as exc:
                raise unittest.SkipTest(f"symlinks unavailable: {exc}")

    def test_orange_contract_replaces_old_content_and_emits_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root, bundle = _fixture(work, ORANGE)
            _write(work / "external-target", b"host pseudo target", 0o644)
            (root / "etc/external-absolute").symlink_to(work / "external-target")
            result = mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))
            self.assertEqual(result.prior_version, "1.0.0")
            self.assertEqual(result.version, "2.0.0")
            self.assertEqual((root / "opt/octessera/releases/2.0.0/octessera-pi").read_bytes(), b"new-runtime")
            self.assertFalse((root / "opt/octessera/releases/1.0.0").exists())
            self.assertEqual((root / "opt/octessera/current").readlink().as_posix(), "/opt/octessera/releases/2.0.0")
            self.assertEqual(result.post_inventory_digest, inventory_digest(build_inventory(root)))
            self.assertEqual(result.notice["preimage"], {"path": "usr/share/doc/octessera", "status": "absent"})
            self.assertEqual(set(result.notice["changed_paths"]), {path for path in result.changed_paths if path == "usr/share/doc/octessera" or path.startswith("usr/share/doc/octessera/")})
            self.assertEqual((root / "usr/share/doc/octessera/LICENSE").read_bytes(), (Path(__file__).resolve().parents[2] / "LICENSE").read_bytes())
            self.assertEqual((root / "usr/share/doc/base-files/copyright").read_bytes(), b"vendor copyright\n")
            self.assertEqual((root / "usr/share/common-licenses/GPL-3").read_bytes(), b"vendor GPL\n")
            self.assertEqual(set(result.parent_identity["prior_release_entries"]), {"octessera-pi", "octessera-runtime.json", "SHA256SUMS"})
            self.assertEqual(result.parent_identity["prior_release_entries"]["octessera-pi"], hashlib.sha256(b"old-runtime").hexdigest())
            self.assertEqual(result.parent_identity["parent_context"], _parent_context(ORANGE))
            self.assertFalse((root / "opt/octessera/update-state.json").exists())
            self.assertIsNone(result.parent_identity["prior_state_preimage_sha256"])
            metadata_lines = (root / "etc/octessera/build-metadata.env").read_text(encoding="utf-8").splitlines()
            self.assertIn("OCTESSERA_RUNTIME_VERSION=2.0.0", metadata_lines)
            self.assertIn("OCTESSERA_IMAGE_MODE=production", metadata_lines)
            metadata = build_inventory(root)["etc/octessera/build-metadata.env"]
            self.assertEqual((metadata["uid"], metadata["gid"], metadata["xattrs"], metadata["capability"]), (0, 0, {}, None))
            if os.name != "nt":
                self.assertEqual(metadata["mode"], 0o644)

    def test_same_version_replacement_removes_stale_prior_content(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, bundle = _fixture(Path(temporary), ORANGE, prior="2.0.0")
            result = mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))
            self.assertEqual(result.prior_version, result.version)
            self.assertEqual((root / "opt/octessera/releases/2.0.0/octessera-pi").read_bytes(), b"new-runtime")
            self.assertNotIn(".image-respin", "\n".join(result.changed_paths))

    def test_bad_checksums_metadata_board_and_semver_are_rejected(self) -> None:
        cases = ("checksum", "metadata", "board", "version")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                root, bundle = _fixture(work, ORANGE)
                if case == "checksum":
                    _write(bundle / "SHA256SUMS", "0" * 64 + "  octessera-pi\n", 0o644)
                elif case == "metadata":
                    value = json.loads((bundle / "octessera-runtime.json").read_text())
                    value["version"] = "9.9.9"
                    _write(bundle / "octessera-runtime.json", json.dumps(value), 0o644)
                elif case == "board":
                    value = json.loads((bundle / "octessera-runtime.json").read_text())
                    value["profile"] = RPI
                    _write(bundle / "octessera-runtime.json", json.dumps(value), 0o644)
                else:
                    with self.assertRaises(MutationError):
                        mutate_runtime(root, bundle, ORANGE, "bad", "source-1", _parent_context(ORANGE))
                    continue
                with self.assertRaises(MutationError):
                    mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))

    def test_symlink_escape_and_extra_release_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root, bundle = _fixture(work, ORANGE)
            outside = work / "outside"
            outside.mkdir()
            (root / "usr/local/bin").rename(root / "usr/local/bin-real")
            (root / "usr/local/bin").symlink_to(outside)
            with self.assertRaises(MutationError):
                mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root, bundle = _fixture(work, ORANGE)
            _mkdir(root / "opt/octessera/releases/3.0.0")
            with self.assertRaises(MutationError):
                mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))

    def test_bundle_shape_modes_and_malformed_json_are_rejected(self) -> None:
        for mutation in ("extra", "symlink", "mode", "json"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                root, bundle = _fixture(work, ORANGE)
                if mutation == "extra":
                    _write(bundle / "unexpected", b"no", 0o644)
                elif mutation == "symlink":
                    external = work / "external"
                    _write(external, b"escape", 0o644)
                    (bundle / "octessera-pi").unlink()
                    (bundle / "octessera-pi").symlink_to(external)
                elif mutation == "mode":
                    os.chmod(bundle / "octessera-pi", 0o444)
                else:
                    _write(bundle / "octessera-runtime.json", "{malformed", 0o644)
                with self.assertRaises(MutationError):
                    mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE))

    def test_parent_context_is_required_exact_and_board_bound(self) -> None:
        cases = ("missing", "extra", "schema", "commit", "image-sha")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                root, bundle = _fixture(work, ORANGE)
                context = _parent_context(ORANGE)
                if case == "missing":
                    context.pop("version")
                elif case == "extra":
                    context["unexpected"] = True
                elif case == "schema":
                    context["schema"] = "wrong"
                elif case == "commit":
                    context["constructor"]["source_sha"] = "not-a-commit"
                else:
                    context["image"]["sha256"] = "not-a-digest"
                with self.assertRaises(MutationError):
                    mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", context)
        with tempfile.TemporaryDirectory() as temporary:
            root, bundle = _fixture(Path(temporary), ORANGE)
            with self.assertRaises(MutationError):
                mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(RPI))

    def test_unauthorized_mutation_is_rejected_and_interrupted_commit_rolls_back(self) -> None:
        points = ("staged", "notice-staged", "notice-published", "prior-release-moved", "release-installed", "current-replaced", "binary-replaced", "build-metadata-replaced")
        for point in points:
            with self.subTest(board=ORANGE, point=point), tempfile.TemporaryDirectory() as temporary:
                root, bundle = _fixture(Path(temporary), ORANGE)
                before = inventory_digest(build_inventory(root))
                def fail(name: str) -> None:
                    if name == point:
                        raise RuntimeError("interrupted")
                with self.assertRaises(MutationError):
                    mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE), mutation_hook=fail)
                self.assertEqual(inventory_digest(build_inventory(root)), before)
                if os.name != "nt":
                    self.assertEqual(build_inventory(root)["etc/octessera/build-metadata.env"]["mode"], 0o664)
        with tempfile.TemporaryDirectory() as temporary:
            root, bundle = _fixture(Path(temporary), ORANGE)
            before = inventory_digest(build_inventory(root))
            def unauthorized(name: str) -> None:
                if name == "staged":
                    _write(root / "opt/octessera/releases/1.0.0/unauthorized", b"no", 0o644)
            with self.assertRaises(MutationError):
                mutate_runtime(root, bundle, ORANGE, "2.0.0", "source-1", _parent_context(ORANGE), mutation_hook=unauthorized)
            self.assertEqual(inventory_digest(build_inventory(root)), before)
if __name__ == "__main__":
    unittest.main()
