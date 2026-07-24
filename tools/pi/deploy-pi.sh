#!/bin/bash
# Deploy octessera to Pi Zero 2W
# Run this script ON the Pi after copying the repo

set -e

BOARD_PROFILE="${OCTESSERA_BOARD_PROFILE:-raspberry-pi-zero-2w}"
if [ "$BOARD_PROFILE" = orange-pi-zero-2w ]; then
    echo "Orange Pi profile is not supported by Raspberry Pi deployment; use the separate Armbian workflow." >&2
    exit 2
fi
if [ "$BOARD_PROFILE" != raspberry-pi-zero-2w ]; then
    echo "Raspberry Pi deployment accepts only raspberry-pi-zero-2w; got $BOARD_PROFILE." >&2
    exit 2
fi

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
    python3-minimal \
    unzip \
    util-linux \
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
if [ -f tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-usb-gadget ]; then
    sudo install -D -m 0755 \
        tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-usb-gadget \
        /usr/local/sbin/octessera-usb-gadget
    sudo install -D -m 0644 \
        tools/pi-image/stage4-octessera/files/root/etc/modules-load.d/octessera-usb-gadget.conf \
        /etc/modules-load.d/octessera-usb-gadget.conf
fi
if [ -f tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-usb-storage ]; then
    sudo install -D -m 0440 \
        tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-usb-storage \
        /etc/sudoers.d/octessera-usb-storage
    sudo visudo -cf /etc/sudoers.d/octessera-usb-storage >/dev/null
fi

# Install journald config for persistent capped logs
sudo install -d -m 0755 /etc/systemd/journald.conf.d
sudo tee /etc/systemd/journald.conf.d/10-octessera.conf > /dev/null <<'EOL'
[Journal]
Storage=persistent
RuntimeMaxUse=32M
RuntimeMaxFileSize=4M
SystemMaxUse=64M
SystemMaxFileSize=8M
EOL

sudo install -d -m 0755 /etc/NetworkManager/conf.d /usr/local/bin
sudo tee /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf > /dev/null <<'EOL'
[connection]
wifi.powersave = 2
EOL
sudo tee /usr/local/bin/octessera-network-health > /dev/null <<'EOL'
#!/bin/sh
set -eu

PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

LOG_DIR=/var/log/octessera
LOG_FILE="$LOG_DIR/network-health.log"
STATE_DIR=/run/octessera-network-health
FAIL_FILE="$STATE_DIR/fail-count"
RECOVERY_STAMP_FILE="$STATE_DIR/wifi-stack-recovery-at"

mkdir -p "$LOG_DIR" "$STATE_DIR"

timestamp=$(date -Is)
uptime_line=$(uptime | tr -s ' ')
gateway=$(ip route show default 0.0.0.0/0 | awk 'NR == 1 { print $3 }')
power_save=$(iw dev wlan0 get power_save 2>/dev/null | tr '\n' ' ' || true)
link=$(iw dev wlan0 link 2>/dev/null | tr '\n' ';' || true)
addr=$(ip -brief addr show wlan0 2>/dev/null | tr -s ' ' | tr '\n' ' ' || true)
route=$(ip route show default 2>/dev/null | tr '\n' ';' || true)
throttled=$(vcgencmd get_throttled 2>/dev/null || true)
temp=$(vcgencmd measure_temp 2>/dev/null || true)
ssh_active=$(systemctl is-active ssh 2>/dev/null || true)
nm_active=$(systemctl is-active NetworkManager 2>/dev/null || true)
wpa_active=$(systemctl is-active wpa_supplicant 2>/dev/null || true)

ping_ok=0
if [ -n "$gateway" ] && ping -I wlan0 -c 1 -W 2 "$gateway" >/dev/null 2>&1; then
    ping_ok=1
fi

printf '%s uptime="%s" gateway=%s ping_gateway=%s ssh=%s networkmanager=%s wpa=%s %s %s power_save="%s" addr="%s" route="%s" link="%s"\n' \
    "$timestamp" "$uptime_line" "${gateway:-none}" "$ping_ok" "$ssh_active" "$nm_active" "$wpa_active" \
    "$throttled" "$temp" "$power_save" "$addr" "$route" "$link" >> "$LOG_FILE"

if [ "$ping_ok" -eq 1 ] && [ "$nm_active" = active ]; then
    printf '0\n' > "$FAIL_FILE"
    exit 0
fi

fail_count=0
if [ -f "$FAIL_FILE" ]; then
    fail_count=$(cat "$FAIL_FILE" 2>/dev/null || printf '0')
