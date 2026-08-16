#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
inspect_path="$root/tools/armbian-image/inspect-path.sh"
inspector="$root/tools/armbian-image/inspect-built-image.sh"
runtime_inspector="$root/tools/armbian-image/inspect-runtime.sh"
real_debugfs="$(command -v debugfs || true)"
real_mkfs_ext4="$(command -v mkfs.ext4 || true)"
real_truncate="$(command -v truncate || true)"
work="$(mktemp -d)"
mock_bin="$work/bin"
trap 'rm -rf "$work"' EXIT

mkdir -p "$mock_bin"
bash -n "$runtime_inspector"
cat > "$mock_bin/debugfs" <<'EOF'
#!/usr/bin/env bash
case "${DEBUGFS_CASE:-unit-valid}" in
  missing)
    printf '%s\n' 'debugfs 1.47.0 (5-Feb-2023)' 'stat: File not found by ext2_lookup'
    ;;
  malformed)
    printf '%s\n' 'stat: File not found by ext2_lookup extra'
    ;;
  error)
    printf '%s\n' 'stat: Filesystem not open' >&2
    exit 1
    ;;
  unit-valid)
    printf '%s\n' 'Inode:   7   Type:   symlink   Mode:    0777   Flags: 0x0' 'Fast    link    dest:    "/dev/null"'
    ;;
  unit-wrong)
    printf '%s\n' 'Inode:   7   Type:   symlink   Mode:    0777   Flags: 0x0' 'Fast    link    dest:    "/etc/passwd"'
    ;;
  unit-regular)
    printf '%s\n' 'Inode:   7   Type:   regular   Mode:    0644   Flags: 0x0' 'Fast    link    dest:    "/dev/null"'
    ;;
  variable-whitespace)
    case "$2" in
      'stat "/opt/octessera"'|'stat "/opt/octessera/releases"'|'stat "/opt/octessera/releases/1.2.3"')
        printf '%s\n' 'Inode:   7   Type:   directory   Mode:    040755   Flags: 0x0'
        ;;
      'stat "/opt/octessera/current"')
        printf '%s\n' 'Inode:   8   Type:   symlink   Mode:    0777   Flags: 0x0' 'Fast    link    dest:    "/opt/octessera/releases/1.2.3"'
        ;;
      'stat "/opt/octessera/releases/1.2.3/SHA256SUMS"'|'stat "/opt/octessera/releases/1.2.3/octessera-pi"'|'stat "/opt/octessera/releases/1.2.3/octessera-runtime.json"')
        printf '%s\n' 'Inode:   9   Type:   regular   Mode:    0100555   Flags: 0x0'
        ;;
      'ls -p "/opt/octessera/releases/1.2.3"')
        printf '%s\n' '/9/0100555/0/0/SHA256SUMS/' '/10/0100555/0/0/octessera-pi/' '/11/0100444/0/0/octessera-runtime.json/'
        ;;
    esac
    ;;
  sample-ext4)
    case "$2" in
      'ls -p "/var/lib/octessera/samples"')
        printf '%s\n' '/2/040755/0/0/./' '/2/040755/0/0/../' '/10/040755/0/0/Drum/'
        ;;
      'ls -p "/var/lib/octessera/samples/Drum"')
        printf '%s\n' '/10/040755/0/0/./' '/10/040755/0/0/../' '/11/040755/0/0/hihat open/'
        ;;
      'ls -p "/var/lib/octessera/samples/Drum/hihat open"')
        printf '%s\n' '/11/040755/0/0/./' '/11/040755/0/0/../' '/12/100644/0/0/space.wav/'
        ;;
      'stat "/var/lib/octessera/samples"')
        printf '%s\n' 'Inode:   2   Type:   directory   Mode:    040755   Flags: 0x0' 'User:   0   Group:   0   Size:   4096'
        ;;
      'stat "/var/lib/octessera/samples/Drum"')
        printf '%s\n' 'Inode:   10   Type:   directory   Mode:    040755   Flags: 0x0' 'User:   0   Group:   0   Size:   4096'
        ;;
      'stat "/var/lib/octessera/samples/Drum/hihat open"')
        printf '%s\n' 'Inode:   11   Type:   directory   Mode:    040755   Flags: 0x0' 'User:   0   Group:   0   Size:   4096'
        ;;
      'stat "/var/lib/octessera/samples/Drum/hihat open/space.wav"')
        printf '%s\n' 'Inode:   12   Type:   regular   Mode:    0644   Flags: 0x0' 'User:   0   Group:   0   Size:   11'
        ;;
    esac
    ;;
  runtime-owner-valid|runtime-owner-wrong-owner|runtime-owner-wrong-mode)
    case "$DEBUGFS_CASE" in
      runtime-owner-valid) owner=990; group=990; mode=040755 ;;
      runtime-owner-wrong-owner) owner=991; group=990; mode=040755 ;;
      runtime-owner-wrong-mode) owner=990; group=990; mode=040700 ;;
    esac
    printf '%s\n' "Inode:   42   Type:   directory   Mode:    $mode   Flags: 0x0" "User:   $owner   Group:   $group   Size:   4096"
    ;;
