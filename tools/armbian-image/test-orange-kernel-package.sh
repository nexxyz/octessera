#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
validator="$root/tools/armbian-image/validate-orange-kernel-package.sh"
finder="$root/tools/armbian-image/find-orange-kernel-packages.sh"
module_inspector="$root/tools/armbian-image/inspect-orange-kernel-module.sh"
provenance_writer="$root/tools/armbian-image/write-orange-kernel-provenance.sh"

for script in "$validator" "$finder" "$module_inspector" "$provenance_writer"; do
  [[ -f "$script" ]] || { echo "Missing Orange kernel package helper: $script" >&2; exit 1; }
  bash -n "$script"
done
command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb is required for Orange kernel package tests." >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "sha256sum is required for Orange kernel package tests." >&2; exit 1; }
command -v strings >/dev/null 2>&1 || { echo "strings is required for Orange kernel package tests." >&2; exit 1; }
command -v readelf >/dev/null 2>&1 || { echo "readelf is required for Orange kernel package tests." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "python3 is required for Orange kernel package tests." >&2; exit 1; }

work="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work"
}
trap cleanup EXIT

good_config=$'# CONFIG_RT_GROUP_SCHED is not set\nCONFIG_SND_SEQUENCER=m\nCONFIG_SND_RAWMIDI=m\nCONFIG_SND_USB_AUDIO=m\nCONFIG_SYNTHETIC_FIXTURE=y'
good_config_sha256="$(printf '%s\n' "$good_config" | sha256sum | awk '{print $1}')"
source_config_sha256="$(python3 -c 'import json; print(json.load(open("tools/kernel-patches/orange-midi-interface-manifest.json"))["build_frameworks"]["armbian"]["config_base"]["sha256"])')"

make_module() {
  local output="$1"
  local compression="$2"
  local vermagic="$3"
  local marker="$4"
  local elf_mode="${5:-valid}"
  local payload="$work/module.payload"
  python3 - "$payload" "$elf_mode" "$vermagic" "$marker" <<'PY'
import struct
import sys

path, mode, vermagic, marker = sys.argv[1:]
if mode == "invalid":
    payload = b"not an ELF module\n"
else:
    marker = {
        "interface_string": "interface_string\nf_midi_opts_attr_interface_string\nmidi_interface_string",
        "missing-options": "interface_string\nmidi_interface_string",
        "missing-runtime": "interface_string\nf_midi_opts_attr_interface_string",
        "noisy-interface": "interface_string\ninterface_string\nf_midi_opts_attr_interface_string\nmidi_interface_string\ninterface_string",
    }.get(marker, marker)
    elf_class = 1 if mode == "elf32" else 2
    machine = 62 if mode == "x86_64" else 183
    header = struct.pack(
        "<16sHHIQQQIHHHHHH",
        b"\x7fELF" + bytes((elf_class, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0)),
        1,
        machine,
        1,
        0,
        0,
        0,
        0,
        64,
        0,
        0,
        0,
        0,
        0,
    )
    payload = header + (f"vermagic={vermagic}\n{marker}\n").encode()
with open(path, "wb") as handle:
    handle.write(payload)
PY
  case "$compression" in
    plain) cp -- "$payload" "$output" ;;
    gzip) gzip -n -c -- "$payload" > "$output" ;;
    xz) xz -c -- "$payload" > "$output" ;;
    *) echo "Unknown module fixture compression: $compression" >&2; exit 1 ;;
  esac
}

make_fdt() {
  python3 - "$1" <<'PY'
import struct
import sys

header = struct.pack(">10I", 0xD00DFEED, 72, 56, 72, 40, 17, 16, 0, 0, 0)
reserve = struct.pack(">QQ", 0, 0)
structure = struct.pack(">IIII", 1, 0, 2, 9)
with open(sys.argv[1], "wb") as handle:
    handle.write(header + reserve + structure)
PY
}

