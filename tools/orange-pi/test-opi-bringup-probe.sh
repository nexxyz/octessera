#!/usr/bin/env bash

set -u

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/octessera-opi-probe.XXXXXX")
FIXTURE="$TEST_ROOT/fixture"
FAKE_BIN="$TEST_ROOT/bin"
OLD_PATH=$PATH

cleanup() {
  PATH=$OLD_PATH
  rm -rf "$TEST_ROOT"
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_equal() {
  local actual="$1"
  local expected="$2"
  local label="$3"

  [ "$actual" = "$expected" ] || fail "$label: expected [$expected], got [$actual]"
}

assert_output_contains() {
  local needle="$1"
  local file_path="$2"

  grep -F -- "$needle" "$file_path" >/dev/null || fail "missing [$needle] in $file_path"
}

write_executable() {
  local name="$1"
  shift
  printf '%s\n' "$@" > "$FAKE_BIN/$name"
  chmod +x "$FAKE_BIN/$name"
}

mkdir -p "$FAKE_BIN"

make_fixture() {
  rm -rf "$FIXTURE"
  mkdir -p \
    "$FIXTURE/etc" \
    "$FIXTURE/etc/octessera" \
    "$FIXTURE/proc/device-tree" \
    "$FIXTURE/boot/overlay-user" \
    "$FIXTURE/dev" \
    "$FIXTURE/sys/class/udc/fake-udc" \
    "$FIXTURE/sys/kernel/config/usb_gadget/gadget" \
    "$FIXTURE/sys/bus/i2c/devices/i2c-2" \
    "$FIXTURE/sys/bus/spi/devices/spi1.0" \
    "$FIXTURE/sys/controllers/i2c@5002400" \
    "$FIXTURE/sys/controllers/spi@5011000/spidev@0"
  printf 'ID=armbian\nVERSION_CODENAME=trixie\n' > "$FIXTURE/etc/os-release"
  printf 'BOARD=orangepizero2w\n' > "$FIXTURE/etc/armbian-release"
  printf 'OCTESSERA_IMAGE_KIND=armbian\nOCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\nOCTESSERA_IMAGE_BUILT_AT=2026-07-25T00:00:00Z\n' > "$FIXTURE/etc/octessera/build-metadata.env"
  printf 'OrangePi Zero 2W\0' > "$FIXTURE/proc/device-tree/model"
  printf 'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
  : > "$FIXTURE/boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo"
  : > "$FIXTURE/dev/i2c-2"
  : > "$FIXTURE/dev/spidev1.0"
  : > "$FIXTURE/dev/gpiochip7"
  : > "$FIXTURE/sys/class/udc/fake-udc/function"
  : > "$FIXTURE/sys/kernel/config/usb_gadget/gadget/UDC"
  ln -s "$FIXTURE/sys/controllers/i2c@5002400" "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"
  ln -s "$FIXTURE/sys/controllers/spi@5011000/spidev@0" "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
}

write_executable gpiodetect '#!/usr/bin/env bash' \
  "case \"\${FIXTURE_GPIODETECT_MODE:-}\" in" \
  '  wrong-label) printf "gpiochip1 [7022000.pinctrl] (32 lines)\\n" ;;' \
  '  duplicate) printf "gpiochip1 [300b000.pinctrl] (288 lines)\\ngpiochip7 [300b000.pinctrl] (288 lines)\\n" ;;' \
  '  *) printf "gpiochip7 [300b000.pinctrl] (288 lines)\\n" ;;' \
  'esac'
write_executable gpioinfo '#!/usr/bin/env bash' \
  "if [ \"\$#\" -ne 2 ] || [ \"\$1\" != \"-c\" ] || [ \"\$2\" != \"gpiochip7\" ]; then" \
  '  printf "unexpected gpioinfo args: %s\\n" "$*" >&2' \
  '  exit 64' \
  'fi' \
  "case \"\${FIXTURE_GPIOINFO_MODE:-}\" in" \
  "  consumer) if [ \"\${FIXTURE_GPIOINFO_CONSUMER_OFFSET:-76}\" = 270 ]; then printf \"gpiochip7 - 288 lines:\\\\n line  76: unnamed input\\\\n line 270: unnamed input consumer=octessera\\\\n\"; else printf \"gpiochip7 - 288 lines:\\\\n line  76: unnamed input consumer=octessera\\\\n line 270: unnamed input\\\\n\"; fi ;;" \
  '  missing) printf "gpiochip7 - 288 lines:\\n line 270: unnamed input\\n" ;;' \
  '  duplicate) printf "gpiochip7 - 288 lines:\\n line  76: unnamed input\\n line  76: unnamed input\\n line 270: unnamed input\\n" ;;' \
  '  wrong-offset) printf "gpiochip7 - 288 lines:\\n line  75: unnamed input\\n line 270: unnamed input\\n" ;;' \
  '  named) printf "gpiochip7 - 288 lines:\\n line  76: RESET input\\n line 270: unnamed input\\n" ;;' \
  '  output) printf "gpiochip7 - 288 lines:\\n line  76: unnamed output\\n line 270: unnamed input\\n" ;;' \
  '  legacy-unused) printf "gpiochip7 - 288 lines:\\n line  76: unused input\\n line 270: unnamed input\\n" ;;' \
  '  trailing) printf "gpiochip7 - 288 lines:\\n line  76: unnamed input active-high\\n line 270: unnamed input\\n" ;;' \
  '  *) printf "gpiochip7 - 288 lines:\\n line  76: unnamed input\\n line 270: unnamed input\\n" ;;' \
  'esac'
write_executable i2cdetect '#!/usr/bin/env bash' 'exit 0'
write_executable aplay '#!/usr/bin/env bash' 'exit 0'
write_executable sudo '#!/usr/bin/env bash' \
  "if [ -n \"\${FIXTURE_SUDO_ARGS:-}\" ]; then printf \"%s\\\\n\" \"\$@\" >> \"\$FIXTURE_SUDO_ARGS\"; fi" \
  "if [ \"\$1\" != \"-n\" ]; then exit 97; fi" \
  "if [ \"\${TEST_GPIO_SUDO_MODE:-}\" = detect-failed ] && [ \"\$2\" = \"gpiodetect\" ]; then exit 2; fi" \
  "if [ \"\${TEST_GPIO_SUDO_MODE:-}\" = info-failed ] && [ \"\$2\" = \"gpioinfo\" ]; then exit 2; fi" \
  'shift' \
  'exec "$@"'
write_executable fuser '#!/usr/bin/env bash' \
  "if [ -n \"\${FIXTURE_FUSER_ARGS:-}\" ]; then printf \"%s\\\\n\" \"\$@\" >> \"\$FIXTURE_FUSER_ARGS\"; fi" \
  "case \"\${FIXTURE_FUSER_MODE:-}\" in" \
  '  owner) printf "/dev/spidev1.0: 1234\\n"; exit 0 ;;' \
  '  error) exit 2 ;;' \
  '  *) exit 1 ;;' \
  'esac'
write_executable path-probe '#!/usr/bin/env bash' 'printf "path-probe-ran\\n"'
write_executable pgrep '#!/usr/bin/env bash' \
  "if [ \"\${FIXTURE_PGREP_MODE:-}\" = owner ]; then" \
  '  printf "1234 /usr/bin/octessera-pi\\n"; exit 0' \
  'fi' \
  'exit 1'
write_executable systemctl '#!/usr/bin/env bash' \
  "if [ \"\${FIXTURE_SYSTEMCTL_MODE:-}\" = owner ] && [ \"\${1:-}\" = list-unit-files ]; then" \
  '  printf "octessera.service disabled enabled\\n"; exit 0' \
  'fi' \
  "case \"\${1:-}\" in list-unit-files) exit 0 ;; is-active) exit 3 ;; *) exit 0 ;; esac"

PATH="$FAKE_BIN:$OLD_PATH"
export PATH
export FIXTURE_GPIOINFO_MODE=
export FIXTURE_GPIODETECT_MODE=
export FIXTURE_FUSER_MODE=
export FIXTURE_PGREP_MODE=
export FIXTURE_SYSTEMCTL_MODE=
export TEST_GPIO_SUDO_MODE=
export FIXTURE_GPIOINFO_CONSUMER_OFFSET=

# shellcheck source=tools/orange-pi/opi-bringup-probe.sh
source "$SCRIPT_DIR/opi-bringup-probe.sh"
PATH="$FAKE_BIN:$PATH"
export PATH

sudo_available() {
  case "${TEST_SUDO_MODE:-}" in
    suppressed)
      : > "${SUDO_MARKER:-/dev/null}"
      return 1
      ;;
    unavailable) return 1 ;;
    *) return 0 ;;
  esac
}

