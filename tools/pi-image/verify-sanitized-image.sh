#!/bin/bash
set -euo pipefail

VERIFICATION_PROFILE=""
CONSTRUCTOR_POLICY_REQUIRED=false
SETUP_LAYER_REQUIRED=false
RUNTIME_BUNDLE=""
usage() {
    echo "Usage: $0 --verification-profile full-constructor|legacy-runtime-only|legacy-setup-layer [--runtime-bundle <dir>] <image.zip>" >&2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --verification-profile)
            [ "$#" -ge 2 ] || { usage; exit 2; }
            [ -z "$VERIFICATION_PROFILE" ] || { echo "verification profile selected more than once" >&2; usage; exit 2; }
            VERIFICATION_PROFILE="$2"
            shift 2
            ;;
        --runtime-bundle)
            [ "$#" -ge 2 ] || { usage; exit 2; }
            RUNTIME_BUNDLE="$2"
            shift 2
            ;;
        --*)
            usage
            exit 2
            ;;
        *)
            break
            ;;
    esac
done

case "$VERIFICATION_PROFILE" in
    full-constructor)
        CONSTRUCTOR_POLICY_REQUIRED=true
        SETUP_LAYER_REQUIRED=true
        ;;
    legacy-runtime-only)
        ;;
    legacy-setup-layer)
        SETUP_LAYER_REQUIRED=true
        ;;
    "")
        echo "--verification-profile is required" >&2
        usage
        exit 2
        ;;
    *)
        echo "invalid verification profile: $VERIFICATION_PROFILE" >&2
        usage
        exit 2
        ;;
esac

if [ "$#" -lt 1 ]; then
    usage
    exit 2
fi
ZIP_PATH="$1"
shift
if [ "$#" -ne 0 ]; then
    usage
    exit 2
fi
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/verify-managed-runtime.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/verify-boot-layout.sh"
WORK_DIR="$(mktemp -d)"
LOOP_DEV=""

require_path() {
    local path="$1"
    local label="$2"
    if [ ! -e "$path" ]; then
        echo "Sanitation check failed: missing $label at $path" >&2
        exit 1
    fi
}

require_root_mode() {
    local path="$1"
    local mode="$2"
    local owner actual_mode
    owner="$(stat -c '%u' "$path")"
    actual_mode="$(stat -c '%a' "$path")"
    if [ "$owner" != 0 ] || [ "$actual_mode" != "$mode" ]; then
        echo "Sanitation check failed: unsafe updater ownership/mode at $path" >&2
        exit 1
    fi
}

require_raspberry_board_profile() {
    local profile_file="$WORK_DIR/root/etc/octessera/board-profile.env"
    local metadata_file="$WORK_DIR/root/etc/octessera/board-profile.json"
    if ! grep -qx 'OCTESSERA_BOARD_PROFILE_ID=raspberry-pi-zero-2w' "$profile_file"; then
        echo "Sanitation check failed: image board profile is not raspberry-pi-zero-2w" >&2
        exit 1
    fi
    if ! grep -q '"board_profile": "raspberry-pi-zero-2w"' "$metadata_file"; then
        echo "Sanitation check failed: image board metadata does not match raspberry-pi-zero-2w" >&2
        exit 1
    fi
}

