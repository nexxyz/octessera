$ErrorActionPreference = "Stop"

$buildScript = Join-Path $PSScriptRoot "build-orange-cross.ps1"
$source = [IO.File]::ReadAllText($buildScript)

foreach ($required in @(
    "docker",
    "run",
    "--rm",
    "octessera-orange-pi-cargo-registry",
    "octessera-orange-pi-cargo-git",
    "octessera-orange-pi-rustup",
    "export PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "dpkg --add-architecture arm64",
    "gcc-aarch64-linux-gnu",
    "libc6-dev-arm64-cross",
    "aarch64-linux-gnu-readelf",
    "base64 --decode",
    'artifact_kind = "diagnostic-only"',
    'runtime_ready = $false',
    "ELF64",
    "AArch64"
  )) {
  if ($source.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
    throw "Orange cross-builder is missing required operation: $required"
  }
}

if ($source -match "(?i)\b(ssh|scp|rsync)\b") {
  throw "Orange cross-builder must not contain board transport commands"
}
if ($source -match 'octessera-pi.*hardware-orange-pi-zero-2w') {
  throw "Orange cross-builder must not define an Orange runtime build"
}

function Invoke-DryRun {
  param([hashtable]$Parameters)

  $output = @(& $buildScript @Parameters -DryRun 2>&1)
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

$default = Invoke-DryRun @{}
foreach ($expected in @(
    "no Docker container was started",
    "target/orange-pi-cross/orange-oled-smoke",
    "--profile.*pi-dev",
    "-p.*octessera-hal",
    "--features.*orange-pi-zero-2w"
  )) {
  if ($default -notmatch $expected) {
    throw "Default Orange cross-builder dry run is missing: $expected"
  }
}

$hal = Invoke-DryRun @{ Profile = "release" }
foreach ($expected in @(
    "target/orange-pi-cross/orange-oled-smoke",
    "--profile.*release",
    "-p.*octessera-hal",
    "--features.*orange-pi-zero-2w"
  )) {
  if ($hal -notmatch $expected) {
    throw "HAL Orange cross-builder dry run is missing: $expected"
  }
}

$dev = Invoke-DryRun @{ Profile = "dev" }
if ($dev -notmatch "target/aarch64-unknown-linux-gnu/debug/orange-oled-smoke") {
  throw "Dev profile dry run did not use Cargo's debug artifact directory"
}

foreach ($parameters in @(
    @{ Binary = "octessera-pi" },
    @{ Profile = "release;touch" },
    @{ Image = "rust:bookworm;touch" },
    @{ Target = "x86_64-unknown-linux-gnu" }
  )) {
  $rejected = $false
  try {
    Invoke-DryRun $parameters | Out-Null
  } catch {
    $rejected = $true
  }
  if (-not $rejected) {
    throw "Orange cross-builder accepted unsafe or unsupported arguments: $($parameters | ConvertTo-Json -Compress)"
  }
}

Write-Output "Orange Pi cross-builder host and dry-run tests passed"
