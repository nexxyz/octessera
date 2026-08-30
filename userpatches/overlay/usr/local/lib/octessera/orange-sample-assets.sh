#!/usr/bin/env bash

install_orange_musical_assets() {
  local overlay_root="$1"
  local target_root="$2"
  local overlay_samples="$overlay_root/usr/share/octessera/samples"
  local overlay_manifest="$overlay_samples/MANIFEST.tsv"
  local target_samples="$target_root/usr/share/octessera/samples"
  local target_manifest="$target_samples/MANIFEST.tsv"
  local target_files="$target_root/var/lib/octessera/samples"
  local header
  local sample_path
  local sample_size
  local sample_sha256
  local sample_source_path
  local sample_destination
  local relative_path
  local extra
  local sample_notice
  local -a manifest_entries=()
  local -A manifest_paths=()
  local -A manifest_sizes=()
  local -A manifest_hashes=()

  [[ -d "$overlay_samples" && ! -L "$overlay_samples" ]] || { echo "Missing staged sample directory: $overlay_samples" >&2; return 1; }
  [[ -d "$overlay_samples/files" && ! -L "$overlay_samples/files" ]] || { echo "Missing staged sample files. Run tools/armbian-image/stage-musical-assets.sh." >&2; return 1; }
  [[ -f "$overlay_manifest" && ! -L "$overlay_manifest" ]] || { echo "Missing staged sample manifest: $overlay_manifest" >&2; return 1; }
  for sample_notice in SOURCE.md upstream/LICENSE; do
    [[ -f "$overlay_samples/$sample_notice" && ! -L "$overlay_samples/$sample_notice" ]] || { echo "Missing staged sample notice: $sample_notice" >&2; return 1; }
  done
  [[ ! -L "$target_samples" && ! -L "$target_samples/files" && ! -L "$target_files" ]] || { echo "Sample destination is symlinked." >&2; return 1; }
  install -d -m 0755 -o root -g root "$target_samples"
  [[ -f "$target_manifest" && ! -L "$target_manifest" ]] || { echo "Installed sample manifest is missing: $target_manifest" >&2; return 1; }
  cmp -s "$overlay_manifest" "$target_manifest" || { echo "Installed sample manifest differs from staged manifest." >&2; return 1; }
  awk -F $'\t' 'NF != 3 { exit 1 }' "$target_manifest" || { echo "Invalid packaged sample manifest rows." >&2; return 1; }

  {
    IFS= read -r header
    [[ "$header" == $'# path\tsize\tsha256' ]] || { echo "Invalid packaged sample manifest header." >&2; return 1; }
    while IFS=$'\t' read -r sample_path sample_size sample_sha256 extra; do
      case "$sample_path" in
        ''|/*|*..*|*\\*|*$'\t'*|*$'\r'*) echo "Invalid packaged sample path: $sample_path" >&2; return 1 ;;
      esac
      [[ -z "$extra" ]] || { echo "Invalid packaged sample manifest row: $sample_path" >&2; return 1; }
      [[ "$sample_size" =~ ^[0-9]+$ ]] || { echo "Invalid packaged sample size: $sample_path" >&2; return 1; }
      [[ "$sample_sha256" =~ ^[a-f0-9]{64}$ ]] || { echo "Invalid packaged sample hash: $sample_path" >&2; return 1; }
      if [[ -n "${manifest_paths["$sample_path"]+set}" ]]; then
        echo "Duplicate packaged sample path: $sample_path" >&2
        return 1
      fi
      manifest_paths["$sample_path"]=1
      manifest_sizes["$sample_path"]="$sample_size"
      manifest_hashes["$sample_path"]="$sample_sha256"
      manifest_entries+=("$sample_path")
    done
  } < "$target_manifest"
  [[ "${#manifest_entries[@]}" == 320 ]] || { echo "Packaged sample manifest does not contain the complete inventory." >&2; return 1; }

  while IFS= read -r -d '' sample_source_path; do
    if [[ -L "$sample_source_path" || ( ! -f "$sample_source_path" && ! -d "$sample_source_path" ) ]]; then
      echo "Unsafe staged sample entry: ${sample_source_path#"$overlay_samples/files/"}" >&2
      return 1
    fi
    if [[ -f "$sample_source_path" ]]; then
      relative_path="${sample_source_path#"$overlay_samples/files/"}"
      [[ -n "${manifest_paths["$relative_path"]+set}" ]] || { echo "Unlisted staged sample: $relative_path" >&2; return 1; }
    fi
  done < <(find -P "$overlay_samples/files" -mindepth 1 -print0)

  for sample_path in "${manifest_entries[@]}"; do
    sample_source_path="$overlay_samples/files/$sample_path"
    [[ -f "$sample_source_path" && ! -L "$sample_source_path" ]] || { echo "Missing packaged sample: $sample_path" >&2; return 1; }
  done

  for sample_notice in SOURCE.md upstream/LICENSE; do
    install -D -m 0644 -o root -g root -- "$overlay_samples/$sample_notice" "$target_samples/$sample_notice"
    cmp -s "$overlay_samples/$sample_notice" "$target_samples/$sample_notice" || { echo "Installed sample notice differs from staged notice: $sample_notice" >&2; return 1; }
  done

  rm -rf -- "$target_samples/files" "$target_files"
  install -d -m 0755 -o root -g root "$target_files"
  for sample_path in "${manifest_entries[@]}"; do
    sample_source_path="$overlay_samples/files/$sample_path"
    sample_destination="$target_files/$sample_path"
    install -D -m 0644 -o root -g root -- "$sample_source_path" "$sample_destination"
  done
  chown -R root:root "$target_files"
  find -P "$target_files" -type d -exec chmod 0755 {} +
  find -P "$target_files" -type f -exec chmod 0644 {} +
  while IFS= read -r -d '' sample_source_path; do
    if [[ -L "$sample_source_path" || ( ! -f "$sample_source_path" && ! -d "$sample_source_path" ) ]]; then
      echo "Unsafe installed sample entry: ${sample_source_path#"$target_files/"}" >&2
      return 1
    fi
    relative_path="${sample_source_path#"$target_files/"}"
    if [[ -d "$sample_source_path" ]]; then
      [[ "$(stat -c '%u:%g %a' "$sample_source_path")" == '0:0 755' ]] || { echo "Unsafe installed sample directory: $relative_path" >&2; return 1; }
    else
      [[ -n "${manifest_paths["$relative_path"]+set}" ]] || { echo "Unlisted installed sample: $relative_path" >&2; return 1; }
      [[ "$(stat -c '%u:%g %a' "$sample_source_path")" == '0:0 644' ]] || { echo "Unsafe installed sample file: $relative_path" >&2; return 1; }
    fi
  done < <(find -P "$target_files" -mindepth 1 -print0)

  for sample_path in "${manifest_entries[@]}"; do
    sample_destination="$target_files/$sample_path"
    [[ -f "$sample_destination" && ! -L "$sample_destination" ]] || { echo "Missing packaged sample: $sample_path" >&2; return 1; }
    [[ "$(stat -c '%s' "$sample_destination")" == "${manifest_sizes["$sample_path"]}" ]] || { echo "Packaged sample size mismatch: $sample_path" >&2; return 1; }
    [[ "$(sha256sum "$sample_destination" | awk '{ print $1 }')" == "${manifest_hashes["$sample_path"]}" ]] || { echo "Packaged sample hash mismatch: $sample_path" >&2; return 1; }
  done
}
