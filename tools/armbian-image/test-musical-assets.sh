#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture_root="$(mktemp -d)"
trap 'rm -rf "$fixture_root"' EXIT
bash "$root/tools/armbian-image/stage-musical-assets.sh" "$fixture_root/usr/share/octessera"
staging="$fixture_root/usr/share/octessera"
default_source="$root/config/generated/pi/default.json"
default_staged="$staging/defaults/pi-default.json"
manifest="$staging/samples/sample-manifest.tsv"

cmp "$default_source" "$default_staged"
validate_manifest() {
  local manifest_path="$1"
  local sample_root="$2"
  local ownership_required="${3:-false}"
  python3 - "$root" "$manifest_path" "$sample_root" "$ownership_required" <<'PY'
import hashlib
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(sys.argv[1]) / "tools/samples"))
from sample_library import read_inventory

manifest_path = pathlib.Path(sys.argv[2])
sample_root = pathlib.Path(sys.argv[3])
ownership_mode = sys.argv[4]
ownership_required = ownership_mode in {"root", "orange-final"}
inventory = read_inventory(pathlib.Path(sys.argv[1]) / "samples/ATTRIBUTIONS.tsv")
expected = {record.path: record for record in inventory}
rows = {}
lines = manifest_path.read_text(encoding="utf-8").splitlines()
if lines[0] != "# path\tsize\tsha256\tsource\tlicense_source":
    raise SystemExit("invalid staged sample manifest header")
if len(lines) != len(expected) + 1:
    raise SystemExit("manifest does not contain the complete attribution inventory")
for line in lines[1:]:
    path, size, digest, source, license_source = line.split("\t")
    record = expected.get(path)
    if record is None or (int(size), digest, source, license_source) != (record.size, record.sha256, record.source_url, record.license_url):
        raise SystemExit(f"manifest row differs from attribution inventory: {path}")
    sample = sample_root / path
    if sample.is_symlink() or not sample.is_file():
        raise SystemExit(f"missing staged sample: {sample}")
    if ownership_required and (sample.stat().st_uid != 0 or sample.stat().st_gid != 0):
        raise SystemExit(f"sample is not root-owned: {path}")
    if ownership_required and (sample.stat().st_mode & 0o777) != 0o644:
        raise SystemExit(f"sample has unsafe mode: {path}")
    if sample.stat().st_size != int(size):
        raise SystemExit(f"sample size mismatch: {path}")
    if hashlib.sha256(sample.read_bytes()).hexdigest() != digest:
        raise SystemExit(f"sample hash mismatch: {path}")
    rows[path] = record
actual = []
if sample_root.is_symlink() or not sample_root.is_dir():
    raise SystemExit(f"sample root is not a directory: {sample_root}")
if ownership_mode == "orange-final":
    metadata = sample_root.stat()
    if (metadata.st_uid, metadata.st_gid, metadata.st_mode & 0o777) != (990, 990, 0o755):
        raise SystemExit("Orange sample root does not represent the post-account-setup ownership")
for sample in sample_root.rglob("*"):
    if sample.is_symlink():
        raise SystemExit(f"sample tree contains a symlink: {sample}")
    if sample.is_dir():
        if ownership_required and (sample.stat().st_uid != 0 or sample.stat().st_gid != 0):
            raise SystemExit(f"sample directory is not root-owned: {sample}")
        if ownership_required and (sample.stat().st_mode & 0o777) != 0o755:
            raise SystemExit(f"sample directory has unsafe mode: {sample}")
        continue
    if not sample.is_file():
        raise SystemExit(f"sample tree contains a special entry: {sample}")
    if sample.suffix.lower() in {".aif", ".aiff", ".flac", ".mp3", ".ogg", ".wav"}:
        actual.append(sample.relative_to(sample_root).as_posix())
if rows.keys() != expected.keys():
    raise SystemExit("manifest does not match the complete attribution inventory")
if set(actual) != set(expected):
    raise SystemExit("sample tree does not match the complete attribution inventory")
PY
}
validate_manifest "$manifest" "$staging/samples/files"
grep -qF 'mv -T -n' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
# shellcheck disable=SC2016
grep -qF 'temporary=$(mktemp "$staging_directory/' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
# shellcheck disable=SC2016
if grep -qF 'mktemp "$presets_directory/' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"; then
  echo "Provisioner stages candidates inside the runtime-writable presets directory." >&2
  exit 1
