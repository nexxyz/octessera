#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

assert_rejected() {
  local name="$1" pattern="$2" content="$3" fixture="$work/$1" status
  printf '%s\n' "$content" > "$fixture"
  if octessera_reject_file_match "Fixture contains forbidden pattern: $name" -qE "$pattern" "$fixture"; then
    echo "Negative fixture was accepted: $name" >&2
    exit 1
  else
    status=$?
  fi
  [[ "$status" == 1 ]] || { echo "Negative fixture check failed unexpectedly: $name (status $status)." >&2; exit 1; }
}

assert_rejected security 'BEGIN (OPENSSH |RSA )?PRIVATE KEY' 'BEGIN OPENSSH PRIVATE KEY'
assert_rejected policy 'systemctl enable --now' 'systemctl enable --now octessera-update-recovery.service'
assert_rejected device-tree 'spidev1_0' 'compatible = "spidev1_0";'
assert_rejected runtime 'AmbientCapabilities=' 'AmbientCapabilities=CAP_SYS_NICE'
assert_rejected oled 'octessera-(mark|wordmark)\.svg' 'copy_file asset /usr/share/octessera/oled/octessera-mark.svg'

if octessera_reject_file_match 'Missing negative fixture was treated as clean.' -qF forbidden "$work/missing" 2>"$work/missing.stderr"; then
  echo 'Missing negative fixture was treated as clean.' >&2
  exit 1
else
  status=$?
fi
[[ "$status" != 0 && "$status" != 1 ]] || { echo "Missing negative fixture returned a non-failing status: $status." >&2; exit 1; }

printf '%s\n' 'Validation negative fixtures passed.'
