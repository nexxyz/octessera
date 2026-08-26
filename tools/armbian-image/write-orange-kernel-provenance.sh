#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 4 || $# -gt 7 ]]; then
  echo "Usage: $0 <linux-image.deb> <linux-dtb.deb> <provenance-file> <evidence-file> [armbian-build-directory] [expected-config-sha256] [handoff-directory]" >&2
  exit 2
fi

image_package="$1"
dtb_package="$2"
provenance_file="$3"
evidence_file="$4"
armbian_build_directory="${5:-}"
expected_config_sha256="${6:-}"
handoff_directory="${7:-}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
validator="$root/tools/armbian-image/validate-orange-kernel-package.sh"
manifest="$root/tools/kernel-patches/orange-midi-interface-manifest.json"

[[ -f "$manifest" ]] || { echo "Missing Orange kernel package manifest: $manifest" >&2; exit 1; }
[[ -f "$evidence_file" ]] || { echo "Missing Orange kernel package evidence: $evidence_file" >&2; exit 1; }

work="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work"
}
trap cleanup EXIT

actual_evidence="$work/evidence.env"
validator_args=("$validator" "$image_package" "$dtb_package" --evidence-output "$actual_evidence")
if [[ -n "$expected_config_sha256" ]]; then
  validator_args+=(--expected-config-sha256 "$expected_config_sha256")
fi
bash "${validator_args[@]}" >/dev/null
cmp -- "$actual_evidence" "$evidence_file" || {
  echo "Orange kernel package evidence does not match the packages." >&2
  exit 1
}

if [[ -n "$handoff_directory" ]]; then
  [[ -d "$handoff_directory" ]] || { echo "Missing Orange kernel handoff directory: $handoff_directory" >&2; exit 1; }
fi

[[ "${ARMBIAN_BUILD_REF:-}" =~ ^[0-9a-fA-F]{40}$ ]] || {
  echo "Orange kernel provenance requires a full Armbian build commit SHA." >&2
  exit 1
}
[[ "${GITHUB_SOURCE_SHA:-}" =~ ^[0-9a-fA-F]{40}$ ]] || {
  echo "Orange kernel provenance requires a full GitHub source commit SHA." >&2
  exit 1
}

mkdir -p -- "$(dirname "$provenance_file")"
python3 - "$root" "$manifest" "$image_package" "$dtb_package" "$evidence_file" "$provenance_file" "$armbian_build_directory" "$handoff_directory" <<'PY'
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
from fnmatch import fnmatchcase


root = pathlib.Path(sys.argv[1])
manifest_path = pathlib.Path(sys.argv[2])
image_package = pathlib.Path(sys.argv[3])
dtb_package = pathlib.Path(sys.argv[4])
evidence_path = pathlib.Path(sys.argv[5])
provenance_path = pathlib.Path(sys.argv[6])
armbian_build_directory = pathlib.Path(sys.argv[7]) if sys.argv[7] else None
handoff_directory = pathlib.Path(sys.argv[8]) if sys.argv[8] else None

manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
armbian = manifest["build_frameworks"]["armbian"]
orange = manifest["kernels"]["orange"]
accepted_patch = manifest["patches"]["accepted_upstream"]
follow_up_patch = manifest["patches"]["octessera_follow_up"]
expected_revision = "26.11.0-trunk.22"
expected_release = "6.18.46-current-sunxi64"
expected_source_repository = "https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git"
expected_source_branch = "linux-6.18.y"
expected_source_commit = "1f99e9ab748fc5c32120de9c4eca31abfe54a4d5"
expected_build_commit = "3da49cffcb8ac58a919d86816fec4659c410ff1e"
expected_build_tag = "v26.11.0-trunk.22"
expected_artifact_suffix = "6.18.46-S1f99-D7115-P25bc-C4e0c-H5530-HK01ba-Vc222-Bb84f-R448a"
expected_packages = [
    "linux-image-current-sunxi64_26.11.0-trunk.22_arm64.deb",
    "linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64.deb",
]
expected_architecture = expected_packages[0].rsplit("_", 1)[1].removesuffix(".deb")


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


source_lock = manifest.get("source_lock", {})
source_lock_path = source_lock.get("path")
source_lock_sha256 = source_lock.get("sha256")
if source_lock_path != "userpatches/config/sources/git_sources.json" or source_lock_sha256 != "e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30":
    raise SystemExit("Orange provenance manifest source-lock pin is not approved")
source_lock_file = root / source_lock_path
if not source_lock_file.is_file() or source_lock_file.is_symlink() or sha256(source_lock_file) != source_lock_sha256:
    raise SystemExit("Orange source-lock content does not match the manifest")
