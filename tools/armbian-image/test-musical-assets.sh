#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
fixture_root="$(mktemp -d)"
trap 'rm -rf "$fixture_root"' EXIT
projection="$fixture_root/target/save-documents/pi"
mkdir -p "$(dirname "$projection")"
(cd "$root" && cargo run -p playback-runtime --bin split_default_config -- config/generated/pi/default.json "$projection")
bash "$root/tools/armbian-image/stage-musical-assets.sh" "$fixture_root/usr/share/octessera" "$projection"
staging="$fixture_root/usr/share/octessera"
default_source="$root/config/generated/pi/default.json"
default_staged="$staging/defaults/pi-default.json"
system_source="$projection/system.json"
patch_source="$projection/default.patch.json"
system_staged="$staging/defaults/pi-system.json"
patch_staged="$staging/defaults/pi-default.patch.json"
manifest="$staging/samples/MANIFEST.tsv"

cmp "$default_source" "$default_staged"
cmp "$system_source" "$system_staged"
cmp "$patch_source" "$patch_staged"
test "$(stat -c '%a' "$default_staged")" = 644
test "$(stat -c '%a' "$system_staged")" = 644
test "$(stat -c '%a' "$patch_staged")" = 644
test -f "$system_staged" && test ! -L "$system_staged"
test -f "$patch_staged" && test ! -L "$patch_staged"
python3 - "$system_staged" "$patch_staged" <<'PY'
import json
import pathlib
import sys

expected = ("octessera.system", "octessera.patch")
for path, kind in zip(sys.argv[1:], expected):
    document = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    if document.get("kind") != kind:
        raise SystemExit(f"unexpected projected document kind in {path}")
PY
validate_manifest() {
  local manifest_path="$1"
  local sample_root="$2"
  local ownership_required="${3:-false}"
  python3 - "$root" "$manifest_path" "$sample_root" "$ownership_required" <<'PY'
import hashlib
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(sys.argv[1]) / "tools/samples"))
from sample_library import read_manifest

manifest_path = pathlib.Path(sys.argv[2])
sample_root = pathlib.Path(sys.argv[3])
ownership_mode = sys.argv[4]
ownership_required = ownership_mode in {"root", "orange-final"}
inventory = read_manifest(pathlib.Path(sys.argv[1]) / "samples/MANIFEST.tsv")
expected = {record.path: record for record in inventory}
rows = {}
lines = manifest_path.read_text(encoding="utf-8").splitlines()
if lines[0] != "# path\tsize\tsha256":
    raise SystemExit("invalid staged sample manifest header")
if len(lines) != len(expected) + 1:
    raise SystemExit("manifest does not contain the complete sample inventory")
for line in lines[1:]:
    path, size, digest = line.split("\t")
    record = expected.get(path)
    if record is None or (int(size), digest) != (record.size, record.sha256):
        raise SystemExit(f"manifest row differs from sample manifest: {path}")
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
    raise SystemExit("manifest does not match the complete sample inventory")
if set(actual) != set(expected):
    raise SystemExit("sample tree does not match the complete sample inventory")
PY
}
validate_manifest "$manifest" "$staging/samples/files"
grep -qF 'mv -T -n' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
# shellcheck disable=SC2016
grep -qF 'temporary_system=$(mktemp "$staging_directory/system.XXXXXX")' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
# shellcheck disable=SC2016
grep -qF 'temporary_patch=$(mktemp "$staging_directory/patch.XXXXXX")' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
# shellcheck disable=SC2016
octessera_reject_file_match "Provisioner stages candidates inside the runtime-writable presets directory." -qF 'mktemp "$presets_directory/' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
grep -q 'ExecStart=/usr/local/sbin/octessera-provision-musical-default' "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
grep -qFx 'Description=Seed missing Octessera split save documents' "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
grep -qF 'install_overlay_file usr/share/octessera/defaults/pi-system.json /usr/share/octessera/defaults/pi-system.json 0644' "$root/userpatches/customize-image.sh"
grep -qF 'install_overlay_file usr/share/octessera/defaults/pi-default.patch.json /usr/share/octessera/defaults/pi-default.patch.json 0644' "$root/userpatches/customize-image.sh"
if grep -qF 'pi-default.json' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"; then
  echo 'Orange save seeder must not consume the mixed metadata input.' >&2
  exit 1
fi
grep -qF 'CONFIG=/var/lib/octessera/presets/system.json' "$root/userpatches/overlay/usr/local/sbin/octessera-orange-usb-gadget"
grep -qF 'CONFIG_PATH = "/var/lib/octessera/presets/system.json"' "$root/userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot"
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
# shellcheck source=userpatches/overlay/usr/local/lib/octessera/orange-sample-assets.sh
source "$installer"
install_orange_musical_assets "$overlay_root" "$target_root"
EOF
chmod 0755 "$install_work/run-install-musical-assets.sh"
run_as_root "$install_work/run-install-musical-assets.sh" "$install_work/install-musical-assets.sh" "$fake_overlay" "$fake_root"
run_as_root chown 990:990 "$fake_root/var/lib/octessera/samples"
validate_manifest "$fake_root/usr/share/octessera/samples/MANIFEST.tsv" "$fake_root/var/lib/octessera/samples" orange-final
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
  mkdir -p "$provision_work/etc" "$provision_work/usr/share/octessera/defaults" "$provision_work/usr/local/lib/octessera" "$provision_work/var/lib/octessera" "$provision_work/var/lib/octessera/samples"
  printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' "octessera-runtime:x:$runtime_uid_fixture:$runtime_gid_fixture:Octessera runtime:/nonexistent:/usr/sbin/nologin" > "$provision_work/etc/passwd"
  printf '%s\n' 'root:x:0:' "octessera-runtime:x:$runtime_group_gid_fixture:" > "$provision_work/etc/group"
  cp "$system_source" "$provision_work/usr/share/octessera/defaults/pi-system.json"
  cp "$patch_source" "$provision_work/usr/share/octessera/defaults/pi-default.patch.json"
  cp "$default_source" "$provision_work/usr/share/octessera/defaults/pi-default.json"
  cp "$root/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" "$provision_work/usr/local/lib/octessera/device_config.py"
  printf 'keep this user sample\n' > "$provision_work/var/lib/octessera/samples/user-sample.wav"
}
run_provision() {
  OCTESSERA_PROVISION_ROOT="$provision_work" sh "$provision_script"
}
expect_provision_failure() {
  if run_provision >/dev/null 2>&1; then
    echo "Provisioner accepted unsafe save-document fixture: $1" >&2
    exit 1
  fi
}

