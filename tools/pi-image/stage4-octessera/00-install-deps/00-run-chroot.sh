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
    spi-tools \
    unzip \
    util-linux

grep -qxF "i2c-dev" /etc/modules || echo "i2c-dev" >> /etc/modules
grep -qxF "spi-bcm2835" /etc/initramfs-tools/modules || echo "spi-bcm2835" >> /etc/initramfs-tools/modules
grep -qxF "spidev" /etc/initramfs-tools/modules || echo "spidev" >> /etc/initramfs-tools/modules
