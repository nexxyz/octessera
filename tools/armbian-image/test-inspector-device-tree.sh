#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/armbian-image/test-inspector-fixture.sh
source "$script_dir/test-inspector-fixture.sh"

target=''
# shellcheck disable=SC2317
require_root_mode() {
  local path="$1" mode="$2"
  [[ "$(stat -c '%a' "$target/$path")" == "$mode" ]] || return 1
  [[ "$(id -u)" != 0 || "$(stat -c '%u:%g' "$target/$path")" == 0:0 ]]
}
hash_path() { sha256sum "$target/$1" | awk '{ print $1 }'; }
sample_path='Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav'
kick_path='Drum/kick/Kick2.wav'
sample_stage="$work/sample-stage"
python3 "$root/tools/samples/sample_library.py" --repository-root "$root" --media-destination "$sample_stage/samples/files" --metadata-destination "$sample_stage/samples" --manifest-destination "$sample_stage/samples/sample-manifest.tsv"
sample_size="$(awk -F $'\t' -v path="$sample_path" '$1 == path { print $2; exit }' "$sample_stage/samples/sample-manifest.tsv")"
sample_hash="$(awk -F $'\t' -v path="$sample_path" '$1 == path { print $3; exit }' "$sample_stage/samples/sample-manifest.tsv")"
kick_size="$(awk -F $'\t' -v path="$kick_path" '$1 == path { print $2; exit }' "$sample_stage/samples/sample-manifest.tsv")"
kick_hash="$(awk -F $'\t' -v path="$kick_path" '$1 == path { print $3; exit }' "$sample_stage/samples/sample-manifest.tsv")"
sample_manifest="$(cat "$sample_stage/samples/sample-manifest.tsv")"
make_sample_fixture() {
  local fixture="$1"
  mkdir -p "$fixture/usr/share/octessera/samples" "$fixture/var/lib/octessera/samples"
  cp -a "$sample_stage/samples/files/." "$fixture/var/lib/octessera/samples/"
  find -P "$fixture/var/lib/octessera/samples" -type d -exec chmod 0755 {} +
  find -P "$fixture/var/lib/octessera/samples" -type f -exec chmod 0644 {} +
  if [[ "$(id -u)" == 0 ]]; then chown -R root:root "$fixture/var/lib/octessera/samples"; chown 990:990 "$fixture/var/lib/octessera/samples"; [[ "$(stat -c '%u:%g:%a' "$fixture/var/lib/octessera/samples")" == '990:990:755' ]]; fi
}
validate_sample_fixture() {
  local fixture="$1" manifest="$2"
  target="$fixture"
  mkdir -p "$work/sample-inspect-$3"
  octessera_validate_sample_tree "$fixture" "$manifest" "$work/sample-inspect-$3"
}
valid_samples="$work/valid-samples"
make_sample_fixture "$valid_samples"
validate_sample_fixture "$valid_samples" "$sample_manifest" valid
duplicate_manifest="$sample_manifest"$'\n'"$sample_path"$'\t'"$sample_size"$'\t'"$sample_hash"$'\t''https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/freesound/drums/cymbal/open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav'$'\t''https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE'
if validate_sample_fixture "$valid_samples" "$duplicate_manifest" duplicate; then echo 'Duplicate packaged sample path was accepted.' >&2; exit 1; fi
extra_samples="$work/extra-samples"; cp -a "$valid_samples" "$extra_samples"; printf '%s' extra > "$extra_samples/var/lib/octessera/samples/Kick2.wav"; if validate_sample_fixture "$extra_samples" "$sample_manifest" extra; then echo 'Extra packaged sample file was accepted.' >&2; exit 1; fi
symlink_samples="$work/symlink-samples"; cp -a "$valid_samples" "$symlink_samples"; ln -s 165028__rodrigo-the-mad__mini-909ish-open-hat.wav "$symlink_samples/var/lib/octessera/samples/Drum/hihat open/extra-link.wav"; if validate_sample_fixture "$symlink_samples" "$sample_manifest" symlink; then echo 'Packaged sample symlink was accepted.' >&2; exit 1; fi
special_samples="$work/special-samples"; cp -a "$valid_samples" "$special_samples"; mkfifo "$special_samples/var/lib/octessera/samples/extra.fifo"; if validate_sample_fixture "$special_samples" "$sample_manifest" special; then echo 'Packaged sample special entry was accepted.' >&2; exit 1; fi
size_mismatch_samples="$work/size-mismatch-samples"; cp -a "$valid_samples" "$size_mismatch_samples"
size_mismatch_manifest="$(printf '%s\n%s\t%s\t%s\t%s\t%s\n%s\t%s\t%s\t%s\t%s\n' '# path\tsize\tsha256\tsource\tlicense_source' "$sample_path" "$((sample_size + 1))" "$sample_hash" 'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/freesound/drums/cymbal/open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav' 'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE' "$kick_path" "$kick_size" "$kick_hash" 'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/microlag/One-Shots/Drums/Kick2.wav' 'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE')"
if validate_sample_fixture "$size_mismatch_samples" "$size_mismatch_manifest" size; then echo 'Packaged sample size mismatch was accepted.' >&2; exit 1; fi
wrong_directory="$work/wrong-directory"; cp -a "$valid_samples" "$wrong_directory"; chmod 0700 "$wrong_directory/var/lib/octessera/samples/Drum"; if [[ "$(id -u)" == 0 ]]; then chown nobody:nogroup "$wrong_directory/var/lib/octessera/samples/Drum" 2>/dev/null || chown 65534:65534 "$wrong_directory/var/lib/octessera/samples/Drum"; fi; if validate_sample_fixture "$wrong_directory" "$sample_manifest" wrong-directory; then echo 'Wrong packaged sample directory owner/mode was accepted.' >&2; exit 1; fi
export DEBUGFS_CASE=sample-ext4
ext4_inventory="$work/ext4-inventory"
octessera_collect_sample_inventory "$fake_image" var/lib/octessera/samples "$ext4_inventory"
grep -Fqx $'d\tDrum\t4096' "$ext4_inventory"
grep -Fqx $'d\tDrum/hihat open\t4096' "$ext4_inventory"
grep -Fqx $'f\tDrum/hihat open/space.wav\t11' "$ext4_inventory"
if [[ -n "$real_debugfs" && -n "$real_mkfs_ext4" && -n "$real_truncate" ]]; then
  real_image="$work/real-sample.ext4"; real_host_sample="$work/real sample.wav"; "$real_truncate" -s 16M "$real_image"; "$real_mkfs_ext4" -q -F "$real_image"
  for directory in usr usr/share usr/share/octessera var var/lib var/lib/octessera var/lib/octessera/samples var/lib/octessera/samples/Drum 'var/lib/octessera/samples/Drum/hihat open'; do "$real_debugfs" -w -R "mkdir \"/$directory\"" "$real_image" >/dev/null 2>&1; done
  printf '%s' 'real sample' > "$real_host_sample"; "$real_debugfs" -w -R "write \"$real_host_sample\" \"/var/lib/octessera/samples/Drum/hihat open/space.wav\"" "$real_image" >/dev/null 2>&1
  real_ext4_inventory="$work/real-ext4-inventory"; real_path="${PATH#"$mock_bin:"}"; PATH="$real_path" octessera_collect_sample_inventory "$real_image" var/lib/octessera/samples "$real_ext4_inventory"; real_symlink_path='var/lib/octessera/samples/quoted-target'; "$real_debugfs" -w -R "symlink \"/$real_symlink_path\" \"/opt/octessera/releases/1.2.3\"" "$real_image" >/dev/null 2>&1; real_symlink_metadata="$(PATH="$real_path" octessera_debugfs_stat_metadata "$real_image" "$real_symlink_path")"; [[ "$(octessera_debugfs_fast_link_target "$real_symlink_metadata")" == /opt/octessera/releases/1.2.3 ]] || { echo 'Real ext4 fast-link target was not normalized.' >&2; exit 1; }; grep -Fq $'d\tDrum\t' "$real_ext4_inventory"; grep -Fq $'d\tDrum/hihat open\t' "$real_ext4_inventory"; grep -Fqx $'f\tDrum/hihat open/space.wav\t11' "$real_ext4_inventory"
fi
