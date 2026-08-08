#!/usr/bin/env bash
# shellcheck disable=SC2317,SC2329
# Deterministic repository integrity gate with four independently addressable
# check classes. Runs offline and exits non-zero if any enabled class fails.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

root="$REPO_ROOT"
run_stale_assets=1
run_legal=1
run_links=1
run_quality=1

usage() {
  cat <<'EOF'
Usage: repository-integrity.sh [--root DIR] [--skip-stale-assets] [--skip-legal] [--skip-links] [--skip-quality]

Deterministic repository integrity gate with four independently addressable check classes:
  stale-assets  Generated config/capabilities/palette/logo outputs match their sources.
  legal         Lockfile-hash and checksum inventories are fresh.
  links         Internal Markdown links and anchors resolve.
  quality       Code quality regression thresholds hold.

Exit code 0 when all enabled classes pass, 1 when any class fails.
EOF
}

while (($#)); do
  case "$1" in
    --root)
      root="$(cd "$2" && pwd)"
      shift 2
      ;;
    --skip-stale-assets) run_stale_assets=0; shift ;;
    --skip-legal) run_legal=0; shift ;;
    --skip-links) run_links=0; shift ;;
    --skip-quality) run_quality=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

log_dir="$(mktemp -d)"
trap 'rm -rf "$log_dir"' EXIT
results=()

run_class() {
  local name="$1"
  shift
  if "$@" >"$log_dir/$name.log" 2>&1; then
    echo "[PASS] $name"
    results+=("PASS $name")
  else
    echo "[FAIL] $name"
    results+=("FAIL $name")
    if [ -s "$log_dir/$name.log" ]; then
      sed "s/^/  /" "$log_dir/$name.log"
    fi
  fi
}

check_stale_assets() {
  node "$REPO_ROOT/tools/config/generate-default-configs.mjs" --check --root "$root" || return 1
  node "$REPO_ROOT/tools/resources/generate-platform-capabilities.mjs" --check --root "$root" || return 1
  node "$REPO_ROOT/tools/resources/generate-display-palette.mjs" --check --root "$root" || return 1
  python3 "$REPO_ROOT/tools/assets/generate_pi_logo_pngs.py" --check --root "$root" || return 1
}

check_legal() {
  python3 "$REPO_ROOT/tools/legal/verify_inventory_freshness.py" --root "$root" || return 1
}

check_links() {
  (cd "$root" && python3 "$REPO_ROOT/tools/docs/check_links.py" .)
}

check_quality() {
  (cd "$root" && node "$REPO_ROOT/tools/quality/quality-audit.mjs")
}

if ((run_stale_assets)); then
  run_class stale-assets check_stale_assets
fi
if ((run_legal)); then
  run_class legal check_legal
fi
if ((run_links)); then
  run_class links check_links
fi
if ((run_quality)); then
  run_class quality check_quality
fi

failed=0
for result in "${results[@]}"; do
  if [[ "$result" == FAIL* ]]; then
    failed=1
    break
  fi
done
exit "$failed"
