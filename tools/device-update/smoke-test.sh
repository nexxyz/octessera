#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OCTESSERA_UPDATE_MODULE="$here/updater_protocol.py" python3 -m unittest discover -s "$here" -p 'test_*.py'
echo "device updater smoke test passed"
