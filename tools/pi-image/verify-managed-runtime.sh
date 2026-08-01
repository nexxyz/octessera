#!/bin/bash

require_managed_runtime_binary() {
    local image_root="$1"
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
}
