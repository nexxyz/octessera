#!/usr/bin/env bash
# shellcheck disable=SC2154
module_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$module_dir/validation-assertions.sh"

octessera_require_device_apply_lane() {
  local socket_content service_content device_apply_content line
  socket_content="$(read_file etc/systemd/system/octessera-device-apply-reboot.socket)"
  service_content="$(read_file etc/systemd/system/octessera-device-apply-reboot@.service)"
  device_apply_content="$(read_file usr/local/sbin/octessera-device-apply-reboot)"
  for line in 'Before=sound.target octessera.service' 'After=local-fs.target' 'ListenStream=/run/octessera-device-apply/reboot.sock' 'SocketMode=0660' 'SocketUser=root' 'SocketGroup=octessera-runtime' 'Accept=yes'; do printf '%s\n' "$socket_content" | grep -qFx "$line" || { echo "Orange apply socket is missing: $line" >&2; exit 1; }; done
  octessera_reject_text_match 'Orange apply socket must not wait for musical provisioning.' "$socket_content" -qFx 'After=local-fs.target octessera-provision-musical-default.service'
  for line in 'User=root' 'Group=root' 'StandardInput=socket' 'StandardOutput=socket' 'ExecStart=/usr/local/sbin/octessera-device-apply-reboot' 'NoNewPrivileges=yes' 'ProtectSystem=strict' 'ProtectHome=yes' 'RestrictAddressFamilies=AF_UNIX'; do printf '%s\n' "$service_content" | grep -qFx "$line" || { echo "Orange apply service is missing: $line" >&2; exit 1; }; done
  octessera_reject_text_match 'Orange apply service must not invoke systemctl directly.' "$service_content" -qF systemctl
  for line in 'SYSTEMCTL_PATH = "/usr/bin/systemctl"' 'REBOOT_REQUEST = b"reboot\n"' 'POWEROFF_REQUEST = b"poweroff\n"' 'ACCEPTED = b"accepted\n"' 'REJECTED = b"rejected\n"' 'if request == REBOOT_REQUEST:' '_validate_config(CONFIG_PATH)' 'command = "poweroff"' 'subprocess.run([SYSTEMCTL_PATH, command], check=True)' 'output_stream.write(ACCEPTED)' 'output_stream.write(REJECTED)'; do printf '%s\n' "$device_apply_content" | grep -qF "$line" || { echo "Orange device-apply script is missing: $line" >&2; exit 1; }; done
  octessera_reject_text_match 'Orange device-apply script contains an unsafe process execution fallback.' "$device_apply_content" -Eq 'os\.system|shell=True|subprocess\.Popen|subprocess\.call'
}

octessera_require_device_config_assets() {
  require_root_mode usr/local/lib/octessera/device_config.py 644
  require_root_mode usr/local/sbin/octessera-device-apply-reboot 755
  require_root_mode usr/share/octessera/defaults/pi-default.json 644
}
