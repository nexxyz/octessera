#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash -n "$script_dir/build-rpi-kernel.sh"
bash -n "$script_dir/validate-rpi-kernel-package.sh"
bash -n "$script_dir/test-rpi-kernel.sh"
python_cache="$(mktemp -d)"
cleanup() {
  rm -rf -- "$python_cache"
}
trap cleanup EXIT
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPYCACHEPREFIX="$python_cache"
python3 -B - "$python_cache" \
  "$script_dir/rpi_kernel_contract.py" \
  "$script_dir/build-rpi-kernel.py" \
  "$script_dir/validate-rpi-kernel-package.py" \
  "$script_dir/test-rpi-kernel.py" \
  "$script_dir/test-rpi-kernel-builder.py" <<'PY'
from pathlib import Path
import py_compile
import sys


cache = Path(sys.argv[1])
for source_name in sys.argv[2:]:
    source = Path(source_name)
    py_compile.compile(source, cfile=str(cache / f"{source.name}.pyc"), doraise=True)
PY
python3 -B "$script_dir/test-rpi-kernel.py"
if find "$script_dir" \( -type d -name __pycache__ -o -type f \( -name '*.pyc' -o -name '*.pyo' \) \) -print -quit | grep -q .; then
  echo "Python bytecode cache remains under $script_dir" >&2
  exit 1
fi
