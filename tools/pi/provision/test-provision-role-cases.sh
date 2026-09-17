#!/usr/bin/env bash

for role_case in duplicate malformed; do
  new_fixture
  cp "$ROOT/tools/pi-image/fixtures/trusted-parent-v0.7.5/boot/config.txt" "$FIXTURE/boot/firmware/config.txt"
  if [ "$role_case" = duplicate ]; then
    printf '%s\n' 'dtoverlay=dwc2,dr_mode=host' >> "$FIXTURE/boot/firmware/config.txt"
    role_error='managed .all. contains duplicate'
  else
    sed -i 's/^dtoverlay=dwc2,dr_mode=peripheral$/ dtoverlay=dwc2,dr_mode=peripheral/' "$FIXTURE/boot/firmware/config.txt"
    role_error='malformed or competing'
  fi
  run_provision default
  expect_rc "sc-role-$role_case" 1
  expect_err_match "sc-role-$role_case" "$role_error"
  pass "managed [all] $role_case role is rejected without deduplication"
done

assert_stock_non_all() {
  python3 - "$1" "$2" <<'PY'
from pathlib import Path
import sys

def outside_all(path):
    section = None
    result = []
    for line in Path(path).read_bytes().splitlines(keepends=True):
        stripped = line.strip()
        if stripped.startswith(b"[") and stripped.endswith(b"]"):
            section = stripped
        if stripped != b"[all]" and section != b"[all]":
            result.append(line)
    return result

if outside_all(sys.argv[1]) != outside_all(sys.argv[2]):
    raise SystemExit("stock non-[all] config bytes changed")
PY
}

new_fixture
stock_config="$ROOT/tools/pi-image/fixtures/trusted-parent-v0.7.5/boot/config.txt"
cp "$stock_config" "$FIXTURE/boot/firmware/config.txt"
cp "$stock_config" "$TMP/stock-config-before"
run_provision default
expect_rc "sc-stock-gadget" 75
assert_stock_non_all "$TMP/stock-config-before" "$FIXTURE/boot/firmware/config.txt"
test "$(grep -Ec '^dtoverlay=dwc2,dr_mode=host$' "$FIXTURE/boot/firmware/config.txt")" = 1
test "$(grep -Ec '^dtoverlay=dwc2,dr_mode=peripheral$' "$FIXTURE/boot/firmware/config.txt")" = 1
cp "$FIXTURE/boot/firmware/config.txt" "$TMP/stock-gadget-config"
run_provision default
expect_rc "sc-stock-gadget-repeat" 0
cmp -s "$TMP/stock-gadget-config" "$FIXTURE/boot/firmware/config.txt"
mkdir -p "$FIXTURE/home/pi/presets"
printf '%s\n' '{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false},"usb":{"dataRole":"host"}}}' > "$FIXTURE/home/pi/presets/default.json"
run_provision default
expect_rc "sc-stock-host" 75
assert_stock_non_all "$TMP/stock-config-before" "$FIXTURE/boot/firmware/config.txt"
test "$(grep -Ec '^dtoverlay=dwc2,dr_mode=host$' "$FIXTURE/boot/firmware/config.txt")" = 2
cp "$FIXTURE/boot/firmware/config.txt" "$TMP/stock-host-config"
run_provision default
expect_rc "sc-stock-host-repeat" 0
cmp -s "$TMP/stock-host-config" "$FIXTURE/boot/firmware/config.txt"
pass "exact stock config preserves [cm5] across gadget and host idempotence"
