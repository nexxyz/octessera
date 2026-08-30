#!/usr/bin/env bash
# shellcheck disable=SC2016
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
release="$root/.github/workflows/release-artifacts.yml"
boards="$root/.github/workflows/release-board-artifacts.yml"
action="$root/.github/actions/build-armbian-image/action.yml"
bootstrap="$root/.github/workflows/orange-rolling-pin-bootstrap.yml"
assembler="$root/tools/release/assemble_release_assets.py"
board_release="$root/tools/release/board_image_release.py"
desktop_verifier="$root/tools/release/verify_desktop_artifact.py"
device_packager="$root/tools/device-update/package_device_bundle.py"
updater_profiles="$root/tools/device-update/updater_profiles.py"
sanitizer="$root/tools/pi-image/verify-sanitized-image.sh"
runtime_chain_helper="$root/tools/pi-image/verify-managed-runtime.sh"
runtime_chain_test="$root/tools/pi-image/test-sanitized-image-runtime-chain.sh"
boot_layout_test="$root/tools/pi-image/test-sanitized-image-boot-layout.sh"

assert_contains() {
    local file="$1"
    local expected="$2"
    grep -qF -- "$expected" "$file" || {
        echo "$file is missing: $expected" >&2
        exit 1
    }
}

assert_absent() {
    local file="$1"
    local unexpected="$2"
    octessera_reject_file_match "$file contains removed release coupling: $unexpected" -qF -- "$unexpected" "$file"
}

assert_block_contains() {
    local block="$1"
    local expected="$2"
    grep -qF -- "$expected" <<< "$block" || {
        echo "Workflow block is missing: $expected" >&2
        exit 1
    }
}

assert_block_absent() {
    local block="$1"
    local unexpected="$2"
    octessera_reject_text_match "Workflow block contains removed release coupling: $unexpected" "$block" -qF -- "$unexpected"
}

assert_order() {
    local file="$1"
    local first="$2"
    local second="$3"
    local first_line second_line
    first_line="$(grep -nF -- "$first" "$file" | head -n 1 | cut -d: -f1 || true)"
    second_line="$(grep -nF -- "$second" "$file" | head -n 1 | cut -d: -f1 || true)"
    [[ -n "$first_line" && -n "$second_line" && "$first_line" -lt "$second_line" ]] || {
        echo "$file has invalid ordering: $first before $second" >&2
        exit 1
    }
}

