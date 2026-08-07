#!/usr/bin/env bash
# Fixture tests for tools/quality/repository-integrity.sh.
# Proves each of the four check classes fails independently on drift.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE="$HERE/repository-integrity.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

passes=0
failures=0

report() {
  if [ "$1" -eq 0 ]; then
    echo "PASS: $2"
    passes=$((passes + 1))
  else
    echo "FAIL: $2"
    failures=$((failures + 1))
  fi
}

make_fixture() {
  local fixture="$1"
  mkdir -p "$fixture/config/defaults" \
    "$fixture/config/generated/desktop" \
    "$fixture/config/generated/pi" \
    "$fixture/resources" \
    "$fixture/packages/device-contracts/src" \
    "$fixture/apps/desktop/src/ui" \
    "$fixture/userdocs/print" \
    "$fixture/licenses/cargo" \
    "$fixture/licenses/pnpm" \
    "$fixture/resources/legal" \
    "$fixture/docs" \
    "$fixture/src" \
    "$fixture/tools/pi-image/stage4-octessera/files/root/usr/local/sbin" \
    "$fixture/tools/pi-image/stage4-octessera/files/root/etc/systemd/system" \
    "$fixture/tools/pi-image/stage4-octessera/files/root/etc/udev/rules.d"

  printf '{\n  "name": "octessera-fixture"\n}\n' >"$fixture/config/defaults/base.json"
  printf '{\n}\n' >"$fixture/config/defaults/desktop.json"
  printf '{\n}\n' >"$fixture/config/defaults/pi.json"

  node "$HERE/../config/generate-default-configs.mjs" --root "$fixture"

  printf '{\n  "gridWidth": 16,\n  "gridHeight": 16,\n  "layerCount": 2,\n  "instrumentCount": 8,\n  "sampleSlotCount": 4,\n  "audioSampleRate": 48000,\n  "audioBlockFrames": 128,\n  "synthSlotWorkers": 2,\n  "maxSynthVoices": 64,\n  "maxSampleVoices": 64,\n  "maxSynthVoicesPerSlot": 32,\n  "maxSampleVoicesPerSlot": 32,\n  "busFxWarningSlotCount": 4,\n  "busCount": 4,\n  "globalFxSlotCount": 2,\n  "auxEncoderCount": 2,\n  "sparksFxMaxConcurrent": 16,\n  "scanSectionCounts": [16, 8, 4, 2, 1],\n  "panPositionCount": 12,\n  "oledWidth": 128,\n  "oledHeight": 64\n}\n' >"$fixture/resources/platform-capabilities.json"
  node "$HERE/../resources/generate-platform-capabilities.mjs" --root "$fixture"

  printf '{\n  "green": "#00ff00",\n  "red": "#ff0000",\n  "blue": "#0000ff",\n  "yellow": "#ffff00",\n  "gray": "#808080",\n  "white": "#ffffff",\n  "black": "#000000"\n}\n' >"$fixture/resources/display-palette.json"
  node "$HERE/../resources/generate-display-palette.mjs" --root "$fixture"

  printf 'lockfileVersion: "9.0"\n' >"$fixture/pnpm-lock.yaml"
  printf '[workspace]\n' >"$fixture/Cargo.lock"

  local cargo_lock_sha
  cargo_lock_sha="$(sha256sum "$fixture/Cargo.lock" | cut -d' ' -f1)"
  printf '{\n  "cargo_lock_sha256": "%s"\n}\n' "$cargo_lock_sha" >"$fixture/licenses/cargo/inventory.json"

  local pnpm_lock_sha
  pnpm_lock_sha="$(sha256sum "$fixture/pnpm-lock.yaml" | cut -d' ' -f1)"
  printf '{\n  "pnpm_lock_sha256": "%s"\n}\n' "$pnpm_lock_sha" >"$fixture/licenses/pnpm/inventory.json"

  local cargo_inventory_sha pnpm_inventory_sha
  cargo_inventory_sha="$(sha256sum "$fixture/licenses/cargo/inventory.json" | cut -d' ' -f1)"
  pnpm_inventory_sha="$(sha256sum "$fixture/licenses/pnpm/inventory.json" | cut -d' ' -f1)"
  printf '%s  licenses/cargo/inventory.json\n' "$cargo_inventory_sha" >"$fixture/licenses/cargo/SHA256SUMS"
  printf '%s  licenses/pnpm/inventory.json\n' "$pnpm_inventory_sha" >"$fixture/licenses/pnpm/SHA256SUMS"

  local cargo_inventory_size pnpm_inventory_size
  cargo_inventory_size="$(wc -c <"$fixture/licenses/cargo/inventory.json")"
  pnpm_inventory_size="$(wc -c <"$fixture/licenses/pnpm/inventory.json")"
  printf '{\n  "schema": "octessera.legal-notice-bundle/v1",\n  "schema_version": 1,\n  "destination_root": "/usr/share/doc/octessera",\n  "files": [\n    {"source": "licenses/cargo/inventory.json", "destination": "licenses/cargo/inventory.json", "sha256": "%s", "size": %s},\n    {"source": "licenses/pnpm/inventory.json", "destination": "licenses/pnpm/inventory.json", "sha256": "%s", "size": %s}\n  ]\n}\n' "$cargo_inventory_sha" "$cargo_inventory_size" "$pnpm_inventory_sha" "$pnpm_inventory_size" >"$fixture/resources/legal/notice-bundle.json"

  printf '# Fixture docs\n\n## Section\n\nSee [other](other.md) and [section](#section).\n' >"$fixture/docs/index.md"
  printf '# Other doc\n' >"$fixture/docs/other.md"

  : >"$fixture/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-sd-card"
  : >"$fixture/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-sd-card.service"
  : >"$fixture/tools/pi-image/stage4-octessera/files/root/etc/udev/rules.d/99-octessera-sd-card.rules"
  printf 'fn main() {}\n' >"$fixture/src/main.rs"
}

