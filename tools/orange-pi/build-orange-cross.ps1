[CmdletBinding()]
param(
  [ValidateSet("orange-oled-smoke", "orange-seesaw-smoke", "octessera-pi")]
  [string]$Binary = "orange-oled-smoke",
  [ValidateSet("release", "pi-dev", "dev")]
  [string]$Profile = "pi-dev",
  [ValidateSet("aarch64-unknown-linux-gnu")]
  [string]$Target = "aarch64-unknown-linux-gnu",
  [ValidatePattern("^[A-Za-z0-9][A-Za-z0-9._/:@-]{0,127}$")]
  [string]$Image = "rust:1-bookworm",
  [ValidateSet(64, 128, 256)]
  [int]$BenchmarkVoicePoolCapacity = 64,
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$metadataModule = Join-Path $PSScriptRoot "orange-cross-metadata.psm1"
Import-Module $metadataModule -Force
$Binary = $Binary.ToLowerInvariant()
$Profile = $Profile.ToLowerInvariant()
$Target = $Target.ToLowerInvariant()

$CargoRegistryVolume = "octessera-orange-pi-cargo-registry"
$CargoGitVolume = "octessera-orange-pi-cargo-git"
$RustupVolume = "octessera-orange-pi-rustup"
$CargoTargetRelativePath = "target/orange-cross-cargo"

function Convert-ToBashSingleQuoted {
  param([Parameter(Mandatory)][string]$Value)

  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Resolve-RepositoryRoot {
  $root = Join-Path $PSScriptRoot "..\.."
  return (Resolve-Path -LiteralPath $root).Path.TrimEnd("\")
}

function Get-RepositorySourceCommit {
  param([Parameter(Mandatory)][string]$RepositoryRoot)
  $commit = (& git -C $RepositoryRoot rev-parse HEAD 2>$null | Out-String).Trim().ToLowerInvariant()
  if ($commit -notmatch '^[0-9a-f]{40}$') { throw "Could not resolve a full repository source commit for build metadata." }
  return $commit
}

function Convert-ToWslPath {
  param(
    [Parameter(Mandatory)][string]$Path
  )

  $resolved = (Resolve-Path -LiteralPath $Path).Path
  if ($resolved -match "^([A-Za-z]):\\(.*)$") {
    $drive = $Matches[1].ToLowerInvariant()
    $rest = $Matches[2].Replace("\", "/")
    return "/mnt/$drive/$rest"
  }

  $quoted = Convert-ToBashSingleQuoted $resolved
  $converted = @(& wsl bash -lc "wslpath -a $quoted")
  if ($LASTEXITCODE -ne 0 -or $converted.Count -ne 1) {
    throw "Could not convert repository path to a WSL path: $resolved"
  }
  return ([string]$converted[0]).Trim()
}

function Get-WslDockerArguments {
  if (-not (Get-Command "wsl" -ErrorAction SilentlyContinue)) {
    throw "WSL is required; this builder does not use host Docker or native cross-compilers."
  }

  & wsl bash -lc "command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1" *> $null
  if ($LASTEXITCODE -eq 0) {
    return @("bash", "-lc")
  }

  & wsl -u root bash -lc "command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1" *> $null
  if ($LASTEXITCODE -eq 0) {
    return @("-u", "root", "bash", "-lc")
  }

  throw "Docker is not available through WSL. Start Docker Desktop's WSL integration or the WSL Docker daemon."
}

function Get-BuildSpec {
  param(
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][int]$SelectedBenchmarkVoicePoolCapacity
  )

  if ($SelectedBinary -in @("orange-oled-smoke", "orange-seesaw-smoke")) {
    if ($SelectedBenchmarkVoicePoolCapacity -ne 64) {
      throw "Expanded benchmark voice-pool capacities support only octessera-pi."
    }
    return [pscustomobject]@{
      Package = "octessera-hal"
      Feature = "orange-pi-zero-2w"
      ArtifactKind = "diagnostic-only"
    }
  }

  if ($SelectedBinary -eq "octessera-pi") {
    $benchmarkFeature = if ($SelectedBenchmarkVoicePoolCapacity -eq 64) { $null } else { "benchmark-voice-pools-$SelectedBenchmarkVoicePoolCapacity" }
    $feature = if ($null -eq $benchmarkFeature) { "hardware-orange-pi-zero-2w" } else { "hardware-orange-pi-zero-2w $benchmarkFeature" }
    $artifactKind = if ($null -eq $benchmarkFeature) { "runtime-candidate" } else { "diagnostic-only" }
    return [pscustomobject]@{
      Package = "octessera-pi"
      Feature = $feature
      ArtifactKind = $artifactKind
    }
  }

  throw "Unsupported Orange Pi binary: $SelectedBinary"
}

function New-DockerShellCommand {
  param(
    [Parameter(Mandatory)][string]$RepositoryWslPath,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec,
    [Parameter(Mandatory)][string]$SelectedOutputRelativePath
  )

  $targetQuoted = Convert-ToBashSingleQuoted $Target
  $profileQuoted = Convert-ToBashSingleQuoted $Profile
  $binaryQuoted = Convert-ToBashSingleQuoted $Binary
  $packageQuoted = Convert-ToBashSingleQuoted $BuildSpec.Package
  $featureQuoted = Convert-ToBashSingleQuoted $BuildSpec.Feature
  $outputQuoted = Convert-ToBashSingleQuoted "/work/$SelectedOutputRelativePath"
  $cargoTargetDirectory = "/work/$CargoTargetRelativePath"
  $cargoTargetQuoted = Convert-ToBashSingleQuoted $cargoTargetDirectory
  $artifactProfile = if ($Profile -eq "dev") { "debug" } else { $Profile }
  $sourceQuoted = Convert-ToBashSingleQuoted "$cargoTargetDirectory/$Target/$artifactProfile/$Binary"
  $innerScript = @"
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
export PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export CARGO_TARGET_DIR=$cargoTargetQuoted
dpkg --add-architecture arm64
apt-get update
apt-get install -y --no-install-recommends \
  ca-certificates \
  gcc-aarch64-linux-gnu \
  binutils-aarch64-linux-gnu \
  libc6-dev-arm64-cross \
  libasound2-dev:arm64 \
  pkg-config
rm -rf /var/lib/apt/lists/*
rustup target add $targetQuoted
cargo build --target $targetQuoted --profile $profileQuoted --no-default-features -p $packageQuoted --bin $binaryQuoted --features $featureQuoted
test -f $sourceQuoted
mkdir -p $outputQuoted
cp -- $sourceQuoted '/work/$SelectedOutputRelativePath/$Binary'
aarch64-linux-gnu-readelf -h '/work/$SelectedOutputRelativePath/$Binary' | grep -Eq '^[[:space:]]*Class:[[:space:]]*ELF64[[:space:]]*$'
aarch64-linux-gnu-readelf -h '/work/$SelectedOutputRelativePath/$Binary' | grep -Eq '^[[:space:]]*Machine:[[:space:]]*AArch64[[:space:]]*$'
"@
  $innerScript = $innerScript -replace "`r`n", "`n"

  $dockerArguments = @(
    "docker"
    "run"
    "--rm"
    "-v"
    "$RepositoryWslPath`:/work"
    "-v"
    "$CargoRegistryVolume`:/usr/local/cargo/registry"
    "-v"
    "$CargoGitVolume`:/usr/local/cargo/git"
    "-v"
    "$RustupVolume`:/usr/local/rustup"
    "-w"
    "/work"
    "-e"
    "CARGO_HOME=/usr/local/cargo"
    "-e"
    "RUSTUP_HOME=/usr/local/rustup"
    "-e"
    "CARGO_TARGET_DIR=$cargoTargetDirectory"
    "-e"
    "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc"
    "-e"
    "PKG_CONFIG_ALLOW_CROSS=1"
    "-e"
    "PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
    "-e"
    "PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
    $Image
    "bash"
    "-lc"
    $innerScript
  )

  return (($dockerArguments | ForEach-Object { Convert-ToBashSingleQuoted ([string]$_) }) -join " ")
}

function Get-OutputRelativePath {
  param([Parameter(Mandatory)][int]$SelectedBenchmarkVoicePoolCapacity)
  if ($SelectedBenchmarkVoicePoolCapacity -eq 64) { return "target/orange-pi-cross" }
  return "target/orange-pi-cross-diagnostics/benchmark-voice-pools-$SelectedBenchmarkVoicePoolCapacity"
}

$repositoryRoot = Resolve-RepositoryRoot
$sourceCommit = Get-RepositorySourceCommit $repositoryRoot
$buildSpec = Get-BuildSpec -SelectedBinary $Binary -SelectedBenchmarkVoicePoolCapacity $BenchmarkVoicePoolCapacity
$selectedOutputRelativePath = Get-OutputRelativePath $BenchmarkVoicePoolCapacity
$outputDirectory = Join-Path $repositoryRoot $selectedOutputRelativePath.Replace("/", "\")
$outputBinary = Join-Path $outputDirectory $Binary
$outputMetadata = "$outputBinary.metadata.json"
$repositoryWslPath = Convert-ToWslPath $repositoryRoot
$dockerCommand = New-DockerShellCommand -RepositoryWslPath $repositoryWslPath -BuildSpec $buildSpec -SelectedOutputRelativePath $selectedOutputRelativePath

if ($DryRun) {
  Write-Output "Dry run: no Docker container was started and no board connection is attempted."
  Write-Output "wsl bash -lc $dockerCommand"
  Write-Output "Output: $outputBinary"
  Write-Output "Metadata: $outputMetadata"
  return
}

$repositoryStatus = (& git -C $repositoryRoot status --porcelain --untracked-files=all 2>$null | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { throw "Could not inspect repository status before the authoritative Orange build." }
if (-not [string]::IsNullOrWhiteSpace($repositoryStatus)) { throw "Authoritative Orange builds require a clean repository; tracked or untracked source changes are present." }

$wslArguments = Get-WslDockerArguments
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$artifactReady = $false
try {
  Remove-OrangeBuildArtifacts -BinaryPath $outputBinary -MetadataPath $outputMetadata
  Write-Output "Building $Binary for Orange Pi ($Target, $Profile) with WSL Docker."
  & wsl @wslArguments $dockerCommand
  if ($LASTEXITCODE -ne 0) {
    throw "Orange Pi cross-build failed with exit code $LASTEXITCODE"
  }

  if (-not (Test-Path -LiteralPath $outputBinary -PathType Leaf)) {
    throw "Build finished without binary: $outputBinary"
  }
  if ((Get-Item -LiteralPath $outputBinary).Length -le 0) {
    throw "Build produced an empty binary: $outputBinary"
  }
  Invoke-VerifiedOrangeBuildMetadata `
    -MetadataPath $outputMetadata `
    -BinaryPath $outputBinary `
    -SelectedBinary $Binary `
    -SelectedTarget $Target `
    -SelectedProfile $Profile `
    -BuildSpec $buildSpec `
    -SourceCommit $sourceCommit
  $artifactReady = $true
} finally {
  if (-not $artifactReady) {
    Remove-OrangeBuildArtifacts -BinaryPath $outputBinary -MetadataPath $outputMetadata
  }
}
Write-Output "Verified ELF64 AArch64 binary and hash-bound profile metadata: $outputBinary"
