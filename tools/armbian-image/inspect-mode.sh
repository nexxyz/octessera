#!/usr/bin/env bash

octessera_canonical_mode() {
  local mode="$1"
  [[ "$mode" =~ ^[0-7]{3,4}$ ]] || return 1
  [[ "${#mode}" == 3 ]] && mode="0$mode"
  printf '%s' "$mode"
}

octessera_debugfs_mode() {
  local metadata="$1"
  local mode_value
  mode_value="$(printf '%s\n' "$metadata" | awk '
    {
      inode = 0
      for (position = 1; position <= NF; position++) {
        if ($position == "Inode:") {
          inode = 1
        }
      }
      if (inode) {
        for (position = 1; position < NF; position++) {
          if ($position == "Mode:") {
            print $(position + 1)
            exit
          }
        }
      }
    }
  ')"
  [[ "$mode_value" =~ ^[0-7]{4,}$ ]] || return 1
  octessera_canonical_mode "${mode_value: -4}"
}
