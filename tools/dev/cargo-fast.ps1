param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$CargoArgs
)

$ErrorActionPreference = "Stop"

$sccache = Get-Command "sccache" -ErrorAction SilentlyContinue
if ($sccache) {
  $env:RUSTC_WRAPPER = Join-Path $PSScriptRoot "sccache-rustc.cmd"
  if (-not $env:SCCACHE_DIR) {
    $env:SCCACHE_DIR = Join-Path $env:LOCALAPPDATA "Mozilla\sccache"
  }
  Remove-Item Env:CARGO_INCREMENTAL -ErrorAction SilentlyContinue
  [Environment]::SetEnvironmentVariable("CARGO_INCREMENTAL", $null, "Process")
  Write-Output "Using sccache: $($sccache.Source)"
} else {
  Write-Output "sccache not found; running cargo without compiler cache"
}

$cargoPrefix = @()
if ($sccache) {
  $cargoPrefix = @(
    "--config", "profile.dev.incremental=false",
    "--config", "profile.test.incremental=false",
    "--config", "profile.pi-dev.incremental=false"
  )
}

cargo @cargoPrefix @CargoArgs
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

if ($sccache) {
  sccache --show-stats
}
