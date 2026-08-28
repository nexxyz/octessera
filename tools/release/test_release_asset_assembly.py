from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
import zipfile
from copy import deepcopy
from pathlib import Path

from tools.release.assemble_release_assets import (
    ReleaseArtifactError,
    _require_exact_files,
    _verify_checksum_file,
    _verify_device_zip,
    _write_checksums,
)
from tools.release.board_image_release import (
    BASE_REFRESH,
    BOARD_IMAGE_MODES,
    QUALIFIED_RESPIN,
    _package_filenames,
    _qualified_names,
    _require_exact_files as _qualified_require_exact_files,
    _verify_qualified_checksum,
    verify_and_stage_board_images,
)


class ReleaseAssetAssemblyTests(unittest.TestCase):
    def test_unknown_board_image_mode_is_rejected_before_asset_access(self) -> None:
        with self.assertRaises(ReleaseArtifactError):
            verify_and_stage_board_images(
                Path("."),
                Path("."),
                Path("."),
                Path("."),
                Path("."),
                Path("."),
                "1.2.3",
                "a" * 40,
                "unknown",
            )

    def test_board_image_modes_are_explicit_and_qualified_inventory_is_exact(self) -> None:
        self.assertEqual(BOARD_IMAGE_MODES, (BASE_REFRESH, QUALIFIED_RESPIN))
        with tempfile.TemporaryDirectory(prefix="octessera-qualified-handoff-") as temporary:
            directory = Path(temporary)
            artifact, provenance, requested, post_proof, setup_proof, production, checksum = _qualified_names("raspberry-pi-zero-2w", "1.2.3")
            names = (artifact, provenance, requested, post_proof, setup_proof, production)
            for name in names:
                (directory / name).write_bytes(name.encode("utf-8"))
            (directory / checksum).write_text(
                "".join(
                    f"{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}\n"
                    for name in names
                ),
                encoding="utf-8",
            )
            _qualified_require_exact_files(directory, (*names, checksum))
            _verify_qualified_checksum(directory, checksum, names)
            (directory / "unexpected").write_bytes(b"unexpected")
            with self.assertRaises(ReleaseArtifactError):
                _qualified_require_exact_files(directory, (*names, checksum))

    def test_kernel_package_filenames_derive_from_manifest_declarations(self) -> None:
        manifest = {
            "kernels": {
                "raspberry": {
                    "package": {
                        "name": "linux-image-6.12.93-octessera-rpi-v8-0.7.5",
                        "version": "6.12.93-octessera0.7.5-1",
                        "architecture": "arm64",
                    }
                },
                "orange": {
                    "packages": [
                        "linux-image-current-sunxi64_26.11.0-trunk.22_arm64.deb",
                        "linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64.deb",
                    ]
                },
            }
        }

        self.assertEqual(
            _package_filenames(manifest),
            (
                "linux-image-6.12.93-octessera-rpi-v8-0.7.5_6.12.93-octessera0.7.5-1_arm64.deb",
                "linux-image-current-sunxi64_26.11.0-trunk.22_arm64.deb",
                "linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64.deb",
            ),
        )

    def test_kernel_package_filenames_reject_malformed_missing_and_duplicate_declarations(self) -> None:
        manifest = {
            "kernels": {
                "raspberry": {
                    "package": {"name": "linux-image", "version": "1", "architecture": "arm64"}
                },
                "orange": {"packages": ["linux-image_arm64.deb", "linux-dtb_arm64.deb"]},
            }
        }
        cases = (
            ("missing Raspberry package", lambda value: value["kernels"]["raspberry"].pop("package"), "Raspberry package declaration"),
            ("malformed Raspberry package", lambda value: value["kernels"]["raspberry"]["package"].update(version=None), "Raspberry package version declaration"),
            ("missing Orange packages", lambda value: value["kernels"]["orange"].pop("packages"), "Orange package declaration"),
            ("duplicate Orange packages", lambda value: value["kernels"]["orange"].update(packages=["linux-image_arm64.deb"] * 2), "duplicate packages"),
        )
        for name, mutate, expected in cases:
            with self.subTest(name=name):
                invalid = deepcopy(manifest)
                mutate(invalid)
                with self.assertRaisesRegex(ReleaseArtifactError, expected):
                    _package_filenames(invalid)

    def test_exact_root_contract_and_checksum_success(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-release-assets-") as temporary:
            root = Path(temporary)
            names = [f"asset-{index}.bin" for index in range(11)]
            for name in names:
                (root / name).write_bytes(name.encode("utf-8"))
            _write_checksums(root, "SHA256SUMS.txt", names)
            _require_exact_files(root, [*names, "SHA256SUMS.txt"])

    def test_exact_root_contract_rejects_tampered_names_and_hashes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-release-assets-") as temporary:
            root = Path(temporary)
            (root / "expected.bin").write_bytes(b"expected")
            _write_checksums(root, "SHA256SUMS.txt", ["expected.bin"])
            (root / "unexpected.bin").write_bytes(b"unexpected")
            with self.assertRaises(ReleaseArtifactError):
                _require_exact_files(root, ["expected.bin", "SHA256SUMS.txt"])
            (root / "unexpected.bin").unlink()
            (root / "expected.bin").write_bytes(b"tampered")
            with self.assertRaises(ReleaseArtifactError):
                _verify_checksum_file(root, "SHA256SUMS.txt")

    def test_device_zip_success_and_mode_tampering(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-device-zip-") as temporary:
            root = Path(temporary)
            (root / "LICENSE").write_bytes(b"license")
            (root / "NOTICE").write_bytes(b"notice")
            bundle = root / "runtime"
            bundle.mkdir()
            binary = bundle / "octessera-pi"
            binary.write_bytes(b"runtime")
            binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
            metadata = {
                "artifact_kind": "production-runtime",
                "binary_sha256": binary_sha,
                "name": "octessera-pi",
                "profile": "orange-pi-zero-2w",
                "runtime_ready": True,
                "version": "1.2.3",
            }
            (bundle / "octessera-runtime.json").write_text(json.dumps(metadata), encoding="utf-8")
            (bundle / "SHA256SUMS").write_text(f"{binary_sha}  octessera-pi\n", encoding="utf-8")
            archive_path = root / "device.zip"
            self._write_orange_zip(archive_path, bundle, root, mode=0o644)
            _verify_device_zip(root, bundle, archive_path, "1.2.3", "orange-pi-zero-2w")
            tampered = root / "tampered-mode.zip"
            self._write_orange_zip(tampered, bundle, root, mode=0o755)
            with self.assertRaises(ReleaseArtifactError):
                _verify_device_zip(root, bundle, tampered, "1.2.3", "orange-pi-zero-2w")

    @staticmethod
    def _write_orange_zip(path: Path, bundle: Path, root: Path, mode: int) -> None:
        manifest = {
            "updater_supported": False,
            "candidate_health_protocol": 1,
            "distribution": "standalone-manual",
        }
        names = ["octessera-pi", "octessera-runtime.json", "SHA256SUMS", "octessera-device-release.json", "LICENSE", "NOTICE"]
        payloads = {
            "octessera-pi": (bundle / "octessera-pi").read_bytes(),
            "octessera-runtime.json": (bundle / "octessera-runtime.json").read_bytes(),
            "SHA256SUMS": (bundle / "SHA256SUMS").read_bytes(),
            "octessera-device-release.json": json.dumps(manifest).encode("utf-8"),
            "LICENSE": (root / "LICENSE").read_bytes(),
            "NOTICE": (root / "NOTICE").read_bytes(),
        }
        with zipfile.ZipFile(path, "w") as archive:
            for name in names:
                info = zipfile.ZipInfo(name)
                entry_mode = 0o755 if name == "octessera-pi" else mode
                info.external_attr = (0o100000 | entry_mode) << 16
                archive.writestr(info, payloads[name])


if __name__ == "__main__":
    unittest.main()