assert_contains "$release" 'workflow_dispatch:'
assert_contains "$boards" 'workflow_call:'
[[ -f "$bootstrap" ]] || { echo 'Missing Orange rolling-pin bootstrap workflow.' >&2; exit 1; }
assert_absent "$boards" 'rolling_pin_bootstrap: true'
assert_absent "$release" 'orange-rolling-pin-bootstrap'
assert_absent "$boards" 'orange-rolling-pin-bootstrap'
assert_contains "$boards" 'armbian_build_ref: 3da49cffcb8ac58a919d86816fec4659c410ff1e'
assert_contains "$boards" 'ARMBIAN_BUILD_REF: 3da49cffcb8ac58a919d86816fec4659c410ff1e'
assert_absent "$boards" 'fa7a7b2294d9e760a77630950afd460b7a0b2a26'
mapfile -t bootstrap_workflows < <(grep -RIlF --include='*.yml' --include='*.yaml' -- 'rolling_pin_bootstrap: true' "$root/.github/workflows" || true)
[[ "${#bootstrap_workflows[@]}" == 1 && "${bootstrap_workflows[0]}" == "$bootstrap" ]] || {
    echo 'Exactly one workflow may enable rolling-pin bootstrap, and it must be the bootstrap workflow.' >&2
    exit 1
}
armbian_inputs="$root/.github/workflows/armbian-image.yml"
[[ "$(sed -n '/^    inputs:/,/^permissions:/p' "$armbian_inputs" | grep -cE '^      [A-Za-z0-9_-]+:$')" == 10 ]] || {
    echo 'Armbian workflow_dispatch must retain exactly ten inputs.' >&2
    exit 1
}
assert_contains "$armbian_inputs" 'def safe_string($max):'
assert_contains "$armbian_inputs" 'test("[\\r\\n]") | not'
assert_contains "$release" 'group: release-artifacts-${{ inputs.tag }}'
assert_contains "$release" 'cancel-in-progress: false'
assert_contains "$release" $'permissions:\n  contents: read'
[[ "$(grep -cE '^    permissions:$' "$release")" == 2 ]] || {
    echo 'Only the resolver and publisher may have job-level permissions.' >&2
    exit 1
}
[[ "$(grep -cE '^      contents: write$' "$release")" == 2 ]] || {
    echo 'Exactly two job-level contents: write grants are required.' >&2
    exit 1
}
assert_contains "$release" 'resolve_draft:'
assert_contains "$release" 'needs: resolve_draft'
assert_contains "$release" 'workflow_static:'
assert_contains "$release" 'ref: ${{ github.workflow_sha }}'
assert_contains "$release" 'git rev-parse HEAD)" = "${{ github.workflow_sha }}"'
assert_contains "$release" 'bash -n tools/armbian-image/test-release-workflow.sh'
assert_contains "$release" 'shellcheck -x tools/armbian-image/test-release-workflow.sh'
assert_contains "$release" 'release_info:'
assert_contains "$release" 'Release version must be an exact semver.'
assert_contains "$release" 'version=${RELEASE_TAG#v}'
assert_contains "$release" 'tools/release/check_version_consistency.py --tag "$RELEASE_TAG"'
assert_contains "$release" 'python3 -m unittest tools.release.test_release_asset_assembly'
assert_contains "$release" 'python3 -m unittest tools.release.test_board_image_release'
assert_absent "$release" 'test_qualified_release_routing.py'
assert_absent "$release" 'python3 tools/release/test_release_asset_assembly.py'
assert_contains "$release" 'git rev-parse "$RELEASE_TAG^{commit}"'
assert_contains "$release" 'needs: [release_info, updater_protocol, windows, ubuntu, board_artifacts, workflow_static]'
assert_contains "$release" 'The release must remain a draft until manual publication.'
assert_contains "$release" 'The release draft must not already contain assets.'
assert_contains "$release" 'gh api --paginate --slurp "repos/$GITHUB_REPOSITORY/releases?per_page=100"'
assert_contains "$release" '[ .[][] | select(.tag_name == $tag) ] as $matches'
assert_contains "$release" 'if ($matches | length) != 1 then'
assert_contains "$release" 'elif $matches[0].draft != true then'
assert_contains "$release" 'elif ($matches[0].assets | length) != 0 then'
assert_contains "$release" 'release_id: ${{ needs.resolve_draft.outputs.release_id }}'
assert_contains "$release" 'python3 tools/release/assemble_release_assets.py'
assert_absent "$release" 'expected_root_assets=('
assert_absent "$release" 'zip -9 -r "$evidence_zip"'
assert_absent "$release" 'require_exact_files()'
assert_absent "$release" 'copy_asset()'
assert_contains "$release" 'gh release upload'
assert_absent "$release" 'gh release edit'
assert_absent "$release" '--draft=false'
assert_contains "$release" 'Release already has assets before upload.'
assert_contains "$release" 'Revalidate exact draft immediately before upload'
assert_contains "$release" 'Revalidate uploaded asset set after upload'
assert_contains "$release" 'GITHUB_STEP_SUMMARY'
assert_contains "$release" 'draft_ready=true'
assert_contains "$release" 'draft_ready: ${{ steps.draft_handoff.outputs.draft_ready }}'
assert_contains "$release" 'manual exact-artifact FAT and human publication'
assert_absent "$release" 'Publish the verified draft release last'
assert_contains "$release" 'EXPECTED_RELEASE_ID'
assert_contains "$board_release" 'KERNEL_MANIFEST = Path("tools/kernel-patches/orange-midi-interface-manifest.json")'
assert_contains "$board_release" 'def _package_filenames(manifest'
assert_contains "$board_release" 'Raspberry package declaration'
assert_absent "$boards" 'board_image_mode'
assert_absent "$board_release" 'BASE_REFRESH'
assert_absent "$boards" 'qualified-respin'
assert_absent "$boards" 'raspberry_respin:'
assert_absent "$boards" 'orange_respin:'
assert_contains "$assembler" 'package_notice_zip(root, notices)'
assert_contains "$assembler" 'verify_notice_archive(root, portable, "octessera.exe")'
assert_contains "$assembler" 'device ZIP inventory is not exact'
assert_contains "$assembler" 'expected_root_assets = ['
assert_contains "$assembler" 'len(expected_root_assets) == 14'
assert_contains "$assembler" 'expected_names'
assert_contains "$assembler" 'expected_mode = 0o755 if entry.filename == "octessera-pi" else 0o644'
assert_contains "$assembler" '_make_evidence_zip'
assert_contains "$assembler" '_write_checksums(release_assets, "SHA256SUMS.txt"'
assert_contains "$assembler" '_require_exact_files(release_assets, expected_root_assets)'
assert_contains "$desktop_verifier" 'EXPECTED_MEDIA_COUNT'
assert_contains "$desktop_verifier" 'verify_media_tree'
assert_contains "$desktop_verifier" 'portable archive entry mode is not 0644'
assert_contains "$desktop_verifier" 'legal/notice-bundle.json'
assert_contains "$desktop_verifier" 'sample-manifest.tsv'
assert_contains "$release" 'mapfile -t uploaded_assets'
assert_contains "$release" "stat -c '%s'"
assert_contains "$release" '[.name, (.size | tostring)] | @tsv'
assert_contains "$release" 'Uploaded release asset names/count/sizes do not match the verified set.'
assert_contains "$release" 'local bytes/checksums were verified before upload; remote names/count/sizes were revalidated; downloaded bytes/checksums and exact-artifact FAT remain human gates before publication.'
assert_contains "$release" 'if-no-files-found: error'
assert_contains "$boards" 'tools/device-update/package_device_bundle.py'
assert_contains "$boards" '--board-profile raspberry-pi-zero-2w'
assert_contains "$boards" '--board-profile orange-pi-zero-2w'
assert_contains "$boards" 'Stage Raspberry legal notices and copy disposable stage4'
assert_contains "$boards" 'tools/wifi-connect/build-patched-ci.sh'
assert_order "$boards" 'tools/wifi-connect/build-patched-ci.sh' 'Stage the matching runtime into stage4'
assert_contains "$boards" 'tools/legal/stage_notices.py'
assert_contains "$boards" 'tools/pi-image/stage-musical-assets.sh'
assert_contains "$boards" 'sudo bash tools/pi-image/test-musical-assets.sh'
assert_contains "$boards" '--check'
assert_contains "$boards" 'verify-rpi-kernel-image.sh'
assert_contains "$boards" 'verify-orange-image.sh'
assert_contains "$boards" 'octessera-orange-image-proof.json'
assert_contains "$boards" 'octessera-${{ inputs.version }}-raspberry-pi-zero-2w.img.zip'
assert_contains "$boards" 'octessera-${{ inputs.version }}-orange-pi-zero-2w.img.xz'
orange_handoff_block="$(sed -n '/^      - name: Normalize and verify Orange release handoff$/,/^      - uses: actions\/upload-artifact@v4$/p' "$boards")"
assert_block_contains "$orange_handoff_block" '--manifest "$custom_root/tools/kernel-patches/orange-midi-interface-manifest.json"'
assert_block_contains "$orange_handoff_block" '--construction-contract "$custom_root/resources/image-construction/boot-layers/orange-pi-zero-2w.json"'
assert_block_contains "$orange_handoff_block" '--output release-assets/octessera-orange-image-proof.json'
assert_block_absent "$orange_handoff_block" '--image-provenance'
assert_contains "$action" 'tools/legal/stage_notices.py'
assert_contains "$action" 'tools/armbian-image/stage-musical-assets.sh'
assert_contains "$root/resources/image-construction/boot-layers/raspberry-pi-zero-2w.json" 'resources/legal/notice-bundle.json'
assert_absent "$action" 'legal-source'
assert_absent "$release" 'macos'
assert_absent "$release" 'macOS'
assert_absent "$release" 'DMG'
assert_absent "$release" 'dmg'

