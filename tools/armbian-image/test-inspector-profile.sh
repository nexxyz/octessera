#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/verification-profile.sh
source "$root/tools/armbian-image/verification-profile.sh"

device_tree_calls=0
octessera_require_device_tree_contract() {
  device_tree_calls=$((device_tree_calls + 1))
  return 1
}

for profile in legacy-runtime-only legacy-setup-layer; do
  octessera_require_constructor_device_tree_contract "$profile" metadata
done
[[ "$device_tree_calls" == 0 ]] || { echo 'Legacy Orange profile invoked the constructor device-tree check.' >&2; exit 1; }

if octessera_require_constructor_device_tree_contract full-constructor metadata; then
  echo 'Full-constructor Orange profile accepted a failed device-tree check.' >&2
  exit 1
fi
[[ "$device_tree_calls" == 1 ]] || { echo 'Full-constructor Orange profile skipped the device-tree check.' >&2; exit 1; }

printf '%s\n' 'Orange inspector verification-profile device-tree gate tests passed.'
