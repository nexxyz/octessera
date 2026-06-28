#!/bin/bash
set -e

apt-get update
apt-get install -y --no-install-recommends \
    libasound2 \
    alsa-utils \
    libusb-1.0-0 \
    device-tree-compiler \
    i2c-tools \
    spi-tools

grep -qxF "i2c-dev" /etc/modules || echo "i2c-dev" >> /etc/modules