make_pair() {
  local name="$1"
  local config="$2"
  local image_version="${3:-26.8.0-trunk.417}"
  local dtb_version="${4:-26.8.0-trunk.417}"
  local architecture="${5:-arm64}"
  local source="${6:-linux-6.18.38}"
  local kernel_release="${7:-6.18.38-current-sunxi64}"
  local config_release="${8:-6.18.38-current-sunxi64}"
  local image_dtb_mode="${9:-good}"
  local module_mode="${10:-plain}"
  local module_vermagic="${11:-6.18.38-current-sunxi64 SMP}"
  local module_marker="${12:-interface_string}"
  local image_name="${13:-linux-image-current-sunxi64}"
  local dtb_name="${14:-linux-dtb-current-sunxi64}"
  local artifact_suffix="${15:-fixture}"
  local module_elf_mode="${16:-valid}"
  local image_root="$work/$name-image"
  local dtb_root="$work/$name-dtb"
  local module_dir="$image_root/lib/modules/$kernel_release/kernel/drivers/usb/gadget/function"
  local package_dir="$work/$name-packages"

  mkdir -p "$package_dir"
  mkdir -p "$image_root/DEBIAN" "$dtb_root/DEBIAN" "$image_root/boot" "$dtb_root/boot"
  mkdir -p "$module_dir" \
    "$image_root/lib/modules/$kernel_release/kernel/sound/core/seq" \
    "$image_root/lib/modules/$kernel_release/kernel/sound/core" \
    "$image_root/lib/modules/$kernel_release/kernel/sound/usb"
  printf '%s\n' \
    "Package: $image_name" \
    "Version: $image_version" \
    "Source: $source" \
    'Armbian-Kernel-Version: 6.18.38' \
    "Armbian-Kernel-Version-Family: $kernel_release" \
    "Architecture: $architecture" \
    'Maintainer: Octessera tests <tests@octessera.invalid>' \
    'Description: Orange kernel package test fixture' > "$image_root/DEBIAN/control"
  printf '%s\n' \
    "Package: $dtb_name" \
    "Version: $dtb_version" \
    "Architecture: $architecture" \
    'Maintainer: Octessera tests <tests@octessera.invalid>' \
    'Description: Orange DTB package test fixture' > "$dtb_root/DEBIAN/control"
  printf '%s\n' "$config" > "$image_root/boot/config-$config_release"
  [[ "$image_dtb_mode" == missing-image ]] || {
    mkdir -p "$image_root/usr/lib/linux-image-$kernel_release/allwinner"
    make_fdt "$image_root/usr/lib/linux-image-$kernel_release/allwinner/sun50i-h618-orangepi-zero2w.dtb"
    [[ "$image_dtb_mode" == bad-image-magic ]] && printf '\x00\x00\x00\x00' > "$image_root/usr/lib/linux-image-$kernel_release/allwinner/sun50i-h618-orangepi-zero2w.dtb"
  }
  [[ "$image_dtb_mode" == missing-package ]] || {
    mkdir -p "$dtb_root/boot/dtb-$kernel_release/allwinner"
    make_fdt "$dtb_root/boot/dtb-$kernel_release/allwinner/sun50i-h618-orangepi-zero2w.dtb"
    [[ "$image_dtb_mode" == bad-package-magic ]] && printf '\x00\x00\x00\x00' > "$dtb_root/boot/dtb-$kernel_release/allwinner/sun50i-h618-orangepi-zero2w.dtb"
  }
  : > "$image_root/lib/modules/$kernel_release/kernel/sound/core/seq/snd-seq.ko"
  : > "$image_root/lib/modules/$kernel_release/kernel/sound/core/seq/snd-seq-midi.ko"
  : > "$image_root/lib/modules/$kernel_release/kernel/sound/core/snd-rawmidi.ko"
  : > "$image_root/lib/modules/$kernel_release/kernel/sound/usb/snd-usb-audio.ko"
  [[ "$module_mode" == missing ]] || make_module "$module_dir/usb_f_midi.ko" plain "$module_vermagic" "$module_marker" "$module_elf_mode"
  [[ "$module_mode" == multiple ]] && make_module "$module_dir/usb_f_midi.ko.gz" gzip "$module_vermagic" "$module_marker" "$module_elf_mode"
  case "$module_mode" in
    compressed-gzip) rm -f -- "$module_dir/usb_f_midi.ko"; make_module "$module_dir/usb_f_midi.ko.gz" gzip "$module_vermagic" "$module_marker" "$module_elf_mode" ;;
    compressed-xz) rm -f -- "$module_dir/usb_f_midi.ko"; make_module "$module_dir/usb_f_midi.ko.xz" xz "$module_vermagic" "$module_marker" "$module_elf_mode" ;;
  esac
  dpkg-deb --build "$image_root" "$package_dir/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__${artifact_suffix}.deb" >/dev/null
  dpkg-deb --build "$dtb_root" "$package_dir/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__${artifact_suffix}.deb" >/dev/null
}

