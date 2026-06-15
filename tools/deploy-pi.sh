#!/bin/bash
# Deploy Cell Symphony to Pi Zero 2W
# Run this script ON the Pi after copying the repo

set -e

echo "=== Cell Symphony Pi Deployment ==="

# Install system dependencies
echo "Installing system dependencies..."
sudo apt-get update
sudo apt-get install -y \
    libasound2-dev \
    pkg-config \
    alsa-utils \
    git \
    curl \
    build-essential

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Enable I2S audio
echo "Enabling I2S audio..."
if ! grep -q "dtparam=i2s=on" /boot/config.txt; then
    echo "dtparam=i2s=on" | sudo tee -a /boot/config.txt
fi

if ! grep -q "dtoverlay=hifiberry-dac" /boot/config.txt; then
    echo "dtoverlay=hifiberry-dac" | sudo tee -a /boot/config.txt
fi

# Build natively on Pi (simpler than cross-compilation)
echo "Building Cell Symphony for Pi..."
cd /home/pi/cellsymphony
cargo build --release -p cellsymphony-pi --features hardware-pi

# Create systemd service
echo "Creating systemd service..."
sudo tee /etc/systemd/system/cellsymphony.service > /dev/null <<EOL
[Unit]
Description=Cell Symphony Pi Zero 2W
After=sound.target network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/cellsymphony
ExecStartPre=/bin/sleep 2
ExecStart=/home/pi/cellsymphony/target/release/cellsymphony-pi
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOL

# Enable service
sudo systemctl enable cellsymphony
sudo systemctl daemon-reload

echo ""
echo "=== Deployment complete! ==="
echo "Reboot to enable I2S audio, then the service will auto-start."
echo "Check status with: sudo systemctl status cellsymphony"
echo "View logs with: journalctl -u cellsymphony -f"
echo ""
echo "REBOOT NOW? (y/n)"
read -r answer
if [ "$answer" = "y" ]; then
    sudo reboot
fi
