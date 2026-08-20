#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
fake_bin="$work/bin"
trace="$work/trace"
real_bash="$(command -v bash)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$fake_bin"

cat > "$fake_bin/sh" <<'EOF'
#!/usr/bin/bash
module="${1##*/}"
printf '%s\n' "$module" >> "$RUNNER_TRACE"
if [[ "$module" == "$RUNNER_FAIL_MODULE" ]]; then
  exit 23
fi
EOF
cat > "$fake_bin/bash" <<'EOF'
#!/usr/bin/bash
module="${1##*/}"
printf '%s\n' "$module" >> "$RUNNER_TRACE"
if [[ "$module" == "$RUNNER_FAIL_MODULE" ]]; then
  exit 23
fi
EOF
chmod 0755 "$fake_bin/sh" "$fake_bin/bash"

check_raspberry_usb_gadget_runner() {
  local expected=$'test-usb-gadget-layout.sh\ntest-usb-gadget-gadget.sh\ntest-usb-gadget-host.sh\ntest-usb-gadget-electrical.sh'
  : > "$trace"
  PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE= "$real_bash" "$root/tools/pi-image/test-usb-gadget.sh"
  [[ "$(cat "$trace")" == "$expected" ]]
  : > "$trace"
  local status
  if PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE=test-usb-gadget-gadget.sh "$real_bash" "$root/tools/pi-image/test-usb-gadget.sh"; then
    return 1
  else
    status=$?
  fi
  [[ "$status" == 23 ]]
  [[ "$(cat "$trace")" == $'test-usb-gadget-layout.sh\ntest-usb-gadget-gadget.sh' ]]
}

check_orange_usb_gadget_runner() {
  local expected=$'test-orange-pi-usb-gadget-host-enumeration.sh\ntest-orange-pi-usb-gadget-function.sh\ntest-orange-pi-usb-gadget-passive.sh\ntest-orange-pi-usb-gadget-electrical.sh'
  : > "$trace"
  PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE= "$real_bash" "$root/tools/orange-pi/test-orange-pi-usb-gadget.sh"
  [[ "$(cat "$trace")" == "$expected" ]]
  : > "$trace"
  local status
  if PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE=test-orange-pi-usb-gadget-function.sh "$real_bash" "$root/tools/orange-pi/test-orange-pi-usb-gadget.sh"; then
    return 1
  else
    status=$?
  fi
  [[ "$status" == 23 ]]
  [[ "$(cat "$trace")" == $'test-orange-pi-usb-gadget-host-enumeration.sh\ntest-orange-pi-usb-gadget-function.sh' ]]
}

check_inspector_runner() {
  local expected=$'test-inspector-account.sh\ntest-inspector-network.sh\ntest-inspector-device-tree.sh\ntest-inspector-runtime.sh'
  : > "$trace"
  PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE= "$real_bash" "$root/tools/armbian-image/test-inspector.sh"
  [[ "$(cat "$trace")" == "$expected" ]]
  : > "$trace"
  local status
  if PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE=test-inspector-network.sh "$real_bash" "$root/tools/armbian-image/test-inspector.sh"; then
    return 1
  else
    status=$?
  fi
  [[ "$status" == 23 ]]
  [[ "$(cat "$trace")" == $'test-inspector-account.sh\ntest-inspector-network.sh' ]]
}

check_boot_layout_runner() {
  local expected=$'test-sanitized-image-boot-layout-layout.sh\ntest-sanitized-image-boot-layout-sanitization.sh\ntest-sanitized-image-boot-layout-boot.sh'
  : > "$trace"
  PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE= "$real_bash" "$root/tools/pi-image/test-sanitized-image-boot-layout.sh"
  [[ "$(cat "$trace")" == "$expected" ]]
  : > "$trace"
  local status
  if PATH="$fake_bin:$PATH" RUNNER_TRACE="$trace" RUNNER_FAIL_MODULE=test-sanitized-image-boot-layout-sanitization.sh "$real_bash" "$root/tools/pi-image/test-sanitized-image-boot-layout.sh"; then
    return 1
  else
    status=$?
  fi
  [[ "$status" == 23 ]]
  [[ "$(cat "$trace")" == $'test-sanitized-image-boot-layout-layout.sh\ntest-sanitized-image-boot-layout-sanitization.sh' ]]
}

check_raspberry_usb_gadget_runner
check_orange_usb_gadget_runner
check_inspector_runner
check_boot_layout_runner
printf '%s\n' 'Domain test runner ordering and failure propagation passed.'