esac
EOF
chmod 0755 "$mock_bin/debugfs"

# shellcheck disable=SC1090
source "$inspect_path"
export PATH="$mock_bin:$PATH"

assert_status() {
  local expected="$1"
  shift
  local actual
  if "$@"; then
    actual=0
  else
    actual=$?
  fi
  [[ "$actual" == "$expected" ]] || {
    printf 'Expected status %s, got %s: %s\n' "$expected" "$actual" "$*" >&2
    exit 1
  }
}

request="$(octessera_debugfs_stat_request 'samples/space dir\with"quote')"
[[ "$request" == 'stat "/samples/space dir\\with\"quote"' ]] || {
  echo 'Debugfs path escaping changed.' >&2
  exit 1
}
dump_request="$(octessera_debugfs_dump_request 'samples/space dir\with"quote' "$work/destination path")"
printf '%s\n' "$dump_request" | grep -Fq '"/samples/space dir\\with\"quote"' || {
  echo 'Debugfs dump source was not quoted and escaped.' >&2
  exit 1
}
printf '%s\n' "$dump_request" | grep -Fq "\"$work/destination path\"" || {
  echo 'Debugfs dump destination was not quoted.' >&2
  exit 1
}
if octessera_debugfs_stat_request $'unsafe\tpath' >/dev/null; then
  echo 'Debugfs controls were accepted.' >&2
  exit 1
else
  [[ "$?" == 2 ]]
fi

fake_image="$work/not-an-image"
for debugfs_case in missing malformed error; do
  export DEBUGFS_CASE="$debugfs_case"
  case "$debugfs_case" in
    missing) assert_status 1 octessera_stat_path "$fake_image" missing/path ;;
    malformed|error) assert_status 2 octessera_stat_path "$fake_image" missing/path ;;
  esac
done

directory_root="$work/rootfs"
mkdir -p "$directory_root/parent"
assert_status 1 octessera_stat_path "$directory_root" missing/path
ln -s missing-target "$directory_root/dangling"
assert_status 0 octessera_stat_path "$directory_root" dangling
assert_status 2 octessera_stat_path "$directory_root" dangling/child
if octessera_stat_path "$directory_root" $'unsafe\npath'; then
  echo 'Directory control characters were accepted.' >&2
  exit 1
else
  [[ "$?" == 2 ]]
fi

inaccessible="$directory_root/inaccessible"
mkdir "$inaccessible"
chmod 000 "$inaccessible"
if [[ "$(id -u)" == 0 ]] && command -v runuser >/dev/null 2>&1; then
  # shellcheck disable=SC2016
  if runuser -u nobody -- bash -c 'source "$1"; octessera_stat_path "$2" inaccessible/child' _ "$inspect_path" "$directory_root"; then
    echo 'Inaccessible directory parent was reported as present.' >&2
    exit 1
  else
    [[ "$?" == 2 ]] || { echo 'Inaccessible directory parent was reported as absent.' >&2; exit 1; }
  fi
fi
chmod 0755 "$inaccessible"