workflow_static_block="$(sed -n '/^  workflow_static:/,/^  resolve_draft:/p' "$release")"
octessera_reject_text_match 'workflow_static must inherit the top-level read-only permissions.' "$workflow_static_block" -qF 'permissions:'
octessera_require_text_match 'workflow_static must run syntax validation.' "$workflow_static_block" -qF 'bash -n tools/armbian-image/test-release-workflow.sh'
octessera_require_text_match 'workflow_static must run ShellCheck validation.' "$workflow_static_block" -qF 'shellcheck -x tools/armbian-image/test-release-workflow.sh'
resolver_block="$(sed -n '/^  resolve_draft:/,/^  release_info:/p' "$release")"
octessera_reject_text_match 'Draft resolver must remain API-only without checkout, scripts, or artifacts.' "$resolver_block" -qF 'actions/checkout'
octessera_reject_text_match 'Draft resolver must remain API-only without checkout, scripts, or artifacts.' "$resolver_block" -qE '(^|[[:space:]])(python3|bash|sh|pnpm|cargo)[[:space:]]|tools/'
octessera_reject_text_match 'Draft resolver must remain API-only without checkout, scripts, or artifacts.' "$resolver_block" -qF 'upload-artifact'
publisher_block="$(sed -n '/^  publish_release_assets:/,$p' "$release")"
publisher_dependencies_step="$(sed -n '/^      - name: Verify source and install final image proof dependencies$/,/^      - name:/p' "$release")"
octessera_require_text_match 'Contents write must belong to the resolver job.' "$resolver_block" -qF $'    permissions:\n      contents: write'
octessera_require_text_match 'Contents write must belong to the publisher job.' "$publisher_block" -qF $'    permissions:\n      contents: write'
updater_block="$(sed -n '/^  updater_protocol:/,/^  windows:/p' "$release")"
octessera_reject_text_match 'The source-tag updater job must not run the workflow static test.' "$updater_block" -qF 'bash tools/armbian-image/test-release-workflow.sh'
octessera_reject_text_match 'Independent ShellCheck groups must fail separately.' "$updater_block" -qE 'shellcheck .*&&'
octessera_reject_file_match 'Release workflow must not trigger from publication.' -qE '^  release:' "$release"