image_package() { printf '%s\n' "$work/$1-packages/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"; }
dtb_package() { printf '%s\n' "$work/$1-packages/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"; }

run_validator() {
  local name="$1"
  local expected_hash="${2:-}"
  local -a args=("$(image_package "$name")" "$(dtb_package "$name")")
  [[ -n "$expected_hash" ]] && args+=(--expected-config-sha256 "$expected_hash")
  if [[ -n "$expected_hash" ]]; then
    OCTESSERA_ORANGE_TEST_MODE=1 bash "$validator" "${args[@]}" >/dev/null
  else
    bash "$validator" "${args[@]}" >/dev/null
  fi
}

reject_validator() {
  local name="$1"
  if run_validator "$name" "$good_config_sha256" >"$work/$name.out" 2>&1; then
    echo "Orange kernel package validator accepted $name." >&2
    exit 1
  fi
}

make_pair good "$good_config"
if run_validator good >/dev/null 2>&1; then
  echo 'Orange package validator accepted a synthetic artifact without the manifest packaged config hash.' >&2
  exit 1
fi
if bash "$validator" "$(image_package good)" "$(dtb_package good)" --expected-config-sha256 "$good_config_sha256" >/dev/null 2>&1; then
  echo 'Orange package validator accepted a mutable production config hash override.' >&2
  exit 1
fi
wrong_manifest="$work/wrong-packaged-config-manifest.json"
missing_manifest="$work/missing-packaged-config-manifest.json"
python3 - "$root/tools/kernel-patches/orange-midi-interface-manifest.json" "$wrong_manifest" "$missing_manifest" <<'PY'
import copy
import json
import sys

source, wrong, missing = sys.argv[1:]
manifest = json.loads(open(source, encoding="utf-8").read())
wrong_manifest = copy.deepcopy(manifest)
wrong_manifest["build_frameworks"]["armbian"]["packaged_config_sha256"] = "0" * 64
missing_manifest = copy.deepcopy(manifest)
del missing_manifest["build_frameworks"]["armbian"]["packaged_config_sha256"]
for path, value in ((wrong, wrong_manifest), (missing, missing_manifest)):
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(value, handle)
PY
if OCTESSERA_ORANGE_TEST_MODE=1 bash "$validator" "$(image_package good)" "$(dtb_package good)" --manifest "$wrong_manifest" >/dev/null 2>&1; then
  echo 'Orange package validator accepted a wrong manifest packaged config hash.' >&2
  exit 1
fi
if OCTESSERA_ORANGE_TEST_MODE=1 bash "$validator" "$(image_package good)" "$(dtb_package good)" --manifest "$missing_manifest" >/dev/null 2>&1; then
  echo 'Orange package validator accepted a manifest without a packaged config hash.' >&2
  exit 1
