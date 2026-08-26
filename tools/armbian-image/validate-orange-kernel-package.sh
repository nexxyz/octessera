#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <linux-image.deb> <linux-dtb.deb> [--evidence-output <file>] [--expected-config-sha256 <sha256>] [--manifest <test-manifest>]" >&2
  exit 2
fi

image_package="$1"
dtb_package="$2"
shift 2
evidence_output=
expected_config_sha256=
manifest_override=
while [[ $# -gt 0 ]]; do
  case "$1" in
    --evidence-output)
      [[ $# -ge 2 ]] || { echo "--evidence-output requires a path." >&2; exit 2; }
      evidence_output="$2"
      shift 2
      ;;
    --expected-config-sha256)
      [[ $# -ge 2 ]] || { echo "--expected-config-sha256 requires a SHA-256." >&2; exit 2; }
      expected_config_sha256="$2"
      shift 2
      ;;
    --manifest)
      [[ $# -ge 2 ]] || { echo "--manifest requires a path." >&2; exit 2; }
      manifest_override="$2"
      shift 2
      ;;
    *)
      echo "Unexpected validator argument: $1" >&2
      exit 2
      ;;
  esac
done

[[ -f "$image_package" ]] || { echo "Missing linux-image package: $image_package" >&2; exit 1; }
[[ -f "$dtb_package" ]] || { echo "Missing linux-dtb package: $dtb_package" >&2; exit 1; }
command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb is required for Orange kernel package validation." >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "sha256sum is required for Orange kernel package validation." >&2; exit 1; }
command -v strings >/dev/null 2>&1 || { echo "strings is required for Orange kernel package validation." >&2; exit 1; }
command -v readelf >/dev/null 2>&1 || { echo "readelf is required for Orange kernel package validation." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "python3 is required for Orange kernel package validation." >&2; exit 1; }
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
manifest="${manifest_override:-$root/tools/kernel-patches/orange-midi-interface-manifest.json}"
[[ -f "$manifest" ]] || { echo "Missing Orange kernel package manifest: $manifest" >&2; exit 1; }
if [[ -n "$manifest_override" && "${OCTESSERA_ORANGE_TEST_MODE:-}" != 1 ]]; then
  echo "--manifest is test-only." >&2
  exit 2
fi

audio_contract_values=()
mapfile -t audio_contract_values < <(python3 - "$manifest" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    audio = json.load(handle)["build_frameworks"]["armbian"]["audio_overlay"]
print(audio["canonical_dts"])
print(audio["canonical_dts_sha256"])
print(audio["dtbo_name"])
print(audio["stock_i2c1_dtbo_name"])
for line in audio["required_builtin_config_lines"]:
    print(line)
PY
)
[[ "${#audio_contract_values[@]}" == 13 ]] || { echo "Orange audio kernel package manifest contract is incomplete." >&2; exit 1; }
audio_dts_relative="${audio_contract_values[0]}"
audio_dts_sha256_expected="${audio_contract_values[1]}"
audio_dtbo_name="${audio_contract_values[2]}"
stock_i2c1_dtbo_name="${audio_contract_values[3]}"
audio_dts_path="$root/$audio_dts_relative"
[[ -f "$audio_dts_path" && ! -L "$audio_dts_path" ]] || { echo "Canonical Orange audio DTS is missing or symlinked." >&2; exit 1; }
[[ "$(sha256sum -- "$audio_dts_path" | awk '{print $1}')" == "$audio_dts_sha256_expected" ]] || { echo "Canonical Orange audio DTS hash mismatch." >&2; exit 1; }
[[ "$audio_dtbo_name" == octessera-ahub0-pcm5102.dtbo && "$stock_i2c1_dtbo_name" == sun50i-h616-i2c1-pi.dtbo ]] || { echo "Orange audio DTBO identity is not canonical." >&2; exit 1; }

mapfile -t contract_values < <(python3 - "$manifest" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    manifest = json.load(handle)
armbian = manifest["build_frameworks"]["armbian"]
print(armbian["packages"][0])
print(armbian["packages"][1])
print(armbian["native_package_patterns"][0])
print(armbian["native_package_patterns"][1])
print(armbian["package_revision"])
print(armbian["kernel_release"])
print(armbian["required_dtb"])
print(armbian["required_module"])
print(armbian["packaged_config_sha256"])
PY
)
[[ "${#contract_values[@]}" == 9 ]] || { echo "Orange kernel package manifest contract is incomplete." >&2; exit 1; }
expected_image_filename="${contract_values[0]}"
expected_dtb_filename="${contract_values[1]}"
native_image_pattern="${contract_values[2]}"
native_dtb_pattern="${contract_values[3]}"
expected_package_version="${contract_values[4]}"
expected_kernel_release="${contract_values[5]}"
expected_kernel_version="${expected_kernel_release%%-*}"
expected_dtb="${contract_values[6]}"
expected_module="${contract_values[7]}"
manifest_packaged_config_sha256="${contract_values[8]}"
[[ "$manifest_packaged_config_sha256" =~ ^[[:xdigit:]]{64}$ ]] || { echo "Manifest packaged config SHA-256 is invalid." >&2; exit 1; }
if [[ -n "$expected_config_sha256" ]]; then
  [[ "$expected_config_sha256" =~ ^[[:xdigit:]]{64}$ ]] || { echo "Expected config SHA-256 is invalid." >&2; exit 2; }
  if [[ "${expected_config_sha256,,}" != "${manifest_packaged_config_sha256,,}" && "${OCTESSERA_ORANGE_TEST_MODE:-}" != 1 ]]; then
    echo "Expected config SHA-256 must equal the manifest packaged config SHA-256." >&2
    exit 2
  fi
else
  expected_config_sha256="$manifest_packaged_config_sha256"
fi
expected_image_name="${expected_image_filename%%_*}"
expected_dtb_name="${expected_dtb_filename%%_*}"
expected_architecture="${expected_image_filename%.deb}"
expected_architecture="${expected_architecture##*_}"

image_basename="$(basename -- "$image_package")"
dtb_basename="$(basename -- "$dtb_package")"
image_prefix="${expected_image_filename%.deb}"
dtb_prefix="${expected_dtb_filename%.deb}"
manifest_pattern_matches() {
  python3 - "$1" "$2" <<'PY'
import fnmatch
import sys

raise SystemExit(0 if fnmatch.fnmatchcase(sys.argv[1], sys.argv[2]) else 1)
PY
}
manifest_pattern_matches "$image_basename" "$native_image_pattern" || {
  echo "Unexpected native linux-image package filename: $image_basename" >&2
  exit 1
}
manifest_pattern_matches "$dtb_basename" "$native_dtb_pattern" || {
  echo "Unexpected native linux-dtb package filename: $dtb_basename" >&2
  exit 1
}
image_suffix="${image_basename#"${image_prefix}"__}"
dtb_suffix="${dtb_basename#"${dtb_prefix}"__}"
image_suffix="${image_suffix%.deb}"
dtb_suffix="${dtb_suffix%.deb}"
[[ "$image_suffix" =~ ^[A-Za-z0-9][A-Za-z0-9+._-]*$ && "$dtb_suffix" =~ ^[A-Za-z0-9][A-Za-z0-9+._-]*$ ]] || {
  echo "Orange native package artifact suffix is empty or invalid." >&2
  exit 1
}
[[ "$image_suffix" == "$dtb_suffix" ]] || {
  echo "Orange native package artifact suffixes differ: $image_suffix / $dtb_suffix" >&2
  exit 1
}

image_name="$(dpkg-deb -f "$image_package" Package)"
image_version="$(dpkg-deb -f "$image_package" Version)"
image_architecture="$(dpkg-deb -f "$image_package" Architecture)"
image_source="$(dpkg-deb -f "$image_package" Source)"
image_kernel_version="$(dpkg-deb -f "$image_package" Armbian-Kernel-Version)"
image_kernel_release="$(dpkg-deb -f "$image_package" Armbian-Kernel-Version-Family)"
dtb_name="$(dpkg-deb -f "$dtb_package" Package)"
dtb_version="$(dpkg-deb -f "$dtb_package" Version)"
dtb_architecture="$(dpkg-deb -f "$dtb_package" Architecture)"

[[ "$image_name" == "$expected_image_name" ]] || { echo "Unexpected linux-image package: $image_name" >&2; exit 1; }
[[ "$dtb_name" == "$expected_dtb_name" ]] || { echo "Unexpected linux-dtb package: $dtb_name" >&2; exit 1; }
[[ "$image_version" == "$expected_package_version" && "$dtb_version" == "$expected_package_version" ]] || {
  echo "Orange kernel package versions must both be $expected_package_version." >&2
  exit 1
}
[[ "$image_architecture" == "$expected_architecture" && "$dtb_architecture" == "$expected_architecture" ]] || {
  echo "Orange kernel packages must both have $expected_architecture architecture." >&2
  exit 1
}
[[ "$image_source" == "linux-$expected_kernel_version" ]] || { echo "Unexpected Linux source version: $image_source" >&2; exit 1; }
[[ "$image_kernel_version" == "$expected_kernel_version" ]] || { echo "Unexpected Armbian kernel version: $image_kernel_version" >&2; exit 1; }
[[ "$image_kernel_release" == "$expected_kernel_release" ]] || { echo "Unexpected Armbian kernel release: $image_kernel_release" >&2; exit 1; }

work="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work"
}
trap cleanup EXIT

dpkg-deb -x "$image_package" "$work/image"
dpkg-deb -x "$dtb_package" "$work/dtb"
image_root="$work/image"
dtb_root="$work/dtb"

if find "$image_root" "$dtb_root" -type f -iname '*octessera-ahub0-pcm5102*' -print -quit | grep -q .; then
  echo "Orange kernel packages must not embed the Octessera audio DTBO." >&2
  exit 1
fi

mapfile -t configs < <(find "$image_root/boot" -maxdepth 1 -type f -name 'config-*' -print 2>/dev/null | LC_ALL=C sort)
[[ "${#configs[@]}" == 1 ]] || { echo "Expected exactly one packaged kernel config." >&2; exit 1; }
config="${configs[0]}"
expected_config="$image_root/boot/config-$expected_kernel_release"
[[ "$config" == "$expected_config" ]] || { echo "Unexpected packaged kernel config: $(basename -- "$config")" >&2; exit 1; }
config_sha256="$(sha256sum -- "$config" | awk '{print $1}')"
if [[ -n "$expected_config_sha256" && "${config_sha256,,}" != "${expected_config_sha256,,}" ]]; then
  echo "Packaged kernel config SHA-256 mismatch: $config_sha256" >&2
  exit 1
fi

assert_config_line_once() {
  local expected="$1"
  local count
  count="$(grep -cFx -- "$expected" "$config" || true)"
  [[ "$count" == 1 ]] || {
    echo "Packaged kernel config must contain exactly one: $expected" >&2
    exit 1
  }
}

assert_config_line_once '# CONFIG_RT_GROUP_SCHED is not set'
octessera_reject_file_match "Packaged kernel config must not enable or modularize RT_GROUP_SCHED." -qE '^CONFIG_RT_GROUP_SCHED=' "$config"
assert_config_line_once 'CONFIG_SPI_SUN6I=y'
assert_config_line_once 'CONFIG_SPI_SPIDEV=y'
assert_config_line_once 'CONFIG_PINCTRL_SUNXI=y'
assert_config_line_once 'CONFIG_MMC=y'
assert_config_line_once 'CONFIG_MMC_BLOCK=y'
mapfile -t mmc_spi_config < <(grep -E '^CONFIG_MMC_SPI=[ym]$' "$config" || true)
[[ "${#mmc_spi_config[@]}" == 1 ]] || { echo 'Packaged kernel config must contain exactly one enabled CONFIG_MMC_SPI.' >&2; exit 1; }
assert_config_line_once 'CONFIG_SND_SEQUENCER=m'
assert_config_line_once 'CONFIG_SND_RAWMIDI=m'
assert_config_line_once 'CONFIG_SND_USB_AUDIO=m'
for audio_config_line in "${audio_contract_values[@]:4}"; do
  assert_config_line_once "$audio_config_line"
done

assert_module_file() {
  local module="$1"
  local module_root="$image_root/lib/modules/$expected_kernel_release"
  local -a modules=()
  mapfile -t modules < <(find "$module_root" -type f \( -name "$module" -o -name "$module.*" \) -print 2>/dev/null | LC_ALL=C sort)
  [[ "${#modules[@]}" == 1 ]] || { echo "Expected exactly one packaged $module module." >&2; exit 1; }
}

assert_module_file snd-seq.ko
assert_module_file snd-seq-midi.ko
assert_module_file snd-rawmidi.ko
assert_module_file snd-usb-audio.ko
if [[ "${mmc_spi_config[0]#*=}" == m ]]; then
  assert_module_file mmc_spi.ko
else
  if find "$image_root/lib/modules/$expected_kernel_release" -type f \( -name 'mmc_spi.ko' -o -name 'mmc_spi.ko.*' \) -print -quit | grep -q .; then
    echo 'Built-in CONFIG_MMC_SPI package must not contain an mmc_spi module.' >&2
    exit 1
  fi
fi

image_dtb="$image_root/usr/lib/linux-image-$expected_kernel_release/allwinner/$expected_dtb"
package_dtb="$dtb_root/boot/dtb-$expected_kernel_release/allwinner/$expected_dtb"
[[ -s "$image_dtb" ]] || { echo "Required Zero 2W DTB is missing from linux-image." >&2; exit 1; }
[[ -s "$package_dtb" ]] || { echo "Required Zero 2W DTB is missing from linux-dtb." >&2; exit 1; }
stock_i2c1_dtbo="$dtb_root/boot/dtb-$expected_kernel_release/allwinner/overlay/$stock_i2c1_dtbo_name"
[[ -f "$stock_i2c1_dtbo" && ! -L "$stock_i2c1_dtbo" ]] || { echo "Required stock i2c1-pi DTBO is missing or symlinked from linux-dtb." >&2; exit 1; }
stock_i2c1_dtbo_sha256="$(sha256sum -- "$stock_i2c1_dtbo" | awk '{print $1}')"
fdt_magic() {
  local value
  value="$(od -An -tx1 -N4 -- "$1" | tr -d '[:space:]')"
  [[ "$value" == d00dfeed ]] || {
    echo "Invalid FDT magic in $1: $value" >&2
    exit 1
  }
}
fdt_magic "$image_dtb"
fdt_magic "$package_dtb"
image_dtb_sha256="$(sha256sum -- "$image_dtb" | awk '{print $1}')"
dtb_package_dtb_sha256="$(sha256sum -- "$package_dtb" | awk '{print $1}')"
cmp -- "$image_dtb" "$package_dtb" || {
  echo "linux-image and linux-dtb contain different Zero 2W DTBs." >&2
  exit 1
}

module_root="$image_root/lib/modules/$expected_kernel_release"
mapfile -t midi_modules < <(
  find "$module_root" -type f \( -name "$expected_module" -o -name "$expected_module.*" \) -print 2>/dev/null | LC_ALL=C sort
)
[[ "${#midi_modules[@]}" == 1 ]] || { echo "Expected exactly one packaged $expected_module module." >&2; exit 1; }

module_evidence="$(ORANGE_KERNEL_MODULE_ROOT="$image_root/" bash "$(dirname "${BASH_SOURCE[0]}")/inspect-orange-kernel-module.sh" "${midi_modules[0]}" "$expected_kernel_release")"
module_relative_path=
module_compressed_sha256=
module_decompressed_sha256=
module_vermagic=
module_interface_string_marker=
module_interface_options_marker=
module_interface_runtime_marker=
while IFS='=' read -r key value; do
  case "$key" in
    module_relative_path) module_relative_path="$value" ;;
    module_compressed_sha256) module_compressed_sha256="$value" ;;
    module_decompressed_sha256) module_decompressed_sha256="$value" ;;
    module_vermagic) module_vermagic="$value" ;;
    module_interface_string_marker) module_interface_string_marker="$value" ;;
    module_interface_options_marker) module_interface_options_marker="$value" ;;
    module_interface_runtime_marker) module_interface_runtime_marker="$value" ;;
    *) echo "Unexpected usb_f_midi evidence field: $key" >&2; exit 1 ;;
  esac
