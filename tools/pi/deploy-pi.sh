#!/bin/bash
# Deploy octessera to Pi Zero 2W
# Run this script ON the Pi after copying the repo

set -e

echo "=== octessera Pi Deployment ==="

# Install system dependencies
echo "Installing system dependencies..."
sudo apt-get update
sudo apt-get install -y \
    libasound2-dev \
    pkg-config \
    alsa-utils \
    device-tree-compiler \
    i2c-tools \
    spi-tools \
    git \
    curl \
    build-essential

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Configure Pi buses and pin muxes
echo "Configuring Pi buses and pin muxes..."
BOOT_CONFIG="/boot/firmware/config.txt"
if [ ! -f "$BOOT_CONFIG" ]; then
    BOOT_CONFIG="/boot/config.txt"
fi

ensure_boot_config_line() {
    local line="$1"
    if ! grep -qxF "$line" "$BOOT_CONFIG"; then
        echo "$line" | sudo tee -a "$BOOT_CONFIG" > /dev/null
    fi
}

disable_service_if_present() {
    local service="$1"
    sudo systemctl disable --now "$service" >/dev/null 2>&1 || true
}

ensure_boot_config_line "dtparam=audio=off"
ensure_boot_config_line "camera_auto_detect=0"
ensure_boot_config_line "display_auto_detect=0"
ensure_boot_config_line "dtoverlay=disable-bt"
sudo tee /boot/firmware/overlays/i2s-dac-no20.dts > /dev/null <<'EOL'
/dts-v1/;
/plugin/;

/ {
    compatible = "brcm,bcm2835";

    fragment@0 {
        target = <&gpio>;
        __overlay__ {
            i2s_nodin_pins: i2s_nodin_pins {
                brcm,pins = <18 19 21>;
                brcm,function = <4>;
            };
        };
    };

    fragment@1 {
        target = <&i2s>;
        __overlay__ {
            pinctrl-names = "default";
            pinctrl-0 = <&i2s_nodin_pins>;
            status = "okay";
        };
    };

    fragment@2 {
        target-path = "/";
        __overlay__ {
            pcm5102a-codec {
                #sound-dai-cells = <0>;
                compatible = "ti,pcm5102a";
                status = "okay";
            };
        };
    };

    fragment@3 {
        target = <&sound>;
        __overlay__ {
            compatible = "hifiberry,hifiberry-dac";
            i2s-controller = <&i2s>;
            status = "okay";
        };
    };
};
EOL
sudo dtc -@ -I dts -O dtb \
    -o /boot/firmware/overlays/i2s-dac-no20.dtbo \
    /boot/firmware/overlays/i2s-dac-no20.dts
sudo sed -i -E 's/^dtoverlay=hifiberry-dac/#dtoverlay=hifiberry-dac/; s/^dtoverlay=i2s-no-gpio20/#dtoverlay=i2s-no-gpio20/' "$BOOT_CONFIG"
ensure_boot_config_line "dtoverlay=i2s-dac-no20"
ensure_boot_config_line "dtparam=spi=on"
ensure_boot_config_line "dtparam=i2c_arm=on"
ensure_boot_config_line "enable_uart=0"
grep -qxF "i2c-dev" /etc/modules || echo "i2c-dev" | sudo tee -a /etc/modules > /dev/null

# Install journald config for volatile capped logs
sudo install -d -m 0755 /etc/systemd/journald.conf.d
sudo tee /etc/systemd/journald.conf.d/10-octessera.conf > /dev/null <<'EOL'
[Journal]
Storage=volatile
RuntimeMaxUse=32M
RuntimeMaxFileSize=4M
EOL

sudo install -d -m 0755 /etc/profile.d
sudo tee /etc/profile.d/octessera-welcome.sh > /dev/null <<'EOL'
case $- in
    *i*) ;;
    *) return 0 ;;
esac

if [ -n "${OCTESSERA_WELCOME_SHOWN:-}" ]; then
    return 0
fi
export OCTESSERA_WELCOME_SHOWN=1

if [ ! -t 1 ]; then
    return 0
fi

cat <<'EOF'
                          ████    ████
                         █████   ██████
                       ███     ███    ███
                     ████    ████       ████
                   ████    ████   ████    ████
                   ████    ████   ████    ████
                      ███       ████    ███
                        ████   ███    ████
                          ██████   █████
                           ████    ████

      █████ █████ ██████ █████ █████ █████ █████ █████ █████
      █   █ █       ██   █     █     █     █     █   █ █   █
      █   █ █       ██   █████ █████ █████ █████ █████ █████
      █   █ █       ██   █         █     █ █     █  ██ █   █
      █████ █████   ██   █████ █████ █████ █████ █   █ █   █
EOF
printf '\n  cellular automata -> music\n'
printf '  service: systemctl status octessera\n'
printf '  logs:    journalctl -u octessera -f\n\n'
EOL
sudo systemctl restart systemd-journald

# Disable Bluetooth services when present
disable_service_if_present bluetooth.service
disable_service_if_present hciuart.service

# Build natively on Pi (simpler than cross-compilation)
echo "Building octessera for Pi..."
cd /home/pi/octessera
cargo build --release -p octessera-pi --features hardware-pi

# Create systemd service
echo "Creating systemd service..."
sudo tee /etc/systemd/system/octessera.service > /dev/null <<EOL
[Unit]
Description=octessera Pi Zero 2W
After=sound.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/octessera
ExecStartPre=/bin/sleep 2
ExecStart=/home/pi/octessera/target/release/octessera-pi
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOL

sudo tee /etc/systemd/system/octessera-performance-governor.service > /dev/null <<'EOL'
[Unit]
Description=octessera Performance CPU Governor
Before=octessera.service

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -e "$gov" ] || continue; printf performance > "$gov" || true; done'
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOL

# Enable service
sudo systemctl daemon-reload
sudo systemctl enable octessera
sudo systemctl enable octessera-performance-governor.service
sudo systemctl start octessera-performance-governor.service

echo ""
echo "=== Deployment complete! ==="
echo "Reboot to enable I2S audio, then the service will auto-start."
echo "Check status with: sudo systemctl status octessera"
echo "View logs with: journalctl -u octessera -f"
echo ""
echo "REBOOT NOW? (y/n)"
read -r answer
if [ "$answer" = "y" ]; then
    sudo reboot
fi
