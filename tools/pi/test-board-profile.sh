#!/usr/bin/env bash
set -euo pipefail

source ./tools/pi/board-profile.sh

[[ "$(get_pi_board_cargo_feature "$RASPBERRY_PI_ZERO_2W_PROFILE_ID")" == "$RASPBERRY_PI_ZERO_2W_CARGO_FEATURE" ]]
[[ "$(get_pi_board_cargo_feature "$ORANGE_PI_ZERO_2W_PROFILE_ID")" == "$ORANGE_PI_ZERO_2W_CARGO_FEATURE" ]]

for profile in "opi-zero-2w" "rpi-zero-2w" "hardware-pi"; do
  if require_supported_pi_board_profile "$profile"; then
    echo "Pi cross-build accepted non-canonical profile: $profile" >&2
    exit 1
  fi
done

if require_raspberry_pi_board_profile "$ORANGE_PI_ZERO_2W_PROFILE_ID"; then
  echo "Raspberry Pi tooling accepted the Orange profile" >&2
  exit 1
fi

echo "Shell board profile validation passed"