privileged_fuser() { fuser "$@"; }

tool_available() {
  if [ "${MISSING_TOOL:-}" = "$1" ]; then
    return 1
  fi
  command -v "$1" >/dev/null 2>&1
}

run_optional "non-login PATH" "path-probe" > "$TEST_ROOT/non-login-path.log" 2>&1
assert_output_contains 'path-probe-ran' "$TEST_ROOT/non-login-path.log"
run_optional "genuine missing tool" "octessera-genuine-missing-tool" > "$TEST_ROOT/genuine-missing-tool.log" 2>&1
assert_output_contains 'missing command: octessera-genuine-missing-tool' "$TEST_ROOT/genuine-missing-tool.log"
grep -F 'export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"' "$SCRIPT_DIR/opi-bringup-probe.sh" >/dev/null || fail 'probe did not set the trusted PATH'
grep -F "bash -c \"\$command\"" "$SCRIPT_DIR/opi-bringup-probe.sh" >/dev/null || fail 'probe did not use non-login bash for inventory'
! grep -F "bash -lc \"\$command\"" "$SCRIPT_DIR/opi-bringup-probe.sh" >/dev/null || fail 'probe still uses login bash for inventory'

run_fixture_validation() {
  local name="$1"
  local output_path="$TEST_ROOT/$name.log"

  FAILURES=0
  BASELINE_FAILURES=0
  validate_baseline_paths \
    "$FIXTURE/etc/armbian-release" \
    "$FIXTURE/etc/os-release" \
    "$FIXTURE/etc/octessera/build-metadata.env" \
    "$FIXTURE/proc/device-tree/model" \
    aarch64 \
    "$FIXTURE/boot/armbianEnv.txt" \
    "$FIXTURE/boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo" \
    "$FIXTURE/dev/i2c-2" \
    "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node" \
    "$FIXTURE/dev/spidev1.0" \
    "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node" \
    "$FIXTURE/dev" \
    "$FIXTURE/sys/class/udc" \
    "$FIXTURE/sys/kernel/config/usb_gadget" > "$output_path" 2>&1
  LAST_STATUS=$?
  LAST_OUTPUT=$output_path
}

