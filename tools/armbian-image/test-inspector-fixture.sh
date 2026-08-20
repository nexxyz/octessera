#!/usr/bin/env bash
# shellcheck disable=SC2317
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
module_dir="$root/tools/armbian-image"
inspect_path="$root/tools/armbian-image/inspect-path.sh"
inspector="$root/tools/armbian-image/inspect-built-image.sh"
runtime_inspector="$root/tools/armbian-image/inspect-runtime.sh"
real_debugfs="$(command -v debugfs || true)"
real_mkfs_ext4="$(command -v mkfs.ext4 || true)"
real_truncate="$(command -v truncate || true)"
work="$(mktemp -d)"
mock_bin="$work/bin"
trap 'rm -rf "$work"' EXIT

mkdir -p "$mock_bin"
for module in \
  "$root/tools/armbian-image/inspect-account-ssh.sh" \
  "$root/tools/armbian-image/inspect-network.sh" \
  "$root/tools/armbian-image/inspect-device-tree.sh" \
  "$root/tools/armbian-image/inspect-runtime-contracts.sh" \
  "$root/tools/armbian-image/inspect-runtime-account.sh" \
  "$root/tools/armbian-image/inspect-runtime-service.sh" \
  "$root/tools/armbian-image/inspect-runtime-udev.sh" \
  "$root/tools/armbian-image/inspect-runtime-device-apply.sh" \
  "$root/tools/armbian-image/inspect-runtime-oled.sh" \
  "$root/tools/armbian-image/inspect-runtime-mode.sh"; do
  bash -n "$module"
done
bash -n "$runtime_inspector"
cat > "$mock_bin/debugfs" <<'EOF'
#!/usr/bin/env bash
case "${DEBUGFS_CASE:-unit-valid}" in
  missing)
    printf '%s\n' 'debugfs 1.47.0 (5-Feb-2023)' 'stat: File not found by ext2_lookup'
    ;;
  malformed)
    printf '%s\n' 'stat: File not found by ext2_lookup extra'
    ;;
  error)
    printf '%s\n' 'stat: Filesystem not open' >&2
    exit 1
    ;;
  unit-valid)
    printf '%s\n' 'Inode:   7   Type:   symlink   Mode:    0777   Flags: 0x0' 'Fast    link    dest:    "/dev/null"'
    ;;
  unit-wrong)
    printf '%s\n' 'Inode:   7   Type:   symlink   Mode:    0777   Flags: 0x0' 'Fast    link    dest:    "/etc/passwd"'
    ;;
  unit-regular)
    printf '%s\n' 'Inode:   7   Type:   regular   Mode:    0644   Flags: 0x0' 'Fast    link    dest:    "/dev/null"'
    ;;
  variable-whitespace)
    case "$2" in
      'stat "/opt/octessera"'|'stat "/opt/octessera/releases"'|'stat "/opt/octessera/releases/1.2.3"')
        printf '%s\n' 'Inode:   7   Type:   directory   Mode:    040755   Flags: 0x0'
        ;;
      'stat "/opt/octessera/current"')
        printf '%s\n' 'Inode:   8   Type:   symlink   Mode:    0777   Flags: 0x0' 'Fast    link    dest:    "/opt/octessera/releases/1.2.3"'
        ;;
      'stat "/opt/octessera/releases/1.2.3/SHA256SUMS"'|'stat "/opt/octessera/releases/1.2.3/octessera-pi"'|'stat "/opt/octessera/releases/1.2.3/octessera-runtime.json"'|'stat "/opt/octessera/releases/1.2.3/update-manifest.json"')
        printf '%s\n' 'Inode:   9   Type:   regular   Mode:    0100555   Flags: 0x0'
        ;;
      'ls -p "/opt/octessera/releases/1.2.3"')
        printf '%s\n' '/9/0100555/0/0/SHA256SUMS/' '/10/0100555/0/0/octessera-pi/' '/11/0100444/0/0/octessera-runtime.json/' '/12/0100444/0/0/update-manifest.json/'
        ;;
    esac
    ;;
  sample-ext4)
    case "$2" in
      'ls -p "/var/lib/octessera/samples"')
        printf '%s\n' '/2/040755/0/0/./' '/2/040755/0/0/../' '/10/040755/0/0/Drum/'
        ;;
      'ls -p "/var/lib/octessera/samples/Drum"')
        printf '%s\n' '/10/040755/0/0/./' '/10/040755/0/0/../' '/11/040755/0/0/hihat open/'
        ;;
      'ls -p "/var/lib/octessera/samples/Drum/hihat open"')
        printf '%s\n' '/11/040755/0/0/./' '/11/040755/0/0/../' '/12/100644/0/0/space.wav/'
        ;;
      'stat "/var/lib/octessera/samples"')
        printf '%s\n' 'Inode:   2   Type:   directory   Mode:    040755   Flags: 0x0' 'User:   0   Group:   0   Size:   4096'
        ;;
      'stat "/var/lib/octessera/samples/Drum"')
        printf '%s\n' 'Inode:   10   Type:   directory   Mode:    040755   Flags: 0x0' 'User:   0   Group:   0   Size:   4096'
        ;;
      'stat "/var/lib/octessera/samples/Drum/hihat open"')
        printf '%s\n' 'Inode:   11   Type:   directory   Mode:    040755   Flags: 0x0' 'User:   0   Group:   0   Size:   4096'
        ;;
      'stat "/var/lib/octessera/samples/Drum/hihat open/space.wav"')
        printf '%s\n' 'Inode:   12   Type:   regular   Mode:    0644   Flags: 0x0' 'User:   0   Group:   0   Size:   11'
        ;;
    esac
    ;;
  runtime-owner-valid|runtime-owner-wrong-owner|runtime-owner-wrong-mode)
    case "$DEBUGFS_CASE" in
      runtime-owner-valid) owner=990; group=990; mode=040755 ;;
      runtime-owner-wrong-owner) owner=991; group=990; mode=040755 ;;
      runtime-owner-wrong-mode) owner=990; group=990; mode=040700 ;;
    esac
    printf '%s\n' "Inode:   42   Type:   directory   Mode:    $mode   Flags: 0x0" "User:   $owner   Group:   $group   Size:   4096"
    ;;
esac
EOF
chmod 0755 "$mock_bin/debugfs"

# shellcheck source=tools/armbian-image/inspect-path.sh
source "$module_dir/inspect-path.sh"
export PATH="$mock_bin:$PATH"

assert_status() {
  local expected="$1"
  shift
  local actual
  if "$@"; then
    actual=0
  else
    actual=$?
  fi
  [[ "$actual" == "$expected" ]] || {
    printf 'Expected status %s, got %s: %s\n' "$expected" "$actual" "$*" >&2
    exit 1
  }
}

fake_image="$work/not-an-image"
export inspector inspect_path real_debugfs real_mkfs_ext4 real_truncate fake_image
