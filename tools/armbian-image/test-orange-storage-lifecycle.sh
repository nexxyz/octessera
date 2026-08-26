#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fixture="$work/fixture"
bin="$work/bin"
mkdir -p "$fixture/config/usb_gadget" "$fixture/udc/musb-hdrc.4.auto" "$fixture/mount" "$bin"
touch "$fixture/udc/musb-hdrc.4.auto/state"
printf '%s\n' configured > "$fixture/udc/musb-hdrc.4.auto/state"

real_mkdir="$(command -v mkdir)"
real_rmdir="$(command -v rmdir)"
real_rm="$(command -v rm)"
real_readlink="$(command -v readlink)"
real_stat="$(command -v stat)"
export REAL_MKDIR="$real_mkdir" REAL_RMDIR="$real_rmdir" REAL_RM="$real_rm" REAL_READLINK="$real_readlink" REAL_STAT="$real_stat"
export TEST_WORK="$fixture" TEST_DEV=/dev/sdb1

cat > "$bin/mkdir" <<'EOF'
#!/bin/sh
"$REAL_MKDIR" "$@"
for argument do
  case "$argument" in
    */mass_storage.usb0)
      "$REAL_MKDIR" -p "$argument/lun.0"
      : > "$argument/lun.0/file"
      : > "$argument/lun.0/forced_eject"
      : > "$argument/lun.0/nofua"
      : > "$argument/lun.0/removable"
      : > "$argument/lun.0/ro"
      : > "$argument/stall"
      ;;
    */octessera-orange-pi)
      if [ "${BIND_FAIL:-0}" = 1 ]; then
        "$REAL_MKDIR" "$argument/UDC"
      fi
      ;;
  esac
done
EOF
cat > "$bin/rmdir" <<'EOF'
#!/bin/sh
target=$1
[ "$target" = -- ] && target=$2
case "$target" in
  */mass_storage.usb0)
    "$REAL_RM" -rf "$target/lun.0"
    "$REAL_RM" -f "$target/stall"
    ;;
  */configs/c.1/strings/0x409)
    "$REAL_RM" -f "$target/configuration"
    ;;
  */configs/c.1)
    "$REAL_RM" -f "$target/MaxPower"
    "$REAL_RM" -rf "$target/strings"
    ;;
  */strings/0x409)
    "$REAL_RM" -f "$target/manufacturer" "$target/product" "$target/serialnumber"
    ;;
  */octessera-orange-pi)
    "$REAL_RM" -rf "$target/configs" "$target/functions" "$target/strings" "$target/UDC"
    "$REAL_RM" -f "$target"/idVendor "$target"/idProduct "$target"/bcdUSB "$target"/bcdDevice
    ;;
esac
"$REAL_RMDIR" "$@"
EOF
cat > "$bin/readlink" <<'EOF'
#!/bin/sh
if [ "${1:-}" = -f ]; then
  printf '%s\n' "$2"
else
  exec "$REAL_READLINK" "$@"
fi
EOF
cat > "$bin/stat" <<'EOF'
#!/bin/sh
case " $* " in
  *' -c %u '*) printf '0\n' ;;
  *' -c %a '*) printf '600\n' ;;
  *) exec "$REAL_STAT" "$@" ;;
esac
EOF
cat > "$bin/cat" <<'EOF'
#!/bin/sh
if [ "${BIND_FAIL:-0}" = 1 ] && [ "${1:-}" = "$TEST_WORK/config/usb_gadget/octessera-orange-pi/UDC" ]; then
  exit 0
fi
exec /bin/cat "$@"
EOF
cat > "$bin/blkid" <<'EOF'
#!/bin/sh
case "$*" in
  *LABEL=OCTESSERA_SD*)
    case "${BLKID_MODE:-one}" in
      one) printf '%s\n' "$TEST_DEV" ;;
      duplicate) printf '%s\n' "$TEST_DEV" /dev/sdc1 ;;
      none) ;;
    esac
    ;;
  *'-s TYPE'*) printf 'vfat\n' ;;
esac
EOF
cat > "$bin/findmnt" <<'EOF'
#!/bin/sh
case "$*" in
  *"-S $TEST_DEV"*)
    [ "${MOUNT_ELSEWHERE:-0}" = 1 ] && printf '/media/elsewhere\n'
    ;;
  *'--mountpoint '* )
    [ -e "$TEST_WORK/mounted" ] && printf '%s\n' "$TEST_DEV"
    ;;
  *FSTYPE*) printf 'vfat\n' ;;
