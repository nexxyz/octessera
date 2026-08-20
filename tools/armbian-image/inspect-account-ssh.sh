#!/usr/bin/env bash
# shellcheck disable=SC2154

octessera_require_account_record() {
  local passwd_content="$1"
  local result
  result="$(printf '%s\n' "$passwd_content" | awk -F: '
    $1 == "octessera" { count++; if (NF != 7 || $3 !~ /^[0-9]+$/ || $6 != "/home/octessera" || $7 != "/bin/bash") invalid = 1 }
    END { if (count != 1) print "missing"; else if (invalid) print "unexpected" }
  ')"
  case "$result" in
    missing) echo 'The image is missing the expected octessera account.' >&2; return 1 ;;
    unexpected) echo 'The image has an unexpected octessera account.' >&2; return 1 ;;
  esac
}

octessera_reject_authorized_keys() {
  local passwd_content login_defs_content uid_min key_path stat_status derived_key_paths
  local -a key_paths=(root/.ssh/authorized_keys etc/ssh/authorized_keys etc/dropbear/authorized_keys)
  passwd_content="$(read_file etc/passwd)" || { echo 'Unable to read required image path: etc/passwd.' >&2; exit 1; }
  login_defs_content="$(read_file etc/login.defs)" || { echo 'Unable to read required image path: etc/login.defs.' >&2; exit 1; }
  uid_min="$(octessera_uid_min "$login_defs_content")"
  octessera_require_account_record "$passwd_content" || exit 1
  key_paths+=(home/octessera/.ssh/authorized_keys)
  derived_key_paths="$(octessera_derive_account_authorized_key_paths "$passwd_content" "$uid_min")" || {
    echo 'Built-image inspection cannot authorize an unsupported account home.' >&2
    exit 1
  }
  while IFS= read -r key_path; do
    [[ -n "$key_path" && "$key_path" != home/octessera/.ssh/authorized_keys ]] || continue
    key_paths+=("$key_path")
  done <<< "$derived_key_paths"
  for key_path in "${key_paths[@]}"; do
    if stat_path "$key_path"; then
      echo "Built image must not contain baked authorized keys: $key_path." >&2
      exit 1
    else
      stat_status=$?
      [[ "$stat_status" == 1 ]] || { echo "Unable to inspect image path: $key_path." >&2; exit 1; }
    fi
  done
}

octessera_require_ssh_clean() {
  local ssh_listing_request ssh_listing_error ssh_listing_error_content ssh_listing_status ssh_listing find_pipeline_status
  if [[ -d "$target" ]]; then
    if find "$target/etc/ssh" -maxdepth 1 -name 'ssh_host_*' | grep -q .; then
      find_pipeline_status=("${PIPESTATUS[@]}")
    else
      find_pipeline_status=("${PIPESTATUS[@]}")
    fi
    if [[ "${find_pipeline_status[0]}" != 0 ]]; then
      echo "Unable to inspect image path: etc/ssh (find status ${find_pipeline_status[0]})." >&2
      exit 1
    fi
    if [[ "${find_pipeline_status[1]}" != 0 && "${find_pipeline_status[1]}" != 1 ]]; then
      echo "Unable to complete the SSH host-key negative scan (grep status ${find_pipeline_status[1]})." >&2
      exit 1
    fi
    if [[ "${find_pipeline_status[1]}" == 0 ]]; then
      echo 'Built image must not contain baked SSH host keys.' >&2
      exit 1
    fi
  else
    ssh_listing_request="$(octessera_debugfs_ls_request etc/ssh)" || { echo 'Unable to inspect image path: etc/ssh.' >&2; exit 1; }
    ssh_listing_error="$inspect_work/debugfs-ssh-list.stderr"
    if ssh_listing="$(debugfs -R "$ssh_listing_request" "$target" 2>"$ssh_listing_error")"; then ssh_listing_status=0; else ssh_listing_status=$?; fi
    ssh_listing_error_content="$(cat -- "$ssh_listing_error")"
    if [[ "$ssh_listing_status" != 0 ]] || ! octessera_debugfs_stderr_is_startup_banner "$ssh_listing_error_content" || printf '%s\n' "$ssh_listing" | grep -Eq '(^ls:|File not found by ext2_lookup)'; then
      [[ -z "$ssh_listing_error_content" ]] || printf '%s\n' "$ssh_listing_error_content" >&2
      echo 'Unable to inspect image path: etc/ssh.' >&2
      exit 1
    fi
    if printf '%s\n' "$ssh_listing" | grep -q 'ssh_host_'; then
      echo 'Built image must not contain baked SSH host keys.' >&2
      exit 1
    fi
  fi
}

octessera_require_account_ssh_contract() {
  local shadow shadow_record shadow_account_count hash ssh_config
  shadow="$(read_file etc/shadow)" || { echo 'Unable to read required image path: etc/shadow.' >&2; exit 1; }
  shadow_record="$(printf '%s\n' "$shadow" | awk -F: '$1 == "octessera" { count++; hash = $2 } END { print count "\t" hash }')"
  IFS=$'\t' read -r shadow_account_count hash <<< "$shadow_record"
  [[ "$shadow_account_count" == 1 ]] || { echo 'The image is missing the expected octessera shadow account.' >&2; exit 1; }
  case "$hash" in
    ''|\!*|\**|x) ;;
    *) echo 'Octessera user has a usable baked password hash.' >&2; exit 1 ;;
  esac
  octessera_require_ssh_clean
  octessera_reject_authorized_keys
  ssh_config="$(read_file etc/ssh/sshd_config.d/10-octessera-setup.conf)"
  printf '%s\n' "$ssh_config" | grep -q '^PermitRootLogin no$' || { echo 'Missing PermitRootLogin no.' >&2; exit 1; }
  printf '%s\n' "$ssh_config" | grep -q '^PasswordAuthentication no$' || { echo 'Missing default PasswordAuthentication no.' >&2; exit 1; }
  printf '%s\n' "$ssh_config" | grep -q '^AllowUsers octessera$' || { echo 'Missing AllowUsers octessera.' >&2; exit 1; }
}
