#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

overlay_dir=/tmp/overlay
install -d -m 0755 /etc/octessera /usr/local/sbin /usr/local/lib/octessera /var/lib/octessera/samples

apt-get update
apt-get install -y --no-install-recommends ca-certificates coreutils curl tar xz-utils jq gpiod alsa-utils i2c-tools network-manager dnsmasq wireless-tools iw python3-minimal openssh-server sudo unzip util-linux

wifi_connect_version=4.11.84
wifi_connect_sha256=413d70e6d1c1366cbe2b32555e8476f3e92878178ed1b9c82205985f055f1936
wifi_connect_url="https://github.com/balena-os/wifi-connect/releases/download/v${wifi_connect_version}/wifi-connect-aarch64-unknown-linux-gnu.tar.gz"
wifi_work="$(mktemp -d)"
curl --fail --location --proto '=https' --tlsv1.2 --output "$wifi_work/wifi-connect.tar.gz" "$wifi_connect_url"
echo "$wifi_connect_sha256  $wifi_work/wifi-connect.tar.gz" | sha256sum -c -
tar -xf "$wifi_work/wifi-connect.tar.gz" -C "$wifi_work"
install -D -m 0755 "$wifi_work/wifi-connect" /usr/local/bin/wifi-connect
install -d -m 0755 /usr/local/share/doc/octessera
cat >/usr/local/share/doc/octessera/wifi-connect.metadata <<EOF
wifi-connect ${wifi_connect_version}
Source: ${wifi_connect_url}
SHA256: ${wifi_connect_sha256}
License: Apache-2.0
EOF
cat >/usr/local/share/doc/octessera/wifi-connect.NOTICE <<'EOF'
wifi-connect is distributed by balena under the Apache License 2.0.
See https://github.com/balena-os/wifi-connect for upstream license text.
EOF
rm -rf "$wifi_work"

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
install_overlay_file usr/local/sbin/octessera-update /usr/local/sbin/octessera-update 0755
install_overlay_file usr/local/sbin/octessera-wifi-connect /usr/local/sbin/octessera-wifi-connect 0755
install_overlay_file usr/local/sbin/octessera-setup-sidecar /usr/local/sbin/octessera-setup-sidecar 0755
install_overlay_file etc/systemd/system/octessera-setup.service /etc/systemd/system/octessera-setup.service 0644
install_overlay_file etc/systemd/system/octessera.service /etc/systemd/system/octessera.service 0644
install_overlay_file etc/sudoers.d/octessera-update /etc/sudoers.d/octessera-update 0440
if [[ -d "$overlay_dir/usr/local/share/octessera-setup-ui" ]]; then
  cp -a "$overlay_dir/usr/local/share/octessera-setup-ui" /usr/local/share/
fi

if ! id octessera >/dev/null 2>&1; then
  useradd --create-home --shell /bin/bash --groups sudo octessera
fi
passwd -l octessera >/dev/null || true
install -d -m 0755 /etc/ssh/sshd_config.d
cat >/etc/ssh/sshd_config.d/10-octessera-setup.conf <<'EOF'
PermitRootLogin no
PasswordAuthentication no
AllowUsers octessera
EOF
systemctl disable ssh.service >/dev/null 2>&1 || true
systemctl mask ssh.service >/dev/null 2>&1 || true
systemctl disable ssh.socket >/dev/null 2>&1 || true
systemctl mask ssh.socket >/dev/null 2>&1 || true
if systemctl list-unit-files sshd.service >/dev/null 2>&1; then
  systemctl disable sshd.service >/dev/null 2>&1 || true
  systemctl mask sshd.service >/dev/null 2>&1 || true
fi
if systemctl list-unit-files sshd.socket >/dev/null 2>&1; then
  systemctl disable sshd.socket >/dev/null 2>&1 || true
  systemctl mask sshd.socket >/dev/null 2>&1 || true
fi
rm -f /etc/ssh/ssh_host_*
systemctl enable octessera-setup.service >/dev/null

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
      /*|..|../*|*/..|*/../*) echo "Unsafe payload path: $entry" >&2; exit 1 ;;
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
      if [[ -x /opt/octessera/octessera-pi && -f /etc/systemd/system/octessera.service ]]; then
        payload_version="$(jq -r '.version // empty' "$work/extract/octessera-payload.json")"
        payload_tag="$(jq -r '.tag // empty' "$work/extract/octessera-payload.json")"
        if [[ ! "$payload_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ || "$payload_tag" != "v$payload_version" ]]; then
          payload_version=0.0.0
          payload_tag=v0.0.0
        fi
        release_dir="/opt/octessera/releases/$payload_version"
        install -D -m 0755 /opt/octessera/octessera-pi "$release_dir/octessera-pi"
        cat >"$release_dir/update-manifest.json" <<EOF
{
  "schema_version": 1,
  "tag": "$payload_tag",
  "version": "$payload_version",
  "arch": "aarch64-unknown-linux-gnu",
  "binary": "octessera-pi",
  "platforms": ["orange-pi-zero-2w", "linux-aarch64-device"]
}
EOF
        ln -sfn "$release_dir" /opt/octessera/current
        ln -sfn /opt/octessera/current/octessera-pi /usr/local/bin/octessera-pi
        cat >/opt/octessera/update-state.json <<EOF
{
  "current": "$payload_version",
  "previous": null,
  "next": null,
  "updated_at": "1970-01-01T00:00:00Z",
  "release": {
    "tag": "$payload_tag",
    "version": "$payload_version",
    "arch": "aarch64-unknown-linux-gnu",
    "binary": "octessera-pi",
    "platforms": ["orange-pi-zero-2w", "linux-aarch64-device"]
  },
  "asset": null
}
EOF
        systemctl enable octessera.service >/dev/null
        sed -i 's/OCTESSERA_RUNTIME_ENABLED_DEFAULT=false/OCTESSERA_RUNTIME_ENABLED_DEFAULT=true/' /etc/octessera/build-metadata.env
      fi
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
