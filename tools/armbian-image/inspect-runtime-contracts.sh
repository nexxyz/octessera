#!/usr/bin/env bash
# shellcheck disable=SC2154
module_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$module_dir/validation-assertions.sh"

octessera_require_built_updater_contract() {
  local path
  for path in \
    etc/systemd/system/octessera-update-guard.service \
    etc/systemd/system/octessera-update-recovery.service \
    etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service \
    usr/local/sbin/octessera-update usr/local/sbin/octessera-update-broker \
    usr/local/sbin/octessera-update-guard usr/local/sbin/octessera-update-recovery \
    usr/local/lib/octessera/updater_protocol.py usr/local/lib/octessera/updater_contract.py \
    usr/local/lib/octessera/updater_state.py usr/local/lib/octessera/updater_assets.py \
    usr/local/lib/octessera/updater_guard.py usr/local/lib/octessera/updater_cli.py \
    usr/local/lib/octessera/updater_profiles.py etc/systemd/system/octessera-update.socket \
    etc/systemd/system/octessera-update@.service etc/sudoers.d/octessera-update; do
    stat_path "$path" || { echo "Missing updater protocol path: $path" >&2; exit 1; }
  done
  for path in usr/local/sbin/octessera-update usr/local/sbin/octessera-update-broker usr/local/sbin/octessera-update-guard usr/local/sbin/octessera-update-recovery; do require_root_mode "$path" 755; done
  for path in usr/local/lib/octessera/updater_protocol.py usr/local/lib/octessera/updater_contract.py usr/local/lib/octessera/updater_state.py usr/local/lib/octessera/updater_assets.py usr/local/lib/octessera/updater_guard.py usr/local/lib/octessera/updater_cli.py usr/local/lib/octessera/updater_profiles.py etc/systemd/system/octessera-update.socket etc/systemd/system/octessera-update@.service; do require_root_mode "$path" 644; done
  require_root_mode etc/sudoers.d/octessera-update 440
  octessera_require_image_symlink etc/systemd/system/sockets.target.wants/octessera-update.socket ../octessera-update.socket /etc/systemd/system/octessera-update.socket
  update_socket_unit="$(read_file etc/systemd/system/octessera-update.socket)"
  printf '%s\n' "$update_socket_unit" | grep -q '^ListenStream=/run/octessera-update/update.sock$'
  printf '%s\n' "$update_socket_unit" | grep -q '^SocketMode=0660$'
  printf '%s\n' "$update_socket_unit" | grep -q '^SocketGroup=octessera-runtime$'
  update_broker_service="$(read_file etc/systemd/system/octessera-update@.service)"
  printf '%s\n' "$update_broker_service" | grep -q '^User=root$'
  printf '%s\n' "$update_broker_service" | grep -q '^ExecStart=/usr/local/sbin/octessera-update-broker$'
  recovery_unit="$(read_file etc/systemd/system/octessera-update-recovery.service)"
  printf '%s\n' "$recovery_unit" | grep -q '^RemainAfterExit=yes$'
  octessera_reject_text_match 'Updater recovery must run once per boot, not only when a transaction file exists.' "$recovery_unit" -q '^ConditionPathExists='
  sudoers="$(read_file etc/sudoers.d/octessera-update)"
  octessera_reject_text_match 'Updater sudoers must not contain runtime or guard internals.' "$sudoers" -Eq 'octessera-runtime|ALL=\(ALL\)|octessera-update-(guard|recovery)'
}
