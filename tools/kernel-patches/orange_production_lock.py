import hashlib
import json
from pathlib import Path
from typing import Any


EXPECTED_ARMBIAN_COMMIT = "3da49cffcb8ac58a919d86816fec4659c410ff1e"
EXPECTED_ARMBIAN_TAG = "v26.11.0-trunk.22"
EXPECTED_ARMBIAN_REPOSITORY = "https://github.com/armbian/build.git"
EXPECTED_ORANGE_KERNEL_REPOSITORY = "https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git"
EXPECTED_ORANGE_KERNEL_BRANCH = "linux-6.18.y"
EXPECTED_ORANGE_KERNEL_COMMIT = "1f99e9ab748fc5c32120de9c4eca31abfe54a4d5"
EXPECTED_ORANGE_KERNEL_RELEASE = "6.18.46-current-sunxi64"
EXPECTED_ORANGE_PACKAGE_REVISION = "26.11.0-trunk.22"
EXPECTED_ORANGE_REVISION_ARGUMENT = "REVISION=26.11.0-trunk.22"
EXPECTED_ORANGE_ARTIFACT_SUFFIX = "6.18.46-S1f99-D7115-P25bc-C4e0c-H5530-HK01ba-Vc222-Bb84f-R448a"
EXPECTED_SOURCE_LOCK_PATH = "userpatches/config/sources/git_sources.json"
EXPECTED_SOURCE_LOCK_SHA256 = "e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30"
EXPECTED_CONFIG_BASE_PATH = "config/kernel/linux-sunxi64-current.config"
EXPECTED_CONFIG_BASE_SHA256 = "03a427fed857cc598ef95c5c8a2dccb43bb515d513df1eff9c010e6aa56ab155"
EXPECTED_PACKAGED_CONFIG_SHA256 = "922e8037090e2202afdf70d46ea50c29790dcece17b62155c28212e7b6554cbc"
EXPECTED_ORANGE_PACKAGES = (
    "linux-image-current-sunxi64_26.11.0-trunk.22_arm64.deb",
    "linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64.deb",
)


def validate_source_lock(manifest: dict[str, object], root: Path) -> None:
    source_lock = manifest.get("source_lock", {})
    if not isinstance(source_lock, dict) or source_lock.get("path") != EXPECTED_SOURCE_LOCK_PATH or source_lock.get("sha256") != EXPECTED_SOURCE_LOCK_SHA256:
        raise ValueError("manifest source-lock identity is not the approved source lock")
    path = root / EXPECTED_SOURCE_LOCK_PATH
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"missing or symlinked source lock: {path}")
    if hashlib.sha256(path.read_bytes()).hexdigest() != EXPECTED_SOURCE_LOCK_SHA256:
        raise ValueError("source-lock SHA-256 does not match the manifest")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot parse source lock: {error}") from error
    if document != [{"source": EXPECTED_ORANGE_KERNEL_REPOSITORY, "branch": EXPECTED_ORANGE_KERNEL_BRANCH, "sha1": EXPECTED_ORANGE_KERNEL_COMMIT}]:
        raise ValueError("source lock entry is not the approved stable Linux source")


def validate_production_manifest(manifest: dict[str, Any], root: Path) -> None:
    validate_source_lock(manifest, root)
    frameworks = manifest.get("build_frameworks", {})
    armbian = frameworks.get("armbian", {})
    kernels = manifest.get("kernels", {})
    orange = kernels.get("orange", {})
    if (
        armbian.get("repository") != EXPECTED_ARMBIAN_REPOSITORY
        or armbian.get("commit") != EXPECTED_ARMBIAN_COMMIT
        or armbian.get("tag") != EXPECTED_ARMBIAN_TAG
    ):
        raise ValueError("manifest Armbian production identity is not approved")
    if (
        orange.get("repository") != EXPECTED_ORANGE_KERNEL_REPOSITORY
        or orange.get("branch") != EXPECTED_ORANGE_KERNEL_BRANCH
        or orange.get("commit") != EXPECTED_ORANGE_KERNEL_COMMIT
    ):
        raise ValueError("manifest Orange stable Linux source identity is not approved")
    if (
        orange.get("release") != EXPECTED_ORANGE_KERNEL_RELEASE
        or orange.get("package_revision") != EXPECTED_ORANGE_PACKAGE_REVISION
        or tuple(orange.get("packages", ())) != EXPECTED_ORANGE_PACKAGES
    ):
        raise ValueError("manifest Orange ABI, revision, or package identity is not approved")
    if (
        armbian.get("kernel_commit") != EXPECTED_ORANGE_KERNEL_COMMIT
        or armbian.get("kernel_release") != EXPECTED_ORANGE_KERNEL_RELEASE
        or armbian.get("package_revision") != EXPECTED_ORANGE_PACKAGE_REVISION
        or armbian.get("revision_argument") != EXPECTED_ORANGE_REVISION_ARGUMENT
        or tuple(armbian.get("packages", ())) != EXPECTED_ORANGE_PACKAGES
    ):
        raise ValueError("manifest Armbian Orange build tuple is not approved")
    if armbian.get("native_artifact_suffix") != EXPECTED_ORANGE_ARTIFACT_SUFFIX:
        raise ValueError("manifest Orange native artifact suffix is not approved")
    config_base = armbian.get("config_base", {})
    if (
        config_base.get("path") != EXPECTED_CONFIG_BASE_PATH
        or config_base.get("sha256") != EXPECTED_CONFIG_BASE_SHA256
        or armbian.get("packaged_config_sha256") != EXPECTED_PACKAGED_CONFIG_SHA256
    ):
        raise ValueError("manifest Orange config identity is not approved")