fi
run_validator good "$good_config_sha256"
if run_validator good "$source_config_sha256" >/dev/null 2>&1; then
  echo 'Orange package validation accepted the source config hash as the final config hash.' >&2
  exit 1
fi
make_pair compressed-gzip "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good compressed-gzip
run_validator compressed-gzip "$good_config_sha256"
make_pair compressed-xz "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good compressed-xz
run_validator compressed-xz "$good_config_sha256"

mkdir -p "$work/discovery"
cp -- "$(image_package good)" "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"
cp -- "$(dtb_package good)" "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"
cp -- "$(image_package good)" "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64.deb"
printf '%s\n' "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb" "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb" > "$work/discovery.expected"
bash "$finder" "$work/discovery" > "$work/discovery.actual"
cmp -- "$work/discovery.expected" "$work/discovery.actual"
rm -- "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"
if bash "$finder" "$work/discovery" >/dev/null 2>&1; then echo 'Package discovery accepted a missing DTB package.' >&2; exit 1; fi
cp -- "$(dtb_package good)" "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"
cp -- "$(image_package good)" "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__extra.deb"
if bash "$finder" "$work/discovery" >/dev/null 2>&1; then echo 'Package discovery accepted multiple image packages.' >&2; exit 1; fi
rm -- "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__extra.deb"
rm -- "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"
cp -- "$(dtb_package good)" "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__wrong.deb"
if bash "$finder" "$work/discovery" >/dev/null 2>&1; then echo 'Package discovery accepted a mismatched package pair.' >&2; exit 1; fi
rm -- "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__wrong.deb"
cp -- "$(dtb_package good)" "$work/discovery/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb"
cp -- "$(image_package good)" "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__.deb"
if bash "$finder" "$work/discovery" >/dev/null 2>&1; then echo 'Package discovery accepted an empty artifact suffix.' >&2; exit 1; fi
rm -- "$work/discovery/linux-image-current-sunxi64_26.8.0-trunk.417_arm64__.deb"

make_pair bad-architecture "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 all
reject_validator bad-architecture
make_pair bad-version "$good_config" 26.8.0-trunk.416
reject_validator bad-version
make_pair bad-dtb-version "$good_config" 26.8.0-trunk.417 26.8.0-trunk.416
reject_validator bad-dtb-version
make_pair bad-image-package "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" interface_string bad-image-package linux-dtb-current-sunxi64
reject_validator bad-image-package
make_pair bad-dtb-package "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" interface_string linux-image-current-sunxi64 bad-dtb-package
reject_validator bad-dtb-package
make_pair bad-source "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.37
reject_validator bad-source
make_pair bad-abi "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.39-current-sunxi64
reject_validator bad-abi
make_pair bad-dtb "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 missing-image
reject_validator bad-dtb
make_pair bad-config-name "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" interface_string linux-image-current-sunxi64 linux-dtb-current-sunxi64
rm -f -- "$work/bad-config-name-image/boot/config-6.18.38-current-sunxi64"
printf '%s\n' "$good_config" > "$work/bad-config-name-image/boot/config-6.18.39-current-sunxi64"
dpkg-deb --build "$work/bad-config-name-image" "$(image_package bad-config-name)" >/dev/null
reject_validator bad-config-name
make_pair bad-config-hash "$good_config"
printf '%s\n' "$good_config" CONFIG_EXTRA=y > "$work/bad-config-hash-image/boot/config-6.18.38-current-sunxi64"
dpkg-deb --build "$work/bad-config-hash-image" "$(image_package bad-config-hash)" >/dev/null
reject_validator bad-config-hash
make_pair bad-config-line $'# CONFIG_RT_GROUP_SCHED is not set\nCONFIG_SND_SEQUENCER=y\nCONFIG_SND_RAWMIDI=m\nCONFIG_SND_USB_AUDIO=m'
reject_validator bad-config-line
make_pair bad-module "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good missing
reject_validator bad-module
make_pair bad-module-elf "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" interface_string linux-image-current-sunxi64 linux-dtb-current-sunxi64 fixture invalid
reject_validator bad-module-elf
make_pair bad-module-machine "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" interface_string linux-image-current-sunxi64 linux-dtb-current-sunxi64 fixture x86_64
reject_validator bad-module-machine
make_pair bad-vermagic "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain wrong-release interface_string
reject_validator bad-vermagic
make_pair bad-marker "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" wrong-marker
reject_validator bad-marker
make_pair duplicate-vermagic "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain $'6.18.38-current-sunxi64 SMP\nvermagic=6.18.38-current-sunxi64 SMP' interface_string
reject_validator duplicate-vermagic
make_pair noisy-interface "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" noisy-interface
run_validator noisy-interface "$good_config_sha256"
make_pair missing-interface-options "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" missing-options
reject_validator missing-interface-options
make_pair missing-interface-runtime "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good plain "6.18.38-current-sunxi64 SMP" missing-runtime
reject_validator missing-interface-runtime
make_pair multiple-module "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 good multiple
reject_validator multiple-module
make_pair bad-image-magic "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 bad-image-magic
reject_validator bad-image-magic
make_pair bad-package-magic "$good_config" 26.8.0-trunk.417 26.8.0-trunk.417 arm64 linux-6.18.38 6.18.38-current-sunxi64 6.18.38-current-sunxi64 bad-package-magic
reject_validator bad-package-magic
make_pair bad-dtb-equality "$good_config"
printf 'x' >> "$work/bad-dtb-equality-dtb/boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb"
dpkg-deb --build "$work/bad-dtb-equality-dtb" "$(dtb_package bad-dtb-equality)" >/dev/null
reject_validator bad-dtb-equality

