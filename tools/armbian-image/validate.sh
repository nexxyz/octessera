#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

inspect_payload_tar() {
  local tar_path="$1"
  tar -tf "$tar_path" | while IFS= read -r entry; do
    case "$entry" in
      /*|..|../*|*/..|*/../*) echo "Unsafe payload path: $entry" >&2; exit 1 ;;
    esac
  done
  tar -tvf "$tar_path" | while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; exit 1 ;;
    esac
  done
}

required_files=(
  "$root/tools/armbian-image/inspect-output-images.sh"
  "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
  "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
  "$root/userpatches/overlay/etc/sudoers.d/octessera-update"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-guard.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"
  "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"
  "$root/userpatches/overlay/etc/systemd/system/octessera-setup.service"
  "$root/userpatches/overlay/etc/systemd/system/octessera.service"
)

bash -n "$root/userpatches/customize-image.sh"
bash -n "$root/tools/armbian-image/inspect-built-image.sh"
bash -n "$root/tools/armbian-image/inspect-output-images.sh"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
bash -n "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
python3 -m py_compile "$root/tools/device-update/updater_protocol.py" "$root/tools/device-update/updater_state.py" "$root/tools/device-update/updater_assets.py" "$root/tools/device-update/updater_guard.py" "$root/tools/device-update/updater_cli.py"

for file in "${required_files[@]}"; do
  [[ -f "$file" ]] || { echo "Missing required setup file: $file" >&2; exit 1; }
done

grep -q 'wifi_connect_version=4.11.84' "$root/userpatches/customize-image.sh" || { echo "Missing pinned wifi-connect version." >&2; exit 1; }
grep -q 'wifi_connect_sha256=413d70e6d1c1366cbe2b32555e8476f3e92878178ed1b9c82205985f055f1936' "$root/userpatches/customize-image.sh" || { echo "Missing pinned wifi-connect SHA256." >&2; exit 1; }
grep -q 'OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w' "$root/userpatches/customize-image.sh" || { echo "Missing Orange Pi board profile metadata." >&2; exit 1; }

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "$root/userpatches/customize-image.sh" "$root/tools/armbian-image/inspect-built-image.sh" "$root/tools/armbian-image/inspect-output-images.sh" "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-connect" "$root/userpatches/overlay/usr/local/sbin/octessera-update" "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard" "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery" "$0"
fi

cmp "$root/tools/device-update/octessera-update" "$root/userpatches/overlay/usr/local/sbin/octessera-update"
cmp "$root/tools/device-update/octessera-update-guard" "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
cmp "$root/tools/device-update/octessera-update-recovery" "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
if grep -Eq 'octessera-update-(guard|recovery)' "$root/userpatches/overlay/etc/sudoers.d/octessera-update"; then
  echo "Updater guard internals must not be present in sudoers." >&2
  exit 1
fi
if grep -q '^ConditionPathExists=' "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"; then
  echo "Updater recovery must run once per boot, not only when a transaction file exists." >&2
  exit 1
fi

if command -v python3 >/dev/null 2>&1; then
  PYTHONDONTWRITEBYTECODE=1 python3 - <<'PY' "$root/userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
compile(path.read_text(encoding="utf-8"), str(path), "exec")
PY
  PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test_setup_sidecar.py"
  python3 - <<'PY' "$root/.github/workflows/armbian-image.yml"
import sys
try:
    import yaml
except Exception:
    sys.exit(0)
with open(sys.argv[1], 'r', encoding='utf-8') as handle:
    yaml.safe_load(handle)
PY
fi

if command -v node >/dev/null 2>&1; then
  node --check "$root/userpatches/overlay/usr/local/share/octessera-setup-ui/app.js"
fi

if command -v actionlint >/dev/null 2>&1; then
  actionlint "$root/.github/workflows/armbian-image.yml"
fi

for path in "$root/userpatches/overlay" "$root/.github/workflows/armbian-image.yml"; do
  if grep -RInE '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|usb[_-]?gadget|g_mass_storage|wpa_passphrase|BEGIN OPENSSH PRIVATE KEY|BEGIN RSA PRIVATE KEY|BEGIN PRIVATE KEY|default_password|changeme|raspberry)' "$path"; then
    echo "Forbidden Raspberry Pi assumption or secret-like pattern found under $path" >&2
    exit 1
  fi
done

if find "$root/userpatches/overlay" -path '*/.ssh/authorized_keys' -o -name 'ssh_host_*' | grep -q .; then
  echo "Overlay must not bake SSH keys or authorized keys." >&2
  exit 1
fi

if grep -nE '^      (wifi|wi-fi|password|ssh_key|private_key|authorized_keys|user):' "$root/.github/workflows/armbian-image.yml"; then
  echo "Workflow must not expose raw first-run secret inputs." >&2
  exit 1
fi

payload_url="${PAYLOAD_URL:-${OCTESSERA_PAYLOAD_URL:-}}"
payload_sha256="${PAYLOAD_SHA256:-${OCTESSERA_PAYLOAD_SHA256:-}}"
if [[ -n "$payload_url" ]]; then
  [[ "$payload_url" == https://* ]] || { echo "Payload URL must use HTTPS." >&2; exit 1; }
  [[ "$payload_sha256" =~ ^[a-fA-F0-9]{64}$ ]] || { echo "Payload SHA256 is required and must be 64 hex characters." >&2; exit 1; }
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  curl --fail --location --proto '=https' --tlsv1.2 --output "$work/payload.tar" "$payload_url"
  echo "$payload_sha256  $work/payload.tar" | sha256sum -c -
  inspect_payload_tar "$work/payload.tar"
elif [[ -n "$payload_sha256" ]]; then
  echo "Payload URL is required when payload SHA256 is set." >&2
  exit 1
fi

preset_url="${PUBLIC_PRESET_CONFIGURATION_URL:-}"
if [[ -n "$preset_url" ]]; then
  [[ "$preset_url" == https://* ]] || { echo "Public PRESET_CONFIGURATION URL must use HTTPS." >&2; exit 1; }
  case " ${ARMBIAN_EXTENSIONS:-} " in
    *" preset-firstrun "*) ;;
    *) echo "PRESET_CONFIGURATION requires the preset-firstrun extension." >&2; exit 1 ;;
  esac
fi

echo "Armbian image validation passed."