fi
grep -q 'ExecStart=/usr/local/sbin/octessera-provision-musical-default' "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
grep -qFx 'Description=Seed a missing Octessera Pi default' "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
run_as_root() {
  if [[ "$(id -u)" == 0 ]]; then
    "$@"
    return
  fi
  command -v sudo >/dev/null 2>&1 || { echo "Root privileges are required for musical asset installation tests." >&2; return 1; }
  sudo -n -- "$@"
}
install_work=
provision_work=
cleanup() {
  local fixture
  for fixture in "$install_work" "$provision_work"; do
    [[ -n "$fixture" ]] || continue
    run_as_root rm -rf -- "$fixture"
  done
  rm -rf -- "$fixture_root"
}
trap cleanup EXIT
install_work="$(mktemp -d)"
provision_work="$(mktemp -d)"
fake_overlay="$install_work/overlay"
fake_root="$install_work/root"
mkdir -p "$fake_overlay/usr/share/octessera" "$fake_root/usr/share/octessera/samples" "$fake_root/var/lib/octessera/samples"
cp -a "$staging/defaults" "$fake_overlay/usr/share/octessera/"
cp -a "$staging/samples" "$fake_overlay/usr/share/octessera/"
cp -a "$staging/samples/." "$fake_root/usr/share/octessera/samples/"
cp "$root/userpatches/overlay/usr/local/lib/octessera/orange-sample-assets.sh" "$install_work/install-musical-assets.sh"
grep -qF 'install_orange_musical_assets() {' "$install_work/install-musical-assets.sh" || { echo "Could not stage musical asset installer." >&2; exit 1; }
cat > "$install_work/run-install-musical-assets.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

installer="$1"
overlay_root="$2"
target_root="$3"
# shellcheck disable=SC1090,SC1091
source "$installer"
install_orange_musical_assets "$overlay_root" "$target_root"
EOF
chmod 0755 "$install_work/run-install-musical-assets.sh"
run_as_root "$install_work/run-install-musical-assets.sh" "$install_work/install-musical-assets.sh" "$fake_overlay" "$fake_root"
run_as_root chown 990:990 "$fake_root/var/lib/octessera/samples"
validate_manifest "$fake_root/usr/share/octessera/samples/sample-manifest.tsv" "$fake_root/var/lib/octessera/samples" orange-final
test -f "$fake_root/var/lib/octessera/samples/Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav"
test ! -e "$fake_root/usr/share/octessera/samples/files"

expect_installer_rejects() {
  local name="$1"
  local invalid_overlay="$install_work/${name}-overlay"
  local invalid_root="$install_work/${name}-root"
  cp -a "$fake_overlay" "$invalid_overlay"
  mkdir -p "$invalid_root/usr/share/octessera/samples" "$invalid_root/var/lib/octessera/samples"
  cp -a "$staging/samples/." "$invalid_root/usr/share/octessera/samples/"
  if run_as_root "$install_work/run-install-musical-assets.sh" "$install_work/install-musical-assets.sh" "$invalid_overlay" "$invalid_root" >/dev/null 2>&1; then
    echo "Musical asset installer accepted ${name} fixture." >&2
    return 1
  fi
}

extra_overlay="$install_work/extra-overlay"
cp -a "$fake_overlay" "$extra_overlay"
printf 'extra\n' > "$extra_overlay/usr/share/octessera/samples/files/extra sample.wav"
expect_installer_rejects extra

symlink_overlay="$install_work/symlink-overlay"
cp -a "$fake_overlay" "$symlink_overlay"
symlink_sample="$symlink_overlay/usr/share/octessera/samples/files/Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav"
rm -f "$symlink_sample"
ln -s "../kick/Kick2.wav" "$symlink_sample"
expect_installer_rejects symlink

if command -v mkfifo >/dev/null 2>&1; then
  special_overlay="$install_work/special-overlay"
  cp -a "$fake_overlay" "$special_overlay"
  mkfifo "$special_overlay/usr/share/octessera/samples/files/special entry"
  expect_installer_rejects special
fi

invalid_overlay="$install_work/invalid-overlay"
invalid_root="$install_work/invalid-root"
cp -a "$fake_overlay" "$invalid_overlay"
rm "$invalid_overlay/usr/share/octessera/samples/files/Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav"
mkdir -p "$invalid_root/usr/share/octessera/samples" "$invalid_root/var/lib/octessera/samples"
cp -a "$staging/samples/." "$invalid_root/usr/share/octessera/samples/"
if run_as_root "$install_work/run-install-musical-assets.sh" "$install_work/install-musical-assets.sh" "$invalid_overlay" "$invalid_root" >/dev/null 2>&1; then
  echo "Musical asset installer accepted a missing manifest asset." >&2
  exit 1