octessera_reject_file_match 'Draft release validation must not use the by-tag releases endpoint.' -qF '/releases/tags/' "$release"
[[ "$(grep -cF 'gh api "repos/$GITHUB_REPOSITORY/releases/$EXPECTED_RELEASE_ID"' "$release")" == 2 ]] || {
    echo 'Both final release validations must fetch by the resolved release ID.' >&2
    exit 1
}
[[ "$(grep -cF 'EXPECTED_RELEASE_ID: ${{ needs.release_info.outputs.release_id }}' "$release")" == 2 ]] || {
    echo 'Upload and revalidation guards must receive the resolved release ID.' >&2
    exit 1
}
upload_guard_block="$(sed -n '/^      - name: Revalidate exact draft immediately before upload$/,/^      - name: Upload assets without collision hiding$/p' "$release")"
publish_guard_block="$(sed -n '/^      - name: Revalidate uploaded asset set after upload$/,/^      - name: Report populated draft ready for manual FAT and publication$/p' "$release")"
assert_block_contains "$upload_guard_block" 'EXPECTED_RELEASE_ID'
assert_block_contains "$publish_guard_block" 'EXPECTED_RELEASE_ID'
[[ "$(grep -cF 'gh release upload' "$release")" == 1 && "$(grep -cF 'gh release edit' "$release")" == 0 ]] || {
    echo 'Release upload must remain single and automatic publication must be absent.' >&2
    exit 1
}
assert_order "$release" 'Revalidate exact draft immediately before upload' 'Upload assets without collision hiding'
assert_order "$release" 'Upload assets without collision hiding' 'Revalidate uploaded asset set after upload'
assert_order "$release" 'Revalidate uploaded asset set after upload' 'Report populated draft ready for manual FAT and publication'

release_source_block="$(sed -n '/^  release_info:/,/^  updater_protocol:/p' "$release")"
assert_block_contains "$release_source_block" 'corepack pnpm run config:check'
assert_block_contains "$release_source_block" 'corepack pnpm run capabilities:check'
assert_order "$release" 'corepack pnpm run config:check' '  board_artifacts:'
assert_order "$release" 'corepack pnpm run capabilities:check' '  board_artifacts:'

