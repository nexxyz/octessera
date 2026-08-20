#!/usr/bin/env bash

# shellcheck disable=SC2154
module_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck source=tools/armbian-image/inspect-runtime-account.sh
source "$module_dir/inspect-runtime-account.sh"
# shellcheck source=tools/armbian-image/inspect-runtime-service.sh
source "$module_dir/inspect-runtime-service.sh"
# shellcheck source=tools/armbian-image/inspect-runtime-udev.sh
source "$module_dir/inspect-runtime-udev.sh"
# shellcheck source=tools/armbian-image/inspect-runtime-device-apply.sh
source "$module_dir/inspect-runtime-device-apply.sh"
# shellcheck source=tools/armbian-image/inspect-runtime-oled.sh
source "$module_dir/inspect-runtime-oled.sh"
# shellcheck source=tools/armbian-image/inspect-runtime-mode.sh
source "$module_dir/inspect-runtime-mode.sh"

octessera_image_metadata_value() {
  local content="$1" key="$2" record
  record="$(printf '%s\n' "$content" | awk -F= -v wanted="$key" '$1 == wanted { count++; value = substr($0, length(wanted) + 2) } END { if (count == 1) print value; else exit 1 }')" || return 1
  printf '%s' "$record"
}

octessera_require_image_contract() {
  local expected_mode="$1" contract_content
  require_root_mode etc/octessera/image-contract.json 644
  contract_content="$(read_file etc/octessera/image-contract.json)"
  jq -e --arg expected_mode "$expected_mode" 'type == "object" and ((keys | sort) == ["image_kind", "runtime_enabled_default", "schema_version"]) and .schema_version == 1 and .image_kind == $expected_mode and (.runtime_enabled_default == (.image_kind == "production"))' <<< "$contract_content" >/dev/null || { echo "Image contract does not explicitly match mode $expected_mode." >&2; exit 1; }
  [[ "$(hash_path etc/octessera/image-contract.json)" == "$(octessera_image_metadata_value "$profile_metadata" OCTESSERA_IMAGE_CONTRACT_SHA256)" ]] || { echo 'Image contract hash is not recorded exactly in build metadata.' >&2; exit 1; }
}

octessera_require_image_symlink() {
  local path="$1" expected_target metadata actual_target
  shift
  stat_path "$path" || { echo "Missing required runtime symlink: $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then
    [[ -L "$target/$path" ]] || { echo "Runtime path is not a symlink: $path." >&2; exit 1; }
    actual_target="$(readlink -- "$target/$path")"
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$path")" || { echo "Unable to inspect runtime symlink: $path." >&2; exit 1; }
    actual_target="$(octessera_debugfs_fast_link_target "$metadata")" || { echo "Unable to inspect runtime symlink: $path." >&2; exit 1; }
    [[ "$(octessera_debugfs_type "$metadata")" == symlink ]] || { echo "Runtime path is not a symlink: $path." >&2; exit 1; }
  fi
  for expected_target in "$@"; do [[ "$actual_target" == "$expected_target" ]] && return 0; done
  echo "Runtime symlink target mismatch at $path." >&2
  exit 1
}

octessera_require_absent_path() {
  local path="$1" status
  if stat_path "$path"; then
    echo "Production image contains unsupported runtime path: $path." >&2
    exit 1
  else
    status=$?
    [[ "$status" == 1 ]] || { echo "Unable to inspect unsupported runtime path: $path." >&2; exit 1; }
  fi
}

octessera_require_runtime_entry_set() {
  local release_path="$1" entry entry_type metadata listing listing_line entries_text
  local -a entries=()
  if [[ -d "$target" ]]; then
    while IFS=$'\t' read -r entry_type entry; do [[ -n "$entry" ]] && entries+=("$entry_type:$entry"); done < <(find -P "$target/$release_path" -mindepth 1 -maxdepth 1 -printf '%y\t%f\n' | LC_ALL=C sort -k2)
  else
    listing="$(octessera_debugfs_list_path "$target" "$release_path")" || { echo "Unable to enumerate runtime release: $release_path." >&2; exit 1; }
    while IFS= read -r listing_line; do
      [[ -n "$listing_line" ]] || continue
      entry="$(octessera_debugfs_ls_entry_name "$listing_line")" || { echo 'Malformed runtime release entry.' >&2; exit 1; }
      [[ "$entry" == . || "$entry" == .. ]] && continue
      metadata="$(octessera_debugfs_stat_metadata "$target" "$release_path/$entry")" || { echo "Unable to inspect runtime release entry: $entry." >&2; exit 1; }
      entry_type="$(octessera_debugfs_type "$metadata")" || { echo 'Malformed runtime release entry.' >&2; exit 1; }
      entries+=("$entry_type:$entry")
    done <<< "$listing"
  fi
  entries_text="$(printf '%s\n' "${entries[@]}" | LC_ALL=C sort | paste -sd ' ' -)"
  [[ "$entries_text" == 'regular:SHA256SUMS regular:octessera-pi regular:octessera-runtime.json regular:update-manifest.json' || "$entries_text" == 'f:SHA256SUMS f:octessera-pi f:octessera-runtime.json f:update-manifest.json' ]] || { echo "Runtime release contains unexpected entries: $release_path." >&2; exit 1; }
}

octessera_require_real_directory() {
  local path="$1" metadata
  stat_path "$path" || { echo "Missing required runtime directory: $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then
    [[ -d "$target/$path" && ! -L "$target/$path" ]] || { echo "Runtime directory is unsafe: $path." >&2; exit 1; }
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$path")" || { echo "Unable to inspect runtime directory: $path." >&2; exit 1; }
    [[ "$(octessera_debugfs_type "$metadata")" == directory ]] || { echo "Runtime path is not a directory: $path." >&2; exit 1; }
  fi
}

octessera_require_owned_mode() {
  local path="$1" owner="$2" mode="$3" actual metadata actual_user actual_group actual_mode expected_mode
  expected_mode="$(octessera_canonical_mode "$mode")" || { echo "Invalid expected runtime mode for $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then
    actual="$(stat -c '%u:%g %a' "$target/$path")"
    actual_user="${actual%% *}"
    actual_mode="$(octessera_canonical_mode "${actual#* }")" || { echo "Invalid runtime mode: $path." >&2; exit 1; }
    actual="$actual_user $actual_mode"
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$path")" || { echo "Unable to inspect runtime ownership: $path." >&2; exit 1; }
    actual_user="$(printf '%s\n' "$metadata" | awk '/^User:/ { for (position = 1; position < NF; position++) if ($position == "User:") print $(position + 1) }')"
    actual_group="$(printf '%s\n' "$metadata" | awk '/^User:/ { for (position = 1; position < NF; position++) if ($position == "Group:") print $(position + 1) }')"
    actual_mode="$(octessera_debugfs_mode "$metadata")" || { echo "Missing runtime mode: $path." >&2; exit 1; }
    actual_mode="$(octessera_canonical_mode "$actual_mode")" || { echo "Invalid runtime mode: $path." >&2; exit 1; }
    actual="$actual_user:$actual_group $actual_mode"
  fi
  [[ "$actual" == "$owner $expected_mode" ]] || { echo "Unsafe runtime ownership or mode at $path." >&2; exit 1; }
}
