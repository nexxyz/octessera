#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
validator="$root/tools/armbian-image/validate-orange-kernel-package.sh"
finder="$root/tools/armbian-image/find-orange-kernel-packages.sh"
release=6.18.46-current-sunxi64
suffix=6.18.46-S1f99-D7115-P2185-C4e0c-H5530-HK01ba-Vc222-Bb84f-R448a
image_name="linux-image-current-sunxi64_26.11.0-trunk.22_arm64__$suffix.deb"
dtb_name="linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64__$suffix.deb"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT

# make_pair <name> [mmc_spi] [vermagic] [markers] [omit]
make_pair() {
  local name="$1" mmc_spi="${2:-CONFIG_MMC_SPI=m}" vermagic="${3:-$release SMP}" markers="${4:-interface_string f_midi_opts_attr_interface_string midi_interface_string}" omit="${5:-}"
  local image="$work/$name/image" dtb="$work/$name/dtb" module_dir dtb_dir marker
  module_dir="$image/lib/modules/$release/kernel/drivers/usb/gadget/function"
  dtb_dir="$dtb/boot/dtb-$release/allwinner"
  mkdir -p "$image/DEBIAN" "$dtb/DEBIAN" "$image/boot" "$module_dir" "$dtb_dir/overlay"
  printf 'Package: linux-image-current-sunxi64\nVersion: 26.11.0-trunk.22\nArchitecture: arm64\nMaintainer: tests <tests@octessera.invalid>\nDescription: fixture\n' > "$image/DEBIAN/control"
  printf 'Package: linux-dtb-current-sunxi64\nVersion: 26.11.0-trunk.22\nArchitecture: arm64\nMaintainer: tests <tests@octessera.invalid>\nDescription: fixture\n' > "$dtb/DEBIAN/control"
  printf 'CONFIG_MMC=y\n%s\n' "$mmc_spi" > "$image/boot/config-$release"
  { printf 'vermagic=%s\n' "$vermagic"; for marker in $markers; do printf '%s\n' "$marker"; done; } | xz -c > "$module_dir/usb_f_midi.ko.xz"
  [[ "$omit" == dtb ]] || printf '\xd0\x0d\xfe\xedfixture' > "$dtb_dir/sun50i-h618-orangepi-zero2w.dtb"
  [[ "$omit" == i2c1 ]] || printf 'fixture\n' > "$dtb_dir/overlay/sun50i-h616-i2c1-pi.dtbo"
  dpkg-deb --build "$image" "$work/$name/$image_name" >/dev/null
  dpkg-deb --build "$dtb" "$work/$name/$dtb_name" >/dev/null
}

reject() {
  local name="$1"
  shift
  make_pair "$name" "$@"
  if bash "$validator" "$work/$name/$image_name" "$work/$name/$dtb_name" >/dev/null 2>&1; then
    echo "Orange kernel package check accepted $name." >&2
    exit 1
  fi
}

make_pair good
bash "$validator" "$work/good/$image_name" "$work/good/$dtb_name" >/dev/null
make_pair builtin-mmc-spi CONFIG_MMC_SPI=y
bash "$validator" "$work/builtin-mmc-spi/$image_name" "$work/builtin-mmc-spi/$dtb_name" >/dev/null
reject missing-mmc-spi '# CONFIG_MMC_SPI is not set'
reject wrong-vermagic CONFIG_MMC_SPI=m "6.18.47-current-sunxi64 SMP"
reject missing-options-string CONFIG_MMC_SPI=m "$release SMP" "interface_string midi_interface_string"
reject missing-runtime-string CONFIG_MMC_SPI=m "$release SMP" "interface_string f_midi_opts_attr_interface_string"
reject missing-dtb CONFIG_MMC_SPI=m "$release SMP" "interface_string f_midi_opts_attr_interface_string midi_interface_string" dtb
reject missing-i2c1 CONFIG_MMC_SPI=m "$release SMP" "interface_string f_midi_opts_attr_interface_string midi_interface_string" i2c1

discovery="$work/discovery"
mkdir -p "$discovery"
cp -- "$work/good/$image_name" "$work/good/$dtb_name" "$discovery/"
cp -- "$work/good/$image_name" "$discovery/linux-image-current-sunxi64_26.11.0-trunk.22_arm64.deb"
printf '%s\n' "$discovery/$image_name" "$discovery/$dtb_name" > "$work/discovery.expected"
bash "$finder" "$discovery" > "$work/discovery.actual"
cmp -- "$work/discovery.expected" "$work/discovery.actual"
cp -- "$work/good/$image_name" "$discovery/linux-image-current-sunxi64_26.11.0-trunk.22_arm64__extra.deb"
if bash "$finder" "$discovery" >/dev/null 2>&1; then echo 'Package discovery accepted multiple image packages.' >&2; exit 1; fi
rm -- "$discovery/linux-image-current-sunxi64_26.11.0-trunk.22_arm64__extra.deb" "$discovery/$dtb_name"
cp -- "$work/good/$dtb_name" "$discovery/linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64__wrong.deb"
if bash "$finder" "$discovery" >/dev/null 2>&1; then echo 'Package discovery accepted a mismatched package pair.' >&2; exit 1; fi

printf 'Orange linux-image/linux-dtb package tests passed\n'
