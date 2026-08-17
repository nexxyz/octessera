#!/bin/bash

require_managed_runtime_binary() {
    local image_root="$1"
    local runtime_bundle="${2:-}"
    local bin_path="$image_root/usr/local/bin/octessera-pi"
    local current_path="$image_root/opt/octessera/current"
    local bin_target current_target version release_binary

    if [ ! -L "$bin_path" ]; then
        echo "Sanitation check failed: octessera-pi is not a managed symlink at $bin_path" >&2
        return 1
    fi
    bin_target="$(readlink "$bin_path")"
    if [ "$bin_target" != '/opt/octessera/current/octessera-pi' ]; then
        echo "Sanitation check failed: octessera-pi symlink target is not managed" >&2
        return 1
    fi

    if [ ! -L "$current_path" ]; then
        echo "Sanitation check failed: current release is not a managed symlink at $current_path" >&2
        return 1
    fi
    current_target="$(readlink "$current_path")"
    if [[ "$current_target" =~ ^/opt/octessera/releases/([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
        version="${BASH_REMATCH[1]}"
    else
        echo "Sanitation check failed: current release symlink target is not a shipped semver" >&2
        return 1
    fi

    release_binary="$image_root/opt/octessera/releases/$version/octessera-pi"
    if [ -L "$release_binary" ] || [ ! -f "$release_binary" ]; then
        echo "Sanitation check failed: release binary is not a regular file at $release_binary" >&2
        return 1
    fi
    if [ ! -x "$release_binary" ]; then
        echo "Sanitation check failed: release binary is not executable at $release_binary" >&2
        return 1
    fi
    if [ -n "$runtime_bundle" ]; then
        require_raspberry_runtime_bundle_match "$release_binary" "$version" "$runtime_bundle"
    fi
}

require_raspberry_runtime_bundle_match() {
    local release_binary="$1"
    local release_version="$2"
    local runtime_bundle="$3"
    local bundle_binary="$runtime_bundle/octessera-pi"
    local bundle_metadata="$runtime_bundle/octessera-runtime.json"
    local bundle_version

    if [ ! -d "$runtime_bundle" ] || [ -L "$runtime_bundle" ]; then
        echo "Sanitation check failed: Raspberry runtime bundle is not a directory at $runtime_bundle" >&2
        return 1
    fi
    if [ ! -f "$bundle_binary" ] || [ -L "$bundle_binary" ]; then
        echo "Sanitation check failed: Raspberry runtime bundle binary is missing or not regular" >&2
        return 1
    fi
    if [ ! -f "$bundle_metadata" ] || [ -L "$bundle_metadata" ]; then
        echo "Sanitation check failed: Raspberry runtime bundle metadata is missing or not regular" >&2
        return 1
    fi
    bundle_version="$(python3 - "$bundle_metadata" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    metadata = json.load(handle)
version = metadata.get("version")
if not isinstance(version, str):
    raise SystemExit("runtime bundle version is not a string")
print(version)
PY
    )" || {
        echo "Sanitation check failed: Raspberry runtime bundle metadata is invalid" >&2
        return 1
    }
    if [ "$bundle_version" != "$release_version" ]; then
        echo "Sanitation check failed: mounted Raspberry release version does not match the runtime bundle" >&2
        return 1
    fi
    if ! cmp -s "$release_binary" "$bundle_binary"; then
        echo "Sanitation check failed: mounted Raspberry release binary does not match the runtime bundle" >&2
        return 1
    fi
}
