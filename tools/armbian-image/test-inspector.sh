#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
for module in \
  test-inspector-account.sh \
  test-inspector-network.sh \
  test-inspector-device-tree.sh \
  test-inspector-runtime.sh; do
  bash "$root/tools/armbian-image/$module"
done

printf '%s\n' 'Armbian inspector fixtures passed.'
