param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$RemoteRepo = "/home/pi/octessera-dev",
  [string]$InstallDir = "/opt/octessera",
  [string]$Service = "octessera.service",
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

  Invoke-PiSsh "set -e; sudo install -d '$InstallDir'; sudo ln -sfn '$InstallDir/releases/dev' '$InstallDir/current'; sudo ln -sfn '$InstallDir/current/octessera-pi' /usr/local/bin/octessera-pi"
}

$serviceCommand = "set -e; sudo systemctl restart '$Service'; systemctl --no-pager --lines=8 status '$Service'"

if ($AllowServiceFailure) {
  Invoke-PiSsh "$serviceCommand || true"
} else {
  Invoke-PiSsh $serviceCommand
}

if (-not $NoTail) {
  ssh @sshArgs "journalctl -u '$Service' -f"
}