assert_validation_failed_for() {
  local name="$1"
  local needle="$2"

  run_fixture_validation "$name"
  [ "$LAST_STATUS" -ne 0 ] || fail "$name unexpectedly passed"
  assert_output_contains "$needle" "$LAST_OUTPUT"
}

make_fixture
gpiodetect >/dev/null
gpioinfo -c gpiochip7 >/dev/null
FIXTURE_SUDO_ARGS="$TEST_ROOT/sudo-args.log"
export FIXTURE_SUDO_ARGS
: > "$FIXTURE_SUDO_ARGS"
run_fixture_validation passing
assert_equal "$LAST_STATUS" 0 "passing baseline status"
assert_equal "$FAILURES" 0 "passing baseline failures"
assert_output_contains '-n' "$FIXTURE_SUDO_ARGS"
assert_output_contains 'gpiodetect' "$FIXTURE_SUDO_ARGS"
assert_output_contains 'gpioinfo' "$FIXTURE_SUDO_ARGS"
assert_output_contains 'gpiochip7' "$FIXTURE_SUDO_ARGS"

printf 'OrangePi Zero 2W' > "$FIXTURE/proc/device-tree/model"
assert_validation_failed_for model-without-nul "device-tree model"
printf 'OrangePi Zero 2W\0\0' > "$FIXTURE/proc/device-tree/model"
assert_validation_failed_for model-with-extra-nul "device-tree model"
printf 'Orange Pi Zero 2W\0' > "$FIXTURE/proc/device-tree/model"
assert_validation_failed_for inserted-model-space "device-tree model"
printf 'OrangePi Zero2W\0' > "$FIXTURE/proc/device-tree/model"
assert_validation_failed_for missing-model-space "device-tree model"
printf 'OrangePi\0 Zero 2W\0' > "$FIXTURE/proc/device-tree/model"
assert_validation_failed_for embedded-model-nul "device-tree model"
printf 'OrangePi Zero 2W\0' > "$FIXTURE/proc/device-tree/model"

