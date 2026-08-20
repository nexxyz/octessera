#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

assert_rejected() {
  local name="$1" pattern="$2" content="$3" fixture="$work/$1" status
  printf '%s\n' "$content" > "$fixture"
  if octessera_reject_file_match "Fixture contains forbidden pattern: $name" -qE "$pattern" "$fixture"; then
    echo "Negative fixture was accepted: $name" >&2
    exit 1
  else
    status=$?
  fi
  [[ "$status" == 1 ]] || { echo "Negative fixture check failed unexpectedly: $name (status $status)." >&2; exit 1; }
}

assert_rejected security 'BEGIN (OPENSSH |RSA )?PRIVATE KEY' 'BEGIN OPENSSH PRIVATE KEY'
assert_rejected policy 'systemctl enable --now' 'systemctl enable --now octessera-update-recovery.service'
assert_rejected device-tree 'spidev1_0' 'compatible = "spidev1_0";'
assert_rejected runtime 'AmbientCapabilities=' 'AmbientCapabilities=CAP_SYS_NICE'
assert_rejected oled 'octessera-(mark|wordmark)\.svg' 'copy_file asset /usr/share/octessera/oled/octessera-mark.svg'

if octessera_reject_file_match 'Missing negative fixture was treated as clean.' -qF forbidden "$work/missing" 2>"$work/missing.stderr"; then
  echo 'Missing negative fixture was treated as clean.' >&2
  exit 1
else
  status=$?
fi
[[ "$status" != 0 && "$status" != 1 ]] || { echo "Missing negative fixture returned a non-failing status: $status." >&2; exit 1; }

real_path="$PATH"
mock_bin="$work/bin"
mkdir -p "$mock_bin"
cat > "$mock_bin/find" <<'EOF'
#!/usr/bin/env bash
echo 'fixture find failure' >&2
exit 2
EOF
chmod 0755 "$mock_bin/find"

if PATH="$mock_bin:$PATH" OCTESSERA_IMAGE_MODE=diagnostic bash "$root/tools/armbian-image/validate-security-policy.sh" >"$work/security.stdout" 2>"$work/security.stderr"; then
  echo 'Security validation accepted a failing find fixture.' >&2
  exit 1
fi
grep -qF 'find status 2' "$work/security.stderr"

mkdir -p "$work/image/etc/ssh"
if PATH="$mock_bin:$PATH" TARGET="$work/image" bash -c 'target="$TARGET"; source "$1"; octessera_require_ssh_clean' bash "$root/tools/armbian-image/inspect-account-ssh.sh" >"$work/account.stdout" 2>"$work/account.stderr"; then
  echo 'Account SSH inspection accepted a failing find fixture.' >&2
  exit 1
fi
grep -qF 'find status 2' "$work/account.stderr"

mkdir -p "$work/missing-image/etc"
if PATH="$real_path" TARGET="$work/missing-image" bash -c 'target="$TARGET"; source "$1"; octessera_require_ssh_clean' bash "$root/tools/armbian-image/inspect-account-ssh.sh" >"$work/missing-account.stdout" 2>"$work/missing-account.stderr"; then
  echo 'Account SSH inspection accepted a missing SSH directory fixture.' >&2
  exit 1
fi
grep -qF 'find status 1' "$work/missing-account.stderr"

printf '%s\n' 'Validation negative security, policy, device-tree, runtime, and OLED fixtures passed.'
