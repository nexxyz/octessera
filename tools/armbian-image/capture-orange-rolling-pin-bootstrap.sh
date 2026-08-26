#!/usr/bin/env bash
set -euo pipefail

expected_framework_sha=3da49cffcb8ac58a919d86816fec4659c410ff1e
candidate_revision=26.11.0-trunk.22

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <armbian-build-directory> <captured-candidate-lock> <evidence-directory>" >&2
  exit 2
fi

build_input="$1"
captured_lock_input="$2"
evidence_input="$3"
[[ -d "$build_input" && ! -L "$build_input" ]] || { echo 'Armbian build directory is missing or symlinked.' >&2; exit 1; }
build_dir="$(realpath -e -- "$build_input")"
[[ -f "$build_dir/compile.sh" && ! -L "$build_dir/compile.sh" ]] || { echo 'Armbian compile.sh is missing or symlinked.' >&2; exit 1; }
[[ -f "$captured_lock_input" && ! -L "$captured_lock_input" ]] || { echo 'Captured candidate source lock is missing or symlinked.' >&2; exit 1; }
captured_lock="$(realpath -e -- "$captured_lock_input")"
case "$captured_lock" in
  "$build_dir/"*) echo 'Captured candidate source lock must be outside the Armbian build directory.' >&2; exit 1 ;;
esac

output_root="$build_dir/output"
[[ -d "$output_root" && ! -L "$output_root" ]] || { echo 'Armbian output directory is missing or symlinked.' >&2; exit 1; }
evidence_parent_input="$(dirname -- "$evidence_input")"
evidence_name="$(basename -- "$evidence_input")"
[[ "$evidence_name" =~ ^[A-Za-z0-9._-]+$ && "$evidence_name" != . && "$evidence_name" != .. ]] || { echo 'Evidence directory name is unsafe.' >&2; exit 1; }
[[ -d "$evidence_parent_input" && ! -L "$evidence_parent_input" ]] || { echo 'Evidence directory parent is missing or symlinked.' >&2; exit 1; }
evidence_parent="$(realpath -e -- "$evidence_parent_input")"
evidence_dir="$evidence_parent/$evidence_name"
[[ "$evidence_dir" == "$output_root/"* ]] || { echo 'Evidence directory must be under Armbian output.' >&2; exit 1; }
[[ ! -e "$evidence_dir" && ! -L "$evidence_dir" ]] || { echo 'Evidence directory already exists.' >&2; exit 1; }

actual_framework_head="$(git -C "$build_dir" rev-parse HEAD 2>/dev/null)" || { echo 'Unable to read Armbian framework HEAD.' >&2; exit 1; }
[[ "$actual_framework_head" == "$expected_framework_sha" ]] || { echo "Unexpected Armbian framework HEAD: $actual_framework_head" >&2; exit 1; }
effective_source_lock="$build_dir/config/sources/git_sources.json"
[[ -f "$effective_source_lock" && ! -L "$effective_source_lock" ]] || { echo 'Effective candidate source lock is missing or symlinked.' >&2; exit 1; }
if ! cmp -- "$captured_lock" "$effective_source_lock"; then
  echo 'Captured and effective candidate source locks differ.' >&2
  exit 1
fi

mkdir -- "$evidence_dir"
sha256_value() {
  sha256sum -- "$1" | awk '{print $1}'
}

printf '%s\n' \
  'framework_tag=v26.11.0-trunk.22' \
  "expected_framework_sha=$expected_framework_sha" \
  "actual_framework_head=$actual_framework_head" > "$evidence_dir/framework.txt"
printf '%s\n' \
  'board=orangepizero2w' \
  'release=trixie' \
  'branch=current' \
  "revision=$candidate_revision" \
  'kernelbranch_argument=omitted' > "$evidence_dir/build-tuple.env"
cp -- "$captured_lock" "$evidence_dir/captured-candidate-source-lock.json"
cp -- "$effective_source_lock" "$evidence_dir/effective-source-lock.json"
printf '%s\n' \
  'source_lock_equal=true' \
  "captured_source_lock_sha256=$(sha256_value "$captured_lock")" \
  "effective_source_lock_sha256=$(sha256_value "$effective_source_lock")" > "$evidence_dir/source-lock.env"