fi
case "$fail_count" in
    ''|*[!0-9]*) fail_count=0 ;;
esac
fail_count=$((fail_count + 1))
printf '%s\n' "$fail_count" > "$FAIL_FILE"

if [ "$fail_count" -eq 2 ]; then
    printf '%s recovery=wifi_reconnect fail_count=%s gateway=%s\n' "$timestamp" "$fail_count" "${gateway:-none}" >> "$LOG_FILE"
    iw dev wlan0 set power_save off >/dev/null 2>&1 || true
    nmcli radio wifi on >/dev/null 2>&1 || true
    nmcli device connect wlan0 >/dev/null 2>&1 || true
    exit 0
fi

last_recovery=0
if [ -f "$RECOVERY_STAMP_FILE" ]; then
    last_recovery=$(cat "$RECOVERY_STAMP_FILE" 2>/dev/null || printf '0')
fi
case "$last_recovery" in
    ''|*[!0-9]*) last_recovery=0 ;;
esac
now_epoch=$(date +%s)

if [ "$fail_count" -ge 5 ] && [ $((now_epoch - last_recovery)) -ge 600 ]; then
    printf '%s recovery=wifi_stack_reset fail_count=%s gateway=%s\n' "$timestamp" "$fail_count" "${gateway:-none}" >> "$LOG_FILE"
    printf '%s\n' "$now_epoch" > "$RECOVERY_STAMP_FILE"
    printf '0\n' > "$FAIL_FILE"
    systemctl stop NetworkManager wpa_supplicant >/dev/null 2>&1 || true
    modprobe -r brcmfmac brcmutil >/dev/null 2>&1 || true
    sleep 2
    modprobe brcmfmac >/dev/null 2>&1 || true
    systemctl start wpa_supplicant NetworkManager >/dev/null 2>&1 || true
    iw dev wlan0 set power_save off >/dev/null 2>&1 || true
    nmcli radio wifi on >/dev/null 2>&1 || true
    nmcli device connect wlan0 >/dev/null 2>&1 || true
fi
EOL
sudo chmod 0755 /usr/local/bin/octessera-network-health
sudo tee /etc/systemd/system/octessera-network-health.service > /dev/null <<'EOL'
[Unit]
Description=octessera Network Health Logger
After=NetworkManager.service ssh.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/octessera-network-health
EOL
sudo tee /etc/systemd/system/octessera-network-health.timer > /dev/null <<'EOL'
[Unit]
Description=Run octessera network health checks

[Timer]
OnBootSec=2min
OnUnitActiveSec=1min
AccuracySec=15s
Persistent=true

[Install]
WantedBy=timers.target
EOL
sudo sed -i 's/\r$//' /usr/local/bin/octessera-network-health /etc/systemd/system/octessera-network-health.service /etc/systemd/system/octessera-network-health.timer /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf /etc/systemd/journald.conf.d/10-octessera.conf
sudo systemctl daemon-reload
sudo systemctl enable --now octessera-network-health.timer >/dev/null
sudo iw dev wlan0 set power_save off >/dev/null 2>&1 || true
sudo nmcli connection modify preconfigured 802-11-wireless.powersave 2 >/dev/null 2>&1 || true
sudo nmcli device reapply wlan0 >/dev/null 2>&1 || true

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

printf '\n'
cat <<'EOF'
                OOOO    OOOO
               OOOOO   OOOOOO
             OOO     OOO    OOO
           OOOO    OOOO       OOOO
         OOOO    OOOO   OOOO    OOOO
         OOOO    OOOO   OOOO    OOOO
            OOO       OOOO    OOO
              OOOO   OOO    OOOO
                OOOOOO   OOOOO
                 OOOO    OOOO

OOOO OOOO OOOOO OOOO OOOO OOOO OOOO OOOO OOOO
O  O O      O   O    O    O    O    O  O O  O
O  O O      O   OOOO OOOO OOOO OOOO OOOO OOOO
O  O O      O   O       O    O O    O OO O  O
OOOO OOOO   O   OOOO OOOO OOOO OOOO O  O O  O
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
cargo build --release -p octessera-pi --features hardware-raspberry-pi-zero-2w

sudo install -d -m 0755 /etc/octessera
printf 'OCTESSERA_BOARD_PROFILE_ID=%s\n' "$BOARD_PROFILE" | sudo tee /etc/octessera/board-profile.env >/dev/null

PACKAGE_VERSION=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(next(package["version"] for package in json.load(sys.stdin)["packages"] if package["name"] == "octessera-pi"))')
if ! printf '%s\n' "$PACKAGE_VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "Invalid octessera-pi package version: $PACKAGE_VERSION" >&2
    exit 1