source_lock_document = json.loads(source_lock_file.read_text(encoding="utf-8"))
if source_lock_document != [{"source": expected_source_repository, "branch": expected_source_branch, "sha1": expected_source_commit}]:
    raise SystemExit("Orange source-lock entry is not the approved stable Linux source")
if armbian["commit"] != expected_build_commit or armbian.get("tag") != expected_build_tag or armbian["package_revision"] != expected_revision:
    raise SystemExit("Orange provenance manifest build pin is not the approved revision")
if orange["repository"] != expected_source_repository or orange.get("branch") != expected_source_branch or orange["commit"] != expected_source_commit or orange["release"] != expected_release:
    raise SystemExit("Orange provenance manifest source pin is not the approved release")
if armbian["packages"] != expected_packages or orange["packages"] != expected_packages:
    raise SystemExit("Orange provenance manifest package pair changed")
if armbian.get("native_artifact_suffix") != expected_artifact_suffix:
    raise SystemExit("Orange provenance manifest native artifact suffix changed")
if os.environ["ARMBIAN_BUILD_REF"].lower() != expected_build_commit:
    raise SystemExit("Armbian build ref is not the approved source pin")


def evidence_values(path: pathlib.Path) -> dict[str, str]:
    values = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if not separator or not key or key in values:
            raise SystemExit(f"Malformed Orange kernel evidence line: {line}")
        values[key] = value
    return values


evidence = evidence_values(evidence_path)
required_evidence = {
    "image_package_native_basename",
    "dtb_package_native_basename",
    "artifact_suffix",
    "image_package_sha256",
    "dtb_package_sha256",
    "image_dtb_sha256",
    "dtb_package_dtb_sha256",
    "dtb_byte_equal",
    "stock_i2c1_dtbo_path",
    "stock_i2c1_dtbo_sha256",
    "audio_dts_path",
    "audio_dts_sha256",
    "audio_dtbo_forbidden",
    "packaged_config_expected_sha256",
    "final_config_sha256",
    "module_relative_path",
    "module_compressed_sha256",
    "module_decompressed_sha256",
    "module_vermagic",
    "module_interface_string_marker",
    "module_interface_options_marker",
    "module_interface_runtime_marker",
}
if set(evidence) != required_evidence:
    raise SystemExit("Orange kernel evidence fields changed")
audio = armbian["audio_overlay"]
if evidence["audio_dts_path"] != audio["canonical_dts"] or evidence["audio_dts_sha256"] != audio["canonical_dts_sha256"] or evidence["audio_dtbo_forbidden"] != audio["dtbo_name"]:
    raise SystemExit("Orange audio evidence does not match the manifest")
expected_stock_path = f"boot/dtb-{armbian['kernel_release']}/allwinner/overlay/{audio['stock_i2c1_dtbo_name']}"
if evidence["stock_i2c1_dtbo_path"] != expected_stock_path or not re.fullmatch(r"[0-9a-f]{64}", evidence["stock_i2c1_dtbo_sha256"]):
    raise SystemExit("Orange stock i2c1-pi evidence does not match the manifest")
native_patterns = armbian["native_package_patterns"]
if len(native_patterns) != 2:
    raise SystemExit("Orange provenance native package patterns changed")
if not fnmatchcase(image_package.name, native_patterns[0]) or not fnmatchcase(dtb_package.name, native_patterns[1]):
    raise SystemExit("Orange native package names do not match the manifest patterns")
if evidence["image_package_native_basename"] != image_package.name:
    raise SystemExit("Orange native image package name does not match evidence")
if evidence["dtb_package_native_basename"] != dtb_package.name:
    raise SystemExit("Orange native DTB package name does not match evidence")
if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9+._-]*", evidence["artifact_suffix"]):
    raise SystemExit("Orange artifact suffix is invalid")
if evidence["artifact_suffix"] != expected_artifact_suffix:
    raise SystemExit("Orange artifact suffix is not the manifest-approved suffix")
native_image_prefix = expected_packages[0][:-4] + "__"
native_dtb_prefix = expected_packages[1][:-4] + "__"
if not image_package.name.startswith(native_image_prefix) or not image_package.name.endswith(".deb"):
    raise SystemExit("Orange native image package name is not based on the canonical package")
if not dtb_package.name.startswith(native_dtb_prefix) or not dtb_package.name.endswith(".deb"):
    raise SystemExit("Orange native DTB package name is not based on the canonical package")
if image_package.name[len(native_image_prefix):-4] != evidence["artifact_suffix"]:
    raise SystemExit("Orange native image artifact suffix does not match evidence")
if dtb_package.name[len(native_dtb_prefix):-4] != evidence["artifact_suffix"]:
    raise SystemExit("Orange native DTB artifact suffix does not match evidence")