unit_root="$work/unit-root"
mkdir -p "$unit_root/etc/systemd/system"
ln -s /dev/null "$unit_root/etc/systemd/system/ssh.service"
assert_status 0 octessera_unit_masked_path "$unit_root" etc/systemd/system/ssh.service
rm "$unit_root/etc/systemd/system/ssh.service"
ln -s /etc/passwd "$unit_root/etc/systemd/system/ssh.service"
assert_status 1 octessera_unit_masked_path "$unit_root" etc/systemd/system/ssh.service
rm "$unit_root/etc/systemd/system/ssh.service"
printf '%s\n' /dev/null > "$unit_root/etc/systemd/system/ssh.service"
assert_status 1 octessera_unit_masked_path "$unit_root" etc/systemd/system/ssh.service

export DEBUGFS_CASE=unit-valid
assert_status 0 octessera_unit_masked_path "$fake_image" etc/systemd/system/ssh.service
export DEBUGFS_CASE=unit-wrong
assert_status 1 octessera_unit_masked_path "$fake_image" etc/systemd/system/ssh.service
export DEBUGFS_CASE=unit-regular
assert_status 1 octessera_unit_masked_path "$fake_image" etc/systemd/system/ssh.service
[[ "$(octessera_debugfs_fast_link_target 'Fast link dest: /dev/null')" == /dev/null ]] || {
  echo 'Raw fast-link target was not normalized.' >&2
  exit 1
}
assert_status 1 octessera_debugfs_fast_link_target $'Fast link dest: "/dev/null"\nFast link dest: "/dev/null"'
assert_status 1 octessera_debugfs_fast_link_target 'Fast link dest: "/dev/null" trailing'

target=''
# shellcheck disable=SC2317
require_root_mode() {
  local path="$1"
  local mode="$2"
  [[ "$(stat -c '%a' "$target/$path")" == "$mode" ]] || return 1
  [[ "$(id -u)" != 0 || "$(stat -c '%u:%g' "$target/$path")" == 0:0 ]]
}
# shellcheck disable=SC2317
hash_path() {
  sha256sum "$target/$1" | awk '{ print $1 }'
}
sample_path='Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav'
kick_path='Drum/kick/Kick2.wav'
sample_stage="$work/sample-stage"
python3 "$root/tools/samples/sample_library.py" \
  --repository-root "$root" \
  --media-destination "$sample_stage/samples/files" \
  --metadata-destination "$sample_stage/samples" \
  --manifest-destination "$sample_stage/samples/sample-manifest.tsv"
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
  if [[ "$(id -u)" == 0 ]]; then
    chown -R root:root "$fixture/var/lib/octessera/samples"
    chown 990:990 "$fixture/var/lib/octessera/samples"
    [[ "$(stat -c '%u:%g:%a' "$fixture/var/lib/octessera/samples")" == '990:990:755' ]]
  fi
}

validate_sample_fixture() {
  local fixture="$1"
  local manifest="$2"
  target="$fixture"
  mkdir -p "$work/sample-inspect-$3"
  octessera_validate_sample_tree "$fixture" "$manifest" "$work/sample-inspect-$3"
}

valid_samples="$work/valid-samples"
make_sample_fixture "$valid_samples"
validate_sample_fixture "$valid_samples" "$sample_manifest" valid

duplicate_manifest="$sample_manifest"$'\n'"$sample_path"$'\t'"$sample_size"$'\t'"$sample_hash"$'\t'"https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/freesound/drums/cymbal/open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav"$'\t'"https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE"
if validate_sample_fixture "$valid_samples" "$duplicate_manifest" duplicate; then
  echo 'Duplicate packaged sample path was accepted.' >&2
  exit 1
fi

extra_samples="$work/extra-samples"
cp -a "$valid_samples" "$extra_samples"
printf '%s' extra > "$extra_samples/var/lib/octessera/samples/Kick2.wav"
if validate_sample_fixture "$extra_samples" "$sample_manifest" extra; then
  echo 'Extra packaged sample file was accepted.' >&2
  exit 1
fi

symlink_samples="$work/symlink-samples"
cp -a "$valid_samples" "$symlink_samples"
ln -s 165028__rodrigo-the-mad__mini-909ish-open-hat.wav "$symlink_samples/var/lib/octessera/samples/Drum/hihat open/extra-link.wav"
if validate_sample_fixture "$symlink_samples" "$sample_manifest" symlink; then
  echo 'Packaged sample symlink was accepted.' >&2
  exit 1
