#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <debs-directory>" >&2
  exit 2
fi

debs_directory="$1"
[[ -d "$debs_directory" ]] || { echo "Missing Armbian package directory: $debs_directory" >&2; exit 1; }
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$root/tools/kernel-patches/orange-midi-interface-manifest.json"
[[ -f "$manifest" ]] || { echo "Missing Orange kernel package manifest: $manifest" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "python3 is required to read the Orange kernel package manifest." >&2; exit 1; }

mapfile -t native_patterns < <(python3 - "$manifest" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    patterns = json.load(handle)["build_frameworks"]["armbian"]["native_package_patterns"]
if len(patterns) != 2:
    raise SystemExit("Orange native package patterns must contain exactly two entries")
if not patterns[0].startswith("linux-image-") or not patterns[1].startswith("linux-dtb-"):
    raise SystemExit("Orange native package patterns must be image then DTB")
print(patterns[0])
print(patterns[1])
PY
)
[[ "${#native_patterns[@]}" == 2 ]] || { echo "Unable to read the Orange native package patterns." >&2; exit 1; }

mapfile -t image_packages < <(find "$debs_directory" -maxdepth 1 -type f -name "${native_patterns[0]}" -print | LC_ALL=C sort)
mapfile -t dtb_packages < <(find "$debs_directory" -maxdepth 1 -type f -name "${native_patterns[1]}" -print | LC_ALL=C sort)

[[ "${#image_packages[@]}" == 1 ]] || {
  echo "Expected exactly one Orange linux-image package, found ${#image_packages[@]}." >&2
  exit 1
}
[[ "${#dtb_packages[@]}" == 1 ]] || {
  echo "Expected exactly one Orange linux-dtb package, found ${#dtb_packages[@]}." >&2
  exit 1
}

image_basename="$(basename -- "${image_packages[0]}")"
dtb_basename="$(basename -- "${dtb_packages[0]}")"
image_prefix="${image_basename%%__*.deb}"
dtb_prefix="${dtb_basename%%__*.deb}"
[[ "$image_basename" == "${image_prefix}__"*.deb && "$dtb_basename" == "${dtb_prefix}__"*.deb ]] || {
  echo "Orange native package names do not contain artifact suffixes." >&2
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
  echo "Orange linux-image and linux-dtb artifact suffixes differ: $image_suffix / $dtb_suffix." >&2
  exit 1
}

printf '%s\n' "${image_packages[0]}" "${dtb_packages[0]}"
