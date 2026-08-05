#!/bin/bash
set -e

apt-get update
apt-get install -y --no-install-recommends \
    libasound2 \
    alsa-utils \
    libusb-1.0-0 \
    ca-certificates \
    coreutils \
    curl \
    device-tree-compiler \
    initramfs-tools \
    i2c-tools \
    jq \
    python3-minimal \
    openssh-server \
    spi-tools \
    network-manager \
    dnsmasq \
    wireless-tools \
    iw \
    tar \
    unzip \
    util-linux

wifi_connect_version=4.11.84
wifi_connect_sha256=413d70e6d1c1366cbe2b32555e8476f3e92878178ed1b9c82205985f055f1936
wifi_connect_url="https://github.com/balena-os/wifi-connect/releases/download/v${wifi_connect_version}/wifi-connect-aarch64-unknown-linux-gnu.tar.gz"
wifi_work=$(mktemp -d)
trap 'rm -rf "$wifi_work"' EXIT
curl --fail --location --proto '=https' --tlsv1.2 --output "$wifi_work/wifi-connect.tar.gz" "$wifi_connect_url"
echo "$wifi_connect_sha256  $wifi_work/wifi-connect.tar.gz" | sha256sum -c -
tar -xzf "$wifi_work/wifi-connect.tar.gz" -C "$wifi_work"
test -f "$wifi_work/wifi-connect"
install -D -o root -g root -m 0755 "$wifi_work/wifi-connect" /usr/local/bin/wifi-connect

grep -qxF "i2c-dev" /etc/modules || echo "i2c-dev" >> /etc/modules
grep -qxF "spi-bcm2835" /etc/initramfs-tools/modules || echo "spi-bcm2835" >> /etc/initramfs-tools/modules
grep -qxF "spidev" /etc/initramfs-tools/modules || echo "spidev" >> /etc/initramfs-tools/modules
