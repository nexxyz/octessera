#!/bin/sh
set -eu

PACKAGE_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
IMAGE_ROOT="$PACKAGE_ROOT/root"
PROVISION_ROOT="$PACKAGE_ROOT/files"
DEVICE_UPDATE_ROOT="$PACKAGE_ROOT/device-update"
REMOTE_REPO=${REMOTE_REPO:-/home/pi/octessera-dev}
SERVICE=${SERVICE:-octessera.service}
BOARD_PROFILE=${BOARD_PROFILE:-raspberry-pi-zero-2w}
UPDATE_INITRAMFS=${UPDATE_INITRAMFS-}
WAKE_TRACE=${WAKE_TRACE:-0}
SYSROOT=${SYSROOT:-}

update_initramfs_argument=0
for argument do
    case "$argument" in
        --update-initramfs)
            update_initramfs_argument=1
            ;;
        *)
            echo "Unknown provisioning option: $argument" >&2
            exit 2
            ;;
    esac
done
case "$UPDATE_INITRAMFS" in
    ''|0|1) ;;
    *)
        echo "UPDATE_INITRAMFS must be 0 or 1; use --update-initramfs for an explicit rebuild." >&2
        exit 2
        ;;
esac
if [ "$update_initramfs_argument" -eq 1 ]; then
    UPDATE_INITRAMFS=1
elif [ -z "$UPDATE_INITRAMFS" ]; then
    UPDATE_INITRAMFS=0
fi

if [ "$SERVICE" != octessera.service ]; then
    echo "Pi provisioning supports only the managed service name octessera.service; got $SERVICE." >&2
    exit 2
fi
if [ "$BOARD_PROFILE" = orange-pi-zero-2w ]; then
    echo "Orange Pi profile is not supported by Raspberry Pi provisioning; use the separate Armbian workflow." >&2
    exit 2
fi
if [ "$BOARD_PROFILE" != raspberry-pi-zero-2w ]; then
    echo "Raspberry Pi provisioning accepts only raspberry-pi-zero-2w; got $BOARD_PROFILE." >&2
    exit 2