printf 'ID=debian\nVERSION_CODENAME=trixie\n' > "$FIXTURE/etc/os-release"
run_fixture_validation os-release-id-ignored
assert_equal "$LAST_STATUS" 0 "os-release ID is not the Armbian proof"
printf 'ID=armbian\nVERSION_CODENAME=trixie\n' > "$FIXTURE/etc/os-release"

printf 'VERSION_CODENAME=trixie\n' > "$FIXTURE/etc/armbian-release"
assert_validation_failed_for missing-armbian-board "Armbian board identity"
printf 'BOARD=wrongboard\n' > "$FIXTURE/etc/armbian-release"
assert_validation_failed_for wrong-armbian-board "Armbian board identity"
printf 'BOARD=orangepizero2w\nBOARD=orangepizero2w\n' > "$FIXTURE/etc/armbian-release"
assert_validation_failed_for duplicate-armbian-board "duplicate BOARD assignments"
printf 'BOARD=orangepizero2w\n' > "$FIXTURE/etc/armbian-release"

printf 'ID=armbian\n' > "$FIXTURE/etc/os-release"
assert_validation_failed_for missing-trixie-codename "Armbian OS codename"
printf 'ID=armbian\nVERSION_CODENAME=bookworm\n' > "$FIXTURE/etc/os-release"
assert_validation_failed_for wrong-trixie-codename "Armbian OS codename"
printf 'ID=armbian\nVERSION_CODENAME=trixie\nVERSION_CODENAME=trixie\n' > "$FIXTURE/etc/os-release"
assert_validation_failed_for duplicate-trixie-codename "duplicate VERSION_CODENAME assignments"
printf 'ID=armbian\nVERSION_CODENAME=trixie\n' > "$FIXTURE/etc/os-release"

rm "$FIXTURE/etc/octessera/build-metadata.env"
assert_validation_failed_for missing-artifact-metadata "Orange artifact metadata evidence is missing"
[ ! -e "$FIXTURE/etc/octessera/build-metadata.env" ] || fail 'missing metadata fixture unexpectedly reappeared'
printf 'OCTESSERA_IMAGE_KIND=armbian\nOCTESSERA_BOARD_PROFILE_ID=wrong-profile\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\n' > "$FIXTURE/etc/octessera/build-metadata.env"
assert_validation_failed_for wrong-artifact-profile "requires OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w"
printf 'OCTESSERA_IMAGE_KIND=armbian\nOCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=true\n' > "$FIXTURE/etc/octessera/build-metadata.env"
assert_validation_failed_for runtime-enabled "requires OCTESSERA_RUNTIME_ENABLED_DEFAULT=false"
printf 'OCTESSERA_IMAGE_KIND=armbian\nOCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\n' > "$FIXTURE/etc/octessera/build-metadata.env"

printf 'wrong model\0' > "$FIXTURE/proc/device-tree/model"
assert_validation_failed_for identity "device-tree model"
printf 'OrangePi Zero 2W\0' > "$FIXTURE/proc/device-tree/model"

