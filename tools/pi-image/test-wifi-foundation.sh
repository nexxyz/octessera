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
    grep -qF -- '--portal-interface wlan0' "$path"
    grep -qF -- '--portal-gateway 192.168.42.1' "$path"
    grep -qF -- '900s' "$path"
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
    grep -qF '929a5b937a771a0e4f96446242af217c61118aedaaaa053aff75af61151c6acc' "$constructor"
    grep -qF '3481ef27637c5c4a176b59f74af4e2c232f6c67de8399eaf705fe6431ffc8939' "$constructor"
    grep -qF 'wifi-connect.metadata.json' "$constructor"
    grep -qF 'cargo-metadata.json' "$constructor"
    grep -qF 'THIRD-PARTY-NOTICES.md' "$constructor"
    grep -qF 'sha256sum -c -' "$constructor"
done
grep -qF 'target/wifi-connect-patched' "$ROOT/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'usr/local/share/doc/octessera/wifi-connect' "$ROOT/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'usr/local/share/octessera/wifi-connect' "$ROOT/userpatches/customize-image.sh"
grep -qF 'sha256sum -c -' "$ROOT/userpatches/customize-image.sh"
for dependency in network-manager dnsmasq wireless-tools iw; do
    grep -qF "$dependency" "$ROOT/tools/pi-image/stage4-octessera/00-install-deps/00-run-chroot.sh"
    grep -qF "$dependency" "$ROOT/userpatches/customize-image.sh"
done
grep -qF 'install -D -o root -g root -m 0755' "$ROOT/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'install -D -o root -g root -m 0644' "$ROOT/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'install_overlay_file usr/local/sbin/octessera-wifi-foundation /usr/local/sbin/octessera-wifi-foundation 0755' "$ROOT/userpatches/customize-image.sh"
grep -qF 'install_overlay_file etc/systemd/system/octessera-wifi-foundation.service /etc/systemd/system/octessera-wifi-foundation.service 0644' "$ROOT/userpatches/customize-image.sh"

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
