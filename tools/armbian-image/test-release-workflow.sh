#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
release="$root/.github/workflows/release-artifacts.yml"
boards="$root/.github/workflows/release-board-artifacts.yml"
sanitizer="$root/tools/pi-image/verify-sanitized-image.sh"

assert_contains() {
    local file="$1"
    local expected="$2"
    grep -qF -- "$expected" "$file" || {
        echo "$file is missing: $expected" >&2
        exit 1
    }
}

assert_contains "$release" 'workflow_dispatch:'
assert_contains "$release" 'group: release-artifacts-${{ inputs.tag }}'
assert_contains "$release" 'cancel-in-progress: false'
assert_contains "$release" $'permissions:\n  contents: read'
assert_contains "$release" $'permissions:\n      contents: write'
if grep -qE '^  release:' "$release"; then
    echo 'Release workflow must not trigger from publication.' >&2
    exit 1
fi
assert_contains "$release" 'The release must remain a draft until the final publish job.'
assert_contains "$release" 'git rev-parse "$RELEASE_TAG^{commit}"'
assert_contains "$release" 'gh release upload'
assert_contains "$release" 'gh release edit'
assert_contains "$release" '--draft=false'
assert_contains "$release" 'Release already has assets before upload.'
assert_contains "$release" 'release_id: ${{ steps.release.outputs.release_id }}'
assert_contains "$release" 'Revalidate exact draft immediately before upload'
assert_contains "$release" 'Revalidate uploaded asset set immediately before publish'
assert_contains "$release" 'EXPECTED_RELEASE_ID'
assert_contains "$release" 'git/ref/tags/$EXPECTED_RELEASE_TAG'
assert_contains "$release" 'git/tags/$tag_object'
assert_contains "$release" 'RELEASE_VERSION}" == 0.7.5'
assert_contains "$release" '0.7.5-1_arm64.deb'
assert_contains "$release" 'native_prefix = canonical_name.removesuffix(".deb") + "__"'
assert_contains "$boards" 'hardware-raspberry-pi-zero-2w'
assert_contains "$boards" 'hardware-orange-pi-zero-2w'
assert_contains "$boards" 'd7a31c6aa09f4b867902c51da2b45807c0a1709e'
assert_contains "$boards" 'STAGE_LIST="stage0 stage1 stage2 stage3-octessera-kernel stage4-octessera"'
assert_contains "$boards" 'tools/pi-kernel/test-rpi-kernel.sh'
assert_contains "$boards" 'tools/pi-image/test-rpi-kernel-image.sh'
assert_contains "$boards" 'verify-rpi-kernel-image.sh'
assert_contains "$boards" 'octessera-${{ inputs.version }}-orange-pi-zero-2w.img.xz'
assert_contains "$boards" 'octessera-${{ inputs.version }}-orange-pi-zero-2w-standalone-manual-aarch64.zip'
assert_contains "$boards" 'runtime_bundle_path:'
assert_contains "$boards" 'CROSS_SHA256: 642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad'
assert_contains "$boards" 'sha256sum -c -'
assert_contains "$boards" 'updater_supported": False'
assert_contains "$boards" 'distribution": "standalone-manual"'
assert_contains "$boards" 'orange-pi-zero-2w-standalone-manual-aarch64.zip'
[[ "$(grep -cF '"updater_protocol": 2' "$boards")" == 1 ]] || {
    echo 'Only the Raspberry device metadata may claim updater_protocol 2.' >&2
    exit 1
}
if grep -qF 'orange-pi-zero-2w-device-aarch64.zip' "$release" "$boards"; then
    echo 'Orange standalone device ZIP must use the explicit manual filename.' >&2
    exit 1
fi
assert_contains "$boards" 'verify-orange-image.sh'
assert_contains "$boards" 'octessera-orange-image-provenance.txt'
assert_contains "$release" 'octessera-orange-image-provenance.txt'
assert_contains "$release" 'kernel_source_remote_url'
assert_contains "$release" 'expected_count=27'
assert_contains "$sanitizer" 'Expected exactly one .img inside'

if grep -Eq 'find[^\n]*\|[[:space:]]*head[[:space:]]+-n[[:space:]]*1' "$release" "$boards"; then
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

raspberry_builds="$(grep -c -- '--features hardware-raspberry-pi-zero-2w' "$boards")"
orange_builds="$(grep -c -- '--features hardware-orange-pi-zero-2w' "$boards")"
[[ "$raspberry_builds" == 1 && "$orange_builds" == 1 ]] || {
    echo "Expected one runtime build per board ($raspberry_builds Raspberry, $orange_builds Orange)." >&2
    exit 1
}

[[ "$(grep -cF 'CROSS_SHA256: 642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad' "$boards")" == 2 ]] || {
    echo 'Both runtime jobs must pin and verify the cross archive digest.' >&2
    exit 1
}
[[ "$(grep -cF 'ref: ${{ inputs.source_sha }}' "$boards")" == 7 ]] || {
    echo 'Every board source-consuming job must checkout source_sha.' >&2
    exit 1
}
[[ "$(grep -cF 'ref: ${{ needs.release_info.outputs.source_sha }}' "$release")" == 5 ]] || {
    echo 'Every main release source-consuming job must checkout source_sha.' >&2
    exit 1
}

printf 'Release workflow static checks passed\n'