fi
case "$REMOTE_REPO" in
    /*) ;;
    *)
        echo "REMOTE_REPO must be an absolute path; got $REMOTE_REPO." >&2
        exit 2
        ;;
esac

target_path() {
    printf '%s%s' "$SYSROOT" "$1"
}

SERVICE_TARGET=$(target_path "/etc/systemd/system/$SERVICE")
GROUP_TARGET=$(target_path /etc/group)
input_group_count="$(awk -F: '$1 == "input" { count++ } END { print count + 0 }' "$GROUP_TARGET" 2>/dev/null || printf '0')"
if [ ! -f "$GROUP_TARGET" ] || [ -L "$GROUP_TARGET" ] || [ "$input_group_count" -ne 1 ]; then
    echo "Raspberry target must contain exactly one input group in /etc/group before provisioning octessera.service." >&2
    exit 1
fi

missing_tools=0
for command in python3 curl flock sha256sum unzip visudo systemctl; do
    if ! command -v "$command" >/dev/null 2>&1; then
        missing_tools=1
    fi
done
if [ "$missing_tools" -eq 1 ]; then
    command -v apt-get >/dev/null 2>&1 || {
        echo "Updater provisioning requires python3, curl, util-linux, unzip, sudo, and systemd tooling." >&2
        exit 2
    }
    sudo apt-get update
    sudo apt-get install -y --no-install-recommends python3-minimal curl util-linux unzip sudo coreutils
fi

install_file() {
    mode="$1"
    source="$2"
    destination="$3"
    test -f "$source" && test ! -L "$source"
    sudo install -D -m "$mode" "$source" "$(target_path "$destination")"
}

ensure_boot_config_line() {
    line="$1"
    if ! grep -qxF "$line" "$BOOT_CONFIG"; then
        printf '%s\n' "$line" | sudo tee -a "$BOOT_CONFIG" >/dev/null
    fi
}

ensure_raspberry_uart_inactive() {
    sudo sed -i -E '/^[[:space:]]*(dtoverlay=disable-bt|enable_uart=)/d' "$BOOT_CONFIG"
    ensure_boot_config_line "dtoverlay=disable-bt"
    ensure_boot_config_line "enable_uart=0"
    while grep -Eq '(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)' "$CMDLINE"; do
        sudo sed -i -E 's/(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)/\1\4/' "$CMDLINE"
    done
    if grep -Eq '(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)' "$CMDLINE"; then
        echo "Serial console token remains in the Raspberry Pi kernel command line." >&2
        exit 1
    fi
    for unit in serial-getty@ttyAMA0.service serial-getty@ttyS0.service serial-getty@serial0.service bluetooth.service hciuart.service; do
        sudo systemctl mask --now "$unit" >/dev/null
        test "$(sudo systemctl is-enabled "$unit")" = masked
    done
    sudo rm -f "$(target_path /usr/local/lib/octessera/rpi_uart_release.py)"
    test ! -e "$(target_path /usr/local/lib/octessera/rpi_uart_release.py)"
}

escape_sed_replacement() {
    printf '%s' "$1" | sed 's/[\\&|]/\\&/g'
}

normalize_raspberry_usb_role() {
    config="$1"
    desired_role="$2"
    sudo python3 - "$config" "$desired_role" <<'PY'
import os
import re
import stat
import sys
import tempfile


config_path = sys.argv[1]
desired_role = sys.argv[2]
role_lines = {
    b"dtoverlay=dwc2,dr_mode=peripheral": b"gadget",
    b"dtoverlay=dwc2,dr_mode=host": b"host",
}
directive = re.compile(rb"^dtoverlay\s*=\s*dwc2(?:,|$)")


def fail(message):
    print(f"Raspberry boot config: {message}", file=sys.stderr)
    raise SystemExit(1)


if desired_role not in {"gadget", "host"}:
    fail(f"unsupported USB role: {desired_role}")
metadata = os.lstat(config_path)
if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
    fail("boot config must be a regular, unlinked file")
with open(config_path, "rb") as handle:
    lines = handle.read().splitlines(keepends=True)

section = None
all_headers = []
managed_roles = []
for index, line in enumerate(lines):
    body = line.rstrip(b"\r\n")
    stripped = body.strip()
    if stripped == b"[all]":
        all_headers.append(index)
        section = "all"
        continue
    if stripped.startswith(b"[") and stripped.endswith(b"]"):
        section = None
        continue
    if not stripped or stripped.startswith(b"#"):
        continue
    if directive.match(stripped):
        if section != "all":
            continue
        if body != stripped or stripped not in role_lines:
            fail("managed [all] contains a malformed or competing DWC2 directive")
        managed_roles.append(index)

if not all_headers:
    fail("boot config must contain a managed [all] block")
if len(managed_roles) > 1:
    fail("managed [all] contains duplicate or competing DWC2 role lines")

desired_line = f"dtoverlay=dwc2,dr_mode={'peripheral' if desired_role == 'gadget' else 'host'}".encode()
if managed_roles:
    index = managed_roles[0]
    ending = lines[index][len(lines[index].rstrip(b"\r\n")):]
    lines[index] = desired_line + ending
else:
    index = all_headers[-1]
    ending = lines[index][len(lines[index].rstrip(b"\r\n")):]
    if not ending:
        ending = b"\n"
    lines.insert(index + 1, desired_line + ending)

directory = os.path.dirname(config_path) or "."
descriptor, temporary = tempfile.mkstemp(prefix=".octessera-usb-role.", dir=directory)
try:
    os.fchmod(descriptor, stat.S_IMODE(metadata.st_mode))
    os.fchown(descriptor, metadata.st_uid, metadata.st_gid)
    with os.fdopen(descriptor, "wb") as handle:
        descriptor = None
        handle.write(b"".join(lines))
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, config_path)
    temporary = None
finally:
    if descriptor is not None:
        os.close(descriptor)
    if temporary is not None:
        try:
            os.unlink(temporary)
        except OSError:
            pass
PY
}

BOOT_CONFIG=$(target_path /boot/firmware/config.txt)
if [ ! -f "$BOOT_CONFIG" ]; then
    BOOT_CONFIG=$(target_path /boot/config.txt)
fi
test -f "$BOOT_CONFIG"
CMDLINE=$(target_path /boot/firmware/cmdline.txt)
[ -f "$CMDLINE" ] || CMDLINE=$(target_path /boot/cmdline.txt)
test -f "$CMDLINE"
BOOT_STATE_BEFORE=$(sha256sum "$BOOT_CONFIG" "$CMDLINE")
sudo systemctl stop "$SERVICE" >/dev/null 2>&1 || true

if ! grep -qxF '[all]' "$BOOT_CONFIG"; then
    printf '\n%s\n' '[all]' | sudo tee -a "$BOOT_CONFIG" >/dev/null
fi

while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
        ''|'#'*) continue ;;
        dtoverlay=disable-bt|dtoverlay=dwc2,dr_mode=peripheral|enable_uart=0|\[all\]) continue ;;
    esac
    ensure_boot_config_line "$line"
done < "$PROVISION_ROOT/boot/config.txt.append"

sudo rm -f \
    "$(target_path /etc/initramfs-tools/hooks/cellsymphony-boot-splash)" \
    "$(target_path /etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash)" \
    "$(target_path /etc/initramfs-tools/hooks/octessera-boot-splash)" \
    "$(target_path /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash)" \
    "$(target_path /etc/systemd/system/cellsymphony-boot-splash.service)" \
    "$(target_path /etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service)" \
    "$(target_path /etc/systemd/system/multi-user.target.wants/cellsymphony-boot-splash.service)"

install_file 0755 "$IMAGE_ROOT/usr/local/sbin/octessera-usb-gadget" /usr/local/sbin/octessera-usb-gadget
install_file 0755 "$IMAGE_ROOT/usr/local/sbin/octessera-usb-role" /usr/local/sbin/octessera-usb-role
install_file 0644 "$IMAGE_ROOT/usr/local/lib/octessera/device_config.py" /usr/local/lib/octessera/device_config.py
install_file 0755 "$IMAGE_ROOT/usr/local/sbin/octessera-update" /usr/local/sbin/octessera-update
install_file 0755 "$IMAGE_ROOT/usr/local/sbin/octessera-update-guard" /usr/local/sbin/octessera-update-guard
install_file 0755 "$IMAGE_ROOT/usr/local/sbin/octessera-update-recovery" /usr/local/sbin/octessera-update-recovery
install_file 0644 "$DEVICE_UPDATE_ROOT/updater_protocol.py" /usr/local/lib/octessera/updater_protocol.py
install_file 0644 "$DEVICE_UPDATE_ROOT/updater_contract.py" /usr/local/lib/octessera/updater_contract.py
install_file 0644 "$DEVICE_UPDATE_ROOT/updater_state.py" /usr/local/lib/octessera/updater_state.py
install_file 0644 "$DEVICE_UPDATE_ROOT/updater_assets.py" /usr/local/lib/octessera/updater_assets.py
install_file 0644 "$DEVICE_UPDATE_ROOT/updater_guard.py" /usr/local/lib/octessera/updater_guard.py
install_file 0644 "$DEVICE_UPDATE_ROOT/updater_cli.py" /usr/local/lib/octessera/updater_cli.py
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-update-guard.service" /etc/systemd/system/octessera-update-guard.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-update-recovery.service" /etc/systemd/system/octessera-update-recovery.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-usb-gadget.service" /etc/systemd/system/octessera-usb-gadget.service
install_file 0644 "$IMAGE_ROOT/etc/modules-load.d/octessera-usb-gadget.conf" /etc/modules-load.d/octessera-usb-gadget.conf
install_file 0440 "$IMAGE_ROOT/etc/sudoers.d/octessera-usb-storage" /etc/sudoers.d/octessera-usb-storage
install_file 0440 "$IMAGE_ROOT/etc/sudoers.d/octessera-usb-role" /etc/sudoers.d/octessera-usb-role
sudo install -d -m 0755 "$SERVICE_TARGET.d"
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera.service.d/audio-realtime.conf" "/etc/systemd/system/$SERVICE.d/audio-realtime.conf"
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-boot-splash.service" /etc/systemd/system/octessera-boot-splash.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-oled-shutdown.service" /etc/systemd/system/octessera-oled-shutdown.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-performance-governor.service" /etc/systemd/system/octessera-performance-governor.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-network-health.service" /etc/systemd/system/octessera-network-health.service
install_file 0644 "$IMAGE_ROOT/etc/systemd/system/octessera-network-health.timer" /etc/systemd/system/octessera-network-health.timer
install_file 0644 "$IMAGE_ROOT/etc/systemd/journald.conf.d/10-octessera.conf" /etc/systemd/journald.conf.d/10-octessera.conf
install_file 0644 "$IMAGE_ROOT/etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf" /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf
install_file 0755 "$IMAGE_ROOT/usr/local/bin/octessera-network-health" /usr/local/bin/octessera-network-health
install_file 0440 "$IMAGE_ROOT/etc/sudoers.d/octessera-shutdown" /etc/sudoers.d/octessera-shutdown
install_file 0440 "$IMAGE_ROOT/etc/sudoers.d/octessera-update" /etc/sudoers.d/octessera-update
install_file 0644 "$IMAGE_ROOT/etc/profile.d/octessera-welcome.sh" /etc/profile.d/octessera-welcome.sh
sudo install -d -m 0755 "$(target_path /etc/octessera)"
printf 'OCTESSERA_BOARD_PROFILE_ID=%s\n' "$BOARD_PROFILE" | sudo tee "$(target_path /etc/octessera/board-profile.env)" >/dev/null

sudo sed -i 's/\r$//' \
    "$(target_path /usr/local/sbin/octessera-usb-gadget)" \
    "$(target_path /usr/local/sbin/octessera-update)" \
    "$(target_path /usr/local/sbin/octessera-update-guard)" \
    "$(target_path /usr/local/sbin/octessera-update-recovery)" \
    "$(target_path /usr/local/bin/octessera-network-health)" \
    "$(target_path /etc/systemd/system/octessera-usb-gadget.service)" \
    "$(target_path /etc/systemd/system/octessera-update-guard.service)" \
    "$(target_path /etc/systemd/system/octessera-update-recovery.service)" \
    "$(target_path /etc/systemd/system/octessera-boot-splash.service)" \
    "$(target_path /etc/systemd/system/octessera-oled-shutdown.service)" \
    "$(target_path /etc/systemd/system/octessera-performance-governor.service)" \
    "$(target_path /etc/systemd/system/octessera-network-health.service)" \
    "$(target_path /etc/systemd/system/octessera-network-health.timer)" \
    "$(target_path /etc/systemd/journald.conf.d/10-octessera.conf)" \
    "$(target_path /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf)" \
    "$(target_path /etc/profile.d/octessera-welcome.sh)"

pi_record="$(getent passwd pi)"
IFS=: read -r pi_user _ pi_uid pi_gid _ pi_home pi_shell <<EOF
$pi_record
EOF
test "$pi_user" = pi && test "$pi_home" = /home/pi && test "$pi_shell" = /bin/bash
pi_home_target=$(target_path "$pi_home")
test -d "$pi_home_target" && test ! -L "$pi_home_target"
hushlogin="$pi_home_target/.hushlogin"
if [ -e "$hushlogin" ] || [ -L "$hushlogin" ]; then
    test -f "$hushlogin" && test ! -L "$hushlogin" && test "$(stat -c '%u:%g:%a:%s' "$hushlogin")" = "$pi_uid:$pi_gid:644:0" && test ! -s "$hushlogin"
else
    sudo install -D -m 0644 /dev/null "$hushlogin"
    sudo chown "$pi_user:$pi_user" "$hushlogin"
fi

REMOTE_REPO_TARGET=$(target_path "$REMOTE_REPO")
sudo install -d -m 0755 "$REMOTE_REPO_TARGET"
sudo chown "$pi_user:$pi_user" "$REMOTE_REPO_TARGET"
test -d "$REMOTE_REPO_TARGET" && test ! -L "$REMOTE_REPO_TARGET"
test "$(stat -c '%u:%g:%a' "$REMOTE_REPO_TARGET")" = "$pi_uid:$pi_gid:755"

REMOTE_REPO_ESCAPED=$(escape_sed_replacement "$REMOTE_REPO")
if [ "$WAKE_TRACE" = "1" ]; then
    WAKE_TRACE_LINE=Environment=OCTESSERA_WAKE_TRACE=1
else
    WAKE_TRACE_LINE=
fi
WAKE_TRACE_ESCAPED=$(escape_sed_replacement "$WAKE_TRACE_LINE")
sed \
    -e "s|@REMOTE_REPO@|$REMOTE_REPO_ESCAPED|g" \
    -e "s|@WAKE_TRACE@|$WAKE_TRACE_ESCAPED|g" \
    "$PROVISION_ROOT/etc/systemd/system/octessera.service.template" |
    sudo tee "$SERVICE_TARGET" >/dev/null
sudo chmod 0644 "$SERVICE_TARGET"
sudo sed -i 's/\r$//' \
    "$SERVICE_TARGET" \
    "$SERVICE_TARGET.d/audio-realtime.conf"

sudo visudo -cf "$(target_path /etc/sudoers.d/octessera-shutdown)" >/dev/null
sudo visudo -cf "$(target_path /etc/sudoers.d/octessera-usb-storage)" >/dev/null
sudo visudo -cf "$(target_path /etc/sudoers.d/octessera-usb-role)" >/dev/null
sudo visudo -cf "$(target_path /etc/sudoers.d/octessera-update)" >/dev/null

if [ "$UPDATE_INITRAMFS" = "1" ]; then
    if ! grep -qxF "# octessera required boot settings" "$BOOT_CONFIG" && ! grep -qxF "# Octessera required boot settings" "$BOOT_CONFIG"; then
        printf '\n' | sudo tee -a "$BOOT_CONFIG" >/dev/null
        # shellcheck disable=SC2024
        sed '/^\[all\]$/d' "$PROVISION_ROOT/boot/config.txt.initramfs.append" |
            sudo tee -a "$BOOT_CONFIG" >/dev/null
    fi
    ensure_boot_config_line "dtparam=spi=on"
    ensure_boot_config_line "auto_initramfs=1"

    if ! command -v update-initramfs >/dev/null 2>&1; then
        sudo apt-get update
        sudo apt-get install -y --no-install-recommends initramfs-tools
    fi
    install_file 0755 "$IMAGE_ROOT/etc/initramfs-tools/hooks/octessera-boot-splash" /etc/initramfs-tools/hooks/octessera-boot-splash
    install_file 0755 "$IMAGE_ROOT/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash
    sudo sed -i 's/\r$//' \
        "$(target_path /etc/initramfs-tools/hooks/octessera-boot-splash)" \
        "$(target_path /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash)"
    sudo install -d -m 0755 "$(target_path /etc/initramfs-tools)"
    grep -qxF "spi-bcm2835" "$(target_path /etc/initramfs-tools/modules)" || printf '%s\n' "spi-bcm2835" | sudo tee -a "$(target_path /etc/initramfs-tools/modules)" >/dev/null
    grep -qxF "spidev" "$(target_path /etc/initramfs-tools/modules)" || printf '%s\n' "spidev" | sudo tee -a "$(target_path /etc/initramfs-tools/modules)" >/dev/null
    sudo update-initramfs -u
else
    echo "Skipping initramfs update; pass -UpdateInitramfs when an OS or boot change requires a rebuild."
fi

desired_usb_role=gadget
device_config=$(target_path /home/pi/presets/default.json)
if [ -e "$device_config" ] || [ -L "$device_config" ]; then
    desired_usb_role=$(python3 "$IMAGE_ROOT/usr/local/lib/octessera/device_config.py" --data-role "$device_config")
fi
normalize_raspberry_usb_role "$BOOT_CONFIG" "$desired_usb_role"

ensure_raspberry_uart_inactive

if [ -n "$SYSROOT" ]; then
    OCTESSERA_USB_ROLE_BOOT_ROOT="$SYSROOT" sudo "$(target_path /usr/local/sbin/octessera-usb-role)" "$desired_usb_role"
else
    sudo /usr/local/sbin/octessera-usb-role "$desired_usb_role"
fi

sudo install -d -m 0750 "$(target_path /etc/sudoers.d)"
sudo systemctl restart systemd-journald
sudo iw dev wlan0 set power_save off >/dev/null 2>&1 || true
sudo nmcli connection modify preconfigured 802-11-wireless.powersave 2 >/dev/null 2>&1 || true
sudo nmcli device reapply wlan0 >/dev/null 2>&1 || true

sudo systemctl daemon-reload
if [ "$BOOT_STATE_BEFORE" != "$(sha256sum "$BOOT_CONFIG" "$CMDLINE")" ]; then
    sudo systemctl stop "$SERVICE" >/dev/null 2>&1 || true
    echo "Boot configuration changed. Reboot, then re-run provisioning before starting octessera." >&2
    exit 75
fi
if ! command -v pinctrl >/dev/null 2>&1 || \
   ! pinctrl get 14 | grep -Eq 'GPIO14[[:space:]]*=[[:space:]]*input([[:space:]]|$)' || \
   ! pinctrl get 15 | grep -Eq 'GPIO15[[:space:]]*=[[:space:]]*input([[:space:]]|$)'; then
    sudo systemctl stop "$SERVICE" >/dev/null 2>&1 || true
    echo "GPIO14/15 are not confirmed as safe inputs. Reboot, then re-run provisioning before starting octessera." >&2
    exit 75
fi
sudo systemctl enable octessera-usb-gadget.service >/dev/null
sudo systemctl enable --now octessera-update-recovery.service >/dev/null
if [ -e "$(target_path /opt/octessera/current)" ] || [ -L "$(target_path /opt/octessera/current)" ]; then
    sudo "$(target_path /usr/local/sbin/octessera-update)" bootstrap >/dev/null
fi
sudo systemctl enable --now octessera-network-health.timer >/dev/null
sudo systemctl enable octessera-oled-shutdown.service >/dev/null
sudo systemctl start octessera-oled-shutdown.service
sudo systemctl enable octessera-performance-governor.service >/dev/null
sudo systemctl start octessera-performance-governor.service
sudo systemctl enable "$SERVICE" >/dev/null
sudo rm -f "$(target_path /etc/systemd/system/multi-user.target.wants/octessera-boot-splash.service)"
sudo systemctl enable octessera-boot-splash.service >/dev/null
