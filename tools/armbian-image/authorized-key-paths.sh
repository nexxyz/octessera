#!/usr/bin/env bash

octessera_uid_min() {
  local login_defs_content="$1"
  local uid_min

  uid_min="$(printf '%s\n' "$login_defs_content" | awk '
    /^[[:space:]]*#/ { next }
    $1 == "UID_MIN" && $2 ~ /^[0-9]+$/ { print $2; exit }
  ')"
  [[ "$uid_min" =~ ^[1-9][0-9]*$ ]] || uid_min=1000
  printf '%s\n' "$uid_min"
}

octessera_derive_account_authorized_key_paths() {
  local passwd_content="$1"
  local uid_min="${2:-1000}"
  local user
  local uid
  local home
  local shell

  while IFS=: read -r user _ uid _ _ home shell; do
    [[ -n "$user" && -n "$home" ]] || continue
    [[ "$uid" =~ ^[0-9]+$ ]] || {
      printf 'Invalid UID for user %s.\n' "$user" >&2
      return 1
    }
    [[ "$uid_min" =~ ^[1-9][0-9]*$ ]] || {
      printf 'Invalid UID_MIN value.\n' >&2
      return 1
    }
    if (( 10#$uid == 0 )); then
      if [[ "$home" == /root ]]; then
        continue
      fi
    elif (( 10#$uid < 10#$uid_min )); then
      continue
    fi
    [[ "$home" == /nonexistent || "$shell" == /usr/sbin/nologin || "$shell" == /sbin/nologin || "$shell" == /bin/false ]] && continue

    if [[ "$home" =~ ^/home/[A-Za-z0-9._-]+$ && "$home" != /home/. && "$home" != /home/.. ]]; then
      printf '%s\n' "${home#/}/.ssh/authorized_keys"
      continue
    fi

    printf 'Unsupported non-system account home for user %s (UID %s): %s.\n' "$user" "$uid" "$home" >&2
    return 1
  done <<< "$passwd_content"
}
