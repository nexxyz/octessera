#!/usr/bin/env bash

octessera_debugfs_quote_argument() {
  local value="$1"
  if [[ "$value" == *[[:cntrl:]]* ]]; then
    printf '%s\n' 'Unsafe debugfs argument contains a control character.' >&2
    return 2
  fi
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '"%s"' "$value"
}

octessera_debugfs_path_argument() {
  octessera_debugfs_quote_argument "/$1"
}

octessera_debugfs_stat_request() {
  local path_argument
  path_argument="$(octessera_debugfs_path_argument "$1")" || return
  printf 'stat %s' "$path_argument"
}

octessera_debugfs_cat_request() {
  local path_argument
  path_argument="$(octessera_debugfs_path_argument "$1")" || return
  printf 'cat %s' "$path_argument"
}

octessera_debugfs_dump_request() {
  local path_argument
  local destination_argument
  path_argument="$(octessera_debugfs_path_argument "$1")" || return
  destination_argument="$(octessera_debugfs_quote_argument "$2")" || return
  printf 'dump %s %s' "$path_argument" "$destination_argument"
}

octessera_debugfs_ls_request() {
  local path_argument
  path_argument="$(octessera_debugfs_path_argument "$1")" || return
  printf 'ls -p %s' "$path_argument"
}

octessera_debugfs_list_path() {
  local target="$1"
  local path="$2"
  local request
  local listing
  local status

  request="$(octessera_debugfs_ls_request "$path")" || return 2
  if listing="$(debugfs -R "$request" "$target" 2>&1)"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" != 0 ]] || printf '%s\n' "$listing" | grep -Eq '^(debugfs:|ls:)|File not found by ext2_lookup'; then
    [[ -z "$listing" ]] || printf '%s\n' "$listing" >&2
    return 2
  fi
  printf '%s\n' "$listing" | awk '!/^debugfs[[:space:]][0-9]+\.[0-9]+(\.[0-9]+)?[[:space:]]+\([^()]*\)$/'
}

octessera_debugfs_ls_entry_name() {
  local line="$1"
  local name

  [[ "$line" != *[[:cntrl:]]* ]] || return 2
  name="$(printf '%s\n' "$line" | awk -F/ '
    NF >= 7 && $1 == "" && $2 ~ /^[0-9]+$/ && $3 ~ /^[0-7]+$/ && $4 ~ /^[0-9]+$/ && $5 ~ /^[0-9]+$/ { print $6; exit }
  ')"
  [[ -n "$name" && "$name" != */* ]] || return 2
  printf '%s' "$name"
}

octessera_debugfs_type() {
  local metadata="$1"
  local type
  type="$(printf '%s\n' "$metadata" | awk '
    $1 == "Inode:" {
      for (position = 1; position < NF; position++) if ($position == "Type:") { count++; type = $(position + 1) }
    }
    END { if (count != 1 || type !~ /^(directory|regular|symlink)$/) exit 1; print type }
  ')" || return 1
  printf '%s' "$type"
}

octessera_debugfs_fast_link_target() {
  local metadata="$1"
  printf '%s\n' "$metadata" | awk '
    $1 == "Fast" && $2 == "link" { count++; if ($3 != "dest:") bad=1; else { target = substr($0, index($0, $3) + length($3)); sub(/^[[:space:]]+/, "", target); if (target ~ /^"[^"]*"$/) target=substr(target, 2, length(target)-2); else if (target !~ /^[^"[:space:]]+$/) bad=1 } }
    END { if (count != 1 || bad || target == "") exit 1; print target }
  '
}
octessera_debugfs_type_size() {
  local metadata="$1"
  local type
  local size
  size="$(printf '%s\n' "$metadata" | awk '
    $1 == "User:" {
      for (position = 1; position < NF; position++) {
        if ($position == "Size:") { count++; size = $(position + 1) }
      }
    }
    END { if (count != 1 || size !~ /^[0-9]+$/) exit 1; print size }
  ')" || return 1
  type="$(octessera_debugfs_type "$metadata")" || return 1
  [[ "$type" == directory || "$type" == regular ]] || return 1
  printf '%s\t%s\n' "$type" "$size"
}

octessera_sample_relative_path_is_safe() {
  local path="$1"
  local component
  local -a components

  [[ -n "$path" && "$path" != /* && "$path" != */ && "$path" != *..* && "$path" != *\\* && "$path" != *[[:cntrl:]]* ]] || return 1
  IFS='/' read -r -a components <<< "$path"
  for component in "${components[@]}"; do
    [[ -n "$component" && "$component" != . && "$component" != .. ]] || return 1
  done
}

