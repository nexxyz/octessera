#!/usr/bin/env bash

set -euo pipefail
umask 077

readonly DEPLOY_USER='octessera'
readonly DEPLOY_GROUP='octessera'
readonly DEPLOY_HOME='/home/octessera'
readonly AUTHORIZED_KEYS="$DEPLOY_HOME/.ssh/authorized_keys"
readonly SUDOERS_FILE='/etc/sudoers.d/octessera-deploy'

ALLOW_DEPLOY_SUDO=0
PUBLIC_KEY=''
SUDOERS_TMP=''
SUDOERS_INSTALLED=0

die() {
  printf 'ERROR: %s\n' "$1" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: sudo bash bootstrap-armbian-ssh.sh [--allow-deploy-sudo] 'ssh-ed25519 ...'

Create the dedicated octessera deployment account and install exactly one
ssh-ed25519 public key. Existing account, key, and sudoers data are preserved.
EOF
}

while (($# > 0)); do
  case "$1" in
    --allow-deploy-sudo)
      ALLOW_DEPLOY_SUDO=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --*)
      die "unknown option: $1"
      ;;
    *)
      [[ -z "$PUBLIC_KEY" ]] || die 'exactly one public key argument is required'
      PUBLIC_KEY="$1"
      shift
      ;;
  esac
done

[[ "$EUID" -eq 0 ]] || die 'run this script with sudo (for example: sudo bash bootstrap-armbian-ssh.sh '\''ssh-ed25519 ...'\'')'
[[ -n "$PUBLIC_KEY" ]] || { usage >&2; exit 2; }

if [[ "$PUBLIC_KEY" == *$'\n'* || "$PUBLIC_KEY" == *$'\r'* ]]; then
  die 'the public key must be one line'
fi
if [[ ! "$PUBLIC_KEY" =~ ^ssh-ed25519[[:space:]]+[A-Za-z0-9+/]+={0,3}([[:space:]].*)?$ ]]; then
  die 'the argument must be one OpenSSH ssh-ed25519 public key'
fi

command -v ssh-keygen >/dev/null 2>&1 || die 'ssh-keygen is required'

KEY_FILE=$(mktemp)
cleanup() {
  rm -f -- "$KEY_FILE"
  if [[ -n "$SUDOERS_TMP" ]]; then
    rm -f -- "$SUDOERS_TMP"
  fi
  if [[ "$SUDOERS_INSTALLED" -eq 1 ]]; then
    rm -f -- "$SUDOERS_FILE"
  fi
}
trap cleanup EXIT
printf '%s\n' "$PUBLIC_KEY" > "$KEY_FILE"
ssh-keygen -lf "$KEY_FILE" -E sha256 >/dev/null 2>&1 || die 'ssh-keygen rejected the public key'
KEY_FINGERPRINT=$(ssh-keygen -lf "$KEY_FILE" -E sha256 | awk '{print $2}')
[[ -n "$KEY_FINGERPRINT" ]] || die 'could not calculate the public-key fingerprint'

if getent group "$DEPLOY_GROUP" >/dev/null 2>&1; then
  :
else
  groupadd --system "$DEPLOY_GROUP"
fi
DEPLOY_GID=$(getent group "$DEPLOY_GROUP" | awk -F: '{print $3}')
[[ -n "$DEPLOY_GID" ]] || die "could not resolve group $DEPLOY_GROUP"

if getent passwd "$DEPLOY_USER" >/dev/null 2>&1; then
  EXISTING_HOME=$(getent passwd "$DEPLOY_USER" | awk -F: '{print $6}')
  EXISTING_GID=$(id -g "$DEPLOY_USER")
  [[ "$EXISTING_HOME" == "$DEPLOY_HOME" ]] || die "existing $DEPLOY_USER account has unexpected home $EXISTING_HOME; refusing to move it"
  [[ "$EXISTING_GID" == "$DEPLOY_GID" ]] || die "existing $DEPLOY_USER account has a different primary group; refusing to change it"
else
  if [[ -e "$DEPLOY_HOME" || -L "$DEPLOY_HOME" ]]; then
    die "$DEPLOY_HOME already exists without a matching $DEPLOY_USER account; refusing to use it"
  fi
  useradd --create-home --home-dir "$DEPLOY_HOME" --gid "$DEPLOY_GROUP" --shell /bin/bash "$DEPLOY_USER"
fi

[[ ! -L "$DEPLOY_HOME" ]] || die "refusing a symlink as $DEPLOY_HOME"
if [[ ! -d "$DEPLOY_HOME" ]]; then
  install -d -m 0750 -o "$DEPLOY_USER" -g "$DEPLOY_GROUP" "$DEPLOY_HOME"
