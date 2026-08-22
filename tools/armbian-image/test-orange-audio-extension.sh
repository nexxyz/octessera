#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
extension="$root/userpatches/extensions/octessera_audio.sh"
[[ -f "$extension" ]] || { echo "Missing Orange AHUB audio extension." >&2; exit 1; }
bash -n "$extension"
# shellcheck source=userpatches/extensions/octessera_audio.sh
source "$extension"
opts_y=()
opts_m=()
opts_n=()
declare -A opts_val=()
custom_kernel_config__octessera_audio
expected=(SND_SOC_SUNXI_AHUB SND_SOC_SUNXI_AHUB_DAM SND_SOC_SUNXI_MACH NVMEM_SUNXI_SID)
[[ "${opts_y[*]}" == "${expected[*]}" ]] || { echo "Orange AHUB audio extension requested unexpected built-in options." >&2; exit 1; }
[[ "${#opts_m[@]}" == 0 && "${#opts_n[@]}" == 0 && "${#opts_val[@]}" == 0 ]] || { echo "Orange AHUB audio extension owns unexpected option classes." >&2; exit 1; }
if grep -q 'CONFIG_SND_SOC_PCM5102A' "$extension"; then
  echo 'Orange AHUB audio extension must not require PCM5102A.' >&2
  exit 1
fi

printf 'Orange AHUB audio extension tests passed\n'
