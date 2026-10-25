param(
  [Parameter(Mandatory = $true)]
  [string]$Target,
  [string]$LocalBinary = "",
  [string]$RemoteRepo = "/home/pi/octessera-dev",
  [switch]$Restore,
  [switch]$SyncOnly,
  [switch]$CleanRemote,
  [switch]$NoTail
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\deployment-target.ps1")
Assert-DeploymentTarget $Target | Out-Null
if (@(@([bool]$Restore, [bool]$SyncOnly, ($LocalBinary -ne "")) | Where-Object { $_ }).Count -ne 1) {
  throw "Pass exactly one of -LocalBinary, -SyncOnly, or -Restore."
}
if ([string]::IsNullOrWhiteSpace($RemoteRepo) -or $RemoteRepo -notlike '/*' -or $RemoteRepo.IndexOfAny([char[]]@([char]0, "`r", "`n", "'")) -ge 0) {
  throw "RemoteRepo must be an absolute POSIX path without quotes or newlines."
}

$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\with-board-ssh.ps1")).Path
$remoteScript = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\dev-runtime.sh")).Path
$boardCheck = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\board-check.sh")).Path

function Invoke-PiPayload {
  param([string]$Contents, [string]$Label)
  $payloadPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-pi-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    [IO.File]::WriteAllText($payloadPath, $Contents, (New-Object System.Text.UTF8Encoding($false)))
    & $transport ssh-payload -Board rpi -Target $Target $payloadPath
    if ($LASTEXITCODE -ne 0) { throw "Raspberry $Label failed (exit $LASTEXITCODE)." }
  } finally {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
}

function Copy-ToPi {
  param([string]$Source, [string]$Destination)
  & $transport scp -Board rpi -Target $Target $Source "${Target}:$Destination"
  if ($LASTEXITCODE -ne 0) { throw "Upload to the Raspberry failed (exit $LASTEXITCODE)." }
}

function Invoke-DevRuntime {
  param([string]$Action, [string]$Hash = "")
  $prefix = "ACTION='$Action'`nBOARD_PROFILE='raspberry-pi-zero-2w'`nHASH='$Hash'`n"
  Invoke-PiPayload ($prefix + [IO.File]::ReadAllText($remoteScript)) $Action
}

if ($Restore) {
  Invoke-DevRuntime "restore"
  return
}

if ($SyncOnly) {
  $archive = Join-Path ([IO.Path]::GetTempPath()) "octessera-pi-source.tar.gz"
  Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
  tar `
    --exclude .git `
    --exclude .github `
    --exclude .slim `
    --exclude .opencode `
    --exclude node_modules `
    --exclude target `
    --exclude dist `
    --exclude '*/dist' `
    --exclude coverage `
    --exclude '*/coverage' `
    --exclude hardware `
    --exclude apps/desktop/dist-desktop `
    -czf $archive `
    -C (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path .
  if ($LASTEXITCODE -ne 0) { throw "Creating the source archive failed (exit $LASTEXITCODE)." }
  try {
    Copy-ToPi $archive "/tmp/octessera-pi-source.tar.gz"
  } finally {
    Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
  }
  $sync = if ($CleanRemote) {
    "rm -rf '$RemoteRepo'`nmkdir -p '$RemoteRepo'`ntar -xzf /tmp/octessera-pi-source.tar.gz -C '$RemoteRepo'"
  } else {
    "rm -rf '$RemoteRepo-sync'`nmkdir -p '$RemoteRepo-sync' '$RemoteRepo'`ntar -xzf /tmp/octessera-pi-source.tar.gz -C '$RemoteRepo-sync'`nrsync -a --checksum --delete --exclude target/ '$RemoteRepo-sync/' '$RemoteRepo/'`nrm -rf '$RemoteRepo-sync'"
  }
  Invoke-PiPayload "set -e`n$sync`nrm -f /tmp/octessera-pi-source.tar.gz`n" "source sync"
  return
}

$binary = (Resolve-Path -LiteralPath $LocalBinary -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "LocalBinary must be a file." }
$hash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
Invoke-DevRuntime "preflight"
Copy-ToPi $binary "/tmp/octessera-pi-dev"
Invoke-DevRuntime "activate" $hash
try {
  Invoke-PiPayload ("BOARD_PROFILE='raspberry-pi-zero-2w'`n" + [IO.File]::ReadAllText($boardCheck)) "board check"
} catch {
  throw "Board check failed; run -Restore. $_"
}
if (-not $NoTail) {
  & $transport ssh -Board rpi -Target $Target "journalctl -u octessera.service -f"
}
