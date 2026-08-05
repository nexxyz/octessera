#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
staging="$root/userpatches/overlay/usr/share/octessera"
default_source="$root/config/generated/pi/default.json"
default_staged="$staging/defaults/pi-default.json"
manifest="$staging/samples/sample-manifest.tsv"
customize="$root/userpatches/customize-image.sh"

cmp "$default_source" "$default_staged"
validate_manifest() {
  local manifest_path="$1"
  local sample_root="$2"
  local ownership_required="${3:-false}"
  python3 - "$default_source" "$manifest_path" "$sample_root" "$ownership_required" <<'PY'
import hashlib
import json
import pathlib
import sys

default_path = pathlib.Path(sys.argv[1])
manifest_path = pathlib.Path(sys.argv[2])
sample_root = pathlib.Path(sys.argv[3])
ownership_required = sys.argv[4] == "root"
payload = json.loads(default_path.read_text(encoding="utf-8"))
expected = sorted(
    {
        slot["path"][len("samples/") :]
        for instrument in payload["runtimeConfig"]["instruments"]
        for slot in instrument["sample"]["slots"]
        if slot.get("path")
    }
)
rows = []
expected_sources = {
    "Drum/claps/distkit-clap.wav": "https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/fugue-state-audio/drums/claps/distkit-clap.wav",
    "Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav": "https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/freesound/drums/cymbal/open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav",
    "Drum/kick/Kick2.wav": "https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/stargate-sample-pack/microlag/One-Shots/Drums/Kick2.wav",
}
expected_license_source = "https://raw.githubusercontent.com/stargatedaw/stargate-sample-pack/dbfd6ec52d4ed53b60bdbea5fc6adf295127c027/LICENSE"
for line in manifest_path.read_text(encoding="utf-8").splitlines()[1:]:
    path, size, digest, source, license_source = line.split("\t")
    if source != expected_sources.get(path):
        raise SystemExit(f"unexpected sample source: {source}")
    if license_source != expected_license_source:
        raise SystemExit(f"unexpected sample license source: {license_source}")
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
    rows.append(path)
actual = []
if sample_root.is_symlink() or not sample_root.is_dir():
    raise SystemExit(f"sample root is not a directory: {sample_root}")
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
    actual.append(sample.relative_to(sample_root).as_posix())
if rows != expected:
    raise SystemExit(f"manifest does not match default sample paths: {rows!r} != {expected!r}")
if sorted(actual) != expected:
    raise SystemExit(f"sample tree does not match manifest: {sorted(actual)!r} != {expected!r}")
PY
}
validate_manifest "$manifest" "$staging/samples/files"
grep -q 'mv -n' "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
grep -q 'ExecStart=/usr/local/sbin/octessera-provision-musical-default' "$root/userpatches/overlay/etc/systemd/system/octessera-provision-musical-default.service"
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
}
trap cleanup EXIT
install_work="$(mktemp -d)"
provision_work="$(mktemp -d)"
fake_overlay="$install_work/overlay"
fake_root="$install_work/root"
mkdir -p "$fake_overlay/usr/share/octessera" "$fake_root/usr/share/octessera/samples"
cp -a "$staging/defaults" "$fake_overlay/usr/share/octessera/"
cp -a "$staging/samples" "$fake_overlay/usr/share/octessera/"
cp "$manifest" "$fake_root/usr/share/octessera/samples/sample-manifest.tsv"
mkdir -p "$fake_root/usr/share/octessera/samples/files/stale"
printf 'stale\n' > "$fake_root/usr/share/octessera/samples/files/stale/old sample.wav"
awk '/^install_musical_assets\(\) \{$/,/^}$/ { print }' "$customize" > "$install_work/install-musical-assets.sh"
grep -qF 'install_musical_assets() {' "$install_work/install-musical-assets.sh" || { echo "Could not extract musical asset installer." >&2; exit 1; }
cat > "$install_work/run-install-musical-assets.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

installer="$1"
overlay_root="$2"
target_root="$3"
# shellcheck disable=SC1090,SC1091
source "$installer"
install_musical_assets "$overlay_root" "$target_root"
EOF
chmod 0755 "$install_work/run-install-musical-assets.sh"
run_as_root "$install_work/run-install-musical-assets.sh" "$install_work/install-musical-assets.sh" "$fake_overlay" "$fake_root"
validate_manifest "$fake_root/usr/share/octessera/samples/sample-manifest.tsv" "$fake_root/usr/share/octessera/samples/files" root
test -f "$fake_root/usr/share/octessera/samples/files/Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav"
test ! -e "$fake_root/usr/share/octessera/samples/files/stale"

expect_installer_rejects() {
  local name="$1"
  local invalid_overlay="$install_work/${name}-overlay"
  local invalid_root="$install_work/${name}-root"
  cp -a "$fake_overlay" "$invalid_overlay"
  mkdir -p "$invalid_root/usr/share/octessera/samples"
  cp "$manifest" "$invalid_root/usr/share/octessera/samples/sample-manifest.tsv"
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
mkdir -p "$invalid_root/usr/share/octessera/samples"
cp "$manifest" "$invalid_root/usr/share/octessera/samples/sample-manifest.tsv"
if run_as_root "$install_work/run-install-musical-assets.sh" "$install_work/install-musical-assets.sh" "$invalid_overlay" "$invalid_root" >/dev/null 2>&1; then
  echo "Musical asset installer accepted a missing manifest asset." >&2
  exit 1
fi
mkdir -p "$provision_work/usr/share/octessera" "$provision_work/var/lib/octessera/presets"
cp -a "$staging/defaults" "$provision_work/usr/share/octessera/"
cp -a "$staging/samples" "$provision_work/usr/share/octessera/"
printf '%s\n' '{"user":"config"}' > "$provision_work/var/lib/octessera/presets/default.json"
OCTESSERA_PROVISION_ROOT="$provision_work" sh "$root/userpatches/overlay/usr/local/sbin/octessera-provision-musical-default"
grep -q '"user":"config"' "$provision_work/var/lib/octessera/presets/default.json"
validate_manifest "$provision_work/usr/share/octessera/samples/sample-manifest.tsv" "$provision_work/var/lib/octessera/samples"
stage_work="$install_work/stage with spaces"
mkdir -p "$stage_work/samples/files"
printf 'stale\n' > "$stage_work/samples/files/stale sample.wav"
bash "$root/tools/armbian-image/stage-musical-assets.sh" "$stage_work"
test ! -e "$stage_work/samples/files/stale sample.wav"
cmp "$default_source" "$stage_work/defaults/pi-default.json"
validate_manifest "$stage_work/samples/sample-manifest.tsv" "$stage_work/samples/files"
printf 'Orange musical assets validation passed\n'
