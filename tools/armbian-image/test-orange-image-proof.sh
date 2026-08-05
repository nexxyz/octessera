#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash -n "$script_dir/verify-orange-image.sh"
if [[ "$(id -u)" != 0 ]] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
  exec sudo -n python3 -B "$script_dir/test-orange-image-proof.py"
fi
python3 -B "$script_dir/test-orange-image-proof.py"
