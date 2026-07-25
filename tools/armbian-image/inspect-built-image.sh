#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <rootfs-dir-or-ext4-image>" >&2
  exit 2
fi

target="$1"
# shellcheck source=tools/armbian-image/inspect-mode.sh
source "$(dirname "${BASH_SOURCE[0]}")/inspect-mode.sh"
inspect_work="$(mktemp -d)"

cleanup() {
  rm -rf "$inspect_work"
}
trap cleanup EXIT

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
  local expected_mode
  local actual_mode
  case "$mode" in
    [0-7][0-7][0-7]) expected_mode="0$mode" ;;
    [0-7][0-7][0-7][0-7]) expected_mode="$mode" ;;
    *) echo "Invalid expected mode for $path." >&2; exit 1 ;;
  esac
  if [[ -d "$target" ]]; then
    actual_mode="$(stat -c '%a' "$target/$path")"
    [[ "${#actual_mode}" == 3 ]] && actual_mode="0$actual_mode"
    [[ "$(stat -c '%u' "$target/$path")" == 0 && "$actual_mode" == "$expected_mode" ]] || {
      echo "Unsafe updater ownership/mode at $path." >&2
      exit 1
    }
    return
  fi
  local metadata
  metadata="$(debugfs -R "stat /$path" "$target" 2>/dev/null)" || {
    echo "Missing image path: $path." >&2
    exit 1
  }
  printf '%s\n' "$metadata" | grep -Eq 'User: +0 +Group: +0' || {
    echo "Unsafe image ownership at $path." >&2
    exit 1
  }
  if ! actual_mode="$(octessera_debugfs_mode "$metadata")"; then
    echo "Missing image mode at $path." >&2
    exit 1
  fi
  [[ "$actual_mode" == "$expected_mode" ]] || {
    echo "Unsafe image mode at $path." >&2
    exit 1
  }
}

hash_path() {
  local path="$1"
  local dump_path="$inspect_work/$(basename "$path")"
  if [[ -d "$target" ]]; then
    sha256sum "$target/$path" | awk '{ print $1 }'
    return
  fi
  debugfs -R "dump -p /$path $dump_path" "$target" >/dev/null 2>&1 || {
    echo "Unable to read image path: $path." >&2
    exit 1
  }
  sha256sum "$dump_path" | awk '{ print $1 }'
}

validate_env_tokens() {
  local content="$1"
  local key="$2"
  local required_token="$3"
  printf '%s\n' "$content" | awk -v key="$key" -v required_token="$required_token" '
    function invalid(message) {
      print "Invalid " key " assignment: " message > "/dev/stderr"
      failed = 1
    }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) {
        if (line ~ ("(^|[^_[:alnum:]])" key "[[:space:]]*=")) {
          invalid("commented assignment")
        }
        next
      }
      if (line ~ ("^" key "=")) {
        if (assignments++) {
          invalid("duplicate assignment")
        }
        value = substr(line, length(key) + 2)
        if (value ~ /#/) {
          invalid("comments are not allowed")
        }
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        count = value == "" ? 0 : split(value, values, /[[:space:]]+/)
        for (position = 1; position <= count; position++) {
          token = values[position]
          if (token !~ /^[A-Za-z0-9][A-Za-z0-9_.-]*$/) {
            invalid("invalid token")
          }
          if (seen[token]++) {
            invalid("duplicate token")
          }
          if (token == required_token) {
            found++
          }
        }
        next
      }
      if (line ~ (("(^|[^_[:alnum:]])" key "[[:space:]]*="))) {
        invalid("malformed assignment")
      }
    }
    END {
      if (!assignments) {
        invalid("missing assignment")
      }
      if (found != 1) {
        invalid("required token must occur exactly once")
      }
      exit(failed ? 1 : 0)
    }
  '
}

unit_masked() {
  local path="$1"
  if [[ -d "$target" ]]; then
    [[ "$(readlink "$target/$path" 2>/dev/null || true)" == "/dev/null" ]]
  else
    debugfs -R "stat /$path" "$target" 2>/dev/null | grep -q '/dev/null'
  fi
}

