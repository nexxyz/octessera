#!/bin/bash
set -e

# Install runtime dependencies for Cell Symphony
apt-get update
apt-get install -y --no-install-recommends \
    libasound2 \
    alsa-utils \
    libusb-1.0-0 \
    device-tree-compiler \
    i2c-tools \
    spi-tools

# Enable I2C and SPI kernel modules at boot
grep -qxF "i2c-dev" /etc/modules || echo "i2c-dev" >> /etc/modules
