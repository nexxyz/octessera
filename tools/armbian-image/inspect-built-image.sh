#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <rootfs-dir-or-ext4-image>" >&2
  exit 2
fi

target="$1"

read_file() {
  local path="$1"
  if [[ -d "$target" ]]; then
    cat "$target/$path" 2>/dev/null || true
  else
    debugfs -R "cat /$path" "$target" 2>/dev/null || true
  fi
}

stat_path() {
  local path="$1"
  if [[ -d "$target" ]]; then
    [[ -e "$target/$path" ]]
  else
    debugfs -R "stat /$path" "$target" >/dev/null 2>&1
  fi
}

require_root_mode() {
  local path="$1"
  local mode="$2"
  if [[ -d "$target" ]]; then
    [[ "$(stat -c '%u %a' "$target/$path")" == "0 $mode" ]] || {
      echo "Unsafe updater ownership/mode at $path." >&2
      exit 1
    }
  fi
}

unit_masked() {
  local path="$1"
  if [[ -d "$target" ]]; then
    [[ "$(readlink "$target/$path" 2>/dev/null || true)" == "/dev/null" ]]
  else
    debugfs -R "stat /$path" "$target" 2>/dev/null | grep -q '/dev/null'
  fi
}

shadow="$(read_file etc/shadow)"
line="$(printf '%s\n' "$shadow" | grep -E '^octessera:' || true)"
if [[ -n "$line" ]]; then
  hash="${line#*:}"
  hash="${hash%%:*}"
  case "$hash" in
    ""|\!*|\**|x) ;;
    *) echo "Octessera user has a usable baked password hash." >&2; exit 1 ;;
  esac
fi

if [[ -d "$target" ]]; then
  if find "$target/etc/ssh" -maxdepth 1 -name 'ssh_host_*' | grep -q .; then
    echo "Built image must not contain baked SSH host keys." >&2
    exit 1
  fi
else
  if debugfs -R 'ls -p /etc/ssh' "$target" 2>/dev/null | grep -q 'ssh_host_'; then
    echo "Built image must not contain baked SSH host keys." >&2
    exit 1
  fi
fi

ssh_config="$(read_file etc/ssh/sshd_config.d/10-octessera-setup.conf)"
printf '%s\n' "$ssh_config" | grep -q '^PermitRootLogin no$' || { echo "Missing PermitRootLogin no." >&2; exit 1; }
printf '%s\n' "$ssh_config" | grep -q '^PasswordAuthentication no$' || { echo "Missing default PasswordAuthentication no." >&2; exit 1; }
printf '%s\n' "$ssh_config" | grep -q '^AllowUsers octessera$' || { echo "Missing AllowUsers octessera." >&2; exit 1; }

profile_metadata="$(read_file etc/octessera/build-metadata.env)"
printf '%s\n' "$profile_metadata" | grep -q '^OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w$' || {
  echo "Armbian image must be labeled orange-pi-zero-2w." >&2
  exit 1
}

for path in \
  etc/systemd/system/octessera-update-guard.service \
  etc/systemd/system/octessera-update-recovery.service \
  etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service \
  usr/local/sbin/octessera-update \
  usr/local/sbin/octessera-update-guard \
  usr/local/sbin/octessera-update-recovery \
  usr/local/lib/octessera/updater_protocol.py \
  usr/local/lib/octessera/updater_state.py \
  usr/local/lib/octessera/updater_assets.py \
  usr/local/lib/octessera/updater_guard.py \
  usr/local/lib/octessera/updater_cli.py \
  etc/sudoers.d/octessera-update; do
  stat_path "$path" || { echo "Missing updater protocol path: $path" >&2; exit 1; }
done
require_root_mode usr/local/sbin/octessera-update 755
require_root_mode usr/local/sbin/octessera-update-guard 755
require_root_mode usr/local/sbin/octessera-update-recovery 755
require_root_mode usr/local/lib/octessera/updater_protocol.py 644
require_root_mode usr/local/lib/octessera/updater_state.py 644
require_root_mode usr/local/lib/octessera/updater_assets.py 644
require_root_mode usr/local/lib/octessera/updater_guard.py 644
require_root_mode usr/local/lib/octessera/updater_cli.py 644
require_root_mode etc/sudoers.d/octessera-update 440

service_unit="$(read_file etc/systemd/system/octessera.service)"
printf '%s\n' "$service_unit" | grep -q '^ExecStart=/usr/local/bin/octessera-pi$' || {
  echo "Armbian service must use the managed updater binary link." >&2
  exit 1
}
printf '%s\n' "$service_unit" | grep -q '^Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json$' || {
  echo "Armbian service is missing the candidate health path." >&2
  exit 1
}
printf '%s\n' "$service_unit" | grep -q '^Requires=octessera-update-recovery.service$' || {
  echo "Armbian service does not require updater recovery." >&2
  exit 1
}
recovery_unit="$(read_file etc/systemd/system/octessera-update-recovery.service)"
printf '%s\n' "$recovery_unit" | grep -q '^RemainAfterExit=yes$' || {
  echo "Armbian recovery service is not retained for the boot." >&2
  exit 1
}
if printf '%s\n' "$recovery_unit" | grep -q '^ConditionPathExists='; then
  echo "Armbian recovery service must run once per boot, not only for pending transactions." >&2
  exit 1
fi
sudoers="$(read_file etc/sudoers.d/octessera-update)"
if printf '%s\n' "$sudoers" | grep -Eq 'octessera-update-(guard|recovery)'; then
  echo "Armbian sudoers must not expose updater internals." >&2
  exit 1
fi

unit_masked etc/systemd/system/ssh.service || { echo "ssh.service is not masked in the built image." >&2; exit 1; }
unit_masked etc/systemd/system/ssh.socket || { echo "ssh.socket is not masked in the built image." >&2; exit 1; }

echo "Built Armbian image inspection passed."
