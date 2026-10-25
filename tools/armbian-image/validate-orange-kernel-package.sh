#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 2 ]] || { echo "Usage: $0 <linux-image.deb> <linux-dtb.deb>" >&2; exit 2; }
image_package="$1"
dtb_package="$2"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fail() { echo "Orange kernel package check failed: $*" >&2; exit 1; }

mapfile -t contract < <(python3 - "$root/tools/kernel-patches/orange-midi-interface-manifest.json" <<'PY'
import json
import sys

armbian = json.load(open(sys.argv[1], encoding="utf-8"))["build_frameworks"]["armbian"]
for key in ("kernel_release", "required_module", "required_dtb", "stock_i2c1_dtbo"):
    print(armbian[key])
PY
)
release="${contract[0]}"
module_name="${contract[1]}"

work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
dpkg-deb -x "$image_package" "$work/image"
dpkg-deb -x "$dtb_package" "$work/dtb"

grep -Eqx 'CONFIG_MMC_SPI=[ym]' "$work/image/boot/config-$release" || fail "CONFIG_MMC_SPI is not enabled"
dtb_dir="$work/dtb/boot/dtb-$release/allwinner"
[[ -s "$dtb_dir/${contract[2]}" ]] || fail "missing ${contract[2]}"
[[ -s "$dtb_dir/overlay/${contract[3]}" ]] || fail "missing ${contract[3]}"

mapfile -t modules < <(find "$work/image/lib/modules/$release" -type f \( -name "$module_name" -o -name "$module_name.*" \) -print)
[[ "${#modules[@]}" == 1 ]] || fail "expected exactly one $module_name, found ${#modules[@]}"
case "${modules[0]}" in
  *.ko) cp -- "${modules[0]}" "$work/module.ko" ;;
  *.ko.xz) xz -dc -- "${modules[0]}" > "$work/module.ko" ;;
  *.ko.gz) gzip -dc -- "${modules[0]}" > "$work/module.ko" ;;
  *.ko.zst) zstd -q -dc -- "${modules[0]}" > "$work/module.ko" ;;
  *) fail "unsupported module compression: ${modules[0]}" ;;
esac
strings -a -- "$work/module.ko" > "$work/module.strings"
mapfile -t vermagic < <(sed -n 's/^vermagic=//p' "$work/module.strings")
[[ "${#vermagic[@]}" == 1 && "${vermagic[0]%% *}" == "$release" ]] || fail "$module_name vermagic does not match $release"
for marker in interface_string f_midi_opts_attr_interface_string midi_interface_string; do
  grep -qxF -- "$marker" "$work/module.strings" || fail "$module_name is missing the $marker string"
done

echo "Orange kernel package check passed: $(basename -- "$image_package") $(basename -- "$dtb_package")"