jq_draft_filter='[ .[][] | select(.tag_name == $tag) ] as $matches
  | if ($matches | length) != 1 then
      error("Expected exactly one release for tag \($tag)")
    elif $matches[0].draft != true then
      error("The release must remain a draft until manual publication.")
    elif ($matches[0].assets | length) != 0 then
      error("The release draft must not already contain assets.")
    else
      $matches[0]
    end'
fixture_dir="$(mktemp -d)"
trap 'rm -rf "$fixture_dir"' EXIT
printf '%s\n' '[[{"tag_name":"v0.7.4","id":41,"draft":true,"assets":[]}],[{"tag_name":"v0.7.5","id":42,"draft":true,"assets":[]}]]' > "$fixture_dir/one.json"
printf '%s\n' '[[{"tag_name":"v0.7.4","id":41,"draft":true,"assets":[]}]]' > "$fixture_dir/zero.json"
printf '%s\n' '[[{"tag_name":"v0.7.5","id":42,"draft":true,"assets":[]}],[{"tag_name":"v0.7.5","id":43,"draft":true,"assets":[]}]]' > "$fixture_dir/duplicate.json"
printf '%s\n' '[[{"tag_name":"v0.7.5","id":42,"draft":false,"assets":[]}]]' > "$fixture_dir/non-draft.json"
printf '%s\n' '[[{"tag_name":"v0.7.5","id":42,"draft":true,"assets":[{"name":"stale.bin"}]}]]' > "$fixture_dir/non-empty.json"
[[ "$(jq -r --arg tag v0.7.5 "$jq_draft_filter" "$fixture_dir/one.json" | jq -r '.id')" == 42 ]] || {
    echo 'The later-page-shaped draft fixture did not resolve exactly one empty draft.' >&2
    exit 1
}
if jq -e --arg tag v0.7.5 "$jq_draft_filter" "$fixture_dir/zero.json" >/dev/null || jq -e --arg tag v0.7.5 "$jq_draft_filter" "$fixture_dir/duplicate.json" >/dev/null || jq -e --arg tag v0.7.5 "$jq_draft_filter" "$fixture_dir/non-draft.json" >/dev/null || jq -e --arg tag v0.7.5 "$jq_draft_filter" "$fixture_dir/non-empty.json" >/dev/null; then
    echo 'Draft resolver fixtures must reject zero, duplicate, non-draft, and non-empty matches.' >&2
    exit 1
fi
size_fixture="$fixture_dir/asset-sizes"
mkdir -p "$size_fixture"
printf '%s' abc > "$size_fixture/one.bin"
printf '%s' 12345 > "$size_fixture/two.bin"
printf '%s\n' '{"assets":[{"name":"one.bin","size":3},{"name":"two.bin","size":5}]}' > "$size_fixture/matching.json"
mapfile -t expected_size_assets < <(
    while IFS= read -r name; do
        printf '%s\t%s\n' "$name" "$(stat -c '%s' "$size_fixture/$name")"
    done < <(find "$size_fixture" -maxdepth 1 -type f -name '*.bin' -printf '%f\n' | sort)
)
mapfile -t uploaded_size_assets < <(jq -r '.assets[] | [.name, (.size | tostring)] | @tsv' "$size_fixture/matching.json" | sort)
[[ "${#uploaded_size_assets[@]}" == "${#expected_size_assets[@]}" && "${uploaded_size_assets[*]}" == "${expected_size_assets[*]}" ]] || {
    echo 'Matching remote asset size fixture did not match local stat output.' >&2
    exit 1
}
printf '%s\n' '{"assets":[{"name":"one.bin","size":4},{"name":"two.bin","size":5}]}' > "$size_fixture/wrong-size.json"
mapfile -t uploaded_size_assets < <(jq -r '.assets[] | [.name, (.size | tostring)] | @tsv' "$size_fixture/wrong-size.json" | sort)
if [[ "${#uploaded_size_assets[@]}" == "${#expected_size_assets[@]}" && "${uploaded_size_assets[*]}" == "${expected_size_assets[*]}" ]]; then
    echo 'Wrong remote asset size fixture was accepted.' >&2
    exit 1
fi
orange_handoff_fixture="$fixture_dir/orange-handoff"
mkdir -p "$orange_handoff_fixture/release-assets"
canonical_packages=(linux-image-fixture.deb linux-dtb-fixture.deb)
canonical_release_paths=("release-assets/${canonical_packages[0]}" "release-assets/${canonical_packages[1]}")
for package in "${canonical_packages[@]}"; do
    printf '%s\n' "$package" > "$orange_handoff_fixture/release-assets/$package"
