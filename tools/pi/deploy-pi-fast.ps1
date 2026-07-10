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
if [ "$UPDATE_INITRAMFS" = "1" ]; then
if ! grep -qxF "# Octessera required boot settings" "$BOOT_CONFIG"; then printf '\n[all]\n# Octessera required boot settings\ndtparam=spi=on\nauto_initramfs=1\n' | sudo tee -a "$BOOT_CONFIG" >/dev/null; fi
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
Storage=volatile
RuntimeMaxUse=32M
RuntimeMaxFileSize=4M
EOF
sudo install -d -m 0750 /etc/sudoers.d
sudo tee /etc/sudoers.d/octessera-shutdown >/dev/null <<'EOF'
pi ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /bin/systemctl poweroff, /usr/sbin/poweroff, /sbin/poweroff, /usr/bin/systemctl reboot, /bin/systemctl reboot, /usr/sbin/reboot, /sbin/reboot
EOF
sudo chmod 0440 /etc/sudoers.d/octessera-shutdown
sudo visudo -cf /etc/sudoers.d/octessera-shutdown >/dev/null
sudo systemctl restart systemd-journald
disable_service_if_present bluetooth.service
disable_service_if_present hciuart.service
sudo tee /etc/systemd/system/octessera-performance-governor.service >/dev/null <<'EOF'
[Unit]
Description=Octessera Performance CPU Governor
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
Description=Octessera Late OLED Shutdown

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
Description=Octessera Early OLED Boot Splash
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
Description=Octessera Pi Zero 2W Headless Music System
After=sound.target

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