fi

special_samples="$work/special-samples"
cp -a "$valid_samples" "$special_samples"
mkfifo "$special_samples/var/lib/octessera/samples/extra.fifo"
if validate_sample_fixture "$special_samples" "$sample_manifest" special; then
  echo 'Packaged sample special entry was accepted.' >&2
  exit 1
fi

size_mismatch_samples="$work/size-mismatch-samples"
cp -a "$valid_samples" "$size_mismatch_samples"
size_mismatch_manifest="$(printf '%s\n%s\t%s\t%s\t%s\t%s\n%s\t%s\t%s\t%s\t%s\n' \
  '# path	size	sha256	source	license_source' \
  "$sample_path" "$((sample_size + 1))" "$sample_hash" \
  'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/freesound/drums/cymbal/open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav' \
  'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE' \
  "$kick_path" "$kick_size" "$kick_hash" \
  'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/microlag/One-Shots/Drums/Kick2.wav' \
  'https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE')"
if validate_sample_fixture "$size_mismatch_samples" "$size_mismatch_manifest" size; then
  echo 'Packaged sample size mismatch was accepted.' >&2
  exit 1
fi

wrong_directory="$work/wrong-directory"
cp -a "$valid_samples" "$wrong_directory"
chmod 0700 "$wrong_directory/var/lib/octessera/samples/Drum"
if [[ "$(id -u)" == 0 ]]; then
  chown nobody:nogroup "$wrong_directory/var/lib/octessera/samples/Drum" 2>/dev/null || chown 65534:65534 "$wrong_directory/var/lib/octessera/samples/Drum"
fi
if validate_sample_fixture "$wrong_directory" "$sample_manifest" wrong-directory; then
  echo 'Wrong packaged sample directory owner/mode was accepted.' >&2
  exit 1
fi

export DEBUGFS_CASE=sample-ext4
ext4_inventory="$work/ext4-inventory"
octessera_collect_sample_inventory "$fake_image" var/lib/octessera/samples "$ext4_inventory"
grep -Fqx $'d\tDrum\t4096' "$ext4_inventory"
grep -Fqx $'d\tDrum/hihat open\t4096' "$ext4_inventory"
grep -Fqx $'f\tDrum/hihat open/space.wav\t11' "$ext4_inventory"

if [[ -n "$real_debugfs" && -n "$real_mkfs_ext4" && -n "$real_truncate" ]]; then
  real_image="$work/real-sample.ext4"
  real_host_sample="$work/real sample.wav"
  "$real_truncate" -s 16M "$real_image"
  "$real_mkfs_ext4" -q -F "$real_image"
  for directory in \
    usr \
    usr/share \
    usr/share/octessera \
    var \
    var/lib \
    var/lib/octessera \
    var/lib/octessera/samples \
    var/lib/octessera/samples/Drum \
    'var/lib/octessera/samples/Drum/hihat open'; do
    "$real_debugfs" -w -R "mkdir \"/$directory\"" "$real_image" >/dev/null 2>&1
  done
  printf '%s' 'real sample' > "$real_host_sample"
  "$real_debugfs" -w -R "write \"$real_host_sample\" \"/var/lib/octessera/samples/Drum/hihat open/space.wav\"" "$real_image" >/dev/null 2>&1
  real_ext4_inventory="$work/real-ext4-inventory"
  real_path="${PATH#"$mock_bin:"}"
  PATH="$real_path" octessera_collect_sample_inventory "$real_image" var/lib/octessera/samples "$real_ext4_inventory"
  real_symlink_path='var/lib/octessera/samples/quoted-target'
  "$real_debugfs" -w -R "symlink \"/$real_symlink_path\" \"/opt/octessera/releases/1.2.3\"" "$real_image" >/dev/null 2>&1
  real_symlink_metadata="$(PATH="$real_path" octessera_debugfs_stat_metadata "$real_image" "$real_symlink_path")"
  [[ "$(octessera_debugfs_fast_link_target "$real_symlink_metadata")" == /opt/octessera/releases/1.2.3 ]] || {
    echo 'Real ext4 fast-link target was not normalized.' >&2
    exit 1
  }
  grep -Fq $'d\tDrum\t' "$real_ext4_inventory"
  grep -Fq $'d\tDrum/hihat open\t' "$real_ext4_inventory"
  grep -Fqx $'f\tDrum/hihat open/space.wav\t11' "$real_ext4_inventory"
