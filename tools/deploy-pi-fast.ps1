param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\cellsymphony_pi_dev",
  [string]$RemoteRepo = "/home/pi/cellsymphony-dev",
  [string]$InstallDir = "/opt/cellsymphony",
  [string]$Service = "cellsymphony.service",
  [string]$LocalBinary = "",
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
    --exclude .opencode/node_modules `
    --exclude node_modules `
    --exclude target `
    --exclude apps/desktop/dist-desktop `
    -czf $archive `
    -C (Resolve-Path ".") .

  Copy-ToPi $archive "/tmp/cellsymphony-pi-source.tar.gz"
  Invoke-PiSsh "set -e; rm -rf '$RemoteRepo'; mkdir -p '$RemoteRepo'; tar -xzf /tmp/cellsymphony-pi-source.tar.gz -C '$RemoteRepo'; rm -f /tmp/cellsymphony-pi-source.tar.gz"

  if ($SyncOnly) {
    exit 0
  }

  if (-not $SkipBuild) {
    Invoke-PiSsh "set -e; . `$HOME/.cargo/env; cd '$RemoteRepo'; CARGO_BUILD_JOBS=1 cargo build --release -p cellsymphony-pi --features hardware-pi; sudo install -d '$InstallDir/releases/dev'; sudo install -m 755 target/release/cellsymphony-pi '$InstallDir/releases/dev/cellsymphony-pi'"
  }
}

$serviceCommand = "set -e; sudo install -d '$InstallDir'; sudo ln -sfn '$InstallDir/releases/dev' '$InstallDir/current'; sudo ln -sfn '$InstallDir/current/cellsymphony-pi' /usr/local/bin/cellsymphony-pi; sudo tee /etc/systemd/system/$Service >/dev/null <<'EOF'
[Unit]
Description=Cell Symphony Pi Zero 2W Headless Music System
After=sound.target network.target

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
