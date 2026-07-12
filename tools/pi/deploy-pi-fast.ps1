param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$RemoteRepo = "/home/pi/octessera-dev",
  [string]$InstallDir = "/opt/octessera",
  [string]$Service = "octessera.service",
  [string]$LocalBinary = "",
  [string]$BuildProfile = "pi-dev",
  [ValidateSet("rpi-zero-2w")]
  [string]$BoardProfile = "rpi-zero-2w",
  [switch]$BuildOnPi,
  [switch]$CleanRemote,
  [switch]$SyncOnly,
  [switch]$SkipBuild,
  [switch]$UpdateInitramfs,
  [switch]$WakeTrace,
  [switch]$AllowServiceFailure,
  [switch]$NoTail
)

$ErrorActionPreference = "Stop"

$sshArgs = @("-i", $Key, "-o", "IdentitiesOnly=yes", $Target)

function Invoke-PiSsh {
  param([string]$Command)
  if ($Command.Contains("`n")) {
    $Command | ssh @sshArgs "tr -d '\r' | bash -s"
  } else {
    ssh @sshArgs $Command
  }
  if ($LASTEXITCODE -ne 0) {
    throw "ssh command failed with exit code $LASTEXITCODE"
  }
}

function Copy-ToPi {
  param([string]$Source, [string]$Destination)
  scp -i $Key -o IdentitiesOnly=yes $Source "${Target}:$Destination"
  if ($LASTEXITCODE -ne 0) {
    throw "scp failed with exit code $LASTEXITCODE"
  }
}

if ($LocalBinary -ne "") {
  Copy-ToPi $LocalBinary "/tmp/octessera-pi"
  Invoke-PiSsh "set -e; sudo install -d '$InstallDir/releases/dev'; sudo install -m 755 /tmp/octessera-pi '$InstallDir/releases/dev/octessera-pi'; sudo ln -sfn '$InstallDir/releases/dev' '$InstallDir/current'; sudo ln -sfn '$InstallDir/current/octessera-pi' /usr/local/bin/octessera-pi; rm -f /tmp/octessera-pi"
} else {
  $archive = Join-Path $env:TEMP "octessera-pi-source.tar.gz"
  if (Test-Path $archive) {
    Remove-Item $archive -Force
  }

  tar `
    --exclude .git `
    --exclude .github `
    --exclude .slim `
    --exclude .opencode `
    --exclude .opencode/node_modules `
    --exclude node_modules `
    --exclude target `
    --exclude dist `
    --exclude '*/dist' `
    --exclude coverage `
    --exclude '*/coverage' `
    --exclude hardware `
    --exclude apps/desktop/dist `
    --exclude apps/desktop/dist-desktop `
    --exclude apps/desktop/coverage `
    -czf $archive `
    -C (Resolve-Path ".") .

  $remoteArchive = "$RemoteRepo-source.tar.gz"
  Copy-ToPi $archive $remoteArchive
  if ($CleanRemote) {
    Invoke-PiSsh "set -e; rm -rf '$RemoteRepo'; mkdir -p '$RemoteRepo'; tar -xzf '$remoteArchive' -C '$RemoteRepo'; rm -f '$remoteArchive'"
  } else {
    $syncCommand = @"
set -e
SYNC_DIR='$RemoteRepo-sync'
rm -rf "`$SYNC_DIR"
mkdir -p "`$SYNC_DIR" '$RemoteRepo'
tar -xzf '$remoteArchive' -C "`$SYNC_DIR"
if command -v rsync >/dev/null 2>&1; then
  rsync -a --checksum --delete --exclude target/ "`$SYNC_DIR"/ '$RemoteRepo'/
else
  echo "warning: rsync not found; falling back to tar extraction, Cargo cache fingerprints may be invalidated" >&2
  tar -xzf '$remoteArchive' -C '$RemoteRepo'
fi
rm -rf "`$SYNC_DIR" '$remoteArchive'
"@
    Invoke-PiSsh $syncCommand
  }

  if ($SyncOnly -or -not $BuildOnPi) {
    exit 0
  }

  if (-not $SkipBuild) {
    Invoke-PiSsh "set -e; . `$HOME/.cargo/env; cd '$RemoteRepo'; CARGO_BUILD_JOBS=1 cargo build --profile '$BuildProfile' -p octessera-pi --features hardware-rpi-zero-2w; sudo install -d '$InstallDir/releases/dev'; sudo install -m 755 target/$BuildProfile/octessera-pi '$InstallDir/releases/dev/octessera-pi'"
  }
}

