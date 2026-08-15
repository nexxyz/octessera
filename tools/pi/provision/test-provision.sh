#!/usr/bin/env bash
# Fixture-root tests for tools/pi/provision/provision.sh.
# Runs the real provisioning script against a synthetic target root (SYSROOT)
# with fake host commands injected via PATH. Never touches the real host.
# Subshell exports (SC2030/SC2031) are intentional: each scenario isolates env.
# shellcheck disable=SC2030,SC2031
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PROVISION_SRC="$ROOT/tools/pi/provision/provision.sh"

for tool in python3 curl flock sha256sum unzip install sed tee stat chmod chown rm grep; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "SKIP: missing required host tool: $tool"
    exit 0
  fi
done

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
REAL_PATH="$PATH"
HOST_PATHS_BEFORE="$TMP/host-paths-before"
for host_path in /etc/octessera /etc/systemd/system/octessera.service \
  /usr/local/sbin/octessera-usb-gadget /opt/octessera \
  /usr/local/lib/octessera/rpi_uart_release.py; do
  if [ -e "$host_path" ] || [ -L "$host_path" ]; then
    printf '%s\n' "$host_path" >> "$HOST_PATHS_BEFORE"
  fi
done

PACKAGE="$TMP/package"
mkdir -p "$PACKAGE"
ln -s "$ROOT/tools/pi/provision/files" "$PACKAGE/files"
ln -s "$ROOT/tools/pi-image/stage4-octessera/files/root" "$PACKAGE/root"
ln -s "$ROOT/tools/device-update" "$PACKAGE/device-update"
cp "$PROVISION_SRC" "$PACKAGE/provision.sh"

FAKE_BIN="$TMP/fake-bin"
mkdir -p "$FAKE_BIN"

cat > "$FAKE_BIN/sudo" <<'EOF'
#!/bin/sh
if [ -z "${SYSROOT:-}" ] || [ -z "${PACKAGE:-}" ]; then
  echo "fake sudo requires SYSROOT and PACKAGE" >&2
  exit 99
