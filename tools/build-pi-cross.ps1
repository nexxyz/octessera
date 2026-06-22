param(
  [string]$Target = "aarch64-unknown-linux-gnu",
  [string]$Profile = "pi-dev",
  [string]$OutDir = "target/pi-cross",
  [string]$Sysroot = "",
  [string]$PkgConfigPath = ""
)

$ErrorActionPreference = "Stop"

function Require-Command {
  param(
    [string]$Name,
    [string]$Message
  )

  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw $Message
  }
}

function Invoke-CheckedCommand {
  param(
    [string]$Label,
    [scriptblock]$Action
  )

  & $Action
  if ($LASTEXITCODE -ne 0) {
    throw "$Label failed with exit code $LASTEXITCODE"
  }
}

function Resolve-ExistingPath {
  param([string]$Path)

  if (-not (Test-Path -LiteralPath $Path)) {
    throw "Path not found: $Path"
  }
  (Resolve-Path -LiteralPath $Path).Path
}

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Push-Location $RepoRoot
try {
  Require-Command "cargo" "cargo is required to build the Pi binary"
  Require-Command "rustup" "rustup is required to install the Pi target"
  Require-Command "pkg-config" "pkg-config is required for ALSA cross-builds; install it or build on a Pi"

  $haveCrossInputs = $Sysroot -ne "" -or $PkgConfigPath -ne ""
  if ($Sysroot -ne "") {
    $env:PKG_CONFIG_SYSROOT_DIR = Resolve-ExistingPath $Sysroot
    $env:PKG_CONFIG_ALLOW_CROSS = "1"
  }
  if ($PkgConfigPath -ne "") {
    $env:PKG_CONFIG_PATH = Resolve-ExistingPath $PkgConfigPath
    $env:PKG_CONFIG_ALLOW_CROSS = "1"
  }

  if ($haveCrossInputs) {
    & pkg-config --exists alsa
    if ($LASTEXITCODE -ne 0) {
      throw "pkg-config could not resolve alsa with the supplied Sysroot/PkgConfigPath. Check the cross ALSA headers and pkg-config directories, then rerun."
    }
  } else {
    & pkg-config --exists alsa
    if ($LASTEXITCODE -ne 0) {
      throw "pkg-config could not find alsa. Supply -Sysroot and/or -PkgConfigPath for the ARM sysroot, or build on a Pi with hardware-pi."
    }
  }

  Write-Output "Building cellsymphony-pi for $Target ($Profile)"
  Invoke-CheckedCommand "rustup target add" { & rustup target add $Target }
  Invoke-CheckedCommand "cargo build" { & cargo build --target $Target --profile $Profile -p cellsymphony-pi --features hardware-pi }

  $binaryPath = Join-Path (Join-Path (Join-Path $RepoRoot "target") $Target) $Profile
  $binaryPath = Join-Path $binaryPath "cellsymphony-pi"
  if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "Build finished but binary was not found at $binaryPath"
  }

  $outputDir = if ([System.IO.Path]::IsPathRooted($OutDir)) { $OutDir } else { Join-Path $RepoRoot $OutDir }
  New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
  $outputBinary = Join-Path $outputDir "cellsymphony-pi"
  Copy-Item -Force -LiteralPath $binaryPath -Destination $outputBinary
  Write-Output $outputBinary
}
finally {
  Pop-Location
}
