#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
prepare="$root/tools/armbian-image/prepare-orange-rolling-pin-bootstrap.sh"
capture="$root/tools/armbian-image/capture-orange-rolling-pin-bootstrap.sh"
workflow="$root/.github/workflows/orange-rolling-pin-bootstrap.yml"

for script in "$prepare" "$capture"; do
  [[ -f "$script" ]] || { echo "Missing rolling-pin bootstrap script: $script" >&2; exit 1; }
  bash -n "$script"
done
[[ -f "$workflow" ]] || { echo 'Missing rolling-pin bootstrap workflow.' >&2; exit 1; }

assert_workflow_contains() {
  local expected="$1"
  grep -qF -- "$expected" "$workflow" || {
    echo "Rolling-pin bootstrap workflow is missing: $expected" >&2
    exit 1
  }
}

assert_workflow_absent() {
  local unexpected="$1"
  if grep -qF -- "$unexpected" "$workflow"; then
    echo "Rolling-pin bootstrap workflow contains forbidden text: $unexpected" >&2
    exit 1
  fi
}

assert_workflow_contains 'on:'
assert_workflow_contains '  workflow_dispatch:'
assert_workflow_contains $'permissions:\n  contents: read'
assert_workflow_contains 'git ls-remote --exit-code --refs --tags https://github.com/armbian/build.git refs/tags/v26.11.0-trunk.22'
assert_workflow_contains '3da49cffcb8ac58a919d86816fec4659c410ff1e'
assert_workflow_contains 'Expected exactly one rolling-pin tag ref'
assert_workflow_contains 'uses: ./custom/.github/actions/build-armbian-image'
assert_workflow_contains 'rolling_pin_bootstrap: true'
assert_workflow_contains 'name: UNQUALIFIED-orange-rolling-pin-bootstrap'
assert_workflow_contains 'retention-days: 3'
assert_workflow_contains 'build/output/bootstrap-evidence/**'
assert_workflow_contains 'build/output/info/**'
assert_workflow_contains 'build/output/logs/**'
assert_workflow_absent 'workflow_call:'
assert_workflow_absent 'inputs:'
assert_workflow_absent 'KERNELBRANCH'
assert_workflow_absent 'include-hidden-files: true'
assert_workflow_absent 'immutable rolling-pin tag'

work="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work"
}
trap cleanup EXIT

fake_bin="$work/bin"
mkdir -p "$fake_bin"
real_git="$(command -v git)"

cat > "$fake_bin/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1-}" == -C && "${3-}" == rev-parse ]]; then
  case "${4-}" in
    --is-inside-work-tree) printf 'true\n' ;;
    HEAD) printf '%s\n' "${FAKE_HEAD:-3da49cffcb8ac58a919d86816fec4659c410ff1e}" ;;
    *) exec "${REAL_GIT:?}" "$@" ;;
  esac
  exit 0
fi
exec "${REAL_GIT:?}" "$@"
EOF
chmod +x "$fake_bin/git"

cat > "$fake_bin/dpkg-deb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
read -r mode_version mode_architecture mode_abi < "${FAKE_DPKG_MODE_FILE:?}"
if [[ "${1-}" == -f ]]; then
  package="$(basename -- "$2")"
  if [[ "$package" == linux-image-* ]]; then
    package_name=linux-image-current-sunxi64
    kernel_source=linux-6.18.42
  else
    package_name=linux-dtb-current-sunxi64
    kernel_source=
  fi
  if [[ $# == 2 ]]; then
    printf '%s\n' "Package: $package_name" "Version: $mode_version" "Architecture: $mode_architecture" "Source: $kernel_source" 'Armbian-Kernel-Version: 6.18.42' "Armbian-Kernel-Version-Family: $mode_abi" 'Description: rolling-pin test package'
    exit 0
  fi
  case "$3" in
    Package) printf '%s\n' "$package_name" ;;
    Version) printf '%s\n' "$mode_version" ;;
    Architecture) printf '%s\n' "$mode_architecture" ;;
    Armbian-Kernel-Version) printf '%s\n' '6.18.42' ;;
    Armbian-Kernel-Version-Family) printf '%s\n' "$mode_abi" ;;
    *) printf '\n' ;;
  esac
  exit 0
