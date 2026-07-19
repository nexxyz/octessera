#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

overlay_dir=/tmp/overlay
install -d -m 0755 /etc/octessera /usr/local/sbin /usr/local/lib/octessera /var/lib/octessera/samples

apt-get update
apt-get install -y --no-install-recommends ca-certificates curl tar xz-utils jq gpiod alsa-utils i2c-tools

install_overlay_file() {
  local src="$1"
  local dest="$2"
  local mode="$3"
  [[ -f "$overlay_dir/$src" ]] || return 0
  install -D -m "$mode" -o root -g root "$overlay_dir/$src" "$dest"
}

if [[ ! -d "$overlay_dir" ]]; then
  echo "Expected Armbian userpatches overlay at $overlay_dir." >&2
  exit 1
fi
if grep -RInE '(/home/pi|config\.txt|dtoverlay|dwc2|BCM[0-9]|usb[_-]?gadget|g_mass_storage)' "$overlay_dir"; then
  echo "Refusing Raspberry Pi-specific overlay content." >&2
  exit 1
fi
[[ -f "$overlay_dir/etc/octessera/armbian-image.txt" ]] || { echo "Missing Octessera Armbian marker overlay." >&2; exit 1; }
[[ -f "$overlay_dir/usr/local/sbin/octessera-armbian-diagnostics" ]] || { echo "Missing Octessera Armbian diagnostics overlay." >&2; exit 1; }
install_overlay_file etc/octessera/armbian-image.txt /etc/octessera/armbian-image.txt 0644
install_overlay_file usr/local/sbin/octessera-armbian-diagnostics /usr/local/sbin/octessera-armbian-diagnostics 0755

cat >/etc/octessera/build-metadata.env <<EOF
OCTESSERA_IMAGE_KIND=armbian
OCTESSERA_IMAGE_BUILT_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
OCTESSERA_RUNTIME_ENABLED_DEFAULT=false
EOF

payload_url="${OCTESSERA_PAYLOAD_URL:-}"
payload_sha256="${OCTESSERA_PAYLOAD_SHA256:-}"
if [[ -n "$payload_url" ]]; then
  [[ "$payload_url" == https://* ]] || { echo "OCTESSERA_PAYLOAD_URL must use HTTPS." >&2; exit 1; }
  [[ "$payload_sha256" =~ ^[a-fA-F0-9]{64}$ ]] || { echo "OCTESSERA_PAYLOAD_SHA256 is required." >&2; exit 1; }
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  curl --fail --location --proto '=https' --tlsv1.2 --output "$work/payload.tar" "$payload_url"
  echo "$payload_sha256  $work/payload.tar" | sha256sum -c -
  tar -tf "$work/payload.tar" | while IFS= read -r entry; do
    case "$entry" in
      /*|*../*|../*|*'/..'|'..') echo "Unsafe payload path: $entry" >&2; exit 1 ;;
    esac
  done
  tar -tvf "$work/payload.tar" | while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; exit 1 ;;
    esac
  done
  mkdir "$work/extract"
  tar -xf "$work/payload.tar" -C "$work/extract" --no-same-owner --no-same-permissions
  if [[ -f "$work/extract/octessera-payload.json" ]]; then
    jq -e '.name == "octessera-armbian-payload"' "$work/extract/octessera-payload.json" >/dev/null
    install -D -m 0644 "$work/extract/octessera-payload.json" /etc/octessera/payload.json
    if jq -e '.enable_runtime == true and .compatible == true' "$work/extract/octessera-payload.json" >/dev/null; then
      install -d -m 0755 /opt/octessera
      cp -a "$work/extract/." /opt/octessera/
    else
      install -d -m 0755 /usr/local/lib/octessera/payload-staged
      cp -a "$work/extract/." /usr/local/lib/octessera/payload-staged/
    fi
  else
    echo "Payload is missing octessera-payload.json." >&2
    exit 1
  fi
fi

if [[ -n "${PUBLIC_PRESET_CONFIGURATION_URL:-}" ]]; then
  export PRESET_CONFIGURATION="$PUBLIC_PRESET_CONFIGURATION_URL"
fi

apt-get clean
rm -rf /var/lib/apt/lists/*
