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

image_root="$work/rootfs"
mkdir -p "$image_root/etc/octessera" "$work/tmp"
printf '%s\n' \
  'OCTESSERA_PI_DEFAULT_SHA256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' \
  'OCTESSERA_SAMPLES_MANIFEST_SHA256=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' > "$image_root/etc/octessera/build-metadata.env"
if TMPDIR="$work/tmp" bash "$root/tools/armbian-image/inspect-built-image.sh" "$image_root" >/dev/null 2>"$work/inspector.stderr"; then
  echo 'Incomplete image fixture unexpectedly passed inspection.' >&2
  exit 1
fi
if find "$work/tmp" -mindepth 1 -print -quit | grep -q .; then
  echo 'Built-image inspection did not clean its temporary work after failure.' >&2
  exit 1
fi

printf '%s\n' 'Validation runner ordering, failure propagation, and inspector cleanup tests passed.'
