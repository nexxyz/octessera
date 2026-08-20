#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
welcome_stager="$root/tools/armbian-image/stage-canonical-welcome.sh"
device_config_stager="$root/tools/armbian-image/stage-device-config.py"
image_mode_helper="$root/userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh"
welcome_overlay="$root/userpatches/overlay/etc/profile.d/octessera-welcome.sh"
welcome_overlay_preexisting=false
if [[ -e "$welcome_overlay" || -L "$welcome_overlay" ]]; then
  welcome_overlay_preexisting=true
fi

cleanup_validation() {
  local status=$?
  if [[ "$welcome_overlay_preexisting" == false ]]; then
    rm -f -- "$welcome_overlay"
  fi
  return "$status"
}
trap cleanup_validation EXIT

if [[ "${ARMBIAN_BOARD+x}" == x && "$ARMBIAN_BOARD" != orangepizero2w ]]; then
  echo 'Armbian image validation accepts only board orangepizero2w.' >&2
  exit 1
fi
if [[ "${ARMBIAN_RUN_BUILD:-false}" == true && ! "${ARMBIAN_BUILD_REF:-}" =~ ^[0-9a-fA-F]{40}$ ]]; then
  echo 'Qualification builds require a reviewed immutable 40-character Armbian commit SHA.' >&2
  exit 1
fi

# shellcheck source=userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh
source "$image_mode_helper"
requested_image_mode="${OCTESSERA_IMAGE_MODE:-}"
octessera_load_image_contract "$root/userpatches/overlay"
if [[ -n "$requested_image_mode" && "$requested_image_mode" != "$OCTESSERA_IMAGE_MODE" ]]; then
  echo "Armbian validation contract mode $OCTESSERA_IMAGE_MODE does not match caller mode $requested_image_mode." >&2
  exit 1
fi

"$welcome_stager"
python3 "$device_config_stager"

# shellcheck source=tools/armbian-image/validation-runner.sh
source "$root/tools/armbian-image/validation-runner.sh"
octessera_run_validation_stages \
  "$root/tools/armbian-image/validate-source-shape.sh" \
  "$root/tools/armbian-image/validate-device-tree.sh" \
  "$root/tools/armbian-image/validate-security-policy.sh" \
  "$root/tools/armbian-image/validate-image-proof.sh"

echo 'Armbian image validation passed.'
