#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

inspect_payload_tar() {
  local tar_path="$1"
  tar -tf "$tar_path" | while IFS= read -r entry; do
    case "$entry" in
      /*|*../*|../*|*'/..'|'..') echo "Unsafe payload path: $entry" >&2; exit 1 ;;
    esac
  done
  tar -tvf "$tar_path" | while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; exit 1 ;;
    esac
  done
}

bash -n "$root/userpatches/customize-image.sh"

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "$root/userpatches/customize-image.sh" "$0"
fi

if command -v python3 >/dev/null 2>&1; then
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

if command -v actionlint >/dev/null 2>&1; then
  actionlint "$root/.github/workflows/armbian-image.yml"
fi

for path in "$root/userpatches/overlay" "$root/.github/workflows/armbian-image.yml"; do
  if grep -RInE '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|usb[_-]?gadget|g_mass_storage|wpa_passphrase|BEGIN OPENSSH PRIVATE KEY|BEGIN RSA PRIVATE KEY)' "$path"; then
    echo "Forbidden Raspberry Pi assumption or secret-like pattern found under $path" >&2
    exit 1
  fi
done

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