$updateInitramfsValue = if ($UpdateInitramfs) { "1" } else { "0" }
$wakeTraceEnvironmentLine = if ($WakeTrace) { "Environment=OCTESSERA_WAKE_TRACE=1`n" } else { "" }
$piImageFiles = "tools/pi-image/stage4-octessera/files/root"
Copy-ToPi "$piImageFiles/usr/local/sbin/octessera-usb-gadget" "/tmp/octessera-usb-gadget"
Copy-ToPi "$piImageFiles/etc/systemd/system/octessera-usb-gadget.service" "/tmp/octessera-usb-gadget.service"
Copy-ToPi "$piImageFiles/etc/modules-load.d/octessera-usb-gadget.conf" "/tmp/octessera-usb-gadget.conf"

$osConfigCommand = "UPDATE_INITRAMFS=$updateInitramfsValue`n" + @'
set -e
BOOT_CONFIG="/boot/firmware/config.txt"
if [ ! -f "$BOOT_CONFIG" ]; then
  BOOT_CONFIG="/boot/config.txt"
fi
ensure_boot_config_line() {
  line="$1"
  if ! grep -qxF "$line" "$BOOT_CONFIG"; then
    echo "$line" | sudo tee -a "$BOOT_CONFIG" >/dev/null
  fi
}
disable_service_if_present() {
  service="$1"
  sudo systemctl disable --now "$service" >/dev/null 2>&1 || true
}
ensure_boot_config_line "camera_auto_detect=0"
ensure_boot_config_line "display_auto_detect=0"
ensure_boot_config_line "dtoverlay=disable-bt"
ensure_boot_config_line "dtoverlay=dwc2,dr_mode=peripheral"
sudo rm -f \
  /etc/initramfs-tools/hooks/cellsymphony-boot-splash \
  /etc/initramfs-tools/scripts/init-premount/cellsymphony-boot-splash \
  /etc/systemd/system/cellsymphony-boot-splash.service \
  /etc/systemd/system/sysinit.target.wants/cellsymphony-boot-splash.service
sudo install -D -m 0755 /tmp/octessera-usb-gadget /usr/local/sbin/octessera-usb-gadget
sudo install -D -m 0644 /tmp/octessera-usb-gadget.service /etc/systemd/system/octessera-usb-gadget.service
sudo install -D -m 0644 /tmp/octessera-usb-gadget.conf /etc/modules-load.d/octessera-usb-gadget.conf
sudo systemctl daemon-reload
sudo systemctl enable octessera-usb-gadget.service >/dev/null
if [ "$UPDATE_INITRAMFS" = "1" ]; then
if ! grep -qxF "# octessera required boot settings" "$BOOT_CONFIG" && ! grep -qxF "# Octessera required boot settings" "$BOOT_CONFIG"; then printf '\n[all]\n# octessera required boot settings\ndtparam=spi=on\nauto_initramfs=1\n' | sudo tee -a "$BOOT_CONFIG" >/dev/null; fi
if ! command -v update-initramfs >/dev/null 2>&1; then
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends initramfs-tools
fi
sudo install -d -m 0755 /etc/initramfs-tools/hooks /etc/initramfs-tools/scripts/init-premount
sudo tee /etc/initramfs-tools/hooks/octessera-boot-splash >/dev/null <<'EOF'
#!/bin/sh
set -e

PREREQ=""