if evidence["image_package_sha256"] != sha256(image_package):
    raise SystemExit("Orange image package SHA-256 does not match evidence")
if evidence["dtb_package_sha256"] != sha256(dtb_package):
    raise SystemExit("Orange DTB package SHA-256 does not match evidence")
if evidence["dtb_byte_equal"] != "true":
    raise SystemExit("Orange image and DTB package equality evidence is missing")
expected_packaged_config_sha256 = armbian.get("packaged_config_sha256")
if not isinstance(expected_packaged_config_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_packaged_config_sha256):
    raise SystemExit("Orange manifest packaged config SHA-256 is missing or invalid")
if evidence["packaged_config_expected_sha256"] != expected_packaged_config_sha256:
    raise SystemExit("Orange evidence packaged config expectation does not match the manifest")
for key in ("image_dtb_sha256", "dtb_package_dtb_sha256", "final_config_sha256", "module_compressed_sha256", "module_decompressed_sha256"):
    if not re.fullmatch(r"[0-9a-f]{64}", evidence[key]):
        raise SystemExit(f"Orange evidence hash is invalid: {key}")
if evidence["module_vermagic"] != expected_release and not evidence["module_vermagic"].startswith(expected_release + " "):
    raise SystemExit("Orange usb_f_midi vermagic does not match the ABI")
if evidence["module_interface_string_marker"] != "interface_string":
    raise SystemExit("Orange usb_f_midi interface_string marker is missing")
if evidence["module_interface_options_marker"] != "f_midi_opts_attr_interface_string":
    raise SystemExit("Orange usb_f_midi options interface marker is missing")
if evidence["module_interface_runtime_marker"] != "midi_interface_string":
    raise SystemExit("Orange usb_f_midi runtime interface marker is missing")
config_hash_match = evidence["final_config_sha256"] == expected_packaged_config_sha256
if not config_hash_match and os.environ.get("OCTESSERA_ORANGE_TEST_MODE") != "1":
    raise SystemExit("Orange packaged final config SHA-256 does not match the manifest")

patch_root = root / "userpatches/kernel/archive/sunxi-6.18"
patch_one = patch_root / "zzzz-0001-usb-gadget-f-midi-configfs-interface-string.patch"
patch_two = patch_root / "zzzz-0002-usb-gadget-f-midi-instance-local-string.patch"
if sha256(patch_one) != accepted_patch["sha256"] or sha256(patch_two) != follow_up_patch["sha256"]:
    raise SystemExit("Orange MIDI patch SHA-256 does not match the manifest")

