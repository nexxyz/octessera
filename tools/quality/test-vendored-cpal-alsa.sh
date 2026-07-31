#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$root/third_party/cpal-0.15.3/Cargo.toml"

if [[ "$(uname -s)" != Linux ]]; then
    printf 'Vendored CPAL ALSA tests skipped outside Linux\n'
    exit 0
fi

[[ -f "$manifest" ]] || { echo "Missing vendored CPAL manifest: $manifest" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo 'cargo is required for vendored CPAL tests.' >&2; exit 1; }

cargo test --manifest-path "$manifest" --locked --lib host::alsa::tests