done
for required in "${canonical_release_paths[@]}"; do
    [[ -f "$orange_handoff_fixture/$required" ]] || { echo "Orange handoff fixture missed: $required" >&2; exit 1; }
done
(
    cd "$orange_handoff_fixture/release-assets"
    sha256sum "${canonical_packages[0]}" "${canonical_packages[1]}" > SHA256SUMS-orange-pi-zero-2w.txt
    sha256sum -c SHA256SUMS-orange-pi-zero-2w.txt
)

for file in "$release" "$boards" "$action" "$root/tools/armbian-image/verify-orange-image.py" "$root/tools/pi-image/verify-rpi-kernel-image.py" "$root/resources/image-construction/boot-layers/raspberry-pi-zero-2w.json" "$root/resources/image-construction/boot-layers/orange-pi-zero-2w.json"; do
    for removed in \
        "source""-artifact-contract" "source_""artifact_check" "source_""binding" \
        "source_""git_archive" "source_""git_tree" "source_""base_distribution" \
        "dependency_""source_fetch" "dependency_""source_check" \
        "installed_""package_inventory" "corresponding""_source" \
        "corresponding""-source" "installed""-package-inventory" "verify_""legal_tree"; do
        assert_absent "$file" "$removed"
    done
done

for removed_step in 'Start-Process -FilePath' 'hdiutil attach'; do
    assert_absent "$release" "$removed_step"
done

windows_block="$(sed -n '/^  windows:/,/^  ubuntu:/p' "$release")"
ubuntu_block="$(sed -n '/^  ubuntu:/,/^  board_artifacts:/p' "$release")"
assert_block_contains "$windows_block" 'verify_notice_archive.py'
assert_block_contains "$windows_block" 'python tools/release/verify_desktop_artifact.py --repository-root . --portable-zip'
assert_block_contains "$windows_block" '$sevenZip = "C:\Program Files\7-Zip\7z.exe"'
assert_block_contains "$windows_block" 'Resolve-Path -LiteralPath $sevenZip'
assert_block_contains "$windows_block" '& $sevenZip x $installer'
assert_block_contains "$windows_block" 'Expected exactly one extracted direct samples/legal resource root'
assert_block_contains "$windows_block" '--resource-root $resourceRoots[0].FullName'
assert_absent "$release" '& $installer /S'
assert_block_contains "$ubuntu_block" 'sha256sum ./*.deb ./*.AppImage > SHA256SUMS-ubuntu.txt'
assert_block_contains "$ubuntu_block" 'dpkg-deb -x'
assert_block_contains "$ubuntu_block" '--appimage-extract'
assert_block_contains "$ubuntu_block" 'python3 tools/release/verify_desktop_artifact.py --repository-root . --resource-root "$deb_root/usr/lib/octessera"'
assert_block_contains "$ubuntu_block" 'python3 tools/release/verify_desktop_artifact.py --repository-root . --resource-root "$appimage_root/squashfs-root/usr/lib/octessera"'