evidence="$work/good-evidence.env"
provenance="$work/provenance.txt"
handoff="$work/handoff"
mkdir -p "$handoff"
cp -- "$(image_package good)" "$handoff/linux-image-current-sunxi64_26.8.0-trunk.417_arm64.deb"
cp -- "$(dtb_package good)" "$handoff/linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64.deb"
OCTESSERA_ORANGE_TEST_MODE=1 bash "$validator" "$(image_package good)" "$(dtb_package good)" --expected-config-sha256 "$good_config_sha256" --evidence-output "$evidence" >/dev/null
grep -q '^packaged_config_expected_sha256=fddbc3ff39e27b7e0aeb80b97496b93f5fca91b8fd166f2937f6924dc034c352$' "$evidence"
grep -q "^final_config_sha256=$good_config_sha256$" "$evidence"
GITHUB_SOURCE_SHA="$(git -C "$root" rev-parse HEAD)" \
ARMBIAN_BUILD_REF=fa7a7b2294d9e760a77630950afd460b7a0b2a26 \
  OCTESSERA_ORANGE_TEST_MODE=1 bash "$provenance_writer" "$(image_package good)" "$(dtb_package good)" "$provenance" "$evidence" "" "$good_config_sha256" "$handoff" >/dev/null
grep -q '^image_package_sha256=' "$provenance"
grep -q '^dtb_package_sha256=' "$provenance"
grep -q '^image_package_native=linux-image-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb$' "$provenance"
grep -q '^dtb_package_native=linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64__fixture.deb$' "$provenance"
grep -q '^artifact_suffix=fixture$' "$provenance"
grep -q '^octessera_checkout_head=' "$provenance"
grep -q '^kernel_config_final_sha256=' "$provenance"
grep -q '^kernel_config_expected_packaged_sha256=fddbc3ff39e27b7e0aeb80b97496b93f5fca91b8fd166f2937f6924dc034c352$' "$provenance"
grep -q "^kernel_config_final_sha256=$good_config_sha256$" "$provenance"
grep -q '^kernel_config_sha256_match=false$' "$provenance"
grep -q '^image_dtb_sha256=' "$provenance"
grep -q '^evidence_sha256=' "$provenance"
grep -q '^usb_f_midi_interface_string_marker=interface_string$' "$provenance"
grep -q '^usb_f_midi_interface_options_marker=f_midi_opts_attr_interface_string$' "$provenance"
grep -q '^usb_f_midi_interface_runtime_marker=midi_interface_string$' "$provenance"
grep -q '^armbian_build_repository=https://github.com/armbian/build.git$' "$provenance"
grep -q '^kernel_source_repository=https://github.com/torvalds/linux.git$' "$provenance"
grep -q '^kernel_source_commit=e46dc0adfe39724bcf52cea47b8f9c9aed86a394$' "$provenance"
grep -q '^kernel_config_source_sha256=' "$provenance"
grep -q '^core_series_sha256=' "$provenance"
grep -q '^patching_order_source_sha256=' "$provenance"
grep -q '^accepted_upstream_patch_sha256=' "$provenance"
grep -q '^octessera_follow_up_patch_sha256=' "$provenance"
grep -q '^image_package_handoff_sha256=' "$provenance"
grep -q '^dtb_package_handoff_sha256=' "$provenance"
grep -q '^github_source_sha=' "$provenance"
if grep -q 'unavailable' "$provenance"; then
  echo 'Orange provenance emitted unavailable evidence.' >&2
  exit 1
