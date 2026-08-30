from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools/armbian-image"))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools/pi-kernel"))

import test_orange_image_proof_runtime as orange_runtime  # type: ignore[import-not-found]
import test_orange_image_proof_support as orange_support  # type: ignore[import-not-found]
from tools.release.board_image_release import ReleaseArtifactError, _package_filenames, verify_and_stage_board_images


ROOT = Path(__file__).resolve().parents[2]
VERSION = "0.8.1"
SOURCE_SHA = "a" * 40
RPI = "raspberry-pi-zero-2w"
ORANGE = "orange-pi-zero-2w"
REQUIRED_TOOLS = ("bash", "cpio", "dpkg-deb", "dtc", "fdtoverlay", "find", "git", "losetup", "mkfs.ext4", "mount", "readelf", "sfdisk", "strings", "sudo", "umount", "udevadm", "xz", "zstd")


def _load_rpi_tests() -> Any:
    path = ROOT / "tools/pi-kernel/test-rpi-kernel.py"
    spec = importlib.util.spec_from_file_location("rpi_kernel_fixture_tests", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load Raspberry fixture support: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


RPI_TESTS = _load_rpi_tests()


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _checksums(directory: Path, name: str, paths: list[str]) -> None:
    (directory / name).write_text("".join(f"{_sha(directory / path)}  {path}\n" for path in paths), encoding="utf-8")


def _rpi_constructor_assets(work: Path, gathered: Path) -> None:
    contract = RPI_TESTS.load_contract(ROOT)
    package = RPI_TESTS._make_package(work / "rpi-package", contract, "release")
    inventory = RPI_TESTS.VALIDATOR.validate_package(package, contract)
    provenance = dict(inventory)
    provenance["build"] = RPI_TESTS._build_provenance(ROOT, contract, inventory)
    image_directory = gathered / f"octessera-{RPI}-image-release-assets"
    kernel_directory = gathered / f"octessera-{RPI}-kernel-release-assets"
    image_directory.mkdir(parents=True)
    kernel_directory.mkdir(parents=True)
    shutil.copyfile(package, kernel_directory / package.name)
    (kernel_directory / "inventory.json").write_text(json.dumps(inventory, sort_keys=True), encoding="utf-8")
    (kernel_directory / "provenance.json").write_text(json.dumps(provenance, sort_keys=True), encoding="utf-8")
    _checksums(kernel_directory, "SHA256SUMS", [package.name])
    image_name = f"octessera-{VERSION}-{RPI}.img.zip"
    manifest_name = f"octessera-{VERSION}-{RPI}.rpi-imager-manifest"
    with zipfile.ZipFile(image_directory / image_name, "w") as archive:
        archive.writestr("octessera.img", b"constructor-image")
    (image_directory / manifest_name).write_text("{}\n", encoding="utf-8")
    _checksums(image_directory, "SHA256SUMS-pi.txt", [image_name, manifest_name])


def _make_partitioned_image(work: Path, source: Path) -> Path:
    image = work / "orange.img"
    image.write_bytes(b"\0" * (128 * 1024 * 1024))
    subprocess.run(["sfdisk", str(image)], input="label: dos\nunit: sectors\nstart=2048,type=83\n", text=True, check=True, capture_output=True)
    loop = subprocess.run(["losetup", "--find", "--show", "--partscan", str(image)], check=True, capture_output=True, text=True).stdout.strip()
    mount = Path(tempfile.mkdtemp(prefix="octessera-release-image-"))
    mounted = False
    try:
        subprocess.run(["udevadm", "settle"], check=True, capture_output=True, text=True)
        subprocess.run(["mkfs.ext4", "-F", f"{loop}p1"], check=True, capture_output=True, text=True)
        subprocess.run(["mount", "-o", "rw,noatime", f"{loop}p1", str(mount)], check=True, capture_output=True, text=True)
        mounted = True
        shutil.copytree(source, mount, symlinks=True, dirs_exist_ok=True)
        subprocess.run(["sync"], check=True)
    finally:
        if mounted:
            subprocess.run(["umount", str(mount)], check=True, capture_output=True, text=True)
        mount.rmdir()
        subprocess.run(["losetup", "-d", loop], check=True, capture_output=True, text=True)
    compressed = work / f"octessera-{VERSION}-{ORANGE}.img.xz"
    with compressed.open("wb") as output:
        subprocess.run(["xz", "-c", str(image)], check=True, stdout=output)
    return compressed


def _orange_constructor_assets(work: Path, gathered: Path) -> None:
    fixture_work = work / "orange-proof"
    _, native_image, native_dtb, evidence, provenance = orange_support.make_fixture(fixture_work)
    orange_runtime.run_runtime_proof(fixture_work, native_image, native_dtb, evidence, provenance)
    production = fixture_work / "production"
    enabled = production / "etc/systemd/system/multi-user.target.wants/octessera.service"
    if not enabled.exists() and not enabled.is_symlink():
        enabled.symlink_to("/etc/systemd/system/octessera.service")
    image_directory = gathered / f"octessera-{ORANGE}-image-release-assets"
    image_directory.mkdir(parents=True)
    manifest = json.loads((ROOT / "tools/kernel-patches/orange-midi-interface-manifest.json").read_text(encoding="utf-8"))
    armbian = manifest["build_frameworks"]["armbian"]
    canonical_image, canonical_dtb = armbian["packages"]
    shutil.copyfile(native_image, image_directory / canonical_image)
    shutil.copyfile(native_dtb, image_directory / canonical_dtb)
    final_image = _make_partitioned_image(work, production)
    image_name = final_image.name
    shutil.copyfile(final_image, image_directory / image_name)
    shutil.copyfile(evidence, image_directory / "octessera-orange-kernel-evidence.env")
    provenance_values = dict(line.split("=", 1) for line in provenance.read_text(encoding="utf-8").splitlines())
    provenance_values.update(
        {
            "armbian_build_repository": armbian["repository"],
            "github_source_sha": SOURCE_SHA,
            "package_revision": armbian["package_revision"],
            "revision_argument": armbian["revision_argument"],
        }
    )
    provenance_path = image_directory / "octessera-orange-kernel-provenance.txt"
    provenance_path.write_text("\n".join(f"{key}={value}" for key, value in provenance_values.items()) + "\n", encoding="utf-8")
    proof_path = image_directory / "octessera-orange-image-proof.json"
    proof_command = orange_support.verifier_args(production, image_directory / canonical_image, image_directory / canonical_dtb, image_directory / "octessera-orange-kernel-evidence.env", provenance_path, "production")
    subprocess.run([*proof_command, "--output", str(proof_path)], check=True, capture_output=True, text=True)
    image_checksum = f"{_sha(image_directory / image_name)}  {image_name}\n"
    (image_directory / f"{image_name}.sha256").write_text(image_checksum, encoding="utf-8")
    _checksums(image_directory, "SHA256SUMS-orange-pi-zero-2w.txt", [image_name, f"{image_name}.sha256", canonical_image, canonical_dtb, "octessera-orange-kernel-evidence.env", "octessera-orange-kernel-provenance.txt", "octessera-orange-image-proof.json"])


def _constructor_fixture(work: Path) -> tuple[Path, Path, Path, Path, Path]:
    gathered = work / "gathered"
    gathered.mkdir()
    _rpi_constructor_assets(work, gathered)
    _orange_constructor_assets(work, gathered)
    release = work / "release"
    evidence = work / "evidence"
    release.mkdir()
    for relative in ("raspberry/image", "raspberry/kernel", "orange/image", "orange/kernel"):
        (evidence / relative).mkdir(parents=True)
    return gathered, work / "rpi-runtime", work / "orange-runtime", release, evidence


def _integration_available() -> bool:
    if platform.system() != "Linux" or getattr(os, "geteuid", lambda: -1)() != 0:
        return False
    if any(shutil.which(tool) is None for tool in REQUIRED_TOOLS):
        return False
    return subprocess.run(["sudo", "-n", "true"], check=False, capture_output=True).returncode == 0


class BoardImageReleaseTests(unittest.TestCase):
    def test_respin_workflow_is_not_a_release_publication_path(self) -> None:
        workflow = (ROOT / ".github/workflows/release-board-artifacts.yml").read_text(encoding="utf-8")
        self.assertNotIn("qualified-respin", workflow)
        self.assertNotIn("raspberry_respin", workflow)
        self.assertNotIn("orange_respin", workflow)

    def test_exact_constructor_handoffs_reject_a_bad_checksum_before_staging(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-board-image-reject-") as temporary:
            work = Path(temporary)
            gathered = work / "gathered"
            gathered.mkdir()
            manifest = json.loads((ROOT / "tools/kernel-patches/orange-midi-interface-manifest.json").read_text(encoding="utf-8"))
            rpi_package, orange_image_package, orange_dtb_package = _package_filenames(manifest)
            rpi_images = gathered / f"octessera-{RPI}-image-release-assets"
            rpi_kernel = gathered / f"octessera-{RPI}-kernel-release-assets"
            orange_images = gathered / f"octessera-{ORANGE}-image-release-assets"
            for directory in (rpi_images, rpi_kernel, orange_images):
                directory.mkdir()
            rpi_image = f"octessera-{VERSION}-{RPI}.img.zip"
            rpi_manifest = f"octessera-{VERSION}-{RPI}.rpi-imager-manifest"
            for name, contents in ((rpi_image, b"image"), (rpi_manifest, b"manifest")):
                (rpi_images / name).write_bytes(contents)
            _checksums(rpi_images, "SHA256SUMS-pi.txt", [rpi_image, rpi_manifest])
            (rpi_kernel / rpi_package).write_bytes(b"invalid package")
            (rpi_kernel / "SHA256SUMS").write_text(f"{'0' * 64}  {rpi_package}\n", encoding="utf-8")
            for name in ("inventory.json", "provenance.json"):
                (rpi_kernel / name).write_text("{}\n", encoding="utf-8")
            orange_image = f"octessera-{VERSION}-{ORANGE}.img.xz"
            orange_names = (orange_image, f"{orange_image}.sha256", orange_image_package, orange_dtb_package, "octessera-orange-kernel-evidence.env", "octessera-orange-kernel-provenance.txt", "octessera-orange-image-proof.json", "SHA256SUMS-orange-pi-zero-2w.txt")
            for name in orange_names:
                (orange_images / name).write_bytes(name.encode())
            release = work / "release"
            release.mkdir()
            with self.assertRaises(ReleaseArtifactError):
                verify_and_stage_board_images(ROOT, gathered, release, work / "evidence", work / "rpi-runtime", work / "orange-runtime", VERSION, SOURCE_SHA)
            self.assertFalse(any(release.iterdir()))


@unittest.skipUnless(_integration_available(), "constructor board-image fixture requires Linux root and image-build tools")
class ConstructorBoardImageReleaseTests(unittest.TestCase):
    def test_constructor_images_stage_and_checksum_failures_reject(self) -> None:
        with tempfile.TemporaryDirectory(prefix="octessera-board-image-release-") as temporary:
            work = Path(temporary)
            gathered, raspberry_runtime, orange_runtime, release, evidence = _constructor_fixture(work)
            verify_and_stage_board_images(ROOT, gathered, release, evidence, raspberry_runtime, orange_runtime, VERSION, SOURCE_SHA)
            self.assertEqual(
                sorted(path.name for path in release.iterdir()),
                sorted((f"octessera-{VERSION}-{RPI}.img.zip", f"octessera-{VERSION}-{RPI}.rpi-imager-manifest", f"octessera-{VERSION}-{ORANGE}.img.xz")),
            )
            self.assertTrue((evidence / "raspberry/image/SHA256SUMS-pi.txt").is_file())
            self.assertTrue((evidence / "orange/image/SHA256SUMS-orange-pi-zero-2w.txt").is_file())
            self.assertEqual(len(list((evidence / "orange/image").glob("*.img.xz.sha256"))), 1)

            rpi_directory = gathered / f"octessera-{RPI}-image-release-assets"
            rpi_checksum = rpi_directory / "SHA256SUMS-pi.txt"
            original_rpi_checksum = rpi_checksum.read_bytes()
            rpi_checksum.write_text("0" * 64 + original_rpi_checksum[64:].decode(), encoding="utf-8")
            rejected_release = work / "rejected-rpi-release"
            rejected_evidence = work / "rejected-rpi-evidence"
            rejected_release.mkdir()
            with self.assertRaises(ReleaseArtifactError):
                verify_and_stage_board_images(ROOT, gathered, rejected_release, rejected_evidence, raspberry_runtime, orange_runtime, VERSION, SOURCE_SHA)
            self.assertFalse(any(rejected_release.iterdir()))
            rpi_checksum.write_bytes(original_rpi_checksum)

            orange_directory = gathered / f"octessera-{ORANGE}-image-release-assets"
            orange_image_checksum = next(orange_directory.glob("*.img.xz.sha256"))
            original_orange_checksum = orange_image_checksum.read_bytes()
            orange_image_checksum.write_text("0" * 64 + original_orange_checksum[64:].decode(), encoding="utf-8")
            rejected_orange_release = work / "rejected-orange-release"
            rejected_orange_evidence = work / "rejected-orange-evidence"
            rejected_orange_release.mkdir()
            with self.assertRaises(ReleaseArtifactError):
                verify_and_stage_board_images(ROOT, gathered, rejected_orange_release, rejected_orange_evidence, raspberry_runtime, orange_runtime, VERSION, SOURCE_SHA)
            self.assertFalse(any(rejected_orange_release.iterdir()))


if __name__ == "__main__":
    unittest.main()