fi
chown "$DEPLOY_USER:$DEPLOY_GROUP" "$DEPLOY_HOME"
chmod 0750 "$DEPLOY_HOME"

SSH_DIR="$DEPLOY_HOME/.ssh"
[[ ! -L "$SSH_DIR" ]] || die "refusing a symlink as $SSH_DIR"
if [[ -e "$SSH_DIR" && ! -d "$SSH_DIR" ]]; then
  die "$SSH_DIR exists but is not a directory"
fi
install -d -m 0700 -o "$DEPLOY_USER" -g "$DEPLOY_GROUP" "$SSH_DIR"

[[ ! -L "$AUTHORIZED_KEYS" ]] || die "refusing a symlink as $AUTHORIZED_KEYS"
if [[ -e "$AUTHORIZED_KEYS" && ! -f "$AUTHORIZED_KEYS" ]]; then
  die "$AUTHORIZED_KEYS exists but is not a regular file"
fi
if [[ ! -e "$AUTHORIZED_KEYS" ]]; then
  touch "$AUTHORIZED_KEYS"
fi
chown "$DEPLOY_USER:$DEPLOY_GROUP" "$AUTHORIZED_KEYS"
chmod 0600 "$AUTHORIZED_KEYS"

if grep -Fqx -- "$PUBLIC_KEY" "$AUTHORIZED_KEYS"; then
  printf 'Public key already present in %s.\n' "$AUTHORIZED_KEYS"
else
  if [[ -s "$AUTHORIZED_KEYS" ]]; then
    LAST_BYTE=$(tail -c 1 "$AUTHORIZED_KEYS" | od -An -t x1 | tr -d '[:space:]')
    [[ "$LAST_BYTE" == '0a' ]] || printf '\n' >> "$AUTHORIZED_KEYS"
  fi
  printf '%s\n' "$PUBLIC_KEY" >> "$AUTHORIZED_KEYS"
  printf 'Public key added to %s.\n' "$AUTHORIZED_KEYS"
fi
chown "$DEPLOY_USER:$DEPLOY_GROUP" "$AUTHORIZED_KEYS"
chmod 0600 "$AUTHORIZED_KEYS"

if [[ "$ALLOW_DEPLOY_SUDO" -eq 1 ]]; then
  printf 'WARNING: --allow-deploy-sudo grants %s passwordless sudo for ALL commands.\n' "$DEPLOY_USER"
  printf 'WARNING: this changes %s; omit the flag to leave standard sudo policy unchanged.\n' "$SUDOERS_FILE"
  command -v visudo >/dev/null 2>&1 || die 'visudo is required when --allow-deploy-sudo is used'
  [[ -d /etc/sudoers.d && ! -L /etc/sudoers.d ]] || die '/etc/sudoers.d is missing or is a symlink'
  visudo -c >/dev/null || die 'existing sudoers configuration failed validation'

  SUDOERS_TMP=$(mktemp /etc/sudoers.d/.octessera-deploy.XXXXXX)
  printf '%s ALL=(ALL:ALL) NOPASSWD: ALL\n' "$DEPLOY_USER" > "$SUDOERS_TMP"
  chown root:root "$SUDOERS_TMP"
  chmod 0440 "$SUDOERS_TMP"
  visudo -cf "$SUDOERS_TMP" >/dev/null || die 'new sudoers rule failed validation'

  if [[ -e "$SUDOERS_FILE" || -L "$SUDOERS_FILE" ]]; then
    [[ ! -L "$SUDOERS_FILE" ]] || die "refusing a symlink as $SUDOERS_FILE"
    cmp -s "$SUDOERS_TMP" "$SUDOERS_FILE" || die "$SUDOERS_FILE already exists with different contents; refusing to overwrite it"
    rm -f -- "$SUDOERS_TMP"
    SUDOERS_TMP=''
  else
    mv "$SUDOERS_TMP" "$SUDOERS_FILE"
    SUDOERS_TMP=''
    SUDOERS_INSTALLED=1
  fi
  visudo -c >/dev/null || die 'sudoers validation failed after installing the deploy rule'
  SUDOERS_INSTALLED=0
  printf 'Passwordless deploy sudo is enabled.\n'
else
  printf 'Passwordless deploy sudo is not enabled; standard sudo policy was left unchanged.\n'
fi

printf '\nConfigured %s with key fingerprint %s.\n' "$DEPLOY_USER" "$KEY_FINGERPRINT"
printf 'Verification command (replace <ORANGE_PI_HOST> before running on Windows):\n'
printf "ssh -i ~/.ssh/octessera_orange_pi_ed25519 -o IdentitiesOnly=yes %s@<ORANGE_PI_HOST> 'id -un && hostname && test -r ~/.ssh/authorized_keys'\n" "$DEPLOY_USER"