fi
for arg in "$@"; do
  case "$arg" in
    /dev/null) continue ;;
    /etc/*|/etc|/usr/*|/usr|/boot/*|/boot|/opt/*|/opt|/home/*|/home|/var/*|/var)
      norm=$(readlink -f "$arg" 2>/dev/null || printf '%s' "$arg")
      case "$norm" in
        "$SYSROOT"|"$SYSROOT"/*)
          ;;
        *)
          echo "HOST WRITE ATTEMPT: $arg" | tee -a "$FAKE_STATE/sudo-host-writes.log" >&2
          exit 99
          ;;
      esac
      ;;
  esac
done
exec "$@"
EOF

cat > "$FAKE_BIN/systemctl" <<'EOF'
#!/bin/sh
echo "systemctl $*" >> "$FAKE_STATE/systemctl.log"
case "${1:-}" in
  mask)
    for a in "$@"; do
      case "$a" in
        *.service) echo "$a" >> "$FAKE_STATE/masked" ;;
      esac
    done
    ;;
  is-enabled)
    unit=""
    for a in "$@"; do
      case "$a" in
        *.service) unit="$a" ;;
      esac
    done
    if grep -qxF "$unit" "$FAKE_STATE/masked"; then
      echo masked
    else
      echo disabled
    fi
    ;;
esac
exit 0
EOF

cat > "$FAKE_BIN/getent" <<'EOF'
#!/bin/sh
if [ "${1:-}" = passwd ] && [ "${2:-}" = pi ]; then
  echo "pi:x:$(id -u):$(id -g):,,,:/home/pi:/bin/bash"
  exit 0
fi
exit 2
EOF

cat > "$FAKE_BIN/pinctrl" <<'EOF'
#!/bin/sh
echo "pinctrl $*" >> "$FAKE_STATE/pinctrl.log"
if [ "${FAKE_PINCTRL_OUTPUT:-}" = unsafe ]; then
  echo "GPIO14 = alt3"
  echo "GPIO15 = alt3"
else
  echo "GPIO14 = input"
  echo "GPIO15 = input"
fi
exit 0
EOF

for fake in iw nmcli update-initramfs chown visudo apt-get; do
  cat > "$FAKE_BIN/$fake" <<EOF
#!/bin/sh
echo "$fake \$*" >> "\$FAKE_STATE/$fake.log"
exit 0
EOF
done

chmod +x "$FAKE_BIN"/*

new_fixture() {
  FIXTURE="$TMP/fixture-$RANDOM"
  mkdir -p "$FIXTURE/boot/firmware" "$FIXTURE/home/pi" "$FIXTURE/etc/octessera"
  printf '# fixture boot config\narm_64bit=1\n' > "$FIXTURE/boot/firmware/config.txt"
  printf 'console=serial0,115200 console=tty1 root=/dev/mmcblk0p2 rootfstype=ext4 elevator=deadline fsck.repair=yes rootwait quiet\n' \
    > "$FIXTURE/boot/firmware/cmdline.txt"
  export FAKE_STATE="$TMP/state-$RANDOM"
  mkdir -p "$FAKE_STATE"
}

RC=0
run_provision() {
  local mode="${1:-default}"
  set +e
  (
    export SYSROOT="$FIXTURE"
    export PACKAGE="$PACKAGE"
    export BOARD_PROFILE=raspberry-pi-zero-2w
    export SERVICE=octessera.service
    unset UPDATE_INITRAMFS
    case "$mode" in
      default) ;;
      explicit) export UPDATE_INITRAMFS=1 ;;
      *) echo "unknown fixture provisioning mode: $mode" >&2; exit 2 ;;
    esac
    export WAKE_TRACE=0
    export REMOTE_REPO=/home/pi/octessera-dev
    export PATH="$FAKE_BIN:$REAL_PATH"
    sh "$PACKAGE/provision.sh"
  ) > "$TMP/last.out" 2> "$TMP/last.err"
  RC=$?
  set -e
}

expect_rc() {
  local label="$1" expected="$2"
  if [ "$RC" -ne "$expected" ]; then
    printf 'FAIL[%s]: expected exit %s, got %s\n' "$label" "$expected" "$RC" >&2
    sed 's/^/  err: /' "$TMP/last.err" >&2
    exit 1
  fi
}

expect_err_match() {
  local label="$1" pattern="$2"
  if ! grep -q "$pattern" "$TMP/last.err"; then
    printf 'FAIL[%s]: stderr missing %s\n' "$label" "$pattern" >&2
    exit 1
  fi
}

assert_file() {
  local label="$1" path="$2"
  if [ ! -f "$path" ]; then
    printf 'FAIL[%s]: missing %s\n' "$label" "$path" >&2
    exit 1
  fi
}

assert_mode() {
  local label="$1" path="$2" mode="$3" actual
  actual="$(stat -c '%a' "$path")"
  if [ "$actual" != "$mode" ]; then
    printf 'FAIL[%s]: %s mode %s, expected %s\n' "$label" "$path" "$actual" "$mode" >&2
    exit 1
  fi
}

assert_contains() {
  local label="$1" path="$2" pattern="$3"
  if ! grep -q "$pattern" "$path"; then
    printf 'FAIL[%s]: %s missing %s\n' "$label" "$path" "$pattern" >&2
    exit 1
  fi
}

assert_not_contains() {
  local label="$1" path="$2" pattern="$3"
  if grep -q "$pattern" "$path"; then
    printf 'FAIL[%s]: %s contains %s\n' "$label" "$path" "$pattern" >&2
    exit 1
  fi
}

assert_log_contains() {
  local label="$1" log="$2" pattern="$3"
  if [ ! -f "$FAKE_STATE/$log" ] || ! grep -q -- "$pattern" "$FAKE_STATE/$log"; then
    printf 'FAIL[%s]: fake log %s missing %s\n' "$label" "$log" "$pattern" >&2
    exit 1
  fi
}

PASS_COUNT=0
pass() {
  PASS_COUNT=$((PASS_COUNT + 1))
  printf 'ok - %s\n' "$1"
}

# 1. Invalid service name.
new_fixture
set +e
(
  export SYSROOT="$FIXTURE" PACKAGE="$PACKAGE"
  export SERVICE=other.service
  export PATH="$FAKE_BIN:$REAL_PATH"
  sh "$PACKAGE/provision.sh"
) > "$TMP/last.out" 2> "$TMP/last.err"
RC=$?
set -e
expect_rc "sc1" 2
expect_err_match "sc1" "octessera.service"
pass "invalid service name exits 2"

# 2. Orange profile rejected.
new_fixture
set +e
(
  export SYSROOT="$FIXTURE" PACKAGE="$PACKAGE"
  export BOARD_PROFILE=orange-pi-zero-2w
  export PATH="$FAKE_BIN:$REAL_PATH"
  sh "$PACKAGE/provision.sh"
) > "$TMP/last.out" 2> "$TMP/last.err"
RC=$?
set -e
expect_rc "sc2" 2
expect_err_match "sc2" "Armbian"
pass "orange profile exits 2"

# 3. Unknown profile rejected.
new_fixture
set +e
(
  export SYSROOT="$FIXTURE" PACKAGE="$PACKAGE"
  export BOARD_PROFILE=other
  export PATH="$FAKE_BIN:$REAL_PATH"
  sh "$PACKAGE/provision.sh"
) > "$TMP/last.out" 2> "$TMP/last.err"
RC=$?
set -e
expect_rc "sc3" 2
pass "unknown profile exits 2"

# 4. First run installs payload, cleans legacy initramfs animation inputs,
# and requests reboot (exit 75). Initramfs is not refreshed by default.
new_fixture
mkdir -p "$FIXTURE/etc/initramfs-tools/hooks" "$FIXTURE/etc/initramfs-tools/scripts/init-premount"
printf '%s\n' legacy-hook > "$FIXTURE/etc/initramfs-tools/hooks/octessera-boot-splash"
printf '%s\n' legacy-script > "$FIXTURE/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
run_provision default
expect_rc "sc4" 75
expect_err_match "sc4" "Boot configuration changed"
assert_file "sc4" "$FIXTURE/usr/local/sbin/octessera-usb-gadget"
assert_mode "sc4" "$FIXTURE/usr/local/sbin/octessera-usb-gadget" 755
assert_file "sc4" "$FIXTURE/etc/systemd/system/octessera.service"
assert_mode "sc4" "$FIXTURE/etc/systemd/system/octessera.service" 644
assert_file "sc4" "$FIXTURE/etc/systemd/system/octessera.service.d/audio-realtime.conf"
assert_file "sc4" "$FIXTURE/etc/octessera/board-profile.env"
assert_file "sc4" "$FIXTURE/etc/sudoers.d/octessera-shutdown"
if grep -q $'\r' "$FIXTURE/etc/sudoers.d/octessera-shutdown"; then
  printf 'FAIL[sc4]: extensionless sudoers file contains CR bytes\n' >&2
  exit 1
fi
assert_log_contains "sc4" "visudo.log" "octessera-shutdown"
assert_contains "sc4" "$FIXTURE/etc/octessera/board-profile.env" "OCTESSERA_BOARD_PROFILE_ID=raspberry-pi-zero-2w"
assert_contains "sc4" "$FIXTURE/boot/firmware/config.txt" "^dtoverlay=disable-bt$"
assert_contains "sc4" "$FIXTURE/boot/firmware/config.txt" "^enable_uart=0$"
assert_not_contains "sc4" "$FIXTURE/boot/firmware/cmdline.txt" "serial0"
assert_not_contains "sc4" "$FIXTURE/boot/firmware/cmdline.txt" "ttyAMA0"
assert_not_contains "sc4" "$FIXTURE/boot/firmware/cmdline.txt" "ttyS0"
if [ -e "$FIXTURE/etc/initramfs-tools/hooks/octessera-boot-splash" ] || [ -e "$FIXTURE/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" ]; then
  printf 'FAIL[sc4]: legacy initramfs animation input remains installed\n' >&2
  exit 1
fi
if [ -e "$FAKE_STATE/update-initramfs.log" ]; then
  printf 'FAIL[sc4]: default provisioning invoked update-initramfs\n' >&2
  exit 1
fi
assert_log_contains "sc4" "systemctl.log" "mask --now serial-getty@ttyAMA0.service"
pass "default provisioning removes legacy animation inputs without rebuilding initramfs"

# 5. Idempotent second run completes cleanly (exit 0).
run_provision default
expect_rc "sc5" 0
assert_log_contains "sc5" "systemctl.log" "enable --now octessera-network-health.timer"
assert_log_contains "sc5" "systemctl.log" "enable octessera-oled-shutdown.service"
assert_log_contains "sc5" "systemctl.log" "enable octessera-performance-governor.service"
assert_log_contains "sc5" "systemctl.log" "enable octessera.service"
assert_log_contains "sc5" "systemctl.log" "daemon-reload"
pass "idempotent second run exits 0"

# 6. Explicit initramfs handling installs the current static hook inputs before refreshing the image.
new_fixture
run_provision explicit
expect_rc "sc6" 75
assert_log_contains "sc6" "update-initramfs.log" "-u"
assert_file "sc6" "$FIXTURE/etc/initramfs-tools/modules"
assert_contains "sc6" "$FIXTURE/etc/initramfs-tools/modules" "^spi-bcm2835$"
assert_contains "sc6" "$FIXTURE/etc/initramfs-tools/modules" "^spidev$"
assert_contains "sc6" "$FIXTURE/boot/firmware/config.txt" "^dtparam=spi=on$"
assert_contains "sc6" "$FIXTURE/boot/firmware/config.txt" "^auto_initramfs=1$"
assert_file "sc6" "$FIXTURE/etc/initramfs-tools/hooks/octessera-boot-splash"
assert_file "sc6" "$FIXTURE/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
if ! cmp -s "$FIXTURE/etc/initramfs-tools/hooks/octessera-boot-splash" "$ROOT/tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/hooks/octessera-boot-splash"; then
  printf 'FAIL[sc6]: explicit initramfs rebuild installed a stale hook\n' >&2
  exit 1
fi
if ! cmp -s "$FIXTURE/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$ROOT/tools/pi-image/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"; then
  printf 'FAIL[sc6]: explicit initramfs rebuild installed a stale script\n' >&2
  exit 1
fi
pass "explicit initramfs rebuild installs current static inputs and refreshes the selected image"

# 7. Reboot-required via unsafe GPIO state.
new_fixture
run_provision default
expect_rc "sc7a" 75
FAKE_PINCTRL_OUTPUT=unsafe run_provision default
expect_rc "sc7" 75
expect_err_match "sc7" "GPIO14/15"
pass "unsafe GPIO state exits 75"

# 8. No host writes anywhere.
for host_path in /etc/octessera /etc/systemd/system/octessera.service \
  /usr/local/sbin/octessera-usb-gadget /opt/octessera \
  /usr/local/lib/octessera/rpi_uart_release.py; do
  if { [ -e "$host_path" ] || [ -L "$host_path" ]; } && ! grep -qxF "$host_path" "$HOST_PATHS_BEFORE"; then
    printf 'FAIL[sc8]: host path was written: %s\n' "$host_path" >&2
    exit 1
  fi
done
if grep -q "HOST WRITE ATTEMPT" "$TMP"/state-*/sudo-host-writes.log 2>/dev/null; then
  printf 'FAIL[sc8]: fake sudo detected a host write attempt\n' >&2
  exit 1
fi
pass "no host writes detected"

printf '\nPASS: %d provisioning fixture scenarios\n' "$PASS_COUNT"