fi
if [[ "${1-}" == -x ]]; then
  destination="$3"
  mkdir -p "$destination/boot" "$destination/lib/modules/$mode_abi"
  printf '%s\n' 'CONFIG_ROLLING_PIN_TEST=y' > "$destination/boot/config-$mode_abi"
  exit 0
fi
echo "Unexpected fake dpkg-deb invocation: $*" >&2
exit 2
EOF
chmod +x "$fake_bin/dpkg-deb"

cat > "$work/compile.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > "$PWD/compile.argv"
mkdir -p "$PWD/output/info"
case "${SOURCE_MODE:-good}" in
  good) printf '%s\n' '[{"source":"https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git","branch":"linux-6.18.y","sha1":"0123456789abcdef0123456789abcdef01234567"}]' > "$PWD/output/info/git_sources.json" ;;
  bad) printf '%s\n' '[]' > "$PWD/output/info/git_sources.json" ;;
  *) printf '%s\n' '[{"source":"https://example.invalid/linux.git","branch":"linux-6.18.y","sha1":"0123456789abcdef0123456789abcdef01234567"}]' > "$PWD/output/info/git_sources.json" ;;
esac
EOF
chmod +x "$work/compile.sh"

make_build() {
  local build_dir="$1"
  mkdir -p "$build_dir/config/sources"
  cp -- "$work/compile.sh" "$build_dir/compile.sh"
}

expect_failure() {
  local label="$1"
  shift
  if "$@" > "$work/$label.log" 2>&1; then
    echo "Expected rolling-pin bootstrap failure: $label" >&2
    exit 1
  fi
}

build_dir="$work/build"
capture_dir="$work/capture"
make_build "$build_dir"
PATH="$fake_bin:$PATH" REAL_GIT="$real_git" "$prepare" "$build_dir" "$capture_dir"
expected_argv="$work/expected-compile.argv"
cat > "$expected_argv" <<'EOF'
artifact-config-dump-json
WHAT=kernel
BOARD=orangepizero2w
RELEASE=trixie
BRANCH=current
REVISION=26.11.0-trunk.22
HOSTRELEASE=trixie
BUILD_DESKTOP=no
BUILD_MINIMAL=yes
KERNEL_CONFIGURE=no
ENABLE_EXTENSIONS=octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize
EXPERT=yes
EOF
cmp -- "$expected_argv" "$build_dir/compile.argv"
captured_lock="$capture_dir/captured-candidate-source-lock.json"
cmp -- "$build_dir/output/info/git_sources.json" "$captured_lock"
cmp -- "$captured_lock" "$build_dir/config/sources/git_sources.json"
grep -qF -- 'captured-candidate-source-lock.json' <<< "$captured_lock"
if grep -qF -- 'KERNELBRANCH' "$build_dir/compile.argv"; then
  echo 'Preparation passed KERNELBRANCH to artifact config discovery.' >&2
  exit 1
fi

expect_failure reused-capture env PATH="$fake_bin:$PATH" REAL_GIT="$real_git" "$prepare" "$build_dir" "$capture_dir"
expect_failure unsafe-capture env PATH="$fake_bin:$PATH" REAL_GIT="$real_git" "$prepare" "$build_dir" "$build_dir/capture"

bad_source_build="$work/bad-source-build"
bad_source_capture="$work/bad-source-capture"
make_build "$bad_source_build"
expect_failure bad-source env PATH="$fake_bin:$PATH" REAL_GIT="$real_git" SOURCE_MODE=bad "$prepare" "$bad_source_build" "$bad_source_capture"

