[CmdletBinding(SupportsShouldProcess = $true)]
param(
  [Parameter(Mandatory = $true)]
  [string]$RaspberryTarget,

  [Parameter(Mandatory = $true)]
  [string]$OrangeTarget,

  [string]$DesktopStore = $(if ($env:OCTESSERA_DESKTOP_STORE_DIR) { $env:OCTESSERA_DESKTOP_STORE_DIR } else { Join-Path $env:APPDATA "xyz.nex.octessera" }),

  [switch]$IncludeSystem
)

# Installs the factory default patch on all three stores at once: both boards and this host's
# desktop store, which only ever stands in for the boards. Each store is fetched with its service
# stopped and composed against the new patch first; if any of the three fails, nothing changes.
# -IncludeSystem also installs the factory system.json; by default each device keeps its own.
# -WhatIf stops after the compose.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Boards = [ordered]@{
  rpi = @{ Store = "/home/pi/presets"; Owner = "pi"; Config = "config/generated/pi/default.json" }
  opi = @{ Store = "/var/lib/octessera/presets"; Owner = "octessera-runtime"; Config = "config/generated/pi/default.json" }
}
$DesktopConfig = "config/generated/desktop/default.json"
$DesktopProcessNames = @("octessera", "octessera-desktop")

function Get-RunningDesktopApps {
  param([string[]]$ProcessNames)
  @($ProcessNames | Where-Object { $DesktopProcessNames -contains $_ })
}

function Assert-SameInstalledPatch {
  param([string]$Source, [System.Collections.IDictionary]$Installed)
  foreach ($device in $Installed.Keys) {
    if ($Installed[$device] -ne $Source) {
      throw "default.patch.json on $device has sha256 $($Installed[$device]), expected $Source."
    }
  }
}

