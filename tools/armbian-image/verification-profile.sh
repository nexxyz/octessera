#!/usr/bin/env bash

octessera_require_constructor_device_tree_contract() {
  local profile="$1"
  local profile_metadata="$2"
  case "$profile" in
    full-constructor)
      octessera_require_device_tree_contract "$profile_metadata"
      ;;
    legacy-runtime-only|legacy-setup-layer)
      ;;
    *)
      echo "Invalid verification profile: $profile." >&2
      return 2
      ;;
  esac
}
