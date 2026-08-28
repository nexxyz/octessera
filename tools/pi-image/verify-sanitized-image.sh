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

require_setup_executable() {
    local path="$1" label="$2" resolved target link_count
    require_path "$path" "$label"
    resolved="$path"
    link_count=0
    while [ -L "$resolved" ]; do
        link_count=$((link_count + 1))
        [ "$link_count" -le 8 ] || { echo "Sanitation check failed: setup command symlink chain is too deep at $path" >&2; exit 1; }
        target="$(readlink -- "$resolved")" || { echo "Sanitation check failed: unreadable setup command symlink at $path" >&2; exit 1; }
        if [[ "$target" == /* ]]; then
            resolved="$WORK_DIR/root$target"
        else
            resolved="$(dirname -- "$resolved")/$target"
        fi
        resolved="$(realpath -m -- "$resolved")" || { echo "Sanitation check failed: unsafe setup command symlink at $path" >&2; exit 1; }
    done
    case "$resolved" in
        "$WORK_DIR/root/"*) ;;
        *) echo "Sanitation check failed: setup command symlink escapes the image at $path" >&2; exit 1 ;;
    esac
    if [ ! -f "$resolved" ] || [ ! -x "$resolved" ]; then
        echo "Sanitation check failed: setup command is not an executable regular file at $path" >&2
        exit 1
    fi
    if [ "$resolved" = "$path" ]; then
        require_root_mode "$path" 755
    else
        require_root_mode "$resolved" 755
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
    local wifi_connect_doc_root="$WORK_DIR/root/usr/local/share/doc/octessera/wifi-connect"
    for path in "$helper" "$unit" "$WORK_DIR/root/usr/local/bin/wifi-connect"; do
        require_path "$path" "inactive Wi-Fi foundation path"
    done
    require_root_mode "$helper" 755
    require_root_mode "$unit" 644
    require_root_mode "$WORK_DIR/root/usr/local/bin/wifi-connect" 755
    if [ "$CONSTRUCTOR_POLICY_REQUIRED" = true ]; then
        for path in "$wifi_connect_doc_root/LICENSE" "$wifi_connect_doc_root/THIRD-PARTY-NOTICES.md" "$wifi_connect_doc_root/wifi-connect.metadata.json" "$wifi_connect_doc_root/cargo-metadata.json"; do
            require_path "$path" "patched wifi-connect documentation path"
            require_root_mode "$path" 644
        done
        echo "4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627  $WORK_DIR/root/usr/local/bin/wifi-connect" | sha256sum -c - >/dev/null 2>&1 || {
            echo "Sanitation check failed: patched wifi-connect binary has the wrong SHA-256" >&2
            exit 1
        }
        grep -qF '"binary_sha256": "4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627"' "$wifi_connect_doc_root/wifi-connect.metadata.json" || {
            echo "Sanitation check failed: patched wifi-connect metadata has the wrong binary SHA-256" >&2
            exit 1
        }
        grep -qF '"patch_sha256": "c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6"' "$wifi_connect_doc_root/wifi-connect.metadata.json" || {
            echo "Sanitation check failed: patched wifi-connect metadata has the wrong patch SHA-256" >&2
            exit 1
        }
    fi
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
    local coordinator="$root/usr/local/sbin/octessera-setup"
    local config="$root/usr/local/lib/octessera/setup_config.py"
    local http="$root/usr/local/lib/octessera/setup_http.py"
    local request_tmpfiles="$root/etc/tmpfiles.d/octessera-setup-request.conf"
    local setup_unit="$root/etc/systemd/system/octessera-setup.service"
    local request_path="$root/etc/systemd/system/octessera-setup-request.path"
    for path in "$profile" "$coordinator" "$config" "$http" "$setup_unit" "$request_path" "$request_tmpfiles"; do
        require_path "$path" "setup layer path"
    done
    for entry in "$profile:644" "$coordinator:755" "$config:644" "$http:644" "$setup_unit:644" "$request_path:644" "$request_tmpfiles:644"; do
        IFS=: read -r path mode <<< "$entry"
        require_root_mode "$path" "$mode"
    done
    for command in \
        "$WORK_DIR/root/usr/local/bin/wifi-connect" "$WORK_DIR/root/usr/bin/python3" \
        "$WORK_DIR/root/usr/sbin/iw" "$WORK_DIR/root/usr/bin/nmcli" \
        "$WORK_DIR/root/usr/sbin/ip" "$WORK_DIR/root/usr/bin/ss"; do
        require_setup_executable "$command" "setup prerequisite command"
    done
    grep -qx 'raspberry-pi-zero-2w' "$profile" || { echo "Raspberry setup profile is not fixed" >&2; exit 1; }
    grep -qF 'ALLOWED_ORIGINS' "$http" || { echo "Setup origins are not installed" >&2; exit 1; }
    grep -qF 'ALLOWED_HOSTS' "$http" || { echo "Setup host validation is not installed" >&2; exit 1; }
    grep -qF 'ipaddress.ip_network("192.168.42.0/24")' "$http" || { echo "Setup client network is not exact" >&2; exit 1; }
    grep -qF 'STATUS_DIR = "/run/octessera-setup-status"' "$coordinator" || { echo "Setup status path is not fixed" >&2; exit 1; }
    grep -qF 'MAX_BODY = 16384' "$http" || { echo "Setup body limit is not fixed" >&2; exit 1; }
    grep -qF 'Transfer-Encoding' "$http" || { echo "Setup transfer encoding is not rejected" >&2; exit 1; }
    grep -qF 'Content-Type' "$http" || { echo "Setup content type is not fixed" >&2; exit 1; }
    grep -qF 'class SetupHandler' "$http" || { echo "Setup HTTP handler is not installed" >&2; exit 1; }
    grep -qF 'class SetupHTTPServer' "$http" || { echo "Setup HTTP server is not installed" >&2; exit 1; }
    grep -qF 'PORTAL_WINDOW_SECONDS = 600' "$coordinator" || { echo "Setup portal window is not fixed" >&2; exit 1; }
    grep -qF 'INTERNAL_APPLY_SECONDS = 60' "$coordinator" || { echo "Setup apply window is not fixed" >&2; exit 1; }
    grep -qF 'cleanup_profiles' "$coordinator" || { echo "Setup AP cleanup is not fixed" >&2; exit 1; }
    grep -qFx 'After=systemd-tmpfiles-setup.service' "$request_path" || { echo "Setup request path is not ordered after tmpfiles" >&2; exit 1; }
    grep -qFx 'PathExists=/run/octessera-setup-request/inbox/start' "$request_path" || { echo "Setup request path watches the wrong path" >&2; exit 1; }
    grep -qFx 'Unit=octessera-setup.service' "$request_path" || { echo "Setup request path target is not direct" >&2; exit 1; }
    grep -qFx 'd /run/octessera-setup-request 0711 root root -' "$request_tmpfiles" || { echo "Raspberry setup request root tmpfile is not exact" >&2; exit 1; }
    grep -qFx 'd /run/octessera-setup-request/inbox 0700 pi pi -' "$request_tmpfiles" || { echo "Raspberry setup inbox tmpfile is not exact" >&2; exit 1; }
    grep -qFx 'ExecStart=/usr/local/sbin/octessera-setup' "$setup_unit" || { echo "Setup coordinator is not direct" >&2; exit 1; }
    grep -qFx 'RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK' "$setup_unit" || { echo "Setup address families are not exact" >&2; exit 1; }
    grep -qFx 'NoNewPrivileges=no' "$setup_unit" || { echo "Setup service must retain dnsmasq privilege-transition capability behavior" >&2; exit 1; }
    grep -qFx '# dnsmasq needs privilege-transition capabilities to drop from root while serving the setup AP.' "$setup_unit" || { echo "Setup service NNP exception rationale is missing" >&2; exit 1; }
    grep -qFx 'NoNewPrivileges=yes' "$root/etc/systemd/system/octessera.service" || { echo "Raspberry runtime service lost its NNP boundary" >&2; exit 1; }
    grep -qFx 'ReadWritePaths=/run/octessera-setup-request/inbox' "$root/etc/systemd/system/octessera.service" || { echo "Raspberry request inbox access is not exact" >&2; exit 1; }
    grep -qF 'RuntimeMaxSec=670s' "$setup_unit" || { echo "Setup runtime timeout is not fixed" >&2; exit 1; }
    grep -qF 'TimeoutStopSec=10s' "$setup_unit" || { echo "Setup stop timeout is not fixed" >&2; exit 1; }
    if grep -q '^TimeoutStartSec=' "$setup_unit"; then echo "Setup must not rely on TimeoutStartSec" >&2; exit 1; fi
    if grep -RIE 'setup-status\.py|setup-status-cli\.py|setup-call\.py|sidecar|receipt|active\.json|sequence|attemptId|requestToken|replay|retry|nonce|OCTESSERA_SETUP|setup-force|BEGIN (RSA|OPENSSH|PRIVATE) KEY|wpa_passphrase|ssid=|psk=' "$coordinator" "$config" "$http"; then
        echo "Setup layer contains removed orchestration or secret material" >&2
        exit 1
    fi
    for path in \
        "$root/var/lib/octessera/setup-complete" \
        "$root/var/lib/octessera/setup-force" \
        "$root/var/lib/octessera/setup-finalize-failed" \
        "$root/run/octessera/setup-portal.request" \
        "$root/run/octessera-setup-queue" \
        "$root/run/octessera-setup-request" \
        "$root/run/octessera-setup-request/inbox/start" \
        "$root/run/octessera-setup" \
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