octessera_collect_sample_inventory() {
  local target="$1"
  local sample_root="$2"
  local inventory_path="$3"
  local blob_path="$inventory_path.blob"
  local listing
  local listing_line
  local name
  local directory
  local full_path
  local relative_path
  local metadata
  local type_size
  local entry_type
  local inventory_type
  local entry_size
  local status
  local -a directories

  : > "$inventory_path" || return 2
  if [[ -d "$target" ]]; then
    [[ -d "$target/$sample_root" && ! -L "$target/$sample_root" ]] || return 2
    if ! find -P "$target/$sample_root" -mindepth 1 -printf '%y\0%P\0%s\0' > "$blob_path"; then
      return 2
    fi
    while IFS= read -r -d '' entry_type && IFS= read -r -d '' relative_path && IFS= read -r -d '' entry_size; do
      octessera_sample_relative_path_is_safe "$relative_path" || return 2
      printf '%s\t%s\t%s\n' "$entry_type" "$relative_path" "$entry_size" >> "$inventory_path" || return 2
    done < "$blob_path"
    return 0
  fi

  if metadata="$(octessera_debugfs_stat_metadata "$target" "$sample_root")"; then
    status=0
  else
    status=$?
  fi
  [[ "$status" == 0 ]] || return 2
  if ! type_size="$(octessera_debugfs_type_size "$metadata")"; then
    return 2
  fi
  IFS=$'\t' read -r entry_type entry_size <<< "$type_size"
  [[ "$entry_type" == directory ]] || return 2
  directories=("$sample_root")
  while ((${#directories[@]} > 0)); do
    directory="${directories[0]}"
    directories=("${directories[@]:1}")
    if ! listing="$(octessera_debugfs_list_path "$target" "$directory")"; then
      return 2
    fi
    while IFS= read -r listing_line; do
      [[ -n "$listing_line" ]] || continue
      if ! name="$(octessera_debugfs_ls_entry_name "$listing_line")"; then
        return 2
      fi
      [[ "$name" == . || "$name" == .. ]] && continue
      full_path="$directory/$name"
      if metadata="$(octessera_debugfs_stat_metadata "$target" "$full_path")"; then
        status=0
      else
        status=$?
      fi
      [[ "$status" == 0 ]] || return 2
      if ! type_size="$(octessera_debugfs_type_size "$metadata")"; then
        return 2
      fi
      IFS=$'\t' read -r entry_type entry_size <<< "$type_size"
      relative_path="${full_path#"$sample_root/"}"
      octessera_sample_relative_path_is_safe "$relative_path" || return 2
      if [[ "$entry_type" == directory ]]; then
        inventory_type=d
        directories+=("$full_path")
      else
        inventory_type=f
      fi
      printf '%s\t%s\t%s\n' "$inventory_type" "$relative_path" "$entry_size" >> "$inventory_path" || return 2
    done <<< "$listing"
  done
  rm -f -- "$blob_path"
}

octessera_validate_sample_tree() {
  local target_path="$1"
  local inspect_work_path="$2"
  local sample_root=var/lib/octessera/samples
  local inventory_path="$inspect_work_path/sample-inventory"
  local sample_count=0
  local entry_type
  local entry_path
  local entry_size

  octessera_collect_sample_inventory "$target_path" "$sample_root" "$inventory_path" || {
    echo 'Unable to enumerate packaged sample files.' >&2
    return 1
  }
  while IFS=$'\t' read -r entry_type entry_path entry_size; do
    case "$entry_type" in
      d)
        require_root_mode "$sample_root/$entry_path" 755 || {
          echo "Unsafe packaged sample directory: $entry_path." >&2
          return 1
        }
        ;;
      f)
        [[ "${entry_path,,}" == *.wav ]] || {
          echo "Unexpected packaged sample file: $entry_path." >&2
          return 1
        }
        require_root_mode "$sample_root/$entry_path" 644 || return 1
        sample_count=$((sample_count + 1))
        ;;
      *)
        echo "Unsafe packaged sample entry: $entry_path." >&2
        return 1
        ;;
    esac
  done < "$inventory_path"
  [[ "$sample_count" -gt 0 ]] || {
    echo 'Packaged sample library is empty.' >&2
    return 1
  }
}