function Get-FileSha256 {
  param([string]$Path)
  (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

if ($MyInvocation.InvocationName -eq ".") { return }

$composeOnly = [bool]$WhatIfPreference
$WhatIfPreference = $false
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$transport = Join-Path $PSScriptRoot "with-board-ssh.ps1"
$targets = @{ rpi = $RaspberryTarget; opi = $OrangeTarget }
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$work = Join-Path $repo "target\factory-default\$stamp"

function Invoke-Board {
  param([string]$Board, [string]$Command)
  $output = & $transport ssh -Board $Board -Target $targets[$Board] $Command
  if ($LASTEXITCODE -ne 0) { throw "$Board command failed (exit $LASTEXITCODE): $Command" }
  $output
}

function Copy-FromBoard {
  param([string]$Board, [string]$Remote, [string]$Local)
  & $transport scp -Board $Board -Target $targets[$Board] "$($targets[$Board]):$Remote" $Local
  if ($LASTEXITCODE -ne 0) { throw "Could not fetch $Remote from $Board." }
}

function Copy-ToBoard {
  param([string]$Board, [string]$Local, [string]$Remote)
  & $transport scp -Board $Board -Target $targets[$Board] $Local "$($targets[$Board]):$Remote"
  if ($LASTEXITCODE -ne 0) { throw "Could not copy $Local to $Board." }
}

function Split-DefaultConfig {
  param([string]$Config, [string]$Output)
  & cargo run -q -p playback-runtime --bin split_default_config -- (Join-Path $repo $Config) $Output
  if ($LASTEXITCODE -ne 0) { throw "split_default_config failed for $Config." }
}

$running = @(Get-RunningDesktopApps -ProcessNames @(Get-Process | ForEach-Object { $_.ProcessName }))
if ($running.Count -gt 0) { throw "Close the desktop app first ($($running -join ', ') is running)." }
if (-not (Test-Path -LiteralPath (Join-Path $DesktopStore "system.json"))) { throw "No desktop store at $DesktopStore." }

Push-Location $repo
$stopped = @()
try {
  New-Item -ItemType Directory -Force -Path $work | Out-Null
  $split = @{ pi = Join-Path $work "split-pi"; desktop = Join-Path $work "split-desktop" }
  Split-DefaultConfig $Boards.rpi.Config $split.pi
  Split-DefaultConfig $DesktopConfig $split.desktop
  $sourceHash = Get-FileSha256 (Join-Path $split.pi "default.patch.json")
  Assert-SameInstalledPatch -Source $sourceHash -Installed @{ desktop = Get-FileSha256 (Join-Path $split.desktop "default.patch.json") }

  $raw = Join-Path $work "raw"
  $compose = Join-Path $work "compose"
  foreach ($board in $Boards.Keys) {
    $store = $Boards[$board].Store
    $stopped += $board
    Invoke-Board $board "sudo -n systemctl stop octessera.service && rm -f /tmp/factory-default-store.tgz && sudo -n tar czf /tmp/factory-default-store.tgz -C $store . && sudo -n chown `$(id -un) /tmp/factory-default-store.tgz" | Out-Null
    $archive = Join-Path $work "$board-store.tgz"
    Copy-FromBoard $board "/tmp/factory-default-store.tgz" $archive
    New-Item -ItemType Directory -Force -Path (Join-Path $raw $board) | Out-Null
    & (Join-Path $env:SystemRoot "System32\tar.exe") -xzf $archive -C (Join-Path $raw $board)
    if ($LASTEXITCODE -ne 0) { throw "Could not unpack the $board store." }
  }
  Copy-Item -Recurse -LiteralPath $DesktopStore -Destination (Join-Path $raw "desktop")

  Copy-Item -Recurse -LiteralPath $raw -Destination $compose
  foreach ($device in @("rpi", "opi", "desktop")) {
    $source = if ($device -eq "desktop") { $split.desktop } else { $split.pi }
    $files = @("default.patch.json") + @(if ($IncludeSystem) { "system.json" })
    foreach ($file in $files) {
      Copy-Item -Force -LiteralPath (Join-Path $source $file) -Destination (Join-Path $compose "$device\$file")
    }
  }
  $env:OCTESSERA_STORE_COPY = $compose
  & cargo test -q -p playback-runtime --test board_store_compose -- --ignored --nocapture
  $composeExit = $LASTEXITCODE
  Remove-Item Env:OCTESSERA_STORE_COPY
  if ($composeExit -ne 0) { throw "The new factory default does not compose with every store; nothing was changed. Copies: $work" }
  "All three stores compose with the factory default (sha256 $sourceHash)."

  if ($composeOnly) {
    "WhatIf: nothing installed."
    return
  }

  $files = @("default.patch.json") + @(if ($IncludeSystem) { "system.json" })
  foreach ($board in $Boards.Keys) {
    $store = $Boards[$board].Store
    Invoke-Board $board "sudo -n cp -a $store $store.pre-factory-default-$stamp" | Out-Null
    foreach ($file in $files) {
      Copy-ToBoard $board (Join-Path $split.pi $file) "/tmp/factory-default-$file"
      Invoke-Board $board ("set -e; f=$store/$file; owner=`$(stat -c '%U' `$f); group=`$(stat -c '%G' `$f); mode=`$(stat -c '%a' `$f); " +
        "sudo -n install -o `$owner -g `$group -m `$mode /tmp/factory-default-$file `$f; rm -f /tmp/factory-default-$file") | Out-Null
    }
  }
  Copy-Item -Recurse -LiteralPath $DesktopStore -Destination "$DesktopStore.pre-factory-default-$stamp"
  foreach ($file in $files) {
    Copy-Item -Force -LiteralPath (Join-Path $split.desktop $file) -Destination (Join-Path $DesktopStore $file)
  }

  $installed = [ordered]@{}
  foreach ($board in $Boards.Keys) {
    $installed[$board] = ((Invoke-Board $board "sha256sum $($Boards[$board].Store)/default.patch.json") -split '\s+')[0]
  }
  $installed["desktop"] = Get-FileSha256 (Join-Path $DesktopStore "default.patch.json")
  Assert-SameInstalledPatch -Source $sourceHash -Installed $installed
  "Installed default.patch.json sha256 $sourceHash on rpi, opi and desktop."
  "Backups: <store>.pre-factory-default-$stamp on both boards and the desktop; fetched copies in $work."
} finally {
  foreach ($board in $stopped) {
    & $transport ssh -Board $board -Target $targets[$board] "sudo -n systemctl start octessera.service"
    if ($LASTEXITCODE -ne 0) { Write-Warning "Could not restart octessera.service on $board." }
  }
  Pop-Location
}
