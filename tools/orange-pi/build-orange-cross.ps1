[CmdletBinding()]
param(
  [ValidateSet("orange-oled-smoke")]
  [string]$Binary = "orange-oled-smoke",
  [ValidateSet("release", "pi-dev", "dev")]
  [string]$Profile = "pi-dev",
  [ValidateSet("aarch64-unknown-linux-gnu")]
  [string]$Target = "aarch64-unknown-linux-gnu",
  [ValidatePattern("^[A-Za-z0-9][A-Za-z0-9._/:@-]{0,127}$")]
  [string]$Image = "rust:1-bookworm",
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$Binary = $Binary.ToLowerInvariant()
$Profile = $Profile.ToLowerInvariant()
$Target = $Target.ToLowerInvariant()

$CargoRegistryVolume = "octessera-orange-pi-cargo-registry"
$CargoGitVolume = "octessera-orange-pi-cargo-git"
$RustupVolume = "octessera-orange-pi-rustup"
$OutputRelativePath = "target/orange-pi-cross"

function Convert-ToBashSingleQuoted {
  param([Parameter(Mandatory)][string]$Value)

  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Resolve-RepositoryRoot {
  $root = Join-Path $PSScriptRoot "..\.."
  return (Resolve-Path -LiteralPath $root).Path.TrimEnd("\")
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
  param([Parameter(Mandatory)][string]$SelectedBinary)

  if ($SelectedBinary -eq "orange-oled-smoke") {
    return [pscustomobject]@{
      Package = "octessera-hal"
      Feature = "orange-pi-zero-2w"
    }
  }

  throw "Unsupported Orange Pi binary: $SelectedBinary"
}

function New-DockerShellCommand {
  param(
    [Parameter(Mandatory)][string]$RepositoryWslPath,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec
  )

  $targetQuoted = Convert-ToBashSingleQuoted $Target
  $profileQuoted = Convert-ToBashSingleQuoted $Profile
  $binaryQuoted = Convert-ToBashSingleQuoted $Binary
  $packageQuoted = Convert-ToBashSingleQuoted $BuildSpec.Package
  $featureQuoted = Convert-ToBashSingleQuoted $BuildSpec.Feature
  $outputQuoted = Convert-ToBashSingleQuoted "/work/$OutputRelativePath"
  $artifactProfile = if ($Profile -eq "dev") { "debug" } else { $Profile }
  $sourceQuoted = Convert-ToBashSingleQuoted "target/$Target/$artifactProfile/$Binary"
  $metadata = [ordered]@{
    schema_version = 1
    board_profile = "orange-pi-zero-2w"
    artifact_kind = "diagnostic-only"
    runtime_ready = $false
    binary = $Binary
    package = $BuildSpec.Package
    arch = $Target
    cargo_feature = $BuildSpec.Feature
    profile = $Profile
  } | ConvertTo-Json -Compress
  $metadataBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($metadata))
  $metadataBase64Quoted = Convert-ToBashSingleQuoted $metadataBase64
  $innerScript = @"
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
export PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
dpkg --add-architecture arm64
apt-get update
apt-get install -y --no-install-recommends \
  ca-certificates \
  gcc-aarch64-linux-gnu \
  binutils-aarch64-linux-gnu \
  libc6-dev-arm64-cross \
  libasound2-dev:arm64 \
  libdbus-1-dev:arm64 \
  pkg-config
rm -rf /var/lib/apt/lists/*
rustup target add $targetQuoted
cargo build --target $targetQuoted --profile $profileQuoted -p $packageQuoted --bin $binaryQuoted --features $featureQuoted
test -f $sourceQuoted
mkdir -p $outputQuoted
cp -- $sourceQuoted '/work/$OutputRelativePath/$Binary'
aarch64-linux-gnu-readelf -h '/work/$OutputRelativePath/$Binary' | grep -Eq '^[[:space:]]*Class:[[:space:]]*ELF64[[:space:]]*$'
aarch64-linux-gnu-readelf -h '/work/$OutputRelativePath/$Binary' | grep -Eq '^[[:space:]]*Machine:[[:space:]]*AArch64[[:space:]]*$'
printf '%s' $metadataBase64Quoted | base64 --decode > '/work/$OutputRelativePath/$Binary.metadata.json'
"@

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

function Assert-OutputMetadata {
  param(
    [Parameter(Mandatory)][string]$MetadataPath,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec
  )

  if (-not (Test-Path -LiteralPath $MetadataPath -PathType Leaf)) {
    throw "Build finished without metadata: $MetadataPath"
  }

  $bytes = [IO.File]::ReadAllBytes($MetadataPath)
  if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
    throw "Build metadata must be BOM-less UTF-8: $MetadataPath"
  }

  try {
    $metadata = (New-Object System.Text.UTF8Encoding($false, $true)).GetString($bytes) | ConvertFrom-Json
  } catch {
    throw "Build metadata is not valid UTF-8 JSON: $MetadataPath"
  }

  $expected = [ordered]@{
    schema_version = 1
    board_profile = "orange-pi-zero-2w"
    artifact_kind = "diagnostic-only"
    runtime_ready = $false
    binary = $Binary
    package = $BuildSpec.Package
    arch = $Target
    cargo_feature = $BuildSpec.Feature
    profile = $Profile
  }
  $properties = @($metadata.PSObject.Properties.Name)
  if ($properties.Count -ne $expected.Count) {
    throw "Build metadata has an unexpected field set: $MetadataPath"
  }
  foreach ($name in $expected.Keys) {
    if ($null -eq $metadata.PSObject.Properties[$name] -or $metadata.$name -cne $expected[$name]) {
      throw "Build metadata field '$name' is incorrect: $MetadataPath"
    }
  }
}

$repositoryRoot = Resolve-RepositoryRoot
$buildSpec = Get-BuildSpec $Binary
$outputDirectory = Join-Path $repositoryRoot $OutputRelativePath.Replace("/", "\")
$outputBinary = Join-Path $outputDirectory $Binary
$outputMetadata = "$outputBinary.metadata.json"
$repositoryWslPath = Convert-ToWslPath $repositoryRoot
$dockerCommand = New-DockerShellCommand $repositoryWslPath $buildSpec

if ($DryRun) {
  Write-Output "Dry run: no Docker container was started and no board connection is attempted."
  Write-Output "wsl bash -lc $dockerCommand"
  Write-Output "Output: $outputBinary"
  return
}

$wslArguments = Get-WslDockerArguments
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
Remove-Item -LiteralPath $outputBinary, $outputMetadata -Force -ErrorAction SilentlyContinue
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
Assert-OutputMetadata $outputMetadata $buildSpec
Write-Output "Verified ELF64 AArch64 binary and profile metadata: $outputBinary"