octessera_debugfs_is_exact_not_found() {
  local output="$1"
  local path="$2"
  local line
  local found=0

  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" ]] && continue
    if [[ "$line" =~ ^debugfs[[:space:]][0-9]+\.[0-9]+(\.[0-9]+)?[[:space:]]+\([^()]*\)$ ]]; then
      continue
    fi
    if [[ "$line" =~ ^stat:[[:space:]]File[[:space:]]not[[:space:]]found[[:space:]]by[[:space:]]ext2_lookup[[:space:]]*$ ||
          "$line" == "/$path: File not found by ext2_lookup" ||
          "$line" == "/$path: File not found by ext2_lookup " ]]; then
      found=$((found + 1))
      continue
    fi
    return 1
  done <<< "$output"
  [[ "$found" == 1 ]]
}

octessera_debugfs_stderr_is_startup_banner() {
  local output="$1"
  local line

  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" ]] && continue
    [[ "$line" =~ ^debugfs[[:space:]][0-9]+\.[0-9]+(\.[0-9]+)?[[:space:]]+\([^()]*\)$ ]] || return 1
  done <<< "$output"
}

octessera_debugfs_stat_metadata() {
  local target="$1"
  local path="$2"
  local request
  local metadata
  local status

  request="$(octessera_debugfs_stat_request "$path")" || return 2
  if metadata="$(debugfs -R "$request" "$target" 2>&1)"; then
    status=0
  else
    status=$?
  fi
  if octessera_debugfs_is_exact_not_found "$metadata" "$path"; then
    return 1
  fi
  if printf '%s\n' "$metadata" | grep -Eq '^(debugfs|stat):|File not found by ext2_lookup'; then
    [[ -z "$metadata" ]] || printf '%s\n' "$metadata" >&2
    return 2
  fi
  if [[ "$status" != 0 ]]; then
    [[ -z "$metadata" ]] || printf '%s\n' "$metadata" >&2
    return 2
  fi
  if ! printf '%s\n' "$metadata" | grep -Eq '^Inode:[[:space:]]+[0-9]+([[:space:]]|$)'; then
    [[ -z "$metadata" ]] || printf '%s\n' "$metadata" >&2
    return 2
  fi
  printf '%s\n' "$metadata"
}

octessera_directory_stat_path() {
  local target="$1"
  local path="$2"
  local candidate="$target"
  local metadata
  local previous_symlink=0
  local component
  local -a components

  IFS='/' read -r -a components <<< "$path"
  for component in "${components[@]}"; do
    [[ -n "$component" ]] || continue
    candidate="$candidate/$component"
    if metadata="$(LC_ALL=C stat -c '%F' -- "$candidate" 2>&1)"; then
      if [[ "$metadata" == 'symbolic link' ]]; then
        if LC_ALL=C stat -Lc '%F' -- "$candidate" >/dev/null 2>&1; then
          previous_symlink=0
        else
          previous_symlink=1
        fi
      else
        previous_symlink=0
      fi
      continue
    fi
    if [[ "$previous_symlink" == 1 || "$metadata" != stat:*': No such file or directory' ]]; then
      [[ -z "$metadata" ]] || printf '%s\n' "$metadata" >&2
      return 2
    fi
    return 1
  done
  return 0
}

octessera_unit_masked_path() {
  local target="$1"
  local path="$2"
  local metadata
  local metadata_status
  local actual_target

  octessera_debugfs_path_argument "$path" >/dev/null || return 2
  if [[ -d "$target" ]]; then
    [[ -L "$target/$path" ]] && [[ "$(readlink -- "$target/$path")" == '/dev/null' ]]
    return
  fi
  if metadata="$(octessera_debugfs_stat_metadata "$target" "$path")"; then
    metadata_status=0
  else
    metadata_status=$?
  fi
  [[ "$metadata_status" == 0 ]] || return "$metadata_status"
  [[ "$(octessera_debugfs_type "$metadata")" == symlink ]] || return 1
  actual_target="$(octessera_debugfs_fast_link_target "$metadata")" || return 1
  [[ "$actual_target" == /dev/null ]]
}

octessera_stat_path() {
  local target="$1"
  local path="$2"

  octessera_debugfs_path_argument "$path" >/dev/null || return 2
  if [[ -d "$target" ]]; then
    octessera_directory_stat_path "$target" "$path"
    return
  fi
  octessera_debugfs_stat_metadata "$target" "$path" >/dev/null
}