assert_contains "$boards" 'hardware-raspberry-pi-zero-2w'
assert_contains "$boards" 'hardware-orange-pi-zero-2w'
assert_contains "$boards" 'extensions: octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'
assert_contains "$boards" 'd7a31c6aa09f4b867902c51da2b45807c0a1709e'
assert_contains "$boards" 'STAGE_LIST="stage0 stage1 stage2 stage3-octessera-kernel stage4-octessera"'
assert_contains "$boards" 'tools/pi-kernel/test-rpi-kernel.sh'
assert_contains "$boards" 'tools/pi-image/test-rpi-kernel-image.sh'
raspberry_synthetic_tests_block="$(sed -n '/^      - name: Run synthetic kernel tests first$/,/^      - name: Install Raspberry kernel constructor dependencies$/p' "$boards")"
assert_block_contains "$raspberry_synthetic_tests_block" 'bash tools/pi-kernel/test-rpi-kernel.sh'
assert_block_contains "$raspberry_synthetic_tests_block" 'sudo bash tools/pi-image/test-rpi-kernel-image.sh'
octessera_reject_text_match 'The Raspberry kernel synthetic test must remain unprivileged.' "$raspberry_synthetic_tests_block" -qF 'sudo bash tools/pi-kernel/test-rpi-kernel.sh'
assert_contains "$boards" 'runtime_bundle_path:'
assert_contains "$boards" 'CROSS_SHA256: 642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad'
assert_contains "$boards" 'https://github.com/cross-rs/cross/releases/download/v0.2.5/cross-x86_64-unknown-linux-gnu.tar.gz'
assert_contains "$boards" "curl --fail --location --proto '=https' --tlsv1.2"
assert_contains "$boards" 'sha256sum -c -'
assert_contains "$device_packager" '"updater_supported": False'
assert_contains "$device_packager" '"candidate_health_protocol": 1'
assert_contains "$device_packager" '"distribution": "standalone-manual"'
assert_contains "$device_packager" 'standalone-manual-aarch64.zip'
assert_contains "$updater_profiles" 'runtime-updater-aarch64.zip'
assert_contains "$device_packager" 'updater_manifest = release_manifest(profile, tag, version, updater=True)'
[[ "$(grep -cF '"updater_protocol": 2' "$device_packager")" == 1 ]] || {
    echo 'The shared updater manifest contract must declare updater_protocol 2 exactly once.' >&2
    exit 1
}
octessera_reject_file_match 'Orange standalone device ZIP must use the explicit manual filename.' -qF 'orange-pi-zero-2w-device-aarch64.zip' "$release" "$boards"
assert_contains "$boards" 'octessera-orange-kernel-provenance.txt'
assert_contains "$boards" 'canonical_release_paths='
assert_contains "$boards" '"release-assets/${canonical_packages[0]}"'
assert_contains "$boards" '"release-assets/${canonical_packages[1]}"'
assert_contains "$boards" '--linux-image "${canonical_release_paths[0]}"'
assert_contains "$boards" '--linux-dtb "${canonical_release_paths[1]}"'
assert_contains "$boards" 'for required in "$image" "$image.sha256" "${canonical_release_paths[0]}" "${canonical_release_paths[1]}"'
assert_contains "$boards" 'sha256sum "$(basename "$image.sha256")" "${canonical_packages[0]}" "${canonical_packages[1]}"'
assert_contains "$release" 'git/ref/tags/$EXPECTED_RELEASE_TAG'
assert_contains "$release" 'git/tags/$tag_object'
assert_contains "$board_release" 'expected_native = tuple'
assert_contains "$board_release" 'source_lock_effective_path'
assert_contains "$board_release" 'expected_native_name'
assert_block_contains "$publisher_dependencies_step" 'sudo apt-get install -y --no-install-recommends cpio device-tree-compiler zstd'
assert_contains "$board_release" 'kernel_source_repository'
assert_absent "$release" 'expected_count=28'
assert_absent "$release" 'release-assets/$prefix-notices.zip'
assert_absent "$release" 'release-assets/$rpi_kernel_package'

