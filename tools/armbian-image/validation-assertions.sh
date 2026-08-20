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

octessera_reject_text_match() {
  local message="$1"
  local content="$2"
  shift 2
  local status
  if grep "$@" <<< "$content" >/dev/null; then
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

octessera_require_text_match() {
  local message="$1"
  local content="$2"
  shift 2
  local status
  if grep "$@" <<< "$content" >/dev/null; then
    return 0
  else
    status=$?
  fi
  echo "$message" >&2
  return "$status"
}