prereqs() {
    echo "$PREREQ"
}

case "$1" in
    prereqs)
        prereqs
        exit 0
        ;;
esac

. /usr/share/initramfs-tools/hook-functions

copy_exec /usr/local/bin/octessera-pi /usr/local/bin/octessera-pi
manual_add_modules spi-bcm2835 || true
manual_add_modules spidev || true
EOF
sudo tee /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash >/dev/null <<'EOF'
#!/bin/sh
set -e

PREREQ=""

prereqs() {
    echo "$PREREQ"
}

case "$1" in
    prereqs)
        prereqs
        exit 0
        ;;
esac

modprobe spi-bcm2835 >/dev/null 2>&1 || true
modprobe spidev >/dev/null 2>&1 || true

if [ -x /usr/local/bin/octessera-pi ]; then
    /usr/local/bin/octessera-pi --boot-splash-once >/dev/kmsg 2>&1 &
    splash_pid="$!"
    (sleep 2; kill "$splash_pid" >/dev/null 2>&1 || true) &
    watchdog_pid="$!"
    wait "$splash_pid" >/dev/null 2>&1 || true
    kill "$watchdog_pid" >/dev/null 2>&1 || true
fi
EOF
sudo chmod 0755 /etc/initramfs-tools/hooks/octessera-boot-splash /etc/initramfs-tools/scripts/init-premount/octessera-boot-splash
grep -qxF "spi-bcm2835" /etc/initramfs-tools/modules || echo "spi-bcm2835" | sudo tee -a /etc/initramfs-tools/modules >/dev/null
grep -qxF "spidev" /etc/initramfs-tools/modules || echo "spidev" | sudo tee -a /etc/initramfs-tools/modules >/dev/null
sudo update-initramfs -u
else
  echo "Skipping initramfs update. Pass -UpdateInitramfs to refresh the early boot splash initramfs."
fi
sudo install -d -m 0755 /etc/systemd/journald.conf.d
sudo tee /etc/systemd/journald.conf.d/10-octessera.conf >/dev/null <<'EOF'
[Journal]
Storage=persistent
RuntimeMaxUse=32M
RuntimeMaxFileSize=4M
SystemMaxUse=64M
SystemMaxFileSize=8M
EOF
sudo install -d -m 0755 /etc/NetworkManager/conf.d /usr/local/bin
sudo tee /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf >/dev/null <<'EOF'
[connection]
wifi.powersave = 2
EOF
sudo tee /usr/local/bin/octessera-network-health >/dev/null <<'EOF'
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
EOF
sudo chmod 0755 /usr/local/bin/octessera-network-health
sudo tee /etc/systemd/system/octessera-network-health.service >/dev/null <<'EOF'
[Unit]
Description=octessera Network Health Logger
After=NetworkManager.service ssh.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/octessera-network-health
EOF
sudo tee /etc/systemd/system/octessera-network-health.timer >/dev/null <<'EOF'
[Unit]
Description=Run octessera network health checks

[Timer]
OnBootSec=2min
OnUnitActiveSec=1min
AccuracySec=15s
Persistent=true