reject_authorized_keys() {
  local passwd_content
  local user
  local home
  local key_path
  local key_paths=(root/.ssh/authorized_keys etc/ssh/authorized_keys etc/dropbear/authorized_keys)
  passwd_content="$(read_file etc/passwd)"
  while IFS=: read -r user _ _ _ _ home _; do
    [[ -n "$user" && -n "$home" ]] || continue
    [[ "$home" =~ ^/[A-Za-z0-9._/-]+$ ]] || {
      echo "Unsafe image home path for user $user." >&2
      exit 1
    }
    key_paths+=("${home#/}/.ssh/authorized_keys")
  done <<< "$passwd_content"
  for key_path in "${key_paths[@]}"; do
    if stat_path "$key_path"; then
      echo "Built image must not contain baked authorized keys: $key_path." >&2
      exit 1
    fi
  done
}

reject_path() {
  local path="$1"
  if stat_path "$path"; then
    echo "Diagnostic-only Orange image must not contain runtime path: $path." >&2
    exit 1
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
reject_authorized_keys

ssh_config="$(read_file etc/ssh/sshd_config.d/10-octessera-setup.conf)"
printf '%s\n' "$ssh_config" | grep -q '^PermitRootLogin no$' || { echo "Missing PermitRootLogin no." >&2; exit 1; }
printf '%s\n' "$ssh_config" | grep -q '^PasswordAuthentication no$' || { echo "Missing default PasswordAuthentication no." >&2; exit 1; }
printf '%s\n' "$ssh_config" | grep -q '^AllowUsers octessera$' || { echo "Missing AllowUsers octessera." >&2; exit 1; }

profile_metadata="$(read_file etc/octessera/build-metadata.env)"
printf '%s\n' "$profile_metadata" | grep -q '^OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w$' || {
  echo "Armbian image must be labeled orange-pi-zero-2w." >&2
  exit 1
}
printf '%s\n' "$profile_metadata" | grep -q '^OCTESSERA_RUNTIME_ENABLED_DEFAULT=false$' || {
  echo "Orange image must keep runtime disabled." >&2
  exit 1
}
reject_path etc/systemd/system/octessera.service
reject_path etc/systemd/system/multi-user.target.wants/octessera.service
reject_path usr/local/bin/octessera-pi
reject_path opt/octessera/current

spi_source_path=usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts
spi_dtbo_path=boot/overlay-user/octessera-h618-spi1-cs0.dtbo
armbian_env_path=boot/armbianEnv.txt
for path in "$spi_source_path" "$spi_dtbo_path" "$armbian_env_path"; do
  stat_path "$path" || { echo "Missing Orange Pi SPI image path: $path." >&2; exit 1; }
done
require_root_mode "$spi_source_path" 644
require_root_mode "$spi_dtbo_path" 644
require_root_mode "$armbian_env_path" 644
source_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SPI1_CS0_DTS_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
dtbo_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SPI1_CS0_DTBO_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
[[ -n "$source_hash" && -n "$dtbo_hash" ]] || { echo "Armbian image is missing SPI overlay hashes." >&2; exit 1; }
[[ "$(hash_path "$spi_source_path")" == "$source_hash" ]] || { echo "SPI overlay source hash mismatch." >&2; exit 1; }
[[ "$(hash_path "$spi_dtbo_path")" == "$dtbo_hash" ]] || { echo "SPI overlay DTBO hash mismatch." >&2; exit 1; }
armbian_env_content="$(read_file "$armbian_env_path")"
validate_env_tokens "$armbian_env_content" overlays i2c1-pi || { echo "Armbian image must claim overlays=i2c1-pi exactly once." >&2; exit 1; }
validate_env_tokens "$armbian_env_content" user_overlays octessera-h618-spi1-cs0 || { echo "Armbian image must claim the SPI user overlay exactly once." >&2; exit 1; }
spi_source_content="$(read_file "$spi_source_path")"
if printf '%s\n' "$spi_source_content" "$armbian_env_content" | grep -q 'spidev1_0'; then
  echo "Built Armbian image must not contain the stock spidev1_0 overlay path." >&2
  exit 1
fi

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
