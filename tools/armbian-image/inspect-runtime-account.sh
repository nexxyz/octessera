#!/usr/bin/env bash
# shellcheck disable=SC2154

octessera_reject_runtime_sudoers() {
  local path content status listing listing_line name metadata entry_type
  check_sudoers_file() {
    local candidate="$1"
    content="$(read_file "$candidate")"
    if printf '%s\n' "$content" | grep -Eq '(^|[^[:alnum:]_-])octessera-runtime([^[:alnum:]_-]|$)'; then
      echo "Production runtime account appears in sudoers: $candidate." >&2
      return 1
    fi
  }
  for path in etc/sudoers etc/sudoers.d/octessera-update etc/sudoers.d/octessera-ssh-key-admin; do
    if stat_path "$path"; then check_sudoers_file "$path" || return 1; else status=$?; [[ "$status" == 1 ]] || { echo "Unable to inspect sudoers path: $path." >&2; return 1; }; fi
  done
  if [[ -d "$target" ]]; then
    if [[ -d "$target/etc/sudoers.d" && ! -L "$target/etc/sudoers.d" ]]; then
      while IFS= read -r -d '' path; do check_sudoers_file "${path#"$target/"}" || return 1; done < <(find -P "$target/etc/sudoers.d" -type f -print0)
    fi
  elif stat_path etc/sudoers.d; then
    listing="$(octessera_debugfs_list_path "$target" etc/sudoers.d)" || { echo 'Unable to enumerate sudoers.d.' >&2; return 1; }
    while IFS= read -r listing_line; do
      [[ -n "$listing_line" ]] || continue
      name="$(octessera_debugfs_ls_entry_name "$listing_line")" || { echo "Malformed sudoers.d entry." >&2; return 1; }
      [[ "$name" == . || "$name" == .. ]] && continue
      metadata="$(octessera_debugfs_stat_metadata "$target" "etc/sudoers.d/$name")" || { echo "Unable to inspect sudoers.d entry: $name." >&2; return 1; }
      entry_type="$(octessera_debugfs_type "$metadata")" || { echo "Unable to parse sudoers.d entry: $name." >&2; return 1; }
      if [[ "$entry_type" == regular ]]; then
        check_sudoers_file "etc/sudoers.d/$name" || return 1
      fi
    done <<< "$listing"
  else
    status=$?
    [[ "$status" == 1 ]] || { echo 'Unable to inspect sudoers.d.' >&2; return 1; }
  fi
}

octessera_require_runtime_account() {
  local passwd_content="$1" group_content="$2" runtime_passwd runtime_shadow runtime_group runtime_name runtime_password runtime_uid runtime_gid runtime_home runtime_shell runtime_group_gid protected_group
  runtime_passwd="$(printf '%s\n' "$passwd_content" | awk -F: '$1 == "octessera-runtime" { count++; record = $0 } END { if (count == 1) print record; else exit 1 }')" || { echo 'Production image is missing the unique octessera-runtime account.' >&2; exit 1; }
  IFS=: read -r runtime_name runtime_password runtime_uid runtime_gid _ runtime_home runtime_shell <<< "$runtime_passwd"
  [[ "$runtime_name" == octessera-runtime && "$runtime_uid" =~ ^[0-9]+$ && "$runtime_uid" -lt 1000 && "$runtime_home" == /nonexistent && "$runtime_shell" == /usr/sbin/nologin ]] || { echo 'Production octessera-runtime account is not a locked system no-shell user.' >&2; exit 1; }
  runtime_shadow="$(read_file etc/shadow)"
  runtime_password="$(printf '%s\n' "$runtime_shadow" | awk -F: '$1 == "octessera-runtime" { count++; hash = $2 } END { if (count == 1) print hash; else exit 1 }')" || { echo 'Production image is missing the octessera-runtime shadow entry.' >&2; exit 1; }
  case "$runtime_password" in ''|\!*|\**|x) ;; *) echo 'octessera-runtime has an unlocked password.' >&2; exit 1 ;; esac
  runtime_group="$(printf '%s\n' "$group_content" | awk -F: '$1 == "octessera-runtime" { count++; record = $0 } END { if (count == 1) print record; else exit 1 }')" || { echo 'Production image is missing the octessera-runtime group.' >&2; exit 1; }
  IFS=: read -r _ _ runtime_group_gid _ <<< "$runtime_group"
  [[ "$runtime_gid" == "$runtime_group_gid" ]] || { echo 'octessera-runtime primary group does not match its account.' >&2; exit 1; }
  for protected_group in sudo admin; do
    if printf '%s\n' "$group_content" | awk -F: -v wanted="$protected_group" '$1 == wanted && ("," $4 ",") ~ /,octessera-runtime,/' | grep -q .; then echo "octessera-runtime is present in protected admin group: $protected_group." >&2; exit 1; fi
  done
  octessera_reject_runtime_sudoers || { echo 'Production runtime account sudo separation failed.' >&2; exit 1; }
  printf '%s:%s' "$runtime_uid" "$runtime_gid"
}

octessera_runtime_owner_from_passwd() {
  local passwd_content="$1" record runtime_name runtime_uid runtime_gid
  record="$(printf '%s\n' "$passwd_content" | awk -F: '$1 == "octessera-runtime" { count++; row = $0 } END { if (count == 1) print row; else exit 1 }')" || { echo 'Image is missing the unique octessera-runtime account.' >&2; exit 1; }
  IFS=: read -r runtime_name _ runtime_uid runtime_gid _ _ _ <<< "$record"
  [[ "$runtime_name" == octessera-runtime && "$runtime_uid" =~ ^[0-9]+$ && "$runtime_gid" =~ ^[0-9]+$ && "$runtime_uid" -lt 1000 ]] || { echo 'Image octessera-runtime ownership identity is invalid.' >&2; exit 1; }
  printf '%s:%s' "$runtime_uid" "$runtime_gid"
}