fi
provision_script="$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
reset_provision_work() {
  runtime_uid_fixture="${1:-990}"
  runtime_gid_fixture="${2:-990}"
  runtime_group_gid_fixture="${3:-$runtime_gid_fixture}"
  rm -rf -- "$provision_work"
  mkdir -p "$provision_work/etc" "$provision_work/usr/share/octessera/defaults" "$provision_work/var/lib/octessera" "$provision_work/var/lib/octessera/samples"
  printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' "octessera-runtime:x:$runtime_uid_fixture:$runtime_gid_fixture:Octessera runtime:/nonexistent:/usr/sbin/nologin" > "$provision_work/etc/passwd"
  printf '%s\n' 'root:x:0:' "octessera-runtime:x:$runtime_group_gid_fixture:" > "$provision_work/etc/group"
  cp "$default_source" "$provision_work/usr/share/octessera/defaults/pi-default.json"
  printf 'keep this user sample\n' > "$provision_work/var/lib/octessera/samples/user-sample.wav"
}
run_provision() {
  OCTESSERA_PROVISION_ROOT="$provision_work" sh "$provision_script"
}
expect_provision_failure() {
  if run_provision >/dev/null 2>&1; then
    echo "Provisioner accepted unsafe default fixture: $1" >&2
    exit 1
  fi
}

expect_identity_failure() {
  local name="$1"
  local uid="$2"
  local gid="$3"
  local group_gid="$4"
  reset_provision_work "$uid" "$gid" "$group_gid"
  local source_hash
  source_hash="$(sha256sum "$provision_work/usr/share/octessera/defaults/pi-default.json" | awk '{ print $1 }')"
  expect_provision_failure "$name"
  test ! -e "$provision_work/var/lib/octessera/presets"
  test "$(sha256sum "$provision_work/usr/share/octessera/defaults/pi-default.json" | awk '{ print $1 }')" = "$source_hash"
}

reset_provision_work 990 991 991
run_provision
test "$(stat -c '%u:%g:%a' "$provision_work/var/lib/octessera/presets")" = 990:991:755
test "$(stat -c '%u:%g:%a' "$provision_work/var/lib/octessera/presets/default.json")" = 990:991:644
test ! -e "$provision_work/var/lib/octessera/.provisioning"
expect_identity_failure swapped-ids 991 990 991
expect_identity_failure mismatched-group 990 991 992
expect_identity_failure zero-uid 0 991 991
expect_identity_failure zero-gid 990 0 0

reset_provision_work
ln -s "$provision_work/var/lib" "$provision_work/var/lib/octessera/presets"
expect_provision_failure parent-symlink
test -L "$provision_work/var/lib/octessera/presets"

reset_provision_work
printf 'not a directory\n' > "$provision_work/var/lib/octessera/presets"
expect_provision_failure parent-non-regular
test -f "$provision_work/var/lib/octessera/presets"

reset_provision_work
mkdir "$provision_work/var/lib/octessera/presets"
chown 991:991 "$provision_work/var/lib/octessera/presets"
expect_provision_failure parent-wrong-owner

reset_provision_work
mkdir "$provision_work/var/lib/octessera/presets"
chown 990:990 "$provision_work/var/lib/octessera/presets"
chmod 0700 "$provision_work/var/lib/octessera/presets"
expect_provision_failure parent-wrong-mode

reset_provision_work
mkdir "$provision_work/var/lib/octessera/presets"
chown 990:990 "$provision_work/var/lib/octessera/presets"
ln -s outside-target "$provision_work/var/lib/octessera/presets/default.json"
expect_provision_failure destination-symlink
test -L "$provision_work/var/lib/octessera/presets/default.json"

reset_provision_work
mkdir "$provision_work/var/lib/octessera/presets"
chown 990:990 "$provision_work/var/lib/octessera/presets"
mkdir "$provision_work/var/lib/octessera/presets/default.json"
expect_provision_failure destination-non-regular

reset_provision_work
mkdir "$provision_work/var/lib/octessera/presets"
chown 990:990 "$provision_work/var/lib/octessera/presets"
printf '%s\n' unsafe > "$provision_work/var/lib/octessera/presets/default.json"
chown 991:991 "$provision_work/var/lib/octessera/presets/default.json"
expect_provision_failure destination-wrong-owner