require_updater_protocol() {
    for path in \
        "$WORK_DIR/root/usr/local/sbin/octessera-update" \
        "$WORK_DIR/root/usr/local/sbin/octessera-update-guard" \
        "$WORK_DIR/root/usr/local/sbin/octessera-update-recovery" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_protocol.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_contract.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_state.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_assets.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_guard.py" \
        "$WORK_DIR/root/usr/local/lib/octessera/updater_cli.py" \
        "$WORK_DIR/root/etc/systemd/system/octessera-update-guard.service" \
        "$WORK_DIR/root/etc/systemd/system/octessera-update-recovery.service" \
        "$WORK_DIR/root/etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service"; do
        require_path "$path" "updater protocol path"
    done
    require_path "$WORK_DIR/root/etc/sudoers.d/octessera-update" "updater sudoers rule"
    require_root_mode "$WORK_DIR/root/etc/sudoers.d/octessera-update" 440
    require_root_mode "$WORK_DIR/root/usr/local/sbin/octessera-update" 755
    require_root_mode "$WORK_DIR/root/usr/local/sbin/octessera-update-guard" 755
    require_root_mode "$WORK_DIR/root/usr/local/sbin/octessera-update-recovery" 755
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_protocol.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_contract.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_state.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_assets.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_guard.py" 644
    require_root_mode "$WORK_DIR/root/usr/local/lib/octessera/updater_cli.py" 644
    if grep -Eq 'octessera-update-(guard|recovery)' "$WORK_DIR/root/etc/sudoers.d/octessera-update"; then
        echo "Sanitation check failed: updater internals are exposed through sudoers" >&2
        exit 1
    fi
    grep -qx 'ExecStart=/usr/local/bin/octessera-pi' "$WORK_DIR/root/etc/systemd/system/octessera.service" || {
        echo "Sanitation check failed: service uses a direct executable path" >&2
        exit 1
    }
    grep -qx 'Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json' "$WORK_DIR/root/etc/systemd/system/octessera.service" || {
        echo "Sanitation check failed: service has no candidate health path" >&2
        exit 1
    }
    grep -qx 'Requires=octessera-update-recovery.service' "$WORK_DIR/root/etc/systemd/system/octessera.service" || {
        echo "Sanitation check failed: runtime does not require recovery" >&2
        exit 1
    }
    grep -qx 'RemainAfterExit=yes' "$WORK_DIR/root/etc/systemd/system/octessera-update-recovery.service" || {
        echo "Sanitation check failed: recovery is not retained for the boot" >&2
        exit 1
    }
    if grep -q '^ConditionPathExists=' "$WORK_DIR/root/etc/systemd/system/octessera-update-recovery.service"; then
        echo "Sanitation check failed: recovery is conditional instead of always active" >&2
        exit 1
    fi
}

require_no_unrestricted_sudoers() {
    local path="$WORK_DIR/root/etc/sudoers"
    if [ -f "$path" ] && [ ! -L "$path" ] && grep -Eiq '^[[:space:]]*[^#]*\bNOPASSWD[[:space:]]*:[[:space:]]*ALL([[:space:]]|$)' "$path"; then
        echo "Sanitation check failed: unrestricted passwordless sudo at $path" >&2
        exit 1
    fi
    if [ -d "$WORK_DIR/root/etc/sudoers.d" ] && [ ! -L "$WORK_DIR/root/etc/sudoers.d" ]; then
        while IFS= read -r -d '' path; do
            if grep -Eiq '^[[:space:]]*[^#]*\bNOPASSWD[[:space:]]*:[[:space:]]*ALL([[:space:]]|$)' "$path"; then
                echo "Sanitation check failed: unrestricted passwordless sudo at $path" >&2
                exit 1
            fi
        done < <(find -P "$WORK_DIR/root/etc/sudoers.d" -type f -print0)
    fi
}

require_wifi_foundation() {
    local helper="$WORK_DIR/root/usr/local/sbin/octessera-wifi-foundation"
    local unit="$WORK_DIR/root/etc/systemd/system/octessera-wifi-foundation.service"
    for path in "$helper" "$unit" "$WORK_DIR/root/usr/local/bin/wifi-connect"; do
        require_path "$path" "inactive Wi-Fi foundation path"
    done
    require_root_mode "$helper" 755
    require_root_mode "$unit" 644
    require_root_mode "$WORK_DIR/root/usr/local/bin/wifi-connect" 755
    grep -qF -- '--portal-interface wlan0' "$helper" || {
        echo "Sanitation check failed: Wi-Fi foundation does not fix wlan0" >&2
        exit 1
    }
    grep -qF -- '--portal-gateway 192.168.42.1' "$helper" || {
        echo "Sanitation check failed: Wi-Fi foundation does not fix its gateway" >&2
        exit 1
    }
    grep -qF -- '900s' "$helper" || {
        echo "Sanitation check failed: Wi-Fi foundation is not bounded" >&2
        exit 1
    }
    grep -qFx 'User=root' "$unit" || {
        echo "Sanitation check failed: Wi-Fi foundation unit is not root-owned" >&2
        exit 1
    }
    grep -qFx 'Group=root' "$unit" || {
        echo "Sanitation check failed: Wi-Fi foundation unit is not root-owned" >&2
        exit 1
    }
    grep -qFx 'ExecStart=/usr/local/sbin/octessera-wifi-foundation' "$unit" || {
        echo "Sanitation check failed: Wi-Fi foundation unit has the wrong helper" >&2
        exit 1
    }
    grep -qFx 'TimeoutStartSec=905s' "$unit" || {
        echo "Sanitation check failed: Wi-Fi foundation unit is not bounded" >&2
        exit 1
    }
    if grep -Eiq 'sidecar|hostname|ssh|password|country|setup[-_ ]?(complete|force)|credential|secret|/sys/class/net|iw[[:space:]]+dev|nmcli.*device|mac|wpa_passphrase|chpasswd|ssid=|psk=|BEGIN (RSA|OPENSSH|PRIVATE) KEY' "$helper" "$unit"; then
        echo "Sanitation check failed: Wi-Fi foundation contains forbidden behavior or secret handling" >&2
        exit 1
    fi
    if find "$WORK_DIR/root/etc/systemd/system" -type l -lname '*octessera-wifi-foundation.service' | grep -q .; then
        echo "Sanitation check failed: inactive Wi-Fi foundation unit is enabled" >&2
        exit 1
    fi
}