[Install]
WantedBy=timers.target
EOF
sudo sed -i 's/\r$//' /usr/local/bin/octessera-network-health /etc/systemd/system/octessera-network-health.service /etc/systemd/system/octessera-network-health.timer /etc/NetworkManager/conf.d/10-octessera-wifi-powersave.conf /etc/systemd/journald.conf.d/10-octessera.conf
sudo systemctl daemon-reload
sudo systemctl enable --now octessera-network-health.timer >/dev/null
sudo iw dev wlan0 set power_save off >/dev/null 2>&1 || true
sudo nmcli connection modify preconfigured 802-11-wireless.powersave 2 >/dev/null 2>&1 || true
sudo nmcli device reapply wlan0 >/dev/null 2>&1 || true
sudo install -d -m 0750 /etc/sudoers.d
sudo tee /etc/sudoers.d/octessera-shutdown >/dev/null <<'EOF'
pi ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /bin/systemctl poweroff, /usr/sbin/poweroff, /sbin/poweroff, /usr/bin/systemctl reboot, /bin/systemctl reboot, /usr/sbin/reboot, /sbin/reboot
EOF
sudo chmod 0440 /etc/sudoers.d/octessera-shutdown
sudo visudo -cf /etc/sudoers.d/octessera-shutdown >/dev/null
sudo install -d -m 0755 /etc/profile.d
sudo tee /etc/profile.d/octessera-welcome.sh >/dev/null <<'EOF'
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
cat <<'EOM'
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

      OOOOO OOOOO OOOOOO OOOOO OOOOO OOOOO OOOOO OOOOO OOOOO
      O   O O       OO   O     O     O     O     O   O O   O
      O   O O       OO   OOOOO OOOOO OOOOO OOOOO OOOOO OOOOO
      O   O O       OO   O         O     O O     O  OO O   O
      OOOOO OOOOO   OO   OOOOO OOOOO OOOOO OOOOO O   O O   O
EOM
printf '\n  cellular automata -> music\n'
printf '  service: systemctl status octessera\n'
printf '  logs:    journalctl -u octessera -f\n\n'
EOF
sudo systemctl restart systemd-journald
disable_service_if_present bluetooth.service
disable_service_if_present hciuart.service
sudo tee /etc/systemd/system/octessera-performance-governor.service >/dev/null <<'EOF'
[Unit]
Description=octessera Performance CPU Governor
Before=octessera.service

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -e "$gov" ] || continue; printf performance > "$gov" || true; done'
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF
sudo tee /etc/systemd/system/octessera-oled-shutdown.service >/dev/null <<'EOF'
[Unit]
Description=octessera Late OLED Shutdown

[Service]
Type=oneshot
ExecStart=/bin/true
ExecStop=/bin/sh -c 'sleep 4; /usr/local/bin/octessera-pi --oled-off-once || true'
RemainAfterExit=yes
TimeoutStopSec=8
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable octessera-performance-governor.service
sudo systemctl start octessera-performance-governor.service
'@

Invoke-PiSsh $osConfigCommand

$serviceCommand = "set -e; sudo install -d '$InstallDir'; sudo ln -sfn '$InstallDir/releases/dev' '$InstallDir/current'; sudo ln -sfn '$InstallDir/current/octessera-pi' /usr/local/bin/octessera-pi; sudo tee /etc/systemd/system/octessera-boot-splash.service >/dev/null <<'EOF'
[Unit]
Description=octessera Early OLED Boot Splash
DefaultDependencies=no
After=systemd-modules-load.service systemd-udevd.service
Before=sysinit.target octessera.service

[Service]
Type=oneshot
ExecStart=-/usr/local/bin/octessera-pi --boot-splash-once
TimeoutStartSec=2
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=sysinit.target
EOF
sudo tee /etc/systemd/system/$Service >/dev/null <<'EOF'
[Unit]
Description=octessera Pi Zero 2W Headless Music System
Wants=octessera-usb-gadget.service
After=octessera-usb-gadget.service sound.target

[Service]
Type=simple
User=pi
WorkingDirectory=$RemoteRepo
Environment=OCTESSERA_EARLY_BOOT_SPLASH=1
${wakeTraceEnvironmentLine}ExecStart=/usr/local/bin/octessera-pi
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload; sudo systemctl disable octessera-boot-splash.service >/dev/null 2>&1 || true; sudo systemctl enable octessera-oled-shutdown.service '$Service'; sudo systemctl start octessera-oled-shutdown.service; sudo systemctl restart '$Service'; systemctl --no-pager --lines=8 status '$Service'"

if ($AllowServiceFailure) {
  Invoke-PiSsh "$serviceCommand || true"
} else {
  Invoke-PiSsh $serviceCommand
}

if (-not $NoTail) {
  ssh @sshArgs "journalctl -u '$Service' -f"
}