printf 'overlays=\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
assert_validation_failed_for missing-i2c-overlay "overlays=i2c1-pi"
printf 'overlays=i2c1-pi\noverlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
assert_validation_failed_for duplicate-i2c-assignment "duplicate overlays assignment"
printf 'overlays =i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
assert_validation_failed_for malformed-i2c-assignment "malformed overlays assignment"
printf '# overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
assert_validation_failed_for commented-i2c-assignment "commented overlays assignment"
printf 'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
assert_validation_failed_for duplicate-user-assignment "duplicate user_overlays assignment"
printf 'overlays=i2c1-pi spidev1_0\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"
assert_validation_failed_for stock-spidev-overlay "stock spidev1_0 overlay"
printf 'overlays=i2c1-pi\nuser_overlays=octessera-h618-spi1-oled-sd2\n' > "$FIXTURE/boot/armbianEnv.txt"

rm "$FIXTURE/dev/i2c-2"
assert_validation_failed_for missing-i2c-node "expected I2C device node"
: > "$FIXTURE/dev/i2c-2"
rm "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"
assert_validation_failed_for missing-i2c-controller "expected I2C controller"
ln -s "$FIXTURE/sys/controllers/i2c@5002400" "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"
rm "$FIXTURE/dev/spidev1.0"
assert_validation_failed_for missing-spi-node "expected SPI device node"
: > "$FIXTURE/dev/spidev1.0"
rm "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
assert_validation_failed_for missing-spi-controller "expected SPI controller"
mkdir -p "$FIXTURE/sys/controllers/spi@5011001"
mkdir -p "$FIXTURE/sys/controllers/spi@5011001/spidev@0"
ln -s "$FIXTURE/sys/controllers/spi@5011001/spidev@0" "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
assert_validation_failed_for wrong-spi-controller "does not resolve to /spi@5011000/spidev@0"
rm "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
ln -s "$FIXTURE/sys/controllers/spi@5011000" "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
assert_validation_failed_for missing-spi-child "does not resolve to /spi@5011000/spidev@0"
rm "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
mkdir -p "$FIXTURE/sys/controllers/spi@5011000/spidev@1"
ln -s "$FIXTURE/sys/controllers/spi@5011000/spidev@1" "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
assert_validation_failed_for wrong-spi-child "does not resolve to /spi@5011000/spidev@0"
rm "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"
ln -s "$FIXTURE/sys/controllers/spi@5011000/spidev@0" "$FIXTURE/sys/bus/spi/devices/spi1.0/of_node"

rm "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"
mkdir -p "$FIXTURE/sys/controllers/i2c@50024000"
ln -s "$FIXTURE/sys/controllers/i2c@50024000" "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"
assert_validation_failed_for wrong-i2c-suffix "does not resolve to /i2c@5002400"
rm "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"
ln -s "$FIXTURE/sys/controllers/i2c@5002400" "$FIXTURE/sys/bus/i2c/devices/i2c-2/of_node"