require_raspberry_constructor_policy() {
    local root="$WORK_DIR/root"
    local locale="$root/etc/default/locale"
    require_no_unrestricted_sudoers
    require_path "$locale" "constructor locale"
    require_root_mode "$locale" 644
    [ "$(cat "$locale")" = $'LANG=C.UTF-8\nLANGUAGE=en\nLC_MESSAGES=C.UTF-8' ] || { echo "Sanitation check failed: Raspberry default locale is not exact" >&2; exit 1; }
    if [ -f "$root/home/pi/.bashrc" ] && grep -Eq '^[[:space:]]*(export[[:space:]]+)?(LANG|LANGUAGE|LC_[[:alnum:]_]+)[[:space:]]*=' "$root/home/pi/.bashrc"; then
        echo "Sanitation check failed: Raspberry appliance user profile overrides the default locale" >&2
        exit 1
    fi
    for path in \
        "$root/etc/systemd/system/multi-user.target.wants/dnsmasq.service" \
        "$root/etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service" \
        "$root/etc/systemd/system/network-online.target.wants/NetworkManager-wait-online.service" \
        "$root/etc/systemd/system/multi-user.target.wants/ssh.service" \
        "$root/etc/systemd/system/sockets.target.wants/ssh.socket"; do
        if [ -e "$path" ] || [ -L "$path" ]; then
            echo "Sanitation check failed: standalone or SSH service remains enabled at $path" >&2
            exit 1
        fi
    done
    if [ ! -L "$root/etc/systemd/system/ssh.service" ] || [ "$(readlink "$root/etc/systemd/system/ssh.service")" != /dev/null ]; then
        echo "Sanitation check failed: SSH service is not masked" >&2
        exit 1
    fi
    if [ ! -L "$root/etc/systemd/system/ssh.socket" ] || [ "$(readlink "$root/etc/systemd/system/ssh.socket")" != /dev/null ]; then
        echo "Sanitation check failed: SSH socket is not masked" >&2
        exit 1
    fi
    for unit in NetworkManager.service dnsmasq.service systemd-networkd-wait-online.service NetworkManager-wait-online.service; do
        if [ ! -e "$root/etc/systemd/system/$unit" ] && [ ! -e "$root/lib/systemd/system/$unit" ] && [ ! -e "$root/usr/lib/systemd/system/$unit" ]; then
            echo "Sanitation check failed: missing required systemd unit $unit" >&2
            exit 1
        fi
    done
}