esac
EOF
cat > "$bin/mountpoint" <<'EOF'
#!/bin/sh
[ "${1:-}" = -q ] && [ -e "$TEST_WORK/mounted" ]
EOF
cat > "$bin/mount" <<'EOF'
#!/bin/sh
touch "$TEST_WORK/mounted"
EOF
cat > "$bin/umount" <<'EOF'
#!/bin/sh
rm -f "$TEST_WORK/mounted"
EOF
cat > "$bin/chown" <<'EOF'
#!/bin/sh
exit 0
EOF
cat > "$bin/logger" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$bin"/*

cp "$root/tools/storage/octessera-sd-card-lib.sh" "$fixture/lib.sh"
sed \
  -e "s|CONFIGFS_ROOT=/sys/kernel/config|CONFIGFS_ROOT=$fixture/config|" \
  -e "s|UDC_ROOT=/sys/class/udc|UDC_ROOT=$fixture/udc|" \
  -e "s|GADGET_SCRIPT=/usr/local/sbin/octessera-orange-usb-gadget|GADGET_SCRIPT=$fixture/gadget|" \
  -e "s|GADGET_LOCK=/run/lock/octessera-orange-usb-gadget.lock|GADGET_LOCK=$fixture/gadget.lock|" \
  -e "s|SD_LOCK=/run/octessera-sd-card.lock|SD_LOCK=$fixture/sd.lock|" \
  -e "s|STORAGE_STATE=/run/octessera-usb-storage.state|STORAGE_STATE=$fixture/storage.state|" \
  -e "s|SD_MOUNT=/var/lib/octessera/samples/sd-card|SD_MOUNT=$fixture/mount|" \
  -e "s|\. /usr/local/lib/octessera/octessera-sd-card-lib.sh|. $fixture/lib.sh|" \
  "$root/tools/storage/octessera-orange-storage" > "$fixture/storage"
sed -i '/^storage_log() {/a safe_partition() { return 0; }' "$fixture/storage"

cat > "$fixture/gadget" <<'EOF'
#!/bin/sh
case "$1" in
  teardown)
    [ "${FAIL_TEARDOWN:-0}" = 1 ] && exit 1
    rm -f "$TEST_WORK/normal-active"
    ;;
  setup)
    [ "${FAIL_SETUP:-0}" = 1 ] && exit 1
    touch "$TEST_WORK/normal-active"
    ;;
  *) exit 2 ;;
esac
EOF
chmod +x "$fixture/storage" "$fixture/gadget"

export PATH="$bin:$PATH"

reset_fixture() {
  rm -rf "$fixture/config/usb_gadget"
  mkdir -p "$fixture/config/usb_gadget"
  rm -f "$fixture/storage.state" "$fixture/storage.state.tmp" "$fixture/mounted" "$fixture/normal-active"
  touch "$fixture/normal-active"
  BLKID_MODE=one
  BIND_FAIL=0
  FAIL_SETUP=0
  FAIL_TEARDOWN=0
  MOUNT_ELSEWHERE=0
  export BLKID_MODE BIND_FAIL FAIL_SETUP FAIL_TEARDOWN MOUNT_ELSEWHERE
}

expect_failure() {
  local action="$1"
  if "$fixture/storage" "$action" > "$work/stdout" 2> "$work/stderr"; then
    echo "Expected $action to fail." >&2
    exit 1
  fi
}

assert_restored() {
  test -e "$fixture/normal-active"
  test -e "$fixture/mounted"
  test ! -e "$fixture/storage.state"
  test ! -e "$fixture/config/usb_gadget/octessera-orange-pi"
}

reset_fixture
BLKID_MODE=none
expect_failure storage-start
test -e "$fixture/normal-active"
test ! -e "$fixture/storage.state"

reset_fixture
BLKID_MODE=duplicate
expect_failure storage-start
test -e "$fixture/normal-active"
test ! -e "$fixture/storage.state"

reset_fixture
touch "$fixture/mounted"
"$fixture/storage" storage-start > "$work/stdout"
grep -qFx 'HOST_STATE=configured' "$work/stdout"
test -e "$fixture/storage.state"
grep -qFx 'WAS_MOUNTED=1' "$fixture/storage.state"
test -d "$fixture/config/usb_gadget/octessera-orange-pi"
test ! -e "$fixture/normal-active"
expect_failure storage-start
"$fixture/storage" storage-stop > "$work/stdout"
grep -qFx 'HOST_STATE=not attached' "$work/stdout"
assert_restored

reset_fixture
touch "$fixture/mounted"
BIND_FAIL=1
export BIND_FAIL
expect_failure storage-start
assert_restored

printf '%s\n' 'Orange storage lifecycle fixtures passed'
