#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$SCRIPT_DIR/../.." && pwd)
RASPBERRY_HELPER=$ROOT/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-wifi-foundation
RASPBERRY_UNIT=$ROOT/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-wifi-foundation.service
ORANGE_HELPER=$ROOT/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation
ORANGE_UNIT=$ROOT/userpatches/overlay/etc/systemd/system/octessera-wifi-foundation.service
TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/octessera-wifi-foundation.XXXXXX")
FAKE_BIN=$TEST_ROOT/bin
CALL_LOG=$TEST_ROOT/call

cleanup() {
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT HUP INT TERM

for path in "$RASPBERRY_HELPER" "$RASPBERRY_UNIT" "$ORANGE_HELPER" "$ORANGE_UNIT"; do
    test -f "$path"
done

cmp "$RASPBERRY_HELPER" "$ORANGE_HELPER"
cmp "$RASPBERRY_UNIT" "$ORANGE_UNIT"

for path in "$RASPBERRY_HELPER" "$ORANGE_HELPER"; do
    if grep -Eiq '(/sys/class/net|iw[[:space:]]+dev|nmcli.*device|mac|hostname|ssh|password|country|setup[-_ ]?(complete|force)|sidecar|credential|secret|wpa|ssid=|psk=|BEGIN (RSA|OPENSSH|PRIVATE) KEY)' "$path"; then
        echo "Wi-Fi foundation helper contains forbidden behavior: $path" >&2
        exit 1
    fi
done

grep -qFx 'User=root' "$RASPBERRY_UNIT"
grep -qFx 'Group=root' "$RASPBERRY_UNIT"
grep -qFx 'ExecStart=/usr/local/sbin/octessera-wifi-foundation' "$RASPBERRY_UNIT"
grep -qFx 'TimeoutStartSec=905s' "$RASPBERRY_UNIT"
if grep -Eiq 'sidecar|hostname|ssh|password|country|setup[-_ ]?(complete|force)|credential|secret|ssid=|psk=|BEGIN (RSA|OPENSSH|PRIVATE) KEY' "$RASPBERRY_UNIT"; then
    echo "Wi-Fi foundation unit contains forbidden behavior." >&2
    exit 1
fi
for enabled_path in \
    "$ROOT/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/multi-user.target.wants/octessera-wifi-foundation.service" \
    "$ROOT/userpatches/overlay/etc/systemd/system/multi-user.target.wants/octessera-wifi-foundation.service"; do
    if test -e "$enabled_path" || test -L "$enabled_path"; then
        echo "Wi-Fi foundation unit must not be enabled in an image path." >&2
        exit 1
    fi
done
if grep -Eiq 'enable.*octessera-wifi-foundation|multi-user\.target\.wants.*octessera-wifi-foundation' \
    "$ROOT/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh" \
    "$ROOT/userpatches/customize-image.sh"; then
    echo "Wi-Fi foundation unit must not be enabled by image setup." >&2
    exit 1
fi

if grep -Eq 'wifi-connect-aarch64-unknown-linux-gnu\.tar\.gz|github\.com/balena-os/wifi-connect/releases|curl[[:space:]].*wifi-connect|(^|[[:space:]])tar[[:space:]].*wifi-connect' \
    "$ROOT/tools/pi-image/stage4-octessera/00-install-deps/00-run-chroot.sh" "$ROOT/userpatches/customize-image.sh"; then
    echo "Wi-Fi constructors must not download upstream wifi-connect." >&2
    exit 1
fi
for constructor in "$ROOT/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh" "$ROOT/userpatches/customize-image.sh"; do
    grep -qF 'THIRD-PARTY-NOTICES.md' "$constructor"
done

mkdir "$FAKE_BIN"
cat > "$FAKE_BIN/timeout" <<EOF
#!/bin/sh
printf '%s\n' "\$*" > "$CALL_LOG"
exit 0
EOF
chmod 755 "$FAKE_BIN/timeout"

PATH="$FAKE_BIN:$PATH" "$RASPBERRY_HELPER"
expected='--foreground --signal=TERM --kill-after=5s 900s /usr/local/bin/wifi-connect --portal-ssid Octessera Wi-Fi --portal-interface wlan0 --portal-gateway 192.168.42.1 --portal-listening-port 80'
test "$(cat "$CALL_LOG")" = "$expected"

echo "Wi-Fi foundation static and fake tests passed."