FIXTURE_GPIODETECT_MODE=wrong-label
assert_validation_failed_for wrong-gpio-controller-label "canonical GPIO controller label is missing or not unique"
FIXTURE_FUSER_ARGS="$TEST_ROOT/fuser-args.log"
export FIXTURE_FUSER_ARGS
: > "$FIXTURE_FUSER_ARGS"
assert_validation_failed_for unresolved-gpio-ownership "GPIO ownership is unproven: canonical GPIO device node is unresolved"
! grep -F "$FIXTURE/dev/unresolved" "$FIXTURE_FUSER_ARGS" >/dev/null || fail 'fuser was given unresolved GPIO path'
unset FIXTURE_FUSER_ARGS
FIXTURE_GPIODETECT_MODE=duplicate
assert_validation_failed_for duplicate-gpio-controller-label "canonical GPIO controller label is missing or not unique"
FIXTURE_GPIODETECT_MODE=
rm "$FIXTURE/dev/gpiochip7"
assert_validation_failed_for missing-gpiochip "canonical GPIO device node"
: > "$FIXTURE/dev/gpiochip7"
FIXTURE_GPIOINFO_MODE=consumer
FIXTURE_GPIOINFO_CONSUMER_OFFSET=76
assert_validation_failed_for consumer-reset-gpio "GPIO offset 76 is owned"
FIXTURE_GPIOINFO_CONSUMER_OFFSET=270
assert_validation_failed_for consumer-dc-gpio "GPIO offset 270 is owned"
unset FIXTURE_GPIOINFO_CONSUMER_OFFSET
FIXTURE_GPIOINFO_MODE=missing
assert_validation_failed_for missing-gpio-offset "GPIO offset 76 is missing"
FIXTURE_GPIOINFO_MODE=duplicate
assert_validation_failed_for duplicate-gpio-offset "GPIO offset 76 occurs 2 times"
FIXTURE_GPIOINFO_MODE=wrong-offset
assert_validation_failed_for wrong-gpio-offset "GPIO offset 76 is missing"
FIXTURE_GPIOINFO_MODE=named
assert_validation_failed_for named-gpio "GPIO offset 76 ownership is unknown"
FIXTURE_GPIOINFO_MODE=output
assert_validation_failed_for output-gpio "GPIO offset 76 ownership is unknown"
FIXTURE_GPIOINFO_MODE=legacy-unused
assert_validation_failed_for legacy-unused-gpio "GPIO offset 76 ownership is unknown"
FIXTURE_GPIOINFO_MODE=trailing
assert_validation_failed_for trailing-gpio "GPIO offset 76 ownership is unknown"
FIXTURE_GPIOINFO_MODE=

TEST_GPIO_SUDO_MODE=detect-failed
assert_validation_failed_for gpio-sudo-detect-failure "gpiodetect failed with exit 2"
TEST_GPIO_SUDO_MODE=info-failed
assert_validation_failed_for gpio-sudo-info-failure "gpioinfo failed for gpiochip7"
unset TEST_GPIO_SUDO_MODE
TEST_SUDO_MODE=unavailable
assert_validation_failed_for gpio-sudo-unavailable "GPIO evidence is unproven: passwordless sudo is unavailable"
unset TEST_SUDO_MODE

MISSING_TOOL=i2cdetect
assert_validation_failed_for missing-evidence-tool "required evidence tool is missing: i2cdetect"
unset MISSING_TOOL

rm -rf "$FIXTURE/sys/class/udc/fake-udc"
assert_validation_failed_for empty-udc "UDC class is empty"
mkdir -p "$FIXTURE/sys/class/udc/fake-udc"
: > "$FIXTURE/sys/class/udc/fake-udc/function"
printf 'gadget-name\n' > "$FIXTURE/sys/class/udc/fake-udc/function"
assert_validation_failed_for bound-udc "UDC is already bound"
: > "$FIXTURE/sys/class/udc/fake-udc/function"
printf 'fake-udc\n' > "$FIXTURE/sys/kernel/config/usb_gadget/gadget/UDC"
assert_validation_failed_for configfs-bound-udc "configfs gadget is bound"
: > "$FIXTURE/sys/kernel/config/usb_gadget/gadget/UDC"

FIXTURE_FUSER_MODE=owner
assert_validation_failed_for owned-device "a process owns a target device"
FIXTURE_FUSER_MODE=
TEST_SUDO_MODE=unavailable
assert_validation_failed_for unproven-device-owner "target-device owner absence is unproven"
TEST_SUDO_MODE=
FIXTURE_FUSER_MODE=error
assert_validation_failed_for owner-check-error "privileged read-only check failed"
FIXTURE_FUSER_MODE=
FIXTURE_PGREP_MODE=owner
assert_validation_failed_for owned-process "Octessera runtime process is present"
FIXTURE_PGREP_MODE=
FIXTURE_SYSTEMCTL_MODE=owner
assert_validation_failed_for owned-service "Octessera runtime service is installed"
FIXTURE_SYSTEMCTL_MODE=