fi

make_required_fixture() {
  local fixture="$1"
  mkdir -p "$fixture/etc/ssh"
  printf '%s\n' 'octessera:!:19000:0:99999:7:::' > "$fixture/etc/shadow"
}

assert_inspector_failure() {
  local fixture="$1"
  local expected="$2"
  local stderr_path="$work/inspector.stderr"
  if bash "$inspector" "$fixture" >"$work/inspector.stdout" 2>"$stderr_path"; then
    echo "Inspector accepted malformed required files in $fixture." >&2
    exit 1
  fi
  grep -Fq "$expected" "$stderr_path" || {
    echo "Inspector failure did not identify $expected." >&2
    cat "$stderr_path" >&2
    exit 1
  }
}

missing_shadow="$work/missing-shadow"
mkdir -p "$missing_shadow/etc/ssh"
assert_inspector_failure "$missing_shadow" 'Unable to read required image path: etc/shadow.'

missing_passwd="$work/missing-passwd"
make_required_fixture "$missing_passwd"
assert_inspector_failure "$missing_passwd" 'Unable to read required image path: etc/passwd.'

missing_login_defs="$work/missing-login-defs"
make_required_fixture "$missing_login_defs"
printf '%s\n' 'root:x:0:0:root:/root:/bin/bash' > "$missing_login_defs/etc/passwd"
assert_inspector_failure "$missing_login_defs" 'Unable to read required image path: etc/login.defs.'

missing_account="$work/missing-account"
make_required_fixture "$missing_account"
printf '%s\n' 'UID_MIN 1000' > "$missing_account/etc/login.defs"
printf '%s\n' 'root:x:0:0:root:/root:/bin/bash' > "$missing_account/etc/passwd"
assert_inspector_failure "$missing_account" 'missing the expected octessera account'

malformed_account="$work/malformed-account"
make_required_fixture "$malformed_account"
printf '%s\n' 'UID_MIN 1000' > "$malformed_account/etc/login.defs"
printf '%s\n' 'octessera:x:1000:1000:Octessera:/srv/octessera:/bin/bash' > "$malformed_account/etc/passwd"
assert_inspector_failure "$malformed_account" 'unexpected octessera account'

# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$root/tools/armbian-image/inspect-mode.sh"
# shellcheck source=tools/armbian-image/inspect-runtime.sh
source "$runtime_inspector"
assert_runtime_owned_mode_status() {
  local expected="$1"
  shift
  local actual
  if (octessera_require_owned_mode "$@"); then actual=0; else actual=$?; fi
  [[ "$actual" == "$expected" ]] || { echo "Expected runtime ownership status $expected, got $actual." >&2; exit 1; }
}
target="$fake_image"
export DEBUGFS_CASE=runtime-owner-valid
assert_runtime_owned_mode_status 0 var/lib/octessera/presets 990:990 755
assert_runtime_owned_mode_status 0 var/lib/octessera/presets 990:990 0755
export DEBUGFS_CASE=runtime-owner-wrong-owner
assert_runtime_owned_mode_status 1 var/lib/octessera/presets 990:990 755
export DEBUGFS_CASE=runtime-owner-wrong-mode
assert_runtime_owned_mode_status 1 var/lib/octessera/presets 990:990 755
runtime_directory="$work/runtime-owned"
mkdir -p "$runtime_directory/var/lib/octessera/presets"
chmod 0755 "$runtime_directory/var/lib/octessera/presets"
directory_owner="$(stat -c '%u:%g' "$runtime_directory/var/lib/octessera/presets")"
target="$runtime_directory"
assert_runtime_owned_mode_status 0 var/lib/octessera/presets "$directory_owner" 755
chmod 0700 "$runtime_directory/var/lib/octessera/presets"
assert_runtime_owned_mode_status 1 var/lib/octessera/presets "$directory_owner" 755
assert_runtime_owned_mode_status 1 var/lib/octessera/presets "$directory_owner" 07555
target="$fake_image"
stat_path() { octessera_stat_path "$target" "$1"; }
export DEBUGFS_CASE=variable-whitespace
assert_status 0 octessera_require_real_directory opt/octessera
assert_status 0 octessera_require_runtime_entry_set opt/octessera/releases/1.2.3
# shellcheck disable=SC2218
octessera_require_image_symlink opt/octessera/current /opt/octessera/releases/1.2.3
runtime_contract="$root/userpatches/overlay/etc/octessera/image-contract.json"
runtime_contract_hash="$(sha256sum "$runtime_contract" | awk '{ print $1 }')"
# apply lane fixtures
device_apply_socket_unit="$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket"
device_apply_service_unit="$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service"
device_config_validator="$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
device_apply_helper="$root/userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot"
pi_default="$root/config/generated/pi/default.json"
mkdir -p "$fake_image/etc/systemd/system/sockets.target.wants"
ln -s ../octessera-device-apply-reboot.socket "$fake_image/etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket"
runtime_rejected_paths=()
read_file() {
  case "$1" in
    etc/octessera/image-contract.json) cat -- "$runtime_contract" ;;
    etc/systemd/system/octessera-device-apply-reboot.socket) cat -- "$device_apply_socket_unit" ;;
    etc/systemd/system/octessera-device-apply-reboot@.service) cat -- "$device_apply_service_unit" ;;
    etc/passwd) printf '%s\n' 'octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin' ;;
    usr/local/lib/octessera/device_config.py) cat -- "$device_config_validator" ;;
    usr/local/sbin/octessera-device-apply-reboot) cat -- "$device_apply_helper" ;;
    usr/share/octessera/defaults/pi-default.json) cat -- "$pi_default" ;;
    *) return 1 ;;
  esac
}
require_root_mode() { :; }
hash_path() { [[ "$1" == etc/octessera/image-contract.json ]] && printf '%s\n' "$runtime_contract_hash"; }
reject_path() { runtime_rejected_paths+=("$1"); }
octessera_require_orange_boot_service() { :; }
octessera_require_orange_shutdown_service() { :; }
octessera_require_orange_suspend_service() { :; }
octessera_require_real_directory() { :; }
octessera_require_owned_mode() { :; }
profile_metadata=$'OCTESSERA_IMAGE_MODE=diagnostic\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\nOCTESSERA_IMAGE_CONTRACT_SHA256='"$runtime_contract_hash"$'\nOCTESSERA_RUNTIME_VERSION=none\nOCTESSERA_RUNTIME_BINARY_SHA256=none\nOCTESSERA_RUNTIME_MANIFEST_SHA256=none\nOCTESSERA_RUNTIME_METADATA_SHA256=none'
octessera_inspect_runtime_mode "$profile_metadata" diagnostic
[[ "${runtime_rejected_paths[*]}" == 'etc/systemd/system/octessera.service etc/systemd/system/multi-user.target.wants/octessera.service usr/local/bin/octessera-pi opt/octessera/current opt/octessera/releases' ]] || {
  echo 'Diagnostic inspector did not reject every runtime path.' >&2
  exit 1
}

