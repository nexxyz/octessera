#!/usr/bin/env bash
set -euo pipefail

usage() { echo "Usage: $0 --mode diagnostic|production [--runtime-bundle <dir>] <armbian-output-images-dir>" >&2; exit 2; }
[[ $# -ge 3 && "$1" == --mode && ( "$2" == diagnostic || "$2" == production ) ]] || usage
mode="$2"; shift 2
runtime_bundle=""
if [[ "${1:-}" == --runtime-bundle ]]; then runtime_bundle="${2:-}"; shift 2; fi
[[ $# -eq 1 && ( "$mode" == diagnostic || -n "$runtime_bundle" ) ]] || usage
repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'chmod -R u+w "$work"; rm -rf "$work"' EXIT
fail() { echo "Orange image smoke check failed: $*" >&2; exit 1; }

mapfile -t images < <(find "$1" -maxdepth 1 -type f \( -name '*.img' -o -name '*.img.xz' \) -print)
[[ "${#images[@]}" == 1 ]] || fail "expected exactly one image in $1, found ${#images[@]}"
image="${images[0]}"
if [[ "$image" == *.xz ]]; then xz -dc "$image" > "$work/image.img"; image="$work/image.img"; fi
read -r start sectors < <(fdisk -l "$image" | awk '$1 ~ /[0-9]+$/ { for (i = 2; i <= NF - 2; i++) if ($i ~ /^[0-9]+$/ && $(i + 1) ~ /^[0-9]+$/ && $(i + 2) ~ /^[0-9]+$/) { print $i " " $(i + 2); exit } }')
dd if="$image" of="$work/rootfs.ext4" bs=512 skip="$start" count="$sectors" status=none
root="$work/root"
mkdir -p "$root/usr/share/doc"
debugfs -R "rdump /etc /boot /opt /root /home $root" "$work/rootfs.ext4" 2>/dev/null
debugfs -R "rdump /usr/share/doc/octessera $root/usr/share/doc" "$work/rootfs.ext4" 2>/dev/null

for overlay in octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102; do
  [[ -f "$root/boot/overlay-user/$overlay.dtbo" ]] || fail "missing $overlay.dtbo"
done
grep -qxF 'user_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102' "$root/boot/armbianEnv.txt" || fail "armbianEnv.txt does not list the Octessera user overlays"

service="$root/etc/systemd/system/octessera.service"
if [[ "$mode" == production ]]; then
  [[ -f "$service" && -L "$root/etc/systemd/system/multi-user.target.wants/octessera.service" ]] || fail "octessera.service is not installed and enabled"
  [[ "$(readlink "$root/opt/octessera/current")" == /opt/octessera/releases/* ]] || fail "opt/octessera/current does not point at a release"
  cmp -s "$root$(readlink "$root/opt/octessera/current")/octessera-pi" "$runtime_bundle/octessera-pi" || fail "installed release binary differs from the runtime bundle"
else
  [[ ! -e "$service" && ! -e "$root/opt/octessera/current" ]] || fail "diagnostic image contains the production runtime"
fi

[[ -z "$(find "$root/etc/ssh" "$root/root" "$root/home" \( -name 'ssh_host_*_key' -o -path '*/.ssh/authorized_keys' \) -print -quit 2>/dev/null)" ]] || fail "SSH keys are baked into the image"
[[ ! -d "$root/etc/NetworkManager/system-connections" || -z "$(find "$root/etc/NetworkManager/system-connections" -type f -print -quit)" ]] || fail "NetworkManager connection profiles are baked into the image"
python3 "$repository_root/tools/legal/stage_notices.py" --repository-root "$repository_root" --destination-root "$root" --check >/dev/null || fail "installed legal notice tree is not canonical"

echo "Orange image smoke check passed ($mode mode)"