require_setup_layer() {
    local root="$WORK_DIR/root"
    local profile="$root/etc/octessera/setup-profile"
    local sidecar="$root/usr/local/sbin/octessera-setup-sidecar"
    local wrapper="$root/usr/local/sbin/octessera-wifi-connect"
    local request_helper="$root/usr/local/sbin/octessera-setup-request"
    local request_cleanup="$root/usr/local/sbin/octessera-setup-request-cleanup"
    local start_helper="$root/usr/local/sbin/octessera-setup-start"
    local cleanup_helper="$root/usr/local/sbin/octessera-setup-cleanup"
    local status_tool="$root/usr/local/lib/octessera/setup-status.py"
    local status_cli="$root/usr/local/lib/octessera/setup-status-cli.py"
    local call_tool="$root/usr/local/lib/octessera/setup-call.py"
    local setup_unit="$root/etc/systemd/system/octessera-setup.service"
    local request_path="$root/etc/systemd/system/octessera-setup-request.path"
    local request_unit="$root/etc/systemd/system/octessera-setup-request.service"
    for path in "$profile" "$sidecar" "$wrapper" "$request_helper" "$request_cleanup" "$start_helper" "$cleanup_helper" "$status_tool" "$status_cli" "$call_tool" "$setup_unit" "$request_path" "$request_unit"; do
        require_path "$path" "setup layer path"
    done
    for entry in \
        "$profile:644" "$sidecar:755" "$wrapper:755" "$request_helper:755" \
        "$request_cleanup:755" "$start_helper:755" "$cleanup_helper:755" "$status_tool:755" "$status_cli:644" "$call_tool:755" "$setup_unit:644" "$request_path:644" "$request_unit:644"; do
        IFS=: read -r path mode <<< "$entry"
        require_root_mode "$path" "$mode"
    done
    grep -qx 'raspberry-pi-zero-2w' "$profile" || { echo "Raspberry setup profile is not fixed" >&2; exit 1; }
    grep -qF 'ALLOWED_ORIGINS = frozenset(("http://192.168.42.1", "http://192.168.42.1:80"))' "$sidecar" || { echo "Setup origins are not exact" >&2; exit 1; }
    grep -qF 'ipaddress.ip_network("192.168.42.0/24")' "$sidecar" || { echo "Setup client network is not exact" >&2; exit 1; }
    grep -qF 'PUBLIC_DIR = "/run/octessera-setup-status"' "$status_tool" || { echo "Setup public status path is not fixed" >&2; exit 1; }
    grep -qF 'RECEIPT_DIR' "$status_tool" || { echo "Setup receipts are not staged" >&2; exit 1; }
    grep -qF 'MAX_BODY = 16384' "$sidecar" || { echo "Setup body limit is not fixed" >&2; exit 1; }
    grep -qF 'Transfer-Encoding' "$sidecar" || { echo "Setup transfer encoding is not rejected" >&2; exit 1; }
    grep -qF 'content_type != "application/json"' "$sidecar" || { echo "Setup content type is not fixed" >&2; exit 1; }
    grep -qF 'interface=wlan0' "$wrapper" || { echo "Setup wrapper interface is not fixed" >&2; exit 1; }
    grep -qF "/sys/class/net/\$interface/address" "$wrapper" || { echo "Setup wrapper MAC path is not fixed" >&2; exit 1; }
    grep -qF 'PathExists=/run/octessera/setup-portal.request' "$request_path" || { echo "Setup request path watches the wrong path" >&2; exit 1; }
    grep -qF 'RuntimeDirectory=octessera-setup' "$setup_unit" || { echo "Setup runtime directory is not exact" >&2; exit 1; }
    grep -qF ' -/run/octessera-setup-control -/run/octessera-setup-status' "$setup_unit" || { echo "Optional setup status paths are not namespace-safe" >&2; exit 1; }
    grep -qF 'RuntimeDirectoryMode=0700' "$setup_unit" || { echo "Setup nonce runtime directory is not private" >&2; exit 1; }
    grep -qF 'RuntimeMaxSec=1800s' "$setup_unit" || { echo "Setup runtime timeout is not fixed" >&2; exit 1; }
    if grep -q '^TimeoutStartSec=' "$setup_unit"; then echo "Setup must not rely on TimeoutStartSec" >&2; exit 1; fi
    if grep -RIE 'OCTESSERA_SETUP|setup-force|BEGIN (RSA|OPENSSH|PRIVATE) KEY|wpa_passphrase|ssid=|psk=' "$sidecar" "$wrapper" "$request_helper"; then
        echo "Setup layer contains secret, connection, or persistent-force material" >&2
        exit 1
    fi
    for path in \
        "$root/var/lib/octessera/setup-complete" \
        "$root/var/lib/octessera/setup-force" \
        "$root/var/lib/octessera/setup-finalize-failed" \
        "$root/run/octessera/setup-portal.request" \
        "$root/run/octessera-setup/nonce" \
        "$root/run/octessera-setup-control" \
        "$root/run/octessera-setup-status"; do
        if [ -e "$path" ] || [ -L "$path" ]; then
            echo "Sanitation check failed: setup runtime material remains at $path" >&2
            exit 1
        fi
    done
    require_path "$root/etc/systemd/system/multi-user.target.wants/octessera-setup-request.path" "enabled setup request path"
    if [ -e "$root/etc/systemd/system/multi-user.target.wants/octessera-setup.service" ] || [ -L "$root/etc/systemd/system/multi-user.target.wants/octessera-setup.service" ]; then
        echo "Sanitation check failed: Raspberry first-run setup service is enabled" >&2
        exit 1
    fi
}