runtime_binary_hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
runtime_manifest_hash=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
runtime_metadata_hash=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
runtime_root="$work/runtime-root"
mkdir -p "$runtime_root/etc/udev/rules.d"
printf '%s\n' 'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"' > "$runtime_root/etc/udev/rules.d/70-octessera-orange-runtime.rules"
printf '%s\n' 'KERNEL=="wlan*", ACTION=="add", RUN+="/sbin/iw dev %k set power_save off"' > "$runtime_root/etc/udev/rules.d/10-wifi-power-save.rules"
ln -s /dev/null "$runtime_root/etc/udev/rules.d/09-disabled.rules"
target="$runtime_root"
read_file() {
  case "$1" in
    etc/shadow) printf '%s\n' 'octessera-runtime:!:19000:0:99999:7:::' ;;
    etc/passwd) printf '%s\n' 'octessera:x:1000:1000:Octessera:/home/octessera:/bin/bash' 'octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin' ;;
    etc/group) printf '%s\n' 'octessera:x:1000:' 'octessera-runtime:x:990:' 'audio:x:29:octessera-runtime' 'i2c:x:100:octessera-runtime' 'spi:x:999:octessera-runtime' 'gpio:x:997:octessera-runtime' ;;
    etc/sudoers) printf '%s\n' 'octessera-runtime ALL=(ALL) NOPASSWD:ALL' ;;
    etc/udev/rules.d/70-octessera-orange-runtime.rules) printf '%s\n' 'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"' 'KERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"' ;;
    etc/systemd/system/octessera.service) cat "$root/userpatches/overlay/etc/systemd/system/octessera.service" ;;
    etc/systemd/system/octessera-device-apply-reboot.socket) cat "$device_apply_socket_unit" ;;
    etc/systemd/system/octessera-device-apply-reboot@.service) cat "$device_apply_service_unit" ;;
    usr/local/lib/octessera/device_config.py) cat "$device_config_validator" ;;
    usr/local/sbin/octessera-device-apply-reboot) cat "$device_apply_helper" ;;
    usr/share/octessera/defaults/pi-default.json) cat "$pi_default" ;;
    opt/octessera/releases/1.2.3/octessera-runtime.json) printf '%s\n' "{\"name\":\"octessera-pi\",\"profile\":\"orange-pi-zero-2w\",\"version\":\"1.2.3\",\"artifact_kind\":\"production-runtime\",\"runtime_ready\":true,\"binary_sha256\":\"$runtime_binary_hash\"}" ;;
    opt/octessera/releases/1.2.3/SHA256SUMS) printf '%s  octessera-pi\n' "$runtime_binary_hash" ;;
    *) return 1 ;;
  esac
}
hash_path() {
  case "$1" in
    opt/octessera/releases/1.2.3/octessera-pi) printf '%s\n' "$runtime_binary_hash" ;;
    opt/octessera/releases/1.2.3/SHA256SUMS) printf '%s\n' "$runtime_manifest_hash" ;;
    opt/octessera/releases/1.2.3/octessera-runtime.json) printf '%s\n' "$runtime_metadata_hash" ;;
    *) return 1 ;;
  esac
}
require_root_mode() { :; }
stat_path() { [[ -e "$target/$1" || -L "$target/$1" ]]; }
octessera_require_image_contract() { [[ "$1" == production ]]; }
octessera_require_absent_path() { :; }
octessera_require_runtime_entry_set() { :; }
octessera_require_real_directory() { :; }
octessera_require_runtime_elf() { :; }
octessera_require_owned_mode() { :; }
runtime_links=()
octessera_require_image_symlink() { runtime_links+=("$1=$2"); }
profile_metadata=$'OCTESSERA_IMAGE_MODE=production\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=true\nOCTESSERA_RUNTIME_VERSION=1.2.3\nOCTESSERA_RUNTIME_BINARY_SHA256='"$runtime_binary_hash"$'\nOCTESSERA_RUNTIME_MANIFEST_SHA256='"$runtime_manifest_hash"$'\nOCTESSERA_RUNTIME_METADATA_SHA256='"$runtime_metadata_hash"
octessera_inspect_runtime_mode "$profile_metadata" production
[[ "${runtime_links[*]}" == 'etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket=../octessera-device-apply-reboot.socket opt/octessera/current=/opt/octessera/releases/1.2.3 usr/local/bin/octessera-pi=/opt/octessera/current/octessera-pi etc/systemd/system/multi-user.target.wants/octessera.service=../octessera.service' ]] || {
  echo 'Production inspector did not require the exact symlink chain.' >&2
  exit 1
}
touch "$runtime_root/etc/sudoers"
if ( octessera_require_runtime_account "$(read_file etc/passwd)" "$(read_file etc/group)" ); then
  echo 'Runtime account appeared in sudoers.' >&2
  exit 1
fi
rm -f "$runtime_root/etc/sudoers"
bad_groups="$(read_file etc/group)"$'\n''sudo:x:27:octessera-runtime'
if ( octessera_require_runtime_account "$(read_file etc/passwd)" "$bad_groups" ); then
  echo 'Runtime account appeared in the sudo admin group.' >&2
  exit 1
fi

printf '%s\n' 'Armbian inspector fixtures passed.'