mapfile -t image_packages < <(find -P "$output_root/debs" -maxdepth 1 -type f -name 'linux-image-*__*.deb' -print | LC_ALL=C sort)
mapfile -t dtb_packages < <(find -P "$output_root/debs" -maxdepth 1 -type f -name 'linux-dtb-*__*.deb' -print | LC_ALL=C sort)
[[ "${#image_packages[@]}" == 1 && "${#dtb_packages[@]}" == 1 ]] || { echo 'Expected exactly one native linux-image and linux-dtb package.' >&2; exit 1; }
image_package="${image_packages[0]}"
dtb_package="${dtb_packages[0]}"
image_basename="$(basename -- "$image_package")"
dtb_basename="$(basename -- "$dtb_package")"
[[ "$image_basename" =~ ^linux-image-[A-Za-z0-9+._-]+__([A-Za-z0-9+._-]+)\.deb$ && "$dtb_basename" =~ ^linux-dtb-[A-Za-z0-9+._-]+__([A-Za-z0-9+._-]+)\.deb$ ]] || { echo 'Native package basename is unsafe.' >&2; exit 1; }
image_suffix="${image_basename##*__}"
dtb_suffix="${dtb_basename##*__}"
image_suffix="${image_suffix%.deb}"
dtb_suffix="${dtb_suffix%.deb}"
[[ -n "$image_suffix" && "$image_suffix" == "$dtb_suffix" ]] || { echo 'Native package artifact suffixes do not match.' >&2; exit 1; }
image_name="$(dpkg-deb -f "$image_package" Package)"
image_version="$(dpkg-deb -f "$image_package" Version)"
image_architecture="$(dpkg-deb -f "$image_package" Architecture)"
dtb_name="$(dpkg-deb -f "$dtb_package" Package)"
dtb_version="$(dpkg-deb -f "$dtb_package" Version)"
dtb_architecture="$(dpkg-deb -f "$dtb_package" Architecture)"
[[ "$image_name" == linux-image-* && "$dtb_name" == linux-dtb-* ]] || { echo 'Discovered packages are not native linux-image/linux-dtb packages.' >&2; exit 1; }
[[ "$image_version" == "$candidate_revision" && "$dtb_version" == "$candidate_revision" ]] || { echo 'Native package versions do not match the candidate revision.' >&2; exit 1; }
[[ "$image_architecture" == arm64 && "$dtb_architecture" == arm64 ]] || { echo 'Native packages are not arm64.' >&2; exit 1; }

dpkg-deb -f "$image_package" > "$evidence_dir/$image_basename.dpkg-deb.txt"
dpkg-deb -f "$dtb_package" > "$evidence_dir/$dtb_basename.dpkg-deb.txt"
printf '%s\n' \
  "linux_image_package_basename=$image_basename" \
  "linux_dtb_package_basename=$dtb_basename" \
  "linux_image_package=$image_name" \
  "linux_dtb_package=$dtb_name" \
  "artifact_suffix=$image_suffix" \
  "linux_image_version=$image_version" \
  "linux_dtb_version=$dtb_version" \
  "linux_image_architecture=$image_architecture" \
  "linux_dtb_architecture=$dtb_architecture" \
  "linux_image_package_sha256=$(sha256_value "$image_package")" \
  "linux_dtb_package_sha256=$(sha256_value "$dtb_package")" > "$evidence_dir/native-package.env"

