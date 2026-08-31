#!/usr/bin/env bash
# shellcheck disable=SC2016
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
action="$root/.github/actions/build-armbian-image/action.yml"
provenance_writer="$root/tools/armbian-image/write-orange-kernel-provenance.sh"
source_lock="$root/userpatches/config/sources/git_sources.json"

[[ -f "$action" ]] || { echo "Missing Armbian build action." >&2; exit 1; }
[[ -f "$provenance_writer" ]] || { echo "Missing Orange provenance writer." >&2; exit 1; }
[[ -f "$source_lock" && ! -L "$source_lock" ]] || { echo 'Missing or symlinked reviewed Orange source lock.' >&2; exit 1; }
printf '%s  %s\n' e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30 "$source_lock" | sha256sum -c -

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

assert_action_contains 'REVISION=26.11.0-trunk.22'
assert_action_contains '"HOST=octessera-opi"'
octessera_reject_file_match 'Ordinary Armbian builds must not retain the old Armbian revision.' -qF 'REVISION=26.8.0-trunk.417' "$action"
octessera_reject_file_match 'Ordinary Armbian builds must not retain the old KERNELBRANCH pin.' -qF 'KERNELBRANCH=commit:e46dc0adfe39724bcf52cea47b8f9c9aed86a394' "$action"
octessera_reject_file_match 'Armbian build action must not pass KERNELBRANCH.' -qF 'KERNELBRANCH=' "$action"
[[ "$(grep -cF 'ARMBIAN_BUILD_REF" == 3da49cffcb8ac58a919d86816fec4659c410ff1e' "$action")" == 4 ]] || {
    echo 'All action validation branches must require the reviewed Armbian ref.' >&2
    exit 1
}
assert_action_contains 'OCTESSERA_ARMBIAN_KERNEL_BRANCH" == current'
assert_action_contains 'rolling_pin_bootstrap:'
assert_action_contains 'default: false'
assert_action_contains 'prepare-orange-rolling-pin-bootstrap.sh'
assert_action_contains 'capture-orange-rolling-pin-bootstrap.sh'
assert_action_contains 'Rolling-pin bootstrap is allowed only from workflow_dispatch.'
assert_action_contains 'Stage reviewed Orange source lock'
assert_action_contains 'userpatches/config/sources/git_sources.json'
assert_action_contains 'build/config/sources/git_sources.json'
assert_action_contains 'e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30'
assert_action_contains 'cmp -- "$source_lock" "$effective_source_lock"'
assert_action_contains 'install -m 0644 "$source_lock" "$effective_source_lock"'
stage_lock_block="$(sed -n '/^    - name: Stage reviewed Orange source lock$/,/^    - name:/p' "$action")"
grep -qF "if: \${{ inputs.rolling_pin_bootstrap != 'true' }}" <<< "$stage_lock_block" || {
    echo 'Reviewed source lock staging must skip bootstrap discovery.' >&2
    exit 1
}
octessera_require_text_match 'Reviewed source lock staging must reject symlink replacement.' "$stage_lock_block" -qF '! -L "$effective_source_lock"'
bootstrap_prepare_block="$(sed -n '/^    - name: Prepare rolling-pin candidate source lock$/,/^    - name:/p' "$action")"
grep -qF "if: \${{ inputs.rolling_pin_bootstrap == 'true' }}" <<< "$bootstrap_prepare_block" || {
    echo 'Rolling-pin source discovery must remain bootstrap-only.' >&2
    exit 1
}
assert_action_contains "effective_extensions\" == 'octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'"
assert_action_contains "if: \${{ inputs.rolling_pin_bootstrap != 'true' }}"
inspect_condition="$(sed -n '/^    - name: Inspect built image$/,/^      shell: bash$/p' "$action")"
grep -qF "if: \${{ inputs.rolling_pin_bootstrap != 'true' }}" <<< "$inspect_condition" || {
    echo 'Bootstrap builds must skip the old manifest-bound image inspection.' >&2
    exit 1
}
octessera_reject_file_match 'Armbian build action must not contain the rolling-pin source-lock implementation.' -qF 'artifact-config-dump-json' "$action"
octessera_reject_file_match 'Armbian build action must not contain the rolling-pin evidence implementation.' -qF 'dpkg-deb -f' "$action"
bootstrap_build_step="$(sed -n '/^    - name: Build image$/,/^    - name: Capture rolling-pin bootstrap evidence$/p' "$action")"
grep -qF 'build_args+=(REVISION=26.11.0-trunk.22)' <<< "$bootstrap_build_step" || {
    echo 'Armbian build must use the reviewed candidate revision.' >&2
    exit 1
}
octessera_reject_text_match 'Bootstrap build must omit KERNELBRANCH.' "$bootstrap_build_step" -qF 'KERNELBRANCH='
assert_action_contains 'image_kind:'
assert_action_contains 'default: diagnostic'
assert_action_contains 'runtime_bundle_path:'
assert_action_contains 'Diagnostic images must not receive a runtime bundle.'
assert_action_contains 'Production runtime bundle must contain exactly'
assert_action_contains 'image-contract.json'
assert_action_contains 'runtime_enabled_default": true'
assert_action_contains 'Expected exactly one image artifact'
assert_action_contains 'inspect-output-images.sh" --verification-profile full-constructor --mode "$OCTESSERA_IMAGE_KIND"'
assert_action_contains "find build/output/images -maxdepth 1 -type f -name '*.img.xz.sha'"
assert_action_contains 'sha256sum -c'
assert_action_contains 'Expected exactly one generated .img.xz.sha checksum file'
assert_action_contains 'validate-orange-kernel-package.sh'
assert_action_contains 'find-orange-kernel-packages.sh'
assert_action_contains 'Stage canonical Orange kernel packages for release handoff'
assert_action_contains 'native Orange linux-image/linux-dtb package pair'
assert_action_contains 'orange-midi-interface-manifest.json'
assert_provenance_contains 'git", "-C", str(armbian_build_directory), "rev-parse", "HEAD"'
assert_provenance_contains 'armbian_build_repository'
assert_provenance_contains 'kernel_source_repository'
assert_provenance_contains 'kernel_source_commit'
assert_provenance_contains 'kernel_config_source_sha256'
assert_provenance_contains 'core_series_sha256'
assert_provenance_contains 'patching_order_source_sha256'
assert_provenance_contains 'accepted_upstream_patch_sha256'
assert_provenance_contains 'octessera_follow_up_patch_sha256'
assert_provenance_contains 'image_package_handoff_sha256'
assert_provenance_contains 'dtb_package_handoff_sha256'
assert_provenance_contains 'github_source_sha'
assert_provenance_contains 'module_interface_options_marker'
assert_provenance_contains 'module_interface_runtime_marker'
assert_provenance_contains 'kernel_config_expected_packaged_sha256'
assert_action_contains 'GITHUB_SOURCE_SHA: ${{ inputs.source_sha || github.sha }}'
assert_action_contains 'octessera_audio'
assert_action_contains "octessera_checkout_head=\"\$(git -C \"\$custom_root\" rev-parse HEAD)\""
assert_action_contains "\"\$octessera_checkout_head\" == \"\$GITHUB_SOURCE_SHA\""
assert_action_contains 'build/output/images'
assert_action_contains 'verify-orange-image.sh'
assert_action_contains 'apt-get install -y --no-install-recommends cpio zstd'
assert_action_contains 'octessera-orange-image-proof.json'
assert_action_contains 'Prove final Orange image against exact packages'
assert_action_contains 'tools/legal/stage_notices.py'
assert_action_contains 'tools/wifi-connect/build-patched-ci.sh'
assert_action_contains 'target/wifi-connect-patched'
assert_action_contains 'third_party/wifi-connect-4.11.84'
assert_action_contains '4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627'
assert_action_contains 'c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6'
assert_action_contains 'usr/local/share/octessera/wifi-connect'
assert_action_contains 'custom/tools/storage/octessera-sd-card'
assert_action_contains 'custom/tools/storage/octessera-sd-card-lib.sh'
assert_action_contains 'custom/tools/storage/octessera-orange-storage'
assert_action_contains 'custom/tools/storage/octessera-orange-storage-control'
assert_action_contains 'octessera_sd2'
octessera_reject_file_match 'Armbian image construction must not download upstream wifi-connect.' -qE 'wifi-connect(-aarch64-unknown-linux-gnu)?\.tar\.gz|github\.com/balena-os/wifi-connect/releases' "$action"
octessera_reject_file_match 'Armbian build action must not create a generated legal source tree.' -qF 'legal-source' "$action"
assert_action_contains 'Clean generated legal staging from disposable output'

octessera_reject_file_match 'Armbian build action must not contain a dead provenance printf shim.' -qF "printf '%s\\n' \"github_source_sha=\$GITHUB_SOURCE_SHA\"" "$action"

octessera_reject_file_match 'Armbian build action must not pass the rolling kernel branch as KERNELBRANCH.' -qF "KERNELBRANCH=\"\$OCTESSERA_ARMBIAN_KERNEL_BRANCH\"" "$action"

octessera_reject_file_match 'Armbian build action must use the manifest packaged config hash without a caller-supplied override.' -qF -- '--expected-config-sha256' "$action"

for removed_field in kernel_source_remote_url kernel_source_checkout_path kernel_source_checkout_head kernel_source_base_commit kernel_source_base_is_ancestor; do
    octessera_reject_file_match "Orange provenance must not contain removed kernel worktree field: $removed_field" -qF "$removed_field" "$provenance_writer"
done

printf 'Armbian build action static checks passed\n'
