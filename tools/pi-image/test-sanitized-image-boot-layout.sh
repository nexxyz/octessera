#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for module in \
    test-sanitized-image-boot-layout-layout.sh \
    test-sanitized-image-boot-layout-sanitization.sh \
    test-sanitized-image-boot-layout-boot.sh; do
    bash "$script_dir/$module"
done

printf '%s\n' 'Sanitized image boot layout tests passed'
