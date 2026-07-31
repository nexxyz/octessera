#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <usb_f_midi-module> <expected-kernel-release>" >&2
  exit 2
fi

module="$1"
expected_release="$2"
[[ -f "$module" ]] || { echo "Missing usb_f_midi module: $module" >&2; exit 1; }
command -v readelf >/dev/null 2>&1 || { echo "readelf is required for usb_f_midi validation." >&2; exit 1; }
command -v strings >/dev/null 2>&1 || { echo "strings is required for usb_f_midi validation." >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "sha256sum is required for usb_f_midi validation." >&2; exit 1; }

case "$module" in
  *.ko) decompress() { cat -- "$module"; } ;;
  *.ko.xz) decompress() { xz -dc -- "$module"; } ;;
  *.ko.gz) decompress() { gzip -dc -- "$module"; } ;;
  *.ko.zst) decompress() { zstd -q -dc -- "$module"; } ;;
  *.ko.lz4) decompress() { lz4 -q -dc -- "$module"; } ;;
  *.ko.bz2) decompress() { bzip2 -dc -- "$module"; } ;;
  *) echo "Unsupported usb_f_midi module compression: $module" >&2; exit 1 ;;
esac

work="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work"
}
trap cleanup EXIT

decompressed="$work/usb_f_midi.ko"
decompress > "$decompressed" || { echo "Unable to decompress usb_f_midi module: $module" >&2; exit 1; }
[[ -s "$decompressed" ]] || { echo "Decompressed usb_f_midi module is empty: $module" >&2; exit 1; }

readelf_header="$(readelf -h -- "$decompressed" 2>/dev/null)" || {
  echo "usb_f_midi is not a valid ELF module: $module" >&2
  exit 1
}
grep -Eq '^[[:space:]]*Class:[[:space:]]+ELF64$' <<<"$readelf_header" || {
  echo "usb_f_midi is not an ELF64 module: $module" >&2
  exit 1
}
grep -Eq '^[[:space:]]*Machine:[[:space:]]+AArch64$' <<<"$readelf_header" || {
  echo "usb_f_midi is not an AArch64 module: $module" >&2
  exit 1
}

mapfile -t vermagic_values < <(strings -a -- "$decompressed" | sed -n 's/^vermagic=//p')
[[ "${#vermagic_values[@]}" == 1 ]] || {
  echo "usb_f_midi module must contain exactly one vermagic marker: $module" >&2
  exit 1
}
[[ "${vermagic_values[0]}" == "$expected_release" || "${vermagic_values[0]}" == "$expected_release "* ]] || {
  echo "usb_f_midi vermagic does not match $expected_release: ${vermagic_values[0]}" >&2
  exit 1
}
assert_string_entry() {
  local expected="$1"
  strings -a -- "$decompressed" | grep -qxF -- "$expected" || {
    echo "usb_f_midi module is missing the binary string entry $expected: $module" >&2
    exit 1
  }
}
assert_string_entry interface_string
assert_string_entry f_midi_opts_attr_interface_string
assert_string_entry midi_interface_string

printf 'module_relative_path=%s\n' "${module#"${ORANGE_KERNEL_MODULE_ROOT:-}"}"
printf 'module_compressed_sha256=%s\n' "$(sha256sum -- "$module" | awk '{print $1}')"
printf 'module_decompressed_sha256=%s\n' "$(sha256sum -- "$decompressed" | awk '{print $1}')"
printf 'module_vermagic=%s\n' "${vermagic_values[0]}"
printf 'module_interface_string_marker=interface_string\n'
printf 'module_interface_options_marker=f_midi_opts_attr_interface_string\n'
printf 'module_interface_runtime_marker=midi_interface_string\n'
