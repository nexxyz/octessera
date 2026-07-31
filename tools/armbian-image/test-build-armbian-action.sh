#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
action="$root/.github/actions/build-armbian-image/action.yml"
provenance_writer="$root/tools/armbian-image/write-orange-kernel-provenance.sh"

[[ -f "$action" ]] || { echo "Missing Armbian build action." >&2; exit 1; }
[[ -f "$provenance_writer" ]] || { echo "Missing Orange provenance writer." >&2; exit 1; }

assert_action_contains() {
    local expected="$1"
    grep -qF -- "$expected" "$action" || {
        echo "Armbian build action is missing: $expected" >&2
        exit 1
    }
}

assert_provenance_contains() {
    local expected="$1"
    grep -qF -- "$expected" "$provenance_writer" || {
        echo "Orange provenance writer is missing: $expected" >&2
        exit 1
    }
}

assert_action_contains 'KERNELBRANCH=commit:e46dc0adfe39724bcf52cea47b8f9c9aed86a394'
assert_action_contains 'OCTESSERA_ARMBIAN_KERNEL_BRANCH" == current'
assert_action_contains 'image_kind:'
assert_action_contains 'default: diagnostic'
assert_action_contains 'runtime_bundle_path:'
assert_action_contains 'Diagnostic images must not receive a runtime bundle.'
assert_action_contains 'Production runtime bundle must contain exactly'
assert_action_contains 'image-contract.json'
assert_action_contains 'runtime_enabled_default": true'
assert_action_contains 'Expected exactly one image artifact'
assert_action_contains 'inspect-output-images.sh" --mode "$OCTESSERA_IMAGE_KIND"'
assert_action_contains "find build/output/images -maxdepth 1 -type f -name '*.img.xz.sha'"
assert_action_contains 'sha256sum -c'
assert_action_contains 'Expected exactly one generated .img.xz.sha checksum file'
assert_action_contains 'validate-orange-kernel-package.sh'
assert_action_contains 'find-orange-kernel-packages.sh'
assert_action_contains 'Stage canonical Orange kernel packages for release handoff'
assert_action_contains 'native Orange linux-image/linux-dtb package pair'
assert_action_contains 'orange-midi-interface-manifest.json'
assert_provenance_contains 'linux-kernel-worktree'
assert_provenance_contains 'os.walk(source_root)'
assert_provenance_contains 'merge-base", "--is-ancestor"'
assert_provenance_contains 'kernel_source_base_commit'
assert_provenance_contains 'kernel_source_base_is_ancestor=true'
assert_provenance_contains 'kernel_source_remote_url'
assert_provenance_contains 'module_interface_options_marker'
assert_provenance_contains 'module_interface_runtime_marker'
assert_provenance_contains 'kernel_config_expected_packaged_sha256'
assert_action_contains 'GITHUB_SOURCE_SHA: ${{ inputs.source_sha || github.sha }}'
assert_action_contains "octessera_checkout_head=\"\$(git -C \"\$custom_root\" rev-parse HEAD)\""
assert_action_contains "\"\$octessera_checkout_head\" == \"\$GITHUB_SOURCE_SHA\""
assert_action_contains 'build/output/images'
assert_action_contains 'verify-orange-image.sh'
assert_action_contains 'octessera-orange-image-provenance.txt'
assert_action_contains 'Prove final Orange image against exact packages'

if grep -qF "printf '%s\\n' \"github_source_sha=\$GITHUB_SOURCE_SHA\"" "$action"; then
    echo 'Armbian build action must not contain a dead provenance printf shim.' >&2
    exit 1
fi

if grep -qF "KERNELBRANCH=\"\$OCTESSERA_ARMBIAN_KERNEL_BRANCH\"" "$action"; then
    echo 'Armbian build action must not pass the rolling kernel branch as KERNELBRANCH.' >&2
    exit 1
fi

if grep -qF -- '--expected-config-sha256' "$action"; then
    echo 'Armbian build action must use the manifest packaged config hash without a caller-supplied override.' >&2
    exit 1
fi

if grep -qF 'kernel_source_checkout_path=unavailable' "$provenance_writer" || grep -qF 'kernel_source_checkout_head=unavailable' "$provenance_writer"; then
    echo 'Orange provenance must not emit unavailable kernel checkout evidence.' >&2
    exit 1
fi

printf 'Armbian build action static checks passed\n'
