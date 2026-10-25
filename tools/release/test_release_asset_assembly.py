from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from tools.release.assemble_release_assets import (
    ReleaseArtifactError,
    _require_exact_files,
    _verify_checksum_file,
    _verify_device_zip,
    _write_checksums,
)
from package_device_bundle import package_bundle  # type: ignore[import-not-found]


class ReleaseAssetAssemblyTests(unittest.TestCase):
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
            archive_path = package_bundle(bundle, root / "out", root, "orange-pi-zero-2w", "v1.2.3", "1.2.3")
            _verify_device_zip(root, bundle, archive_path, "orange-pi-zero-2w")
            tampered = root / "tampered-mode.zip"
            with zipfile.ZipFile(archive_path) as source, zipfile.ZipFile(tampered, "w") as target:
                for info in source.infolist():
                    if info.filename == "NOTICE":
                        info.external_attr = (0o100000 | 0o755) << 16
                    target.writestr(info, source.read(info))
            with self.assertRaises(ReleaseArtifactError):
                _verify_device_zip(root, bundle, tampered, "orange-pi-zero-2w")


if __name__ == "__main__":
    unittest.main()
