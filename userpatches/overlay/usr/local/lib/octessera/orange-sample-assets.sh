#!/usr/bin/env bash

install_orange_musical_assets() {
  local overlay_root="$1"
  local target_root="$2"
  local overlay_samples="$overlay_root/usr/share/octessera/samples"
  local target_samples="$target_root/usr/share/octessera/samples"
  local target_files="$target_root/var/lib/octessera/samples"
  local sample_source_path

  [[ -d "$overlay_samples/files" && ! -L "$overlay_samples/files" ]] || { echo "Missing staged sample files. Run tools/armbian-image/stage-musical-assets.sh." >&2; return 1; }
  [[ -f "$overlay_samples/upstream/LICENSE" && ! -L "$overlay_samples/upstream/LICENSE" ]] || { echo "Missing staged sample licence." >&2; return 1; }
  if find -P "$overlay_samples/files" -mindepth 1 ! -type f ! -type d -print -quit | grep -q .; then
    echo "Staged sample tree contains an unsafe entry." >&2
    return 1
  fi
  find -P "$overlay_samples/files" -type f -iname '*.wav' -print -quit | grep -q . || { echo "Staged sample library is empty." >&2; return 1; }
  [[ ! -L "$target_samples" && ! -L "$target_files" ]] || { echo "Sample destination is symlinked." >&2; return 1; }

  install -D -m 0644 -o root -g root -- "$overlay_samples/upstream/LICENSE" "$target_samples/upstream/LICENSE"
  rm -rf -- "$target_samples/files" "$target_files"
  install -d -m 0755 -o root -g root "$target_files"
  while IFS= read -r -d '' sample_source_path; do
    install -D -m 0644 -o root -g root -- "$sample_source_path" "$target_files/${sample_source_path#"$overlay_samples/files/"}"
  done < <(find -P "$overlay_samples/files" -type f -print0)
  find -P "$target_files" -type d -exec chmod 0755 {} +
  chown -R root:root "$target_files"
}
