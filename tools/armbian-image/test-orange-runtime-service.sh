#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
service="$root/userpatches/overlay/etc/systemd/system/octessera.service"
socket="$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket"
update_socket="$root/userpatches/overlay/etc/systemd/system/octessera-update.socket"
update_service="$root/userpatches/overlay/etc/systemd/system/octessera-update@.service"
udev_rule="$root/userpatches/overlay/etc/udev/rules.d/70-octessera-orange-runtime.rules"
gadget="$root/userpatches/overlay/etc/systemd/system/octessera-orange-usb-gadget.service"

[[ -f "$service" ]] || { echo "Missing Orange runtime service." >&2; exit 1; }
[[ -f "$update_socket" && -f "$update_service" ]] || { echo "Missing Orange update broker units." >&2; exit 1; }
grep -qFx 'Before=sound.target octessera.service' "$socket"
grep -qFx 'After=local-fs.target' "$socket"
for required_line in \
  'ListenStream=/run/octessera-update/update.sock' \
  'SocketMode=0660' \
  'SocketUser=root' \
  'SocketGroup=octessera-runtime' \
  'DirectoryMode=0755' \
  'Accept=yes'; do
  grep -qFx "$required_line" "$update_socket" || { echo "Update socket is missing: $required_line" >&2; exit 1; }
done
for required_line in \
  'User=root' \
  'Group=root' \
  'StandardInput=socket' \
  'StandardOutput=socket' \
  'ExecStart=/usr/local/sbin/octessera-update-broker' \
  'ProtectSystem=strict' \
  'ReadWritePaths=/opt/octessera /usr/local/bin /run/octessera'; do
  grep -qFx "$required_line" "$update_service" || { echo "Update broker service is missing: $required_line" >&2; exit 1; }
done
! grep -qE 'sudo|octessera-runtime' "$update_service" || { echo 'Update broker service has an unsafe privilege path.' >&2; exit 1; }
if grep -qFx 'After=local-fs.target octessera-provision-musical-default.service' "$socket"; then
  echo 'Orange apply socket must not wait for musical provisioning.' >&2
  exit 1
fi
grep -qFx 'After=systemd-modules-load.service sys-kernel-config.mount local-fs.target octessera-provision-musical-default.service' "$gadget"
grep -qFx 'Before=sound.target octessera.service' "$gadget"
grep -qFx 'Requires=octessera-provision-musical-default.service' "$gadget"
for required_line in \
  'User=octessera-runtime' \
  'Group=octessera-runtime' \
  'Requires=octessera-device-apply-reboot.socket' \
  'Requires=octessera-provision-musical-default.service' \
  'Requires=octessera-update-recovery.service' \
  'After=octessera-device-apply-reboot.socket' \
  'Wants=octessera-orange-boot-splash.service' \
  'After=octessera-orange-boot-splash.service' \
  'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
  'LimitRTPRIO=70' \
  'NoNewPrivileges=yes' \
  'ProtectSystem=strict' \
  'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot' \
  'PrivateTmp=yes' \
  'ProtectHome=yes'; do
  grep -qFx "$required_line" "$service" || { echo "Runtime service is missing: $required_line" >&2; exit 1; }
done
for required_line in 'StartLimitIntervalSec=30s' 'StartLimitBurst=3' 'Restart=on-failure' 'RestartPreventExitStatus=78' 'RestartSec=5s'; do
  grep -qFx "$required_line" "$service" || { echo "Orange runtime service is missing: $required_line" >&2; exit 1; }
done
! grep -qFx 'Restart=always' "$service" || { echo 'Orange runtime service still restarts always.' >&2; exit 1; }
python3 - "$service" <<'PY'
from pathlib import Path
import sys

lines = set(Path(sys.argv[1]).read_text(encoding="utf-8").splitlines())
assert "Restart=on-failure" in lines
assert "RestartPreventExitStatus=78" in lines

def restarts(exit_status):
    return exit_status != 0 and exit_status != 78

assert not restarts(78)
assert restarts(1)
print("Orange runtime exit-status restart policy fixture passed")
PY
! grep -qE '^(StartLimitAction|OnFailure|Requisite|BindsTo|PartOf)=' "$service" || { echo 'Orange runtime service has an unapproved failure dependency.' >&2; exit 1; }
[[ "$(grep -c '^Requires=' "$service")" == 3 ]] || { echo 'Orange runtime service has an unexpected Requires dependency.' >&2; exit 1; }
if grep -Eq '^(AmbientCapabilities|CapabilityBoundingSet)=|LimitRTPRIO=80' "$service"; then
  echo 'Runtime service grants ambient SYS_NICE or priority 80.' >&2
  exit 1
fi
expected_udev_rule=$'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"'
[[ "$(cat -- "$udev_rule")" == "$expected_udev_rule" ]] || { echo 'Orange runtime udev rule content is not exact.' >&2; exit 1; }
profile_gpio_label="$(sed -n 's/^GPIO_LABEL = "\(.*\)"/\1/p' "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo")"
profile_gpiochip=gpiochip1
[[ "$profile_gpio_label" == 300b000.pinctrl && "$profile_gpiochip" == gpiochip1 ]] || { echo 'Pinned Orange GPIO profile mapping changed.' >&2; exit 1; }
grep -qFx 'GPIO_CHIP = "/dev/gpiochip1"' "$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo" || { echo 'Orange GPIO controller device is not pinned.' >&2; exit 1; }
grep -qFx "KERNEL==\"$profile_gpiochip\", GROUP=\"octessera-runtime\", MODE=\"0660\"" "$udev_rule" || { echo 'Orange udev rule does not match the pinned GPIO profile mapping.' >&2; exit 1; }
if command -v udevadm >/dev/null 2>&1 && getent group octessera-runtime >/dev/null 2>&1 && udevadm help 2>&1 | grep -qE '(^|[[:space:]])verify([[:space:]]|$)'; then
  udevadm verify "$udev_rule"
