param(
  [switch]$NoStats
)

$ErrorActionPreference = "Stop"

$sccache = Get-Command "sccache" -ErrorAction SilentlyContinue
if ($sccache) {
  $env:RUSTC_WRAPPER = Join-Path $PSScriptRoot "..\dev\sccache-rustc.cmd"
  if (-not $env:SCCACHE_DIR) {
    $env:SCCACHE_DIR = Join-Path $env:LOCALAPPDATA "Mozilla\sccache"
  }
  Remove-Item Env:CARGO_INCREMENTAL -ErrorAction SilentlyContinue
  [Environment]::SetEnvironmentVariable("CARGO_INCREMENTAL", $null, "Process")
  Write-Output "Using sccache: $($sccache.Source)"
} else {
  Write-Output "sccache not found; building without compiler cache"
}

corepack pnpm --filter @cellsymphony/desktop tauri:build:exe
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

if ($sccache -and -not $NoStats) {
  sccache --show-stats
}
