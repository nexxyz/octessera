#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$script_dir/verify-managed-runtime.sh"

fixture_root="$(mktemp -d)"
trap 'rm -rf "$fixture_root"' EXIT

reset_fixture() {
    rm -rf "$fixture_root"
    mkdir -p \
        "$fixture_root/usr/local/bin" \
        "$fixture_root/opt/octessera/releases/1.2.3" \
        "$fixture_root/runtime-bundle"
    printf '%s\n' 'runtime' > "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi"
    chmod 0755 "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi"
    ln -s /opt/octessera/releases/1.2.3 "$fixture_root/opt/octessera/current"
    ln -s /opt/octessera/current/octessera-pi "$fixture_root/usr/local/bin/octessera-pi"
    printf '%s\n' 'runtime' > "$fixture_root/runtime-bundle/octessera-pi"
    chmod 0755 "$fixture_root/runtime-bundle/octessera-pi"
    binary_sha256="$(sha256sum "$fixture_root/runtime-bundle/octessera-pi" | awk '{print $1}')"
    printf '%s\n' "{\"artifact_kind\":\"production-runtime\",\"binary_sha256\":\"$binary_sha256\",\"name\":\"octessera-pi\",\"profile\":\"raspberry-pi-zero-2w\",\"runtime_ready\":true,\"version\":\"1.2.3\"}" > "$fixture_root/runtime-bundle/octessera-runtime.json"
    printf '%s  octessera-pi\n' "$binary_sha256" > "$fixture_root/runtime-bundle/SHA256SUMS"
}

expect_rejected() {
    local name="$1"
    if require_managed_runtime_binary "$fixture_root" "$fixture_root/runtime-bundle"; then
        echo "Runtime chain case was accepted: $name" >&2
        return 1
    fi
}

reset_fixture
require_managed_runtime_binary "$fixture_root"

reset_fixture
require_managed_runtime_binary "$fixture_root" "$fixture_root/runtime-bundle"

reset_fixture
mkdir -p "$fixture_root/opt/octessera/releases/1.2.4"
cp "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi" "$fixture_root/opt/octessera/releases/1.2.4/octessera-pi"
ln -sfn /opt/octessera/releases/1.2.4 "$fixture_root/opt/octessera/current"
expect_rejected wrong-version

reset_fixture
printf '%s\n' 'tampered-runtime' > "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi"
expect_rejected byte-tampered-release

reset_fixture
ln -sfn /opt/octessera/releases/1.2.3/octessera-pi "$fixture_root/usr/local/bin/octessera-pi"
expect_rejected wrong-bin-target

reset_fixture
ln -sfn /opt/octessera/releases/1.2.4 "$fixture_root/opt/octessera/current"
expect_rejected wrong-current-target

reset_fixture
ln -sfn /opt/octessera/releases/1.2.3/../1.2.3 "$fixture_root/opt/octessera/current"
expect_rejected current-target-traversal

reset_fixture
ln -sfn /opt/octessera/releases/latest "$fixture_root/opt/octessera/current"
expect_rejected current-target-non-semver

reset_fixture
rm "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi"
printf '%s\n' 'runtime' > "$fixture_root/other-binary"
chmod 0755 "$fixture_root/other-binary"
ln -s "$fixture_root/other-binary" "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi"
expect_rejected symlinked-underlying-binary

reset_fixture
chmod 0644 "$fixture_root/opt/octessera/releases/1.2.3/octessera-pi"
expect_rejected non-executable-binary

printf '%s\n' 'Managed runtime chain tests passed'