fi
RELEASE_DIR="/opt/octessera/releases/$PACKAGE_VERSION"
if [ -e "$RELEASE_DIR" ]; then
    echo "Managed release already exists at $RELEASE_DIR; refusing to overwrite it." >&2
    exit 1
fi
if [ -e /opt/octessera/current ] && [ ! -L /opt/octessera/current ]; then
    echo "Existing /opt/octessera/current is unmanaged; refusing deployment." >&2
    exit 1
fi
if [ -e /usr/local/bin/octessera-pi ] && [ ! -L /usr/local/bin/octessera-pi ]; then
    echo "Existing /usr/local/bin/octessera-pi is unmanaged; refusing deployment." >&2
    exit 1
fi
sudo install -d -m 0755 /opt/octessera/releases
sudo install -D -m 0755 target/release/octessera-pi "$RELEASE_DIR/octessera-pi"
sudo tee "$RELEASE_DIR/update-manifest.json" >/dev/null <<EOL
{
  "schema_version": 2,
  "updater_protocol": 2,
  "candidate_health_protocol": 1,
  "tag": "v$PACKAGE_VERSION",
  "version": "$PACKAGE_VERSION",
  "board_profile": "$BOARD_PROFILE",
  "arch": "aarch64-unknown-linux-gnu",
  "binary": "octessera-pi",
  "platforms": ["$BOARD_PROFILE", "linux-aarch64-device"]
}
EOL
sudo chmod -R a-w "$RELEASE_DIR"
sudo ln -sfn "$RELEASE_DIR" /opt/octessera/current
sudo ln -sfn /opt/octessera/current/octessera-pi /usr/local/bin/octessera-pi
sudo tee /opt/octessera/update-state.json >/dev/null <<EOL
{
  "schema_version": 2,
  "phase": "committed",
  "current": "$PACKAGE_VERSION",
  "previous": null,
  "updated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "release": {
    "schema_version": 2,
    "updater_protocol": 2,
    "candidate_health_protocol": 1,
    "tag": "v$PACKAGE_VERSION",
    "version": "$PACKAGE_VERSION",
    "board_profile": "$BOARD_PROFILE",
    "arch": "aarch64-unknown-linux-gnu",
    "binary": "octessera-pi",
    "platforms": ["$BOARD_PROFILE", "linux-aarch64-device"]
  },
  "asset": null
}
EOL

UPDATE_SOURCE=/home/pi/octessera/tools/device-update
for file in updater_protocol.py updater_state.py updater_assets.py updater_guard.py updater_cli.py; do
    sudo install -D -m 0644 "$UPDATE_SOURCE/$file" "/usr/local/lib/octessera/$file"
done
for file in octessera-update octessera-update-guard octessera-update-recovery; do
    sudo install -D -m 0755 "/home/pi/octessera/tools/pi-image/stage4-octessera/files/root/usr/local/sbin/$file" "/usr/local/sbin/$file"
done
for file in octessera-update-guard.service octessera-update-recovery.service; do
    sudo install -D -m 0644 "/home/pi/octessera/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/$file" "/etc/systemd/system/$file"
done
sudo install -D -m 0440 /home/pi/octessera/tools/pi-image/stage4-octessera/files/root/etc/sudoers.d/octessera-update /etc/sudoers.d/octessera-update
sudo visudo -cf /etc/sudoers.d/octessera-update >/dev/null

# Create systemd service
echo "Creating systemd service..."
sudo tee /etc/systemd/system/octessera.service > /dev/null <<EOL
[Unit]
Description=octessera Raspberry Pi Zero 2W ($BOARD_PROFILE)
After=sound.target
Requires=octessera-update-recovery.service
After=octessera-update-recovery.service

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/octessera
EnvironmentFile=-/etc/octessera/board-profile.env
Environment=OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w
Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json
RuntimeDirectory=octessera
RuntimeDirectoryMode=0755
ExecStartPre=/bin/sleep 2
ExecStart=/usr/local/bin/octessera-pi
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOL

sudo install -d -m 0755 /etc/systemd/system/octessera.service.d
sudo tee /etc/systemd/system/octessera.service.d/audio-realtime.conf > /dev/null <<'EOL'
[Service]
AmbientCapabilities=CAP_SYS_NICE
CapabilityBoundingSet=CAP_SYS_NICE
LimitRTPRIO=80
LimitMEMLOCK=infinity
Nice=-10
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
sudo systemctl enable --now octessera-update-recovery.service
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