cleanup() {
    set +e
    mountpoint -q "$WORK_DIR/root" && umount "$WORK_DIR/root"
    mountpoint -q "$WORK_DIR/boot" && umount "$WORK_DIR/boot"
    if [ -n "$LOOP_DEV" ]; then
        kpartx -dv "$LOOP_DEV" >/dev/null 2>&1 || true
        losetup -d "$LOOP_DEV" >/dev/null 2>&1 || true
    fi
    rm -rf "$WORK_DIR"
}
trap cleanup EXIT

unzip -q "$ZIP_PATH" -d "$WORK_DIR"
mapfile -t IMAGE_PATHS < <(find "$WORK_DIR" -type f -name '*.img' -print | sort)
if [ "${#IMAGE_PATHS[@]}" -ne 1 ]; then
    echo "Expected exactly one .img inside $ZIP_PATH, found ${#IMAGE_PATHS[@]}" >&2
    exit 1
fi
IMG_PATH="${IMAGE_PATHS[0]}"

LOOP_DEV="$(losetup --find --show "$IMG_PATH")"
kpartx -av "$LOOP_DEV" >/dev/null
sleep 2
BASE="$(basename "$LOOP_DEV")"
mkdir -p "$WORK_DIR/boot" "$WORK_DIR/root"
mount -o ro "/dev/mapper/${BASE}p1" "$WORK_DIR/boot"
mount -o ro "/dev/mapper/${BASE}p2" "$WORK_DIR/root"

for path in \
    "$WORK_DIR/boot/ssh" \
    "$WORK_DIR/boot/ssh.txt" \
    "$WORK_DIR/boot/wpa_supplicant.conf" \
    "$WORK_DIR/boot/network-config" \
    "$WORK_DIR/boot/user-data" \
    "$WORK_DIR/root/etc/wpa_supplicant/wpa_supplicant.conf"; do
    if [ -e "$path" ]; then
        echo "Sanitation check failed: found $path" >&2
        exit 1
    fi
done

if find "$WORK_DIR/root" \( -path '*/.ssh/authorized_keys' -o -path '*/.ssh/id_*' \) | grep -q .; then
    echo "Sanitation check failed: found SSH keys" >&2
    exit 1
fi

if find "$WORK_DIR/root/etc/NetworkManager/system-connections" -type f 2>/dev/null | grep -q .; then
    echo "Sanitation check failed: found NetworkManager connection profiles" >&2
    exit 1
fi

if grep -RIE '(BEGIN (RSA|OPENSSH) PRIVATE KEY|ghp_|github_pat_|ssid=|psk=)' \
    "$WORK_DIR/boot" \
    "$WORK_DIR/root/etc" \
    "$WORK_DIR/root/home" \
    "$WORK_DIR/root/root" >/dev/null 2>&1; then
    echo "Sanitation check failed: found credential-like material" >&2
    exit 1
fi

require_managed_runtime_binary "$WORK_DIR/root" "$RUNTIME_BUNDLE"
if [ "$CONSTRUCTOR_POLICY_REQUIRED" = true ]; then
    require_raspberry_constructor_policy
fi
require_path "$WORK_DIR/root/etc/systemd/system/octessera.service" "octessera.service"
require_path "$WORK_DIR/root/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service" "enabled boot splash service"
require_path "$WORK_DIR/root/etc/sudoers.d/octessera-shutdown" "shutdown sudoers rule"
require_path "$WORK_DIR/root/etc/octessera/board-profile.json" "board profile metadata"
require_raspberry_board_profile
require_octessera_boot_config "$WORK_DIR/boot" "$WORK_DIR/root"
require_octessera_boot_overlay "$WORK_DIR/boot" "$WORK_DIR/root"
require_octessera_boot_layer "$WORK_DIR/boot" "$WORK_DIR/root"
require_octessera_raspberry_identity_for_boot_layer "$WORK_DIR/boot" "$WORK_DIR/root"
require_updater_protocol
require_wifi_foundation
if [ "$SETUP_LAYER_REQUIRED" = true ]; then
    require_setup_layer
fi

echo "Pi image sanitation check passed (boot layer: $OCTESSERA_BOOT_LAYER_CLASSIFICATION)"