wrong_source_build="$work/wrong-source-build"
wrong_source_capture="$work/wrong-source-capture"
make_build "$wrong_source_build"
expect_failure wrong-source env PATH="$fake_bin:$PATH" REAL_GIT="$real_git" SOURCE_MODE=wrong "$prepare" "$wrong_source_build" "$wrong_source_capture"

bad_head_build="$work/bad-head-build"
bad_head_capture="$work/bad-head-capture"
make_build "$bad_head_build"
expect_failure bad-head env PATH="$fake_bin:$PATH" REAL_GIT="$real_git" FAKE_HEAD=0123456789012345678901234567890123456789 "$prepare" "$bad_head_build" "$bad_head_capture"

printf '%s %s %s\n' 26.11.0-trunk.22 arm64 6.18.42-current-sunxi64 > "$work/dpkg.mode"
make_post_output() {
  local output_root="$build_dir/output"
  local image_basename=linux-image-current-sunxi64_26.11.0-trunk.22_arm64__fixture.deb
  local dtb_basename=linux-dtb-current-sunxi64_26.11.0-trunk.22_arm64__fixture.deb
  local image_name=octessera-orangepizero2w.img.xz
  rm -rf -- "$output_root/debs" "$output_root/images" "$output_root/bootstrap-evidence"
  mkdir -p "$output_root/debs" "$output_root/images"
  : > "$output_root/debs/$image_basename"
  : > "$output_root/debs/$dtb_basename"
  printf '%s\n' image-fixture > "$output_root/images/$image_name"
  (cd "$output_root/images" && sha256sum "$image_name" > "$image_name.sha")
}

run_capture() {
  PATH="$fake_bin:$PATH" REAL_GIT="$real_git" FAKE_DPKG_MODE_FILE="$work/dpkg.mode" "$capture" "$build_dir" "$captured_lock" "$build_dir/output/bootstrap-evidence"
}

make_post_output
run_capture
evidence_dir="$build_dir/output/bootstrap-evidence"
for evidence_file in framework.txt build-tuple.env source-lock.env native-package.env packaged-kernel.env image.env SHA256SUMS captured-candidate-source-lock.json effective-source-lock.json; do
  [[ -f "$evidence_dir/$evidence_file" ]] || { echo "Missing capture evidence: $evidence_file" >&2; exit 1; }
done
grep -q '^source_lock_equal=true$' "$evidence_dir/source-lock.env"
grep -q '^kernelbranch_argument=omitted$' "$evidence_dir/build-tuple.env"
grep -q '^packaged_config_path=boot/config-6.18.42-current-sunxi64$' "$evidence_dir/packaged-kernel.env"
(cd "$build_dir/output" && sha256sum -c bootstrap-evidence/SHA256SUMS >/dev/null)

make_post_output
cp -- "$build_dir/output/debs/linux-image-current-sunxi64_26.11.0-trunk.22_arm64__fixture.deb" "$build_dir/output/debs/linux-image-current-sunxi64_26.11.0-trunk.22_arm64__extra.deb"
expect_failure ambiguous-package run_capture

make_post_output
printf '%s %s %s\n' 26.10.0-trunk.1 arm64 6.18.42-current-sunxi64 > "$work/dpkg.mode"
expect_failure wrong-version run_capture

make_post_output
printf '%s %s %s\n' 26.11.0-trunk.22 amd64 6.18.42-current-sunxi64 > "$work/dpkg.mode"
expect_failure wrong-architecture run_capture

make_post_output
printf '%s\n' tampered > "$build_dir/output/images/octessera-orangepizero2w.img.xz"
printf '%s %s %s\n' 26.11.0-trunk.22 arm64 6.18.42-current-sunxi64 > "$work/dpkg.mode"
expect_failure bad-checksum run_capture

make_post_output
cp -- "$build_dir/output/images/octessera-orangepizero2w.img.xz" "$build_dir/output/images/extra.img.xz"
expect_failure ambiguous-image run_capture

printf 'Orange rolling-pin bootstrap tests passed\n'