extract_root="$(mktemp -d)"
cleanup() {
  rm -rf -- "$extract_root"
}
trap cleanup EXIT
dpkg-deb -x "$image_package" "$extract_root/image"
mapfile -t packaged_configs < <(find -P "$extract_root/image/boot" -maxdepth 1 -type f -name 'config-*' -print | LC_ALL=C sort)
[[ "${#packaged_configs[@]}" == 1 ]] || { echo 'Expected exactly one packaged boot/config-* file.' >&2; exit 1; }
config_path="${packaged_configs[0]}"
config_basename="$(basename -- "$config_path")"
[[ "$config_basename" =~ ^config-[A-Za-z0-9._+-]+$ ]] || { echo 'Packaged kernel config basename is unsafe.' >&2; exit 1; }
config_abi="${config_basename#config-}"
mapfile -t module_directories < <(find -P "$extract_root/image/lib/modules" -mindepth 1 -maxdepth 1 -type d -print | LC_ALL=C sort)
[[ "${#module_directories[@]}" == 1 ]] || { echo 'Expected exactly one packaged kernel module ABI directory.' >&2; exit 1; }
module_abi="$(basename -- "${module_directories[0]}")"
[[ "$module_abi" == "$config_abi" ]] || { echo 'Packaged config and module ABI names differ.' >&2; exit 1; }
armbian_kernel_version="$(dpkg-deb -f "$image_package" Armbian-Kernel-Version)"
armbian_kernel_release="$(dpkg-deb -f "$image_package" Armbian-Kernel-Version-Family)"
[[ -n "$armbian_kernel_version" && -n "$armbian_kernel_release" ]] || { echo 'Native package kernel ABI/release metadata is incomplete.' >&2; exit 1; }
printf '%s\n' \
  "packaged_config_path=boot/$config_basename" \
  "packaged_config_sha256=$(sha256_value "$config_path")" \
  "packaged_kernel_abi=$config_abi" \
  "packaged_module_abi=$module_abi" \
  "armbian_kernel_version=$armbian_kernel_version" \
  "armbian_kernel_release_family=$armbian_kernel_release" > "$evidence_dir/packaged-kernel.env"

mapfile -t image_artifacts < <(find -P "$output_root/images" -maxdepth 1 -type f -name '*.img.xz' -print | LC_ALL=C sort)
mapfile -t image_checksums < <(find -P "$output_root/images" -maxdepth 1 -type f -name '*.img.xz.sha' -print | LC_ALL=C sort)
[[ "${#image_artifacts[@]}" == 1 && "${#image_checksums[@]}" == 1 ]] || { echo 'Expected exactly one .img.xz and one .img.xz.sha.' >&2; exit 1; }
image_artifact="${image_artifacts[0]}"
image_checksum="${image_checksums[0]}"
image_artifact_basename="$(basename -- "$image_artifact")"
image_checksum_basename="$(basename -- "$image_checksum")"
[[ "$image_artifact_basename" =~ ^[A-Za-z0-9._+-]+\.img\.xz$ && "$image_checksum_basename" == "$image_artifact_basename.sha" ]] || { echo 'Image and checksum basenames do not match safely.' >&2; exit 1; }
(
  cd -- "$(dirname -- "$image_checksum")" || exit 1
  sha256sum -c "$image_checksum_basename"
)
printf '%s\n' \
  "image_basename=$image_artifact_basename" \
  "image_sha256=$(sha256_value "$image_artifact")" \
  "checksum_basename=$image_checksum_basename" \
  "checksum_sha256=$(sha256_value "$image_checksum")" > "$evidence_dir/image.env"

manifest="$evidence_dir/SHA256SUMS"
(
  cd -- "$output_root" || exit 1
  sha256sum -- \
    "bootstrap-evidence/framework.txt" \
    "bootstrap-evidence/build-tuple.env" \
    "bootstrap-evidence/captured-candidate-source-lock.json" \
    "bootstrap-evidence/effective-source-lock.json" \
    "bootstrap-evidence/source-lock.env" \
    "bootstrap-evidence/native-package.env" \
    "bootstrap-evidence/packaged-kernel.env" \
    "bootstrap-evidence/image.env" \
    "bootstrap-evidence/$image_basename.dpkg-deb.txt" \
    "bootstrap-evidence/$dtb_basename.dpkg-deb.txt" \
    "debs/$image_basename" \
    "debs/$dtb_basename" \
    "images/$image_artifact_basename" \
    "images/$image_checksum_basename" > "$manifest"
  sha256sum -c "bootstrap-evidence/SHA256SUMS"
)

printf '%s\n' "Captured rolling-pin bootstrap evidence: $evidence_dir"