assert_gate() {
  local fixture="$1" expect_exit="$2" label="$3" expected_class="${4:-}"
  set +e
  output="$("$GATE" --root "$fixture" 2>&1)"
  status=$?
  set -e
  if [ "$status" -eq "$expect_exit" ]; then
    if [ -n "$expected_class" ]; then
      if printf '%s' "$output" | grep -q "\[FAIL\] $expected_class"; then
        if [ "$expect_exit" -eq 1 ]; then
          local other_ok=1
          for cls in stale-assets legal links quality; do
            if [ "$cls" != "$expected_class" ] && printf '%s' "$output" | grep -q "\[FAIL\] $cls"; then
              other_ok=0
            fi
          done
          if [ "$other_ok" -eq 1 ]; then
            report 0 "$label"
          else
            echo "  unexpected additional class failed for $label:"
            printf '%s\n' "$output" | sed 's/^/    /'
            report 1 "$label"
          fi
        else
          report 0 "$label"
        fi
      else
        echo "  expected FAIL $expected_class missing for $label:"
        printf '%s\n' "$output" | sed 's/^/    /'
        report 1 "$label"
      fi
    else
      report 0 "$label"
    fi
  else
    echo "  expected exit $expect_exit got $status for $label:"
    printf '%s\n' "$output" | sed 's/^/    /'
    report 1 "$label"
  fi
}

run_case() {
  local fixture="$TMP/fixture-$1"
  make_fixture "$fixture"
  assert_gate "$fixture" 0 "base fixture passes ($1)"
}

# Class 1: stale-assets drift (stale generated config output).
run_case stale-assets
sed -i 's/"name": "octessera-fixture"/"name": "drifted"/' "$TMP/fixture-stale-assets/config/generated/pi/default.json"

# Class 2: legal drift (stale pnpm lock hash).
run_case legal
sed -i 's/"pnpm_lock_sha256": "\([a-f0-9]\{64\}\)"/"pnpm_lock_sha256": "1111111111111111111111111111111111111111111111111111111111111111"/' "$TMP/fixture-legal/licenses/pnpm/inventory.json"

# Class 3: links drift (broken internal link).
run_case links
printf '\n[broken](missing.md)\n' >>"$TMP/fixture-links/docs/index.md"

# Class 4: quality drift (file over 500 LOC).
run_case quality
: >"$TMP/fixture-quality/src/oversized.rs"
for _ in $(seq 1 510); do
  printf '// filler line to exceed the 500 LOC file limit\n' >>"$TMP/fixture-quality/src/oversized.rs"
done

# Apply each drift and assert the gate fails with exactly the expected class.
assert_gate "$TMP/fixture-stale-assets" 1 "stale-assets drift detected" stale-assets
assert_gate "$TMP/fixture-legal" 1 "legal drift detected" legal
assert_gate "$TMP/fixture-links" 1 "links drift detected" links
assert_gate "$TMP/fixture-quality" 1 "quality drift detected" quality

echo
echo "repository-integrity fixtures: $passes passed, $failures failed"
[ "$failures" -eq 0 ]