fi
grep -q '^armbian_build_ref=fa7a7b2294d9e760a77630950afd460b7a0b2a26$' "$provenance"
for removed_field in kernel_source_remote_url kernel_source_checkout_path kernel_source_checkout_head kernel_source_base_commit kernel_source_base_is_ancestor; do
  if grep -q "^${removed_field}=" "$provenance"; then
    echo "Orange provenance emitted removed field: $removed_field" >&2
    exit 1
  fi
done
sed 's/^module_decompressed_sha256=.*/module_decompressed_sha256=0000000000000000000000000000000000000000000000000000000000000000/' "$evidence" > "$work/tampered-evidence.env"
if GITHUB_SOURCE_SHA="$(git -C "$root" rev-parse HEAD)" ARMBIAN_BUILD_REF=fa7a7b2294d9e760a77630950afd460b7a0b2a26 OCTESSERA_ORANGE_TEST_MODE=1 bash "$provenance_writer" "$(image_package good)" "$(dtb_package good)" "$work/tampered-provenance.txt" "$work/tampered-evidence.env" "" "$good_config_sha256" >/dev/null 2>&1; then
  echo 'Orange provenance accepted tampered module hashes.' >&2
  exit 1
fi
if GITHUB_SOURCE_SHA=0123456789012345678901234567890123456789 ARMBIAN_BUILD_REF=fa7a7b2294d9e760a77630950afd460b7a0b2a26 OCTESSERA_ORANGE_TEST_MODE=1 bash "$provenance_writer" "$(image_package good)" "$(dtb_package good)" "$work/wrong-checkout-provenance.txt" "$evidence" "" "$good_config_sha256" >/dev/null 2>&1; then
  echo 'Orange provenance accepted a mismatched Octessera checkout.' >&2
  exit 1
fi
sed 's/^image_package_sha256=.*/image_package_sha256=0000000000000000000000000000000000000000000000000000000000000000/' "$evidence" > "$work/tampered-package-evidence.env"
if GITHUB_SOURCE_SHA="$(git -C "$root" rev-parse HEAD)" ARMBIAN_BUILD_REF=fa7a7b2294d9e760a77630950afd460b7a0b2a26 OCTESSERA_ORANGE_TEST_MODE=1 bash "$provenance_writer" "$(image_package good)" "$(dtb_package good)" "$work/tampered-package-provenance.txt" "$work/tampered-package-evidence.env" "" "$good_config_sha256" >/dev/null 2>&1; then
  echo 'Orange provenance accepted tampered package hashes.' >&2
  exit 1
fi

printf 'Orange linux-image/linux-dtb package tests passed\n'