if armbian_build_directory:
    actual_build_commit = subprocess.check_output(
        ["git", "-C", str(armbian_build_directory), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual_build_commit.lower() != expected_build_commit:
        raise SystemExit("Armbian checkout does not match the approved build pin")
    series_path = armbian_build_directory / armbian["core_series"]["path"]
    patching_path = armbian_build_directory / armbian["patching_order"]["source_path"]
    if sha256(series_path) != armbian["core_series"]["sha256"]:
        raise SystemExit("Armbian series.conf SHA-256 changed")
    if sha256(patching_path) != armbian["patching_order"]["source_sha256"]:
        raise SystemExit("Armbian patching-order source SHA-256 changed")

actual_octessera_head = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
if actual_octessera_head.lower() != os.environ["GITHUB_SOURCE_SHA"].lower():
    raise SystemExit("Octessera checkout HEAD does not match GITHUB_SOURCE_SHA")


armbian_checkout_head = None
armbian_checkout_path = None
source_lock_effective_path = None
source_lock_effective_sha256 = None
if armbian_build_directory:
    source_lock_build = armbian_build_directory / "config/sources/git_sources.json"
    if not source_lock_build.is_file() or source_lock_build.is_symlink() or source_lock_build.read_bytes() != source_lock_file.read_bytes():
        raise SystemExit("Armbian checkout source lock does not match the reviewed source lock")
    source_lock_effective_path = str(source_lock_build.relative_to(armbian_build_directory))
    source_lock_effective_sha256 = sha256(source_lock_build)
    armbian_checkout_head = subprocess.check_output(
        ["git", "-C", str(armbian_build_directory), "rev-parse", "HEAD"], text=True
    ).strip()
    armbian_checkout_path = str(armbian_build_directory)

handoff_values = {}
if handoff_directory:
    handoff_image = handoff_directory / expected_packages[0]
    handoff_dtb = handoff_directory / expected_packages[1]
    if not handoff_image.is_file() or not handoff_dtb.is_file():
        raise SystemExit("Orange release handoff is missing canonical package copies")
    if sha256(handoff_image) != sha256(image_package) or sha256(handoff_dtb) != sha256(dtb_package):
        raise SystemExit("Orange release handoff package copies do not match native packages")
    handoff_values = {
        "image_package_handoff": handoff_image.name,
        "image_package_handoff_sha256": sha256(handoff_image),
        "dtb_package_handoff": handoff_dtb.name,
        "dtb_package_handoff_sha256": sha256(handoff_dtb),
    }

lines = [
    "schema=1",
    f"github_source_sha={os.environ['GITHUB_SOURCE_SHA']}",
    f"octessera_checkout_head={actual_octessera_head}",
    f"armbian_build_ref={os.environ['ARMBIAN_BUILD_REF']}",
    f"armbian_build_tag={armbian['tag']}",
    f"armbian_build_repository={armbian['repository']}",
    f"kernel_source_repository={orange['repository']}",
    f"kernel_source_branch={orange['branch']}",
    f"kernel_source_commit={orange['commit']}",
    f"kernel_version={orange['release'].split('-')[0]}",
    f"kernel_release={orange['release']}",
    f"package_revision={armbian['package_revision']}",
    f"revision_argument={armbian['revision_argument']}",
    f"image_package={expected_packages[0]}",
    f"image_package_native={evidence['image_package_native_basename']}",
    f"image_package_sha256={evidence['image_package_sha256']}",
    f"dtb_package={expected_packages[1]}",
    f"dtb_package_native={evidence['dtb_package_native_basename']}",
    f"dtb_package_sha256={evidence['dtb_package_sha256']}",
    f"artifact_suffix={evidence['artifact_suffix']}",
    f"source_lock_path={source_lock_path}",
    f"source_lock_sha256={source_lock_sha256}",
    f"source_lock_source={expected_source_repository}",
    f"source_lock_branch={expected_source_branch}",
    f"source_lock_commit={expected_source_commit}",
    f"package_architecture={expected_architecture}",
    f"required_dtb={armbian['required_dtb']}",
    f"audio_dts_path={evidence['audio_dts_path']}",
    f"audio_dts_sha256={evidence['audio_dts_sha256']}",
    f"audio_dtbo_forbidden={evidence['audio_dtbo_forbidden']}",
    f"kernel_config_path={armbian['config_base']['path']}",
    f"kernel_config_source_sha256={armbian['config_base']['sha256']}",
    f"kernel_config_expected_packaged_sha256={expected_packaged_config_sha256}",
    f"kernel_config_final_sha256={evidence['final_config_sha256']}",
    f"kernel_config_sha256_match={str(config_hash_match).lower()}",
    f"image_dtb_sha256={evidence['image_dtb_sha256']}",
    f"dtb_package_dtb_sha256={evidence['dtb_package_dtb_sha256']}",
    f"stock_i2c1_dtbo_path={evidence['stock_i2c1_dtbo_path']}",
    f"stock_i2c1_dtbo_sha256={evidence['stock_i2c1_dtbo_sha256']}",
    f"core_series_path={armbian['core_series']['path']}",
    f"core_series_sha256={armbian['core_series']['sha256']}",
    f"core_series_active_patch_count={armbian['core_series']['active_patch_count']}",
    f"patching_order_source_path={armbian['patching_order']['source_path']}",
    f"patching_order_source_sha256={armbian['patching_order']['source_sha256']}",
    "accepted_upstream_patch=zzzz-0001-usb-gadget-f-midi-configfs-interface-string.patch",
    f"accepted_upstream_commit={accepted_patch['commit']}",
    f"accepted_upstream_patch_sha256={accepted_patch['sha256']}",
    "octessera_follow_up_patch=zzzz-0002-usb-gadget-f-midi-instance-local-string.patch",
    f"octessera_follow_up_patch_sha256={follow_up_patch['sha256']}",
    f"usb_f_midi_module={evidence['module_relative_path']}",
    f"usb_f_midi_module_compressed_sha256={evidence['module_compressed_sha256']}",
    f"usb_f_midi_module_decompressed_sha256={evidence['module_decompressed_sha256']}",
    f"usb_f_midi_vermagic={evidence['module_vermagic']}",
    "usb_f_midi_interface_string_marker=interface_string",
    "usb_f_midi_interface_options_marker=f_midi_opts_attr_interface_string",
    "usb_f_midi_interface_runtime_marker=midi_interface_string",
    f"evidence_sha256={sha256(evidence_path)}",
]
if armbian_build_directory:
    lines[4:4] = [
        f"armbian_checkout_path={armbian_checkout_path}",
        f"armbian_checkout_head={armbian_checkout_head}",
        f"source_lock_effective_path={source_lock_effective_path}",
        f"source_lock_effective_sha256={source_lock_effective_sha256}",
    ]
lines.extend(f"{key}={value}" for key, value in handoff_values.items())
provenance_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
PY

cat -- "$provenance_file"
