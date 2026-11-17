#!/usr/bin/env bash

octessera_reject_file_match() {
  local message="$1"
  shift
  local status
  if grep "$@" >/dev/null; then
    echo "$message" >&2
    return 1
  else
    status=$?
  fi
  if [[ "$status" != 1 ]]; then
    echo "$message: unable to complete the negative check (grep status $status)." >&2
    return "$status"
  fi
}
