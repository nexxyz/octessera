#!/usr/bin/env bash
# shellcheck disable=SC2016
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
release="$root/.github/workflows/release-artifacts.yml"
boards="$root/.github/workflows/release-board-artifacts.yml"
action="$root/.github/actions/build-armbian-image/action.yml"
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
    if grep -qF -- "$unexpected" "$file"; then
        echo "$file contains removed release coupling: $unexpected" >&2
        exit 1
    fi
}

assert_block_contains() {
    local block="$1"
    local expected="$2"
    grep -qF -- "$expected" <<< "$block" || {
        echo "Workflow block is missing: $expected" >&2
        exit 1
    }
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
assert_contains "$release" 'shellcheck tools/armbian-image/test-release-workflow.sh'
assert_contains "$release" 'release_info:'
assert_contains "$release" 'Release version must be an exact semver.'
assert_contains "$release" 'version=${RELEASE_TAG#v}'
assert_contains "$release" 'Cargo package version'
assert_contains "$release" 'git rev-parse "$RELEASE_TAG^{commit}"'
assert_contains "$release" 'needs: [release_info, updater_protocol, windows, macos, ubuntu, board_artifacts, workflow_static]'
assert_contains "$release" 'The release must remain a draft until the final publish job.'
assert_contains "$release" 'The release draft must not already contain assets.'
assert_contains "$release" 'gh api --paginate --slurp "repos/$GITHUB_REPOSITORY/releases?per_page=100"'
assert_contains "$release" '[ .[][] | select(.tag_name == $tag) ] as $matches'
assert_contains "$release" 'if ($matches | length) != 1 then'
assert_contains "$release" 'elif $matches[0].draft != true then'
assert_contains "$release" 'elif ($matches[0].assets | length) != 0 then'
assert_contains "$release" 'release_id: ${{ needs.resolve_draft.outputs.release_id }}'
assert_contains "$release" 'gh release upload'
assert_contains "$release" 'gh release edit'
assert_contains "$release" '--draft=false'
assert_contains "$release" 'Release already has assets before upload.'
assert_contains "$release" 'Revalidate exact draft immediately before upload'
assert_contains "$release" 'Revalidate uploaded asset set immediately before publish'
assert_contains "$release" 'EXPECTED_RELEASE_ID'
assert_contains "$release" 'RPI_KERNEL_ARTIFACT_VERSION="0.7.5"'
assert_contains "$release" 'RPI_KERNEL_ARTIFACT_VERSION}-1_arm64.deb'
assert_contains "$release" 'package_notice_zip.py'
assert_contains "$release" 'verify_notice_archive.py --repository-root . --archive $portableAsset --payload-name octessera.exe'
assert_contains "$release" 'device ZIP inventory is not exact'
assert_contains "$release" 'device ZIP legal files do not match the release source'
assert_contains "$release" 'expected_count=28'
assert_contains "$release" 'require_exact_files "$root/octessera-windows-release-assets"'
assert_contains "$release" 'require_exact_files "$root/octessera-macos-release-assets"'
assert_contains "$release" 'require_exact_files "$root/octessera-ubuntu-release-assets"'
assert_contains "$release" 'copy_asset()'
assert_contains "$release" '[[ ! -e "$destination" ]]'
assert_contains "$release" 'mapfile -t final_files'
assert_contains "$release" 'mapfile -t uploaded_assets'
assert_contains "$release" 'Uploaded release asset names/count do not match the verified set.'
assert_contains "$release" 'if-no-files-found: error'
assert_contains "$boards" 'install -m 0644 LICENSE NOTICE device-release/'
assert_contains "$boards" 'Stage Raspberry legal notices and copy disposable stage4'
assert_contains "$boards" 'tools/legal/stage_notices.py'
assert_contains "$boards" '--check'
assert_contains "$boards" 'verify-rpi-kernel-image.sh'
assert_contains "$boards" 'verify-orange-image.sh'
assert_contains "$boards" 'octessera-orange-image-proof.json'
assert_contains "$boards" 'octessera-${{ inputs.version }}-raspberry-pi-zero-2w.img.zip'
assert_contains "$boards" 'octessera-${{ inputs.version }}-orange-pi-zero-2w.img.xz'
assert_contains "$action" 'tools/legal/stage_notices.py'
assert_contains "$root/resources/image-construction/boot-layers/raspberry-pi-zero-2w.json" 'resources/legal/notice-bundle.json'
assert_absent "$action" 'legal-source'

workflow_static_block="$(sed -n '/^  workflow_static:/,/^  resolve_draft:/p' "$release")"
if grep -qF 'permissions:' <<< "$workflow_static_block"; then
    echo 'workflow_static must inherit the top-level read-only permissions.' >&2
    exit 1
fi
if ! grep -qF 'bash -n tools/armbian-image/test-release-workflow.sh' <<< "$workflow_static_block" || ! grep -qF 'shellcheck tools/armbian-image/test-release-workflow.sh' <<< "$workflow_static_block"; then
    echo 'workflow_static must run syntax and ShellCheck validation.' >&2
    exit 1
fi
resolver_block="$(sed -n '/^  resolve_draft:/,/^  release_info:/p' "$release")"
if grep -qF 'actions/checkout' <<< "$resolver_block" || grep -qE '(^|[[:space:]])(python3|bash|sh|pnpm|cargo)[[:space:]]|tools/' <<< "$resolver_block" || grep -qF 'upload-artifact' <<< "$resolver_block"; then
    echo 'Draft resolver must remain API-only without checkout, scripts, or artifacts.' >&2
    exit 1
fi
publisher_block="$(sed -n '/^  publish_release_assets:/,$p' "$release")"
if ! grep -qF $'    permissions:\n      contents: write' <<< "$resolver_block" || ! grep -qF $'    permissions:\n      contents: write' <<< "$publisher_block"; then
    echo 'Contents write must belong to the resolver and publisher jobs.' >&2
    exit 1
fi
updater_block="$(sed -n '/^  updater_protocol:/,/^  windows:/p' "$release")"
if grep -qF 'bash tools/armbian-image/test-release-workflow.sh' <<< "$updater_block"; then
    echo 'The source-tag updater job must not run the workflow static test.' >&2
    exit 1
fi
if grep -qE 'shellcheck .*&&' <<< "$updater_block"; then
    echo 'Independent ShellCheck groups must fail separately.' >&2
    exit 1
fi
if grep -qE '^  release:' "$release"; then
    echo 'Release workflow must not trigger from publication.' >&2
    exit 1
fi

if grep -qF '/releases/tags/' "$release"; then
    echo 'Draft release validation must not use the by-tag releases endpoint.' >&2
    exit 1
fi
[[ "$(grep -cF 'gh api "repos/$GITHUB_REPOSITORY/releases/$EXPECTED_RELEASE_ID"' "$release")" == 2 ]] || {
    echo 'Both final release validations must fetch by the resolved release ID.' >&2
    exit 1
}
[[ "$(grep -cF 'EXPECTED_RELEASE_ID: ${{ needs.release_info.outputs.release_id }}' "$release")" == 2 ]] || {
    echo 'Upload and publish guards must receive the resolved release ID.' >&2
    exit 1
}
upload_guard_block="$(sed -n '/^      - name: Revalidate exact draft immediately before upload$/,/^      - name: Upload assets without collision hiding$/p' "$release")"
publish_guard_block="$(sed -n '/^      - name: Revalidate uploaded asset set immediately before publish$/,/^      - name: Publish the verified draft release last$/p' "$release")"
assert_block_contains "$upload_guard_block" 'EXPECTED_RELEASE_ID'
assert_block_contains "$publish_guard_block" 'EXPECTED_RELEASE_ID'
[[ "$(grep -cF 'gh release upload' "$release")" == 1 && "$(grep -cF 'gh release edit' "$release")" == 1 ]] || {
    echo 'Release upload and edit operations must remain single, explicit operations.' >&2
    exit 1
}
assert_order "$release" 'Revalidate exact draft immediately before upload' 'Upload assets without collision hiding'
assert_order "$release" 'Upload assets without collision hiding' 'Revalidate uploaded asset set immediately before publish'
assert_order "$release" 'Revalidate uploaded asset set immediately before publish' 'Publish the verified draft release last'

macos_block="$(sed -n '/^  macos:/,/^  ubuntu:/p' "$release")"
if grep -qF 'mapfile' <<< "$macos_block" || ! grep -qF 'shopt -s nullglob' <<< "$macos_block" || ! grep -qF 'dmg_files=(target/release/bundle/dmg/*.dmg)' <<< "$macos_block" || [[ "$(grep -cF '[[ "${#dmg_files[@]}" == 1 ]]' <<< "$macos_block")" != 1 ]]; then
    echo 'macOS DMG selection must fail closed on exact count.' >&2
    exit 1
fi

jq_draft_filter='[ .[][] | select(.tag_name == $tag) ] as $matches
  | if ($matches | length) != 1 then
      error("Expected exactly one release for tag \($tag)")
    elif $matches[0].draft != true then
      error("The release must remain a draft until the final publish job.")
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

for removed_step in 'Start-Process -FilePath' 'hdiutil attach' 'dpkg-deb -x' '--appimage-extract'; do
    assert_absent "$release" "$removed_step"
done

windows_block="$(sed -n '/^  windows:/,/^  macos:/p' "$release")"
macos_block="$(sed -n '/^  macos:/,/^  ubuntu:/p' "$release")"
ubuntu_block="$(sed -n '/^  ubuntu:/,/^  board_artifacts:/p' "$release")"
assert_block_contains "$windows_block" 'verify_notice_archive.py'
assert_block_contains "$macos_block" 'shasum -a 256 *.dmg > SHA256SUMS-macos.txt'
assert_block_contains "$ubuntu_block" 'sha256sum *.deb *.AppImage > SHA256SUMS-ubuntu.txt'

assert_contains "$boards" 'hardware-raspberry-pi-zero-2w'
assert_contains "$boards" 'hardware-orange-pi-zero-2w'
assert_contains "$boards" 'd7a31c6aa09f4b867902c51da2b45807c0a1709e'
assert_contains "$boards" 'STAGE_LIST="stage0 stage1 stage2 stage3-octessera-kernel stage4-octessera"'
assert_contains "$boards" 'tools/pi-kernel/test-rpi-kernel.sh'
assert_contains "$boards" 'tools/pi-image/test-rpi-kernel-image.sh'
assert_contains "$boards" 'runtime_bundle_path:'
assert_contains "$boards" 'CROSS_SHA256: 642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad'
assert_contains "$boards" 'https://github.com/cross-rs/cross/releases/download/v0.2.5/cross-x86_64-unknown-linux-gnu.tar.gz'
assert_contains "$boards" "curl --fail --location --proto '=https' --tlsv1.2"
assert_contains "$boards" 'sha256sum -c -'
assert_contains "$boards" '"updater_supported": False'
assert_contains "$boards" '"candidate_health_protocol": 1'
assert_contains "$boards" '"distribution": "standalone-manual"'
assert_contains "$boards" 'octessera-${{ inputs.version }}-orange-pi-zero-2w-standalone-manual-aarch64.zip'
[[ "$(grep -cF '"updater_protocol": 2' "$boards")" == 1 ]] || {
    echo 'Only the Raspberry device metadata may claim updater_protocol 2.' >&2
    exit 1
}
if grep -qF 'orange-pi-zero-2w-device-aarch64.zip' "$release" "$boards"; then
    echo 'Orange standalone device ZIP must use the explicit manual filename.' >&2
    exit 1
fi
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
assert_contains "$release" 'native_prefix = canonical_name.removesuffix(".deb") + "__"'
assert_contains "$release" 'apt-get install -y --no-install-recommends cpio zstd'
assert_contains "$release" 'kernel_source_repository'

raspberry_config_setup="$(sed -n '/^      - name: Configure and run pi-gen$/,/^          cat > pi-gen\/config <<EOF$/p' "$boards")"
raspberry_config_block="$(sed -n '/cat > pi-gen\/config <<EOF$/,/^[[:space:]]*cd pi-gen$/p' "$boards")"
raspberry_config_step="$(sed -n '/^      - name: Configure and run pi-gen$/,/^      - name: Select and verify the single Raspberry ZIP$/p' "$boards")"
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
if grep -qE '^[[:space:]]+EOL$' <<< "$raspberry_config_block"; then
    echo 'Raspberry pi-gen config heredoc must not use EOL as its terminator.' >&2
    exit 1
fi

raspberry_builds="$(grep -cF -- 'cross build --release --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-raspberry-pi-zero-2w' "$boards")"
orange_builds="$(grep -cF -- 'cross build --release --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w' "$boards")"
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
[[ "$(grep -cF 'ref: ${{ needs.release_info.outputs.source_sha }}' "$release")" == 5 ]] || {
    echo 'Every main release source-consuming job must checkout source_sha exactly once.' >&2
    exit 1
}

if grep -qE 'find[^\n]*\|[[:space:]]*head[[:space:]]+-n[[:space:]]*1' "$release" "$boards"; then
    echo 'Release workflows must not select an ambiguous artifact with find|head -n1.' >&2
    exit 1
fi
if grep -qF -- '--clobber' "$release" "$boards"; then
    echo 'Release workflows must not hide collisions with --clobber.' >&2
    exit 1
fi
if grep -qF 'curl -sSL' "$boards" || grep -qF '| sudo tar' "$boards"; then
    echo 'Cross installation must verify a downloaded archive before extraction.' >&2
    exit 1
fi

assert_contains "$sanitizer" 'Expected exactly one .img inside'
assert_contains "$sanitizer" 'require_managed_runtime_binary "$WORK_DIR/root"'
assert_contains "$sanitizer" 'source "$SCRIPT_DIR/verify-managed-runtime.sh"'
bash -n "$runtime_chain_helper" "$runtime_chain_test"
bash "$runtime_chain_test"
bash -n "$boot_layout_test"
bash "$boot_layout_test"

printf 'Release workflow static checks passed\n'