done <<< "$module_evidence"
[[ -n "$module_relative_path" && -n "$module_compressed_sha256" && -n "$module_decompressed_sha256" && -n "$module_interface_string_marker" && -n "$module_interface_options_marker" && -n "$module_interface_runtime_marker" ]] || {
  echo "usb_f_midi module evidence is incomplete." >&2
  exit 1
}
[[ "$module_interface_string_marker" == interface_string && "$module_interface_options_marker" == f_midi_opts_attr_interface_string && "$module_interface_runtime_marker" == midi_interface_string ]] || {
  echo "usb_f_midi module interface string evidence is incomplete." >&2
  exit 1
}

if [[ -n "$evidence_output" ]]; then
  mkdir -p -- "$(dirname "$evidence_output")"
  evidence_tmp="$work/evidence.env"
  printf '%s\n' \
    "image_package_native_basename=$image_basename" \
    "dtb_package_native_basename=$dtb_basename" \
    "artifact_suffix=$image_suffix" \
    "image_package_sha256=$(sha256sum -- "$image_package" | awk '{print $1}')" \
    "dtb_package_sha256=$(sha256sum -- "$dtb_package" | awk '{print $1}')" \
    "image_dtb_sha256=$image_dtb_sha256" \
    "dtb_package_dtb_sha256=$dtb_package_dtb_sha256" \
    "dtb_byte_equal=true" \
    "stock_i2c1_dtbo_path=boot/dtb-$expected_kernel_release/allwinner/overlay/$stock_i2c1_dtbo_name" \
    "stock_i2c1_dtbo_sha256=$stock_i2c1_dtbo_sha256" \
    "audio_dts_path=$audio_dts_relative" \
    "audio_dts_sha256=$audio_dts_sha256_expected" \
    "audio_dtbo_forbidden=$audio_dtbo_name" \
    "packaged_config_expected_sha256=$manifest_packaged_config_sha256" \
    "final_config_sha256=$config_sha256" \
    "module_relative_path=$module_relative_path" \
    "module_compressed_sha256=$module_compressed_sha256" \
    "module_decompressed_sha256=$module_decompressed_sha256" \
    "module_vermagic=$module_vermagic" \
    "module_interface_string_marker=$module_interface_string_marker" \
    "module_interface_options_marker=$module_interface_options_marker" \
    "module_interface_runtime_marker=$module_interface_runtime_marker" > "$evidence_tmp"
  mv -f -- "$evidence_tmp" "$evidence_output"
fi

printf 'Orange linux-image and linux-dtb package validation passed: %s %s\n' "$image_package" "$dtb_package"
