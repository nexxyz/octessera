#!/usr/bin/env bash
# shellcheck disable=SC2094
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
runner="$root/tools/armbian-image/validation-runner.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=tools/armbian-image/validation-runner.sh
source "$runner"
trace="$work/trace"
make_stage() {
  local path="$1" status="$2"
  cat > "$path" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$(basename "$path")" >> "$trace"
exit $status
EOF
  chmod 0755 "$path"
}
first="$work/01-first.sh"
second="$work/02-second.sh"
third="$work/03-third.sh"
make_stage "$first" 0
make_stage "$second" 17
make_stage "$third" 0
if octessera_run_validation_stages "$first" "$second" "$third"; then
  echo 'Validation runner accepted a failed stage.' >&2
  exit 1
else
  status=$?
fi
[[ "$status" == 17 ]]
[[ "$(cat "$trace")" == $'01-first.sh\n02-second.sh' ]]

printf '%s\n' 'Validation runner ordering and failure propagation tests passed.'
