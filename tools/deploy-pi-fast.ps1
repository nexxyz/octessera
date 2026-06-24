param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\cellsymphony_pi_dev",
  [string]$RemoteRepo = "/home/pi/cellsymphony-dev",
  [string]$InstallDir = "/opt/cellsymphony",
  [string]$Service = "cellsymphony.service",
  [string]$LocalBinary = "",
  [string]$BuildProfile = "pi-dev",
  [switch]$BuildOnPi,
  [switch]$CleanRemote,
  [switch]$SyncOnly,
  [switch]$SkipBuild,
  [switch]$AllowServiceFailure,
  [switch]$NoTail
)

$ErrorActionPreference = "Stop"

$sshArgs = @("-i", $Key, "-o", "IdentitiesOnly=yes", $Target)

function Invoke-PiSsh {
  param([string]$Command)
  ssh @sshArgs $Command
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
  Copy-ToPi $LocalBinary "/tmp/cellsymphony-pi"
  Invoke-PiSsh "set -e; sudo install -d '$InstallDir/releases/dev'; sudo install -m 755 /tmp/cellsymphony-pi '$InstallDir/releases/dev/cellsymphony-pi'; rm -f /tmp/cellsymphony-pi"
} else {
  $archive = Join-Path $env:TEMP "cellsymphony-pi-source.tar.gz"
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
    Invoke-PiSsh "set -e; . `$HOME/.cargo/env; cd '$RemoteRepo'; CARGO_BUILD_JOBS=1 cargo build --profile '$BuildProfile' -p cellsymphony-pi --features hardware-pi; sudo install -d '$InstallDir/releases/dev'; sudo install -m 755 target/$BuildProfile/cellsymphony-pi '$InstallDir/releases/dev/cellsymphony-pi'"
  }
}

$osConfigCommand = @'
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
sudo install -d -m 0755 /etc/systemd/journald.conf.d
sudo tee /etc/systemd/journald.conf.d/10-cellsymphony.conf >/dev/null <<'EOF'
[Journal]
Storage=volatile
RuntimeMaxUse=32M
RuntimeMaxFileSize=4M
EOF
sudo systemctl restart systemd-journald
disable_service_if_present bluetooth.service
disable_service_if_present hciuart.service
sudo tee /etc/systemd/system/cellsymphony-performance-governor.service >/dev/null <<'EOF'
[Unit]
Description=Cell Symphony Performance CPU Governor
Before=cellsymphony.service

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -e "$gov" ] || continue; printf performance > "$gov" || true; done'
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable cellsymphony-performance-governor.service
sudo systemctl start cellsymphony-performance-governor.service
'@

Invoke-PiSsh $osConfigCommand

$serviceCommand = "set -e; sudo install -d '$InstallDir'; sudo ln -sfn '$InstallDir/releases/dev' '$InstallDir/current'; sudo ln -sfn '$InstallDir/current/cellsymphony-pi' /usr/local/bin/cellsymphony-pi; sudo tee /etc/systemd/system/$Service >/dev/null <<'EOF'
[Unit]
Description=Cell Symphony Pi Zero 2W Headless Music System
After=sound.target

[Service]
Type=simple
User=pi
WorkingDirectory=$RemoteRepo
ExecStartPre=/bin/sleep 2
ExecStart=/usr/local/bin/cellsymphony-pi
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload; sudo systemctl enable '$Service'; sudo systemctl restart '$Service'; systemctl --no-pager --lines=8 status '$Service'"

if ($AllowServiceFailure) {
  Invoke-PiSsh "$serviceCommand || true"
} else {
  Invoke-PiSsh $serviceCommand
}

if (-not $NoTail) {
  ssh @sshArgs "journalctl -u '$Service' -f"
}
