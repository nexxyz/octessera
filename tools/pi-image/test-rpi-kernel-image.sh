#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash -n "$script_dir/stage3-octessera-kernel/prerun.sh"
bash -n "$script_dir/stage3-octessera-kernel/00-install-kernel/00-run.sh"
bash -n "$script_dir/stage3-octessera-kernel/files/root/usr/local/sbin/octessera-finalize-rpi-kernel"
bash -n "$script_dir/stage4-octessera/03-boot-config/00-run.sh"
bash -n "$script_dir/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'OCTESSERA_REPOSITORY_ROOT:?OCTESSERA_REPOSITORY_ROOT must point to the canonical source checkout' "$script_dir/stage4-octessera/02-setup-service/00-run.sh"
grep -qF -- "--repository-root \"\$LEGAL_REPOSITORY_ROOT\"" "$script_dir/stage4-octessera/02-setup-service/00-run.sh"
python3 -B "$script_dir/test-rpi-kernel-mount.py"
python3 -B "$script_dir/test-rpi-kernel-image.py"
