#!/usr/bin/env bash
set -euo pipefail

RASPBERRY_PI_ZERO_2W_PROFILE_ID="raspberry-pi-zero-2w"
ORANGE_PI_ZERO_2W_PROFILE_ID="orange-pi-zero-2w"
RASPBERRY_PI_ZERO_2W_CARGO_FEATURE="hardware-raspberry-pi-zero-2w"
ORANGE_PI_ZERO_2W_CARGO_FEATURE="hardware-orange-pi-zero-2w"

require_supported_pi_board_profile() {
  local profile="${1:-}"
  case "$profile" in
    "$RASPBERRY_PI_ZERO_2W_PROFILE_ID"|"$ORANGE_PI_ZERO_2W_PROFILE_ID")
      return 0
      ;;
    *)
      echo "Pi cross-build accepts only '$RASPBERRY_PI_ZERO_2W_PROFILE_ID' or '$ORANGE_PI_ZERO_2W_PROFILE_ID'; got '$profile'." >&2
      return 1
      ;;
  esac
}

get_pi_board_cargo_feature() {
  local profile="${1:-}"
  case "$profile" in
    "$RASPBERRY_PI_ZERO_2W_PROFILE_ID")
      printf '%s\n' "$RASPBERRY_PI_ZERO_2W_CARGO_FEATURE"
      ;;
    "$ORANGE_PI_ZERO_2W_PROFILE_ID")
      printf '%s\n' "$ORANGE_PI_ZERO_2W_CARGO_FEATURE"
      ;;
    *)
      require_supported_pi_board_profile "$profile"
      return 1
      ;;
  esac
}

require_raspberry_pi_board_profile() {
  local profile="${1:-}"
  if [[ "$profile" == "$ORANGE_PI_ZERO_2W_PROFILE_ID" ]]; then
    echo "Orange Pi profile '$ORANGE_PI_ZERO_2W_PROFILE_ID' is not supported by Raspberry Pi tooling; use the separate Armbian workflow." >&2
    return 1
  fi
  if [[ "$profile" != "$RASPBERRY_PI_ZERO_2W_PROFILE_ID" ]]; then
    echo "Raspberry Pi tooling accepts only '$RASPBERRY_PI_ZERO_2W_PROFILE_ID'; got '$profile'." >&2
    return 1
  fi
}
