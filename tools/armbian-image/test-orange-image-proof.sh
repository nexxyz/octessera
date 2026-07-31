#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash -n "$script_dir/verify-orange-image.sh"
python3 -B "$script_dir/test-orange-image-proof.py"
