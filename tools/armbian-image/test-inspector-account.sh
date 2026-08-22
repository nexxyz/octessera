#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/armbian-image/test-inspector-fixture.sh
source "$script_dir/test-inspector-fixture.sh"

request="$(octessera_debugfs_stat_request 'samples/space dir\with"quote')"
[[ "$request" == 'stat "/samples/space dir\\with\"quote"' ]] || { echo 'Debugfs path escaping changed.' >&2; exit 1; }
dump_request="$(octessera_debugfs_dump_request 'samples/space dir\with"quote' "$work/destination path")"
printf '%s\n' "$dump_request" | grep -Fq '"/samples/space dir\\with\"quote"' || { echo 'Debugfs dump source was not quoted and escaped.' >&2; exit 1; }
printf '%s\n' "$dump_request" | grep -Fq "\"$work/destination path\"" || { echo 'Debugfs dump destination was not quoted.' >&2; exit 1; }
if octessera_debugfs_stat_request $'unsafe\tpath' >/dev/null; then echo 'Debugfs controls were accepted.' >&2; exit 1; else [[ "$?" == 2 ]]; fi

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
if octessera_stat_path "$directory_root" $'unsafe\npath'; then echo 'Directory control characters were accepted.' >&2; exit 1; else [[ "$?" == 2 ]]; fi
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
[[ "$(octessera_debugfs_fast_link_target 'Fast link dest: /dev/null')" == /dev/null ]] || { echo 'Raw fast-link target was not normalized.' >&2; exit 1; }
assert_status 1 octessera_debugfs_fast_link_target $'Fast link dest: "/dev/null"\nFast link dest: "/dev/null"'
assert_status 1 octessera_debugfs_fast_link_target 'Fast link dest: "/dev/null" trailing'

make_required_fixture() {
  local fixture="$1"
  mkdir -p "$fixture/etc/ssh"
  printf '%s\n' 'octessera:!:19000:0:99999:7:::' > "$fixture/etc/shadow"
}
assert_inspector_failure() {
  local fixture="$1" expected="$2" stderr_path="$work/inspector.stderr"
  if bash "$inspector" --verification-profile legacy-runtime-only "$fixture" >"$work/inspector.stdout" 2>"$stderr_path"; then
    echo "Inspector accepted malformed required files in $fixture." >&2
    exit 1
  fi
  grep -Fq "$expected" "$stderr_path" || { echo "Inspector failure did not identify $expected." >&2; cat "$stderr_path" >&2; exit 1; }
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
