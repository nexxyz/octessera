param(
  [ValidateSet("auto", "wsl-docker", "docker", "native")]
  [string]$Backend = "auto",
  [string]$Target = "aarch64-unknown-linux-gnu",
  [string]$Profile = "pi-dev",
  [string]$OutDir = "target/pi-cross",
  [string]$Image = "cellsymphony-pi-cross:latest",
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

function Test-WslDocker {
  if (-not (Get-Command "wsl" -ErrorAction SilentlyContinue)) {
    return $false
  }
  & wsl bash -lc "command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1"
  if ($LASTEXITCODE -eq 0) {
    return $true
  }
  & wsl -u root bash -lc "command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1"
  $LASTEXITCODE -eq 0
}

function Convert-ToWslPath {
  param([string]$Path)
  $resolved = (Resolve-Path -LiteralPath $Path).Path
  if ($resolved -match "^([A-Za-z]):\\(.*)$") {
    $drive = $Matches[1].ToLowerInvariant()
    $rest = $Matches[2].Replace("\", "/")
    return "/mnt/$drive/$rest"
  }
  $escaped = $resolved.Replace("'", "'\''")
  (& wsl bash -lc "wslpath -a '$escaped'").Trim()
}

function Invoke-WslDockerBuild {
  param(
    [string]$RepoRoot,
    [string]$OutputDir
  )

  Require-Command "wsl" "WSL2 is required for the wsl-docker backend"
  $repoWsl = Convert-ToWslPath $RepoRoot
  $repoPrefix = $RepoRoot.TrimEnd("\") + "\"
  if (-not $OutputDir.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "For the WSL Docker backend, OutDir must be inside the repository: $OutputDir"
  }
  $outputRelative = $OutputDir.Substring($repoPrefix.Length).Replace("\", "/")
  $outWsl = "/work/$outputRelative"
  $profileArg = $Profile.Replace("'", "'\''")
  $targetArg = $Target.Replace("'", "'\''")
  $imageArg = $Image.Replace("'", "'\''")

  $script = "cd '$repoWsl' && TARGET='$targetArg' PROFILE='$profileArg' OUT_DIR='$outWsl' IMAGE='$imageArg' bash ./tools/build-pi-cross-wsl.sh"
  & wsl bash -lc "docker info >/dev/null 2>&1"
  if ($LASTEXITCODE -eq 0) {
    Invoke-CheckedCommand "WSL Docker Pi cross-build" { & wsl bash -lc $script }
  } else {
    Invoke-CheckedCommand "WSL Docker Pi cross-build" { & wsl -u root bash -lc $script }
  }
}

function Invoke-DockerBuild {
  param(
    [string]$RepoRoot,
    [string]$OutputDir
  )

  Require-Command "docker" "Docker is required for the docker backend"
  $repoMount = $RepoRoot.Replace("\", "/")
  $outMount = $OutputDir.Replace("\", "/")
  Invoke-CheckedCommand "Docker image build" {
    & docker build -f Dockerfile.pi-zero -t $Image .
  }
  Invoke-CheckedCommand "Docker Pi cross-build" {
    & docker run --rm `
      -v "${repoMount}:/work" `
      -w /work `
      -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc `
      -e PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig/ `
      -e PKG_CONFIG_ALLOW_CROSS=1 `
      $Image `
      bash -lc "set -euo pipefail; rustup target add $Target; cargo build --target $Target --profile $Profile -p cellsymphony-pi --features hardware-pi; mkdir -p '$outMount'; cp target/$Target/$Profile/cellsymphony-pi '$outMount'/cellsymphony-pi"
  }
}

function Invoke-NativeCrossBuild {
  param([string]$RepoRoot)

  Require-Command "cargo" "cargo is required to build the Pi binary"
  Require-Command "rustup" "rustup is required to install the Pi target"
  Require-Command "pkg-config" "pkg-config is required for ALSA cross-builds; use -Backend wsl-docker or configure an ARM sysroot"

  $sccache = Get-Command "sccache" -ErrorAction SilentlyContinue
  if ($sccache) {
    $env:RUSTC_WRAPPER = Join-Path $PSScriptRoot "sccache-rustc.cmd"
    if (-not $env:SCCACHE_DIR) {
      $env:SCCACHE_DIR = Join-Path $env:LOCALAPPDATA "Mozilla\sccache"
    }
    Remove-Item Env:CARGO_INCREMENTAL -ErrorAction SilentlyContinue
    [Environment]::SetEnvironmentVariable("CARGO_INCREMENTAL", $null, "Process")
    Write-Output "Using sccache: $($sccache.Source)"
  }

  $haveCrossInputs = $Sysroot -ne "" -or $PkgConfigPath -ne ""
  if ($Sysroot -ne "") {
    $env:PKG_CONFIG_SYSROOT_DIR = Resolve-ExistingPath $Sysroot
    $env:PKG_CONFIG_ALLOW_CROSS = "1"
  }
  if ($PkgConfigPath -ne "") {
    $env:PKG_CONFIG_PATH = Resolve-ExistingPath $PkgConfigPath
    $env:PKG_CONFIG_ALLOW_CROSS = "1"
  }

  & pkg-config --exists alsa
  if ($LASTEXITCODE -ne 0) {
    if ($haveCrossInputs) {
      throw "pkg-config could not resolve alsa with the supplied Sysroot/PkgConfigPath."
    }
    throw "pkg-config could not find alsa. Use -Backend wsl-docker, supply -Sysroot/-PkgConfigPath, or build on a Pi."
  }

  Write-Output "Building cellsymphony-pi for $Target ($Profile) with native cross tools"
  Invoke-CheckedCommand "rustup target add" { & rustup target add $Target }
  Invoke-CheckedCommand "cargo build" {
    & cargo build --target $Target --profile $Profile -p cellsymphony-pi --features hardware-pi
  }
}

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$outputDir = if ([System.IO.Path]::IsPathRooted($OutDir)) { $OutDir } else { Join-Path $RepoRoot $OutDir }

Push-Location $RepoRoot
try {
  New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
  $selectedBackend = $Backend
  if ($selectedBackend -eq "auto") {
    if (Test-WslDocker) {
      $selectedBackend = "wsl-docker"
    } elseif (Get-Command "docker" -ErrorAction SilentlyContinue) {
      $selectedBackend = "docker"
    } else {
      $selectedBackend = "native"
    }
  }

  Write-Output "Building cellsymphony-pi with backend: $selectedBackend"
  switch ($selectedBackend) {
    "wsl-docker" { Invoke-WslDockerBuild -RepoRoot $RepoRoot -OutputDir $outputDir }
    "docker" { Invoke-DockerBuild -RepoRoot $RepoRoot -OutputDir $outputDir }
    "native" { Invoke-NativeCrossBuild -RepoRoot $RepoRoot }
  }

  if ($selectedBackend -eq "native") {
    $binaryPath = Join-Path (Join-Path (Join-Path $RepoRoot "target") $Target) $Profile
    $binaryPath = Join-Path $binaryPath "cellsymphony-pi"
    if (-not (Test-Path -LiteralPath $binaryPath)) {
      throw "Build finished but binary was not found at $binaryPath"
    }
    Copy-Item -Force -LiteralPath $binaryPath -Destination (Join-Path $outputDir "cellsymphony-pi")
  }

  $outputBinary = Join-Path $outputDir "cellsymphony-pi"
  if (-not (Test-Path -LiteralPath $outputBinary)) {
    throw "Build finished but binary was not found at $outputBinary"
  }
  Write-Output $outputBinary
}
finally {
  Pop-Location
}