reset_provision_work
mkdir "$provision_work/var/lib/octessera/presets"
chown 990:990 "$provision_work/var/lib/octessera/presets"
printf '%s\n' unsafe > "$provision_work/var/lib/octessera/presets/default.json"
chown 990:990 "$provision_work/var/lib/octessera/presets/default.json"
chmod 0600 "$provision_work/var/lib/octessera/presets/default.json"
expect_provision_failure destination-wrong-mode

reset_provision_work
run_provision
cmp "$default_source" "$provision_work/var/lib/octessera/presets/default.json"
test "$(stat -c '%a' "$provision_work/var/lib/octessera/presets/default.json")" = 644
test "$(stat -c '%u:%g' "$provision_work/var/lib/octessera/presets")" = 990:990
test "$(stat -c '%a' "$provision_work/var/lib/octessera/presets")" = 755
test "$(stat -c '%u:%g' "$provision_work/var/lib/octessera/presets/default.json")" = 990:990
test ! -e "$provision_work/var/lib/octessera/.provisioning"

printf '%s\n' '{"user":"config"}' > "$provision_work/var/lib/octessera/presets/default.json"
chown 990:990 "$provision_work/var/lib/octessera/presets/default.json"
chmod 0644 "$provision_work/var/lib/octessera/presets/default.json"
before_default_metadata="$(stat -c '%u:%g:%a:%s' "$provision_work/var/lib/octessera/presets/default.json")"
before_default_hash="$(sha256sum "$provision_work/var/lib/octessera/presets/default.json" | awk '{ print $1 }')"
run_provision
test "$(stat -c '%u:%g:%a:%s' "$provision_work/var/lib/octessera/presets/default.json")" = "$before_default_metadata"
test "$(sha256sum "$provision_work/var/lib/octessera/presets/default.json" | awk '{ print $1 }')" = "$before_default_hash"
run_provision
test "$(stat -c '%u:%g:%a:%s' "$provision_work/var/lib/octessera/presets/default.json")" = "$before_default_metadata"
test "$(sha256sum "$provision_work/var/lib/octessera/presets/default.json" | awk '{ print $1 }')" = "$before_default_hash"
grep -q 'keep this user sample' "$provision_work/var/lib/octessera/samples/user-sample.wav"

reset_provision_work
race_winner="$provision_work/race-winner.json"
race_hook="$provision_work/race-hook.sh"
printf '%s\n' '{"winner":"race"}' > "$race_winner"
cat > "$race_hook" <<EOF
#!/bin/sh
install -o 990 -g 990 -m 0644 "$race_winner" "\$1"
EOF
chmod 0755 "$race_hook"
OCTESSERA_PROVISION_BEFORE_MOVE_HOOK="$race_hook" run_provision
cmp "$race_winner" "$provision_work/var/lib/octessera/presets/default.json"
test "$(stat -c '%u:%g:%a' "$provision_work/var/lib/octessera/presets/default.json")" = 990:990:644
test ! -e "$provision_work/var/lib/octessera/.provisioning"

reset_provision_work
race_outside="$provision_work/outside.json"
race_symlink_hook="$provision_work/race-symlink-hook.sh"
printf '%s\n' 'protected' > "$race_outside"
cat > "$race_symlink_hook" <<EOF
#!/bin/sh
ln -s "$race_outside" "\$1"
EOF
chmod 0755 "$race_symlink_hook"
if OCTESSERA_PROVISION_BEFORE_MOVE_HOOK="$race_symlink_hook" run_provision >/dev/null 2>&1; then
  echo "Provisioner accepted a symlink race winner." >&2
  exit 1
fi
test -L "$provision_work/var/lib/octessera/presets/default.json"
grep -qFx protected "$race_outside"
test ! -e "$provision_work/var/lib/octessera/.provisioning"
stage_work="$install_work/stage with spaces"
mkdir -p "$stage_work/samples/files"
printf 'stale\n' > "$stage_work/samples/files/stale sample.wav"
bash "$root/tools/armbian-image/stage-musical-assets.sh" "$stage_work"
test ! -e "$stage_work/samples/files/stale sample.wav"
cmp "$default_source" "$stage_work/defaults/pi-default.json"
validate_manifest "$stage_work/samples/sample-manifest.tsv" "$stage_work/samples/files"
printf 'Orange musical assets validation passed\n'
