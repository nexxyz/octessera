#!/usr/bin/env bash
set -euo pipefail

octessera_run_validation_stages() {
  local stage
  local status
  for stage in "$@"; do
    printf 'Running Armbian validation stage: %s\n' "$(basename "$stage")"
    if bash "$stage"; then
      continue
    else
      status=$?
    fi
    printf 'Armbian validation stage failed: %s (status %s)\n' "$(basename "$stage")" "$status" >&2
    return "$status"
  done
}
