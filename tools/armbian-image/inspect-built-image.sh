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

unit_masked() {
  local path="$1"
  if [[ -d "$target" ]]; then
    [[ "$(readlink "$target/$path" 2>/dev/null || true)" == "/dev/null" ]]
  else
    debugfs -R "stat /$path" "$target" 2>/dev/null | grep -q '/dev/null'
  fi
}

unit_not_enabled() {
  local path="$1"
  ! stat_path "$path"
}

shadow="$(read_file etc/shadow)"
for user in root octessera orangepi pi armbian; do
  line="$(printf '%s\n' "$shadow" | grep -E "^${user}:" || true)"
  if [[ -n "$line" ]]; then
    hash="${line#*:}"
    hash="${hash%%:*}"
    case "$hash" in
      ""|\!*|\**|x) ;;
      *) echo "User $user has a usable baked password hash." >&2; exit 1 ;;
    esac
  fi
done

if [[ -d "$target" ]]; then
  if find "$target/root" "$target/home" -path '*/.ssh/authorized_keys' 2>/dev/null | grep -q .; then
    echo "Built image must not contain baked authorized_keys." >&2
    exit 1
  fi
else
  passwd="$(read_file etc/passwd)"
  while IFS=: read -r user _ _ _ _ home _; do
    [[ -n "$user" && -n "$home" ]] || continue
    case "$home" in
      /root|/home/*)
        if stat_path "${home#/}/.ssh/authorized_keys"; then
          echo "Built image must not contain baked authorized_keys for $user." >&2
          exit 1
        fi
        ;;
    esac
  done <<<"$passwd"
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

unit_masked etc/systemd/system/ssh.service || { echo "ssh.service is not masked in the built image." >&2; exit 1; }
unit_masked etc/systemd/system/ssh.socket || { echo "ssh.socket is not masked in the built image." >&2; exit 1; }
unit_not_enabled etc/systemd/system/multi-user.target.wants/ssh.service || { echo "ssh.service is enabled before setup." >&2; exit 1; }
unit_not_enabled etc/systemd/system/sockets.target.wants/ssh.socket || { echo "ssh.socket is enabled before setup." >&2; exit 1; }

echo "Built Armbian image inspection passed."