printf 'wrong model\0' > "$FIXTURE/proc/device-tree/model"
printf 'overlays=\nuser_overlays=\n' > "$FIXTURE/boot/armbianEnv.txt"
rm -rf "$FIXTURE/sys/class/udc/fake-udc"
FIXTURE_GPIOINFO_MODE=consumer
MISSING_TOOL=aplay
run_fixture_validation aggregate
[ "$LAST_STATUS" -ne 0 ] || fail 'aggregate failure fixture unexpectedly passed'
aggregate_count=$(grep -c '^FAIL:' "$LAST_OUTPUT")
[ "$aggregate_count" -ge 5 ] || fail "expected aggregated failures, got $aggregate_count"
assert_output_contains 'PASSIVE_BASELINE_FAILURES=' "$LAST_OUTPUT"
FIXTURE_GPIOINFO_MODE=
unset MISSING_TOOL
make_fixture

rm "$FIXTURE/etc/octessera/build-metadata.env"
run_fixture_validation live-missing-metadata
assert_equal "$LAST_STATUS" 1 "live fixture validation status"
live_failures="$(grep -c '^FAIL:' "$LAST_OUTPUT")"
assert_equal "$live_failures" 1 "live fixture failure count"
assert_output_contains 'Orange artifact metadata evidence is missing' "$LAST_OUTPUT"
make_fixture

WITH_SUDO_CHECKS=1
: "$WITH_SUDO_CHECKS"
BASELINE_FAILURES=1
SUDO_MARKER="$TEST_ROOT/sudo-attempted"
run_sudo() {
  if [ "${TEST_SUDO_MODE:-}" = failed ]; then
    record_failure 'simulated sudo command failure'
  fi
  return 0
}
TEST_SUDO_MODE=suppressed
run_optional_sudo_checks > "$TEST_ROOT/sudo-suppressed.log" 2>&1
[ ! -e "$SUDO_MARKER" ] || fail 'sudo availability was checked after baseline failure'
assert_output_contains 'suppressed because the passive baseline failed' "$TEST_ROOT/sudo-suppressed.log"

FAILURES=0
BASELINE_FAILURES=0
TEST_SUDO_MODE=unavailable
run_optional_sudo_checks > "$TEST_ROOT/sudo-unavailable.log" 2>&1
[ "$FAILURES" -eq 1 ] || fail 'unavailable requested sudo did not fail'
assert_output_contains 'passwordless sudo is unavailable' "$TEST_ROOT/sudo-unavailable.log"

FAILURES=0
TEST_SUDO_MODE=failed
run_optional_sudo_checks > "$TEST_ROOT/sudo-failed.log" 2>&1
[ "$FAILURES" -eq 2 ] || fail 'failed requested sudo checks did not aggregate both failures'

MAIN_LOG_DIR="$TEST_ROOT/main-log"
SUDO_MAIN_MARKER="$TEST_ROOT/main-sudo-attempted"
validate_baseline() {
  record_failure 'fixture baseline failure one'
  record_failure 'fixture baseline failure two'
  BASELINE_FAILURES=2
  : "$BASELINE_FAILURES"
  return 1
}
SUDO_MARKER="$SUDO_MAIN_MARKER"
TEST_SUDO_MODE=suppressed
if (main --output-dir "$MAIN_LOG_DIR" --with-sudo-checks > "$TEST_ROOT/main-stdout.log" 2>&1); then
  fail 'failing probe main unexpectedly passed'
fi
[ ! -e "$SUDO_MAIN_MARKER" ] || fail 'main attempted sudo after baseline failure'
latest_log=$(cat "$MAIN_LOG_DIR/latest-log-path")
[ -f "$latest_log" ] || fail 'main did not retain its complete log'
assert_output_contains 'fixture baseline failure one' "$latest_log"
assert_output_contains 'fixture baseline failure two' "$latest_log"
assert_output_contains 'sudo checks' "$latest_log"
assert_output_contains '== done ==' "$latest_log"

printf 'Orange Pi passive probe fixture tests passed\n'