raspberry_config_setup="$(sed -n '/^      - name: Configure and run pi-gen$/,/^          cat > pi-gen\/config <<EOF$/p' "$boards")"
raspberry_config_block="$(sed -n '/cat > pi-gen\/config <<EOF$/,/^[[:space:]]*cd pi-gen$/p' "$boards")"
raspberry_config_step="$(sed -n '/^      - name: Configure and run pi-gen$/,/^      - name: Select and verify the single Raspberry ZIP$/p' "$boards")"
raspberry_stage_copy_step="$(sed -n '/^      - name: Stage Raspberry legal notices and copy disposable stage4$/,/^      - name: Configure and run pi-gen$/p' "$boards")"
raspberry_stage_copy_command="$(grep -F 'cp -a tools/pi-image/stage4-octessera pi-gen/' <<< "$raspberry_stage_copy_step" | sed 's/^[[:space:]]*//' || true)"
[[ "$raspberry_stage_copy_command" == 'sudo cp -a tools/pi-image/stage4-octessera pi-gen/' ]] || {
    echo 'The disposable Raspberry stage4 copy must preserve root ownership with sudo cp -a.' >&2
    exit 1
}
assert_block_contains "$raspberry_config_setup" 'export OCTESSERA_RELEASE_VERSION="${{ inputs.version }}" OCTESSERA_RELEASE_TAG="${{ inputs.tag }}" OCTESSERA_BOARD_PROFILE_ID="raspberry-pi-zero-2w"'
assert_block_contains "$raspberry_config_block" 'OCTESSERA_RELEASE_VERSION=$OCTESSERA_RELEASE_VERSION'
assert_block_contains "$raspberry_config_block" 'OCTESSERA_RELEASE_TAG=$OCTESSERA_RELEASE_TAG'
assert_block_contains "$raspberry_config_block" 'OCTESSERA_BOARD_PROFILE_ID=$OCTESSERA_BOARD_PROFILE_ID'
preserve_env_command="$(grep -F -- 'sudo --preserve-env=' <<< "$raspberry_config_step" | sed 's/^[[:space:]]*//' || true)"
[[ "$preserve_env_command" == 'sudo --preserve-env=OCTESSERA_RELEASE_VERSION,OCTESSERA_RELEASE_TAG,OCTESSERA_BOARD_PROFILE_ID,OCTESSERA_KERNEL_PACKAGE,OCTESSERA_KERNEL_CHECKSUMS,OCTESSERA_KERNEL_PROVENANCE,OCTESSERA_REPOSITORY_ROOT ./build.sh' ]] || {
    echo 'Raspberry pi-gen must preserve exactly the release and kernel environment variables in order.' >&2
    exit 1
}
[[ "$(grep -cE '^[[:space:]]+EOF$' <<< "$raspberry_config_block")" == 1 ]] || {
    echo 'Raspberry pi-gen config heredoc must close with one standalone EOF.' >&2
    exit 1
}
octessera_reject_text_match 'Raspberry pi-gen config heredoc must not use EOL as its terminator.' "$raspberry_config_block" -qE '^[[:space:]]+EOL$'

assert_contains "$boards" 'cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-raspberry-pi-zero-2w'
assert_contains "$boards" 'cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w'
raspberry_builds="$(grep -cF -- 'cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-raspberry-pi-zero-2w' "$boards")"
orange_builds="$(grep -cF -- 'cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w' "$boards")"
[[ "$raspberry_builds" == 1 && "$orange_builds" == 1 ]] || {
    echo "Expected one exact release runtime build per board ($raspberry_builds Raspberry, $orange_builds Orange)." >&2
    exit 1
}
[[ "$(grep -cF 'CROSS_SHA256: 642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad' "$boards")" == 2 ]] || {
    echo 'Both runtime jobs must pin and verify the cross archive digest.' >&2
    exit 1
}
[[ "$(grep -cF 'ref: ${{ inputs.source_sha }}' "$boards")" == 7 ]] || {
    echo 'Every board source-consuming job must checkout source_sha exactly once.' >&2
    exit 1
}
[[ "$(grep -cF 'ref: ${{ needs.release_info.outputs.source_sha }}' "$release")" == 4 ]] || {
    echo 'Every main release source-consuming job must checkout source_sha exactly once.' >&2
    exit 1
}

octessera_reject_file_match 'Release workflows must not select an ambiguous artifact with find|head -n1.' -qE 'find[^\n]*\|[[:space:]]*head[[:space:]]+-n[[:space:]]*1' "$release" "$boards"
octessera_reject_file_match 'Release workflows must not hide collisions with --clobber.' -qF -- '--clobber' "$release" "$boards"
octessera_reject_file_match 'Cross installation must verify a downloaded archive before extraction.' -qF 'curl -sSL' "$boards"
octessera_reject_file_match 'Cross installation must verify a downloaded archive before extraction.' -qF '| sudo tar' "$boards"

assert_contains "$sanitizer" 'Expected exactly one .img inside'
assert_contains "$sanitizer" 'require_managed_runtime_binary "$WORK_DIR/root"'
assert_contains "$sanitizer" 'source "$SCRIPT_DIR/verify-managed-runtime.sh"'
assert_contains "$boards" 'verify-sanitized-image.sh --verification-profile full-constructor --runtime-bundle runtime-bundle "$asset"'
bash -n "$runtime_chain_helper" "$runtime_chain_test"
bash "$runtime_chain_test"
bash -n "$boot_layout_test"
bash "$boot_layout_test"

printf 'Release workflow static checks passed\n'