else
  echo 'udevadm rule verification skipped; target runtime group is unavailable in this host.'
fi
if ! command -v systemd-analyze >/dev/null 2>&1; then
  echo 'systemd-analyze unavailable; static runtime service checks passed.'
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/etc/systemd/system/basic.target.wants" "$work/etc/systemd/system/multi-user.target.wants" "$work/etc/systemd/system/sockets.target.wants" "$work/usr/local/bin" "$work/etc"
cp "$service" "$work/etc/systemd/system/octessera.service"
cp "$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket" "$work/etc/systemd/system/octessera-device-apply-reboot.socket"
cp "$root/userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service" "$work/etc/systemd/system/octessera-device-apply-reboot@.service"
cp "$update_socket" "$work/etc/systemd/system/octessera-update.socket"
cp "$update_service" "$work/etc/systemd/system/octessera-update@.service"
cp "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" "$work/etc/systemd/system/octessera-orange-boot-splash.service"
cp "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service" "$work/etc/systemd/system/octessera-update-recovery.service"
chmod 0644 "$work/etc/systemd/system/octessera.service"
chmod 0644 "$work/etc/systemd/system/octessera-device-apply-reboot.socket"
chmod 0644 "$work/etc/systemd/system/octessera-device-apply-reboot@.service"
chmod 0644 "$work/etc/systemd/system/octessera-update.socket"
chmod 0644 "$work/etc/systemd/system/octessera-update@.service"
chmod 0644 "$work/etc/systemd/system/octessera-orange-boot-splash.service"
printf '%s\n' '#!/bin/sh' 'exit 0' > "$work/usr/local/bin/octessera-pi"
chmod 0755 "$work/usr/local/bin/octessera-pi"
printf '%s\n' \
  'root:x:0:0:root:/root:/bin/sh' \
  'octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin' > "$work/etc/passwd"
printf '%s\n' 'root:x:0:' 'octessera-runtime:x:990:' > "$work/etc/group"
for unit in octessera-provision-musical-default.service octessera-orange-usb-gadget.service; do
  printf '%s\n' '[Unit]' "Description=$unit" '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$work/etc/systemd/system/$unit"
done
printf '%s\n' '[Unit]' 'Description=update recovery' '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$work/etc/systemd/system/octessera-update-recovery.service"
printf '%s\n' '[Unit]' 'Description=Orange boot splash' '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$work/etc/systemd/system/octessera-orange-boot-splash.service"
for unit in sysinit.target sound.target local-fs.target sockets.target; do
  printf '%s\n' '[Unit]' "Description=$unit" > "$work/etc/systemd/system/$unit"
done
printf '%s\n' '[Unit]' 'Description=basic.target' 'Requires=sysinit.target sockets.target' 'After=sysinit.target sockets.target' > "$work/etc/systemd/system/basic.target"
printf '%s\n' '[Unit]' 'Description=multi-user.target' 'Requires=basic.target' 'After=basic.target' > "$work/etc/systemd/system/multi-user.target"
ln -s ../sysinit.target "$work/etc/systemd/system/basic.target.wants/sysinit.target"
ln -s ../sockets.target "$work/etc/systemd/system/basic.target.wants/sockets.target"
ln -s ../basic.target "$work/etc/systemd/system/multi-user.target.wants/basic.target"
ln -s ../octessera-device-apply-reboot.socket "$work/etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket"
ln -s ../octessera-update.socket "$work/etc/systemd/system/sockets.target.wants/octessera-update.socket"
ln -s ../octessera-provision-musical-default.service "$work/etc/systemd/system/multi-user.target.wants/octessera-provision-musical-default.service"
ln -s ../octessera.service "$work/etc/systemd/system/multi-user.target.wants/octessera.service"
ln -s ../octessera-update-recovery.service "$work/etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service"
verify_output="$work/systemd-verify.out"
if ! systemd-analyze --root="$work" verify multi-user.target >"$verify_output" 2>&1; then
  cat "$verify_output" >&2
  exit 1
fi
if grep -Eiq 'ordering cycle|job .* (deleted|deletion)' "$verify_output"; then
  cat "$verify_output" >&2
  echo 'Orange runtime target graph reported an ordering cycle or deleted job.' >&2
  exit 1
fi
rm "$work/etc/systemd/system/octessera-device-apply-reboot.socket"
if systemd-analyze --root="$work" verify octessera.service >"$work/missing-socket.out" 2>&1; then
  echo 'Orange runtime service remained valid without its required socket.' >&2
  exit 1
fi
grep -q 'octessera-device-apply-reboot.socket' "$work/missing-socket.out"
printf '%s\n' 'Orange runtime service static and systemd checks passed'