reset_provision_work 990 991 991
run_provision
presets="$provision_work/var/lib/octessera/presets"
cmp "$system_source" "$presets/system.json"
cmp "$patch_source" "$presets/default.patch.json"
test "$(stat -c '%u:%g:%a' "$presets")" = 990:991:755
for document in system.json default.patch.json; do
  test "$(stat -c '%u:%g:%a' "$presets/$document")" = 990:991:644
done
test ! -e "$presets/default.json"
cmp "$default_source" "$provision_work/usr/share/octessera/defaults/pi-default.json"
test ! -e "$provision_work/var/lib/octessera/.provisioning"

for bad in partial malformed legacy conflict; do
  reset_provision_work
  mkdir -p "$provision_work/var/lib/octessera/presets"
  case "$bad" in
    partial) printf 'user system bytes\n' > "$provision_work/var/lib/octessera/presets/system.json" ;;
    malformed) printf '{broken\n' > "$provision_work/var/lib/octessera/presets/system.json"; cp "$patch_source" "$provision_work/var/lib/octessera/presets/default.patch.json" ;;
    legacy) printf 'legacy bytes\n' > "$provision_work/var/lib/octessera/presets/default.json" ;;
    conflict) cp "$system_source" "$provision_work/var/lib/octessera/presets/system.json"; cp "$patch_source" "$provision_work/var/lib/octessera/presets/default.patch.json"; python3 - "$provision_work/var/lib/octessera/presets/system.json" <<'PY'
import json
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
doc = json.loads(path.read_text(encoding="utf-8"))
doc["runtimeConfig"]["usb"]["dataRole"] = "host"
doc["runtimeConfig"]["audioOutputs"]["usb"] = True
path.write_text(json.dumps(doc), encoding="utf-8")
PY
      ;;
  esac
  chown -R 990:990 "$provision_work/var/lib/octessera/presets"
  before="$(find "$provision_work/var/lib/octessera/presets" -type f -exec sha256sum {} + | sort)"
  expect_provision_failure "$bad"
  after="$(find "$provision_work/var/lib/octessera/presets" -type f -exec sha256sum {} + | sort)"
  test "$before" = "$after"
  test ! -e "$provision_work/var/lib/octessera/presets/default.patch.json" || [ "$bad" != partial ]
done

for identity in mismatched-group zero-uid zero-gid; do
  reset_provision_work
  case "$identity" in
    mismatched-group) printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' 'octessera-runtime:x:990:991:Octessera runtime:/nonexistent:/usr/sbin/nologin' > "$provision_work/etc/passwd"; printf '%s\n' 'root:x:0:' 'octessera-runtime:x:992:' > "$provision_work/etc/group" ;;
    zero-uid) printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' 'octessera-runtime:x:0:991:Octessera runtime:/nonexistent:/usr/sbin/nologin' > "$provision_work/etc/passwd" ;;
    zero-gid) printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' 'octessera-runtime:x:990:0:Octessera runtime:/nonexistent:/usr/sbin/nologin' > "$provision_work/etc/passwd"; printf '%s\n' 'root:x:0:' 'octessera-runtime:x:0:' > "$provision_work/etc/group" ;;
  esac
  expect_provision_failure "$identity"
  test ! -e "$provision_work/var/lib/octessera/presets"
done

reset_provision_work
run_provision
presets="$provision_work/var/lib/octessera/presets"
before_system="$(sha256sum "$presets/system.json" | awk '{ print $1 }')"
before_patch="$(sha256sum "$presets/default.patch.json" | awk '{ print $1 }')"
run_provision
test "$(sha256sum "$presets/system.json" | awk '{ print $1 }')" = "$before_system"
test "$(sha256sum "$presets/default.patch.json" | awk '{ print $1 }')" = "$before_patch"
grep -q 'keep this user sample' "$provision_work/var/lib/octessera/samples/user-sample.wav"
stage_work="$install_work/stage with spaces"
mkdir -p "$stage_work/samples/files"
printf 'stale\n' > "$stage_work/samples/files/stale sample.wav"
bash "$root/tools/armbian-image/stage-musical-assets.sh" "$stage_work" "$projection"
test ! -e "$stage_work/samples/files/stale sample.wav"
cmp "$default_source" "$stage_work/defaults/pi-default.json"
cmp "$system_source" "$stage_work/defaults/pi-system.json"
cmp "$patch_source" "$stage_work/defaults/pi-default.patch.json"
validate_manifest "$stage_work/samples/MANIFEST.tsv" "$stage_work/samples/files"
printf 'Orange musical assets validation passed\n'
