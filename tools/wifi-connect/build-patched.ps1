[CmdletBinding()]
param(
  [ValidatePattern("^[A-Za-z0-9][A-Za-z0-9._/:@-]{0,127}$")]
  [string]$Image = "rust:1.76.0-bookworm@sha256:d36f9d8a9a4c76da74c8d983d0d4cb146dd2d19bb9bd60b704cdcf70ef868d3a",
  [ValidateSet("aarch64-unknown-linux-gnu")]
  [string]$Target = "aarch64-unknown-linux-gnu",
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$UpstreamCommit = "5bd4c1bea548fb5714bedb18bbd12f088d5fa407"
$UpstreamOrigin = "https://github.com/balena-os/wifi-connect.git"
$PatchRelativePath = "third_party/wifi-connect-4.11.84/portal-address-readiness.patch"
$CloneRelativePath = ".slim/clonedeps/repos/balena-os__wifi-connect"
$OutputRelativePath = "target/wifi-connect-patched"

function Convert-ToBashSingleQuoted {
  param([Parameter(Mandatory)][string]$Value)

  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Resolve-RepositoryRoot {
  return (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path.TrimEnd("\")
}

function Convert-ToWslPath {
  param([Parameter(Mandatory)][string]$Path)

  $resolved = (Resolve-Path -LiteralPath $Path).Path
  if ($resolved -match "^([A-Za-z]):\\(.*)$") {
    return "/mnt/$($Matches[1].ToLowerInvariant())/$($Matches[2].Replace("\", "/"))"
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

function Invoke-GitText {
  param(
    [Parameter(Mandatory)][string]$Repository,
    [Parameter(Mandatory)][string[]]$Arguments
  )

  $result = @(& git -C $Repository @Arguments)
  if ($LASTEXITCODE -ne 0) {
    throw "git $($Arguments -join ' ') failed in $Repository"
  }
  return ($result -join "`n").Trim()
}

function New-DockerCommand {
  param([Parameter(Mandatory)][string]$RepositoryWslPath)

  $arguments = @(
    "docker", "run", "--rm",
    "-e", "WIFI_CONNECT_TARGET=$Target",
    "-e", "WIFI_CONNECT_CONTAINER_IMAGE=$Image",
    "-v", "$RepositoryWslPath`:/work",
    "-v", "octessera-wifi-connect-cargo-registry`:/usr/local/cargo/registry",
    "-v", "octessera-wifi-connect-cargo-git`:/usr/local/cargo/git",
    "-v", "octessera-wifi-connect-rustup`:/usr/local/rustup",
    "-w", "/work",
    $Image,
    "bash", "/work/tools/wifi-connect/build-patched.sh"
  )
  return (($arguments | ForEach-Object { Convert-ToBashSingleQuoted ([string]$_) }) -join " ")
}

$root = Resolve-RepositoryRoot
$clone = Join-Path $root $CloneRelativePath.Replace("/", "\")
$patch = Join-Path $root $PatchRelativePath.Replace("/", "\")
$outputDirectory = Join-Path $root $OutputRelativePath.Replace("/", "\")
if (-not (Test-Path -LiteralPath (Join-Path $root "target") -PathType Container)) {
  throw "Repository target directory is missing: $(Join-Path $root 'target')"
}
if (-not (Test-Path -LiteralPath $clone -PathType Container)) {
  throw "Read-only wifi-connect source clone is missing: $clone"
}
if (-not (Test-Path -LiteralPath $patch -PathType Leaf)) {
  throw "wifi-connect patch is missing: $patch"
}

$head = Invoke-GitText $clone @("rev-parse", "HEAD")
$origin = Invoke-GitText $clone @("remote", "get-url", "origin")
$status = Invoke-GitText $clone @("status", "--porcelain")
if ($head -ne $UpstreamCommit) { throw "wifi-connect clone HEAD is not pinned to ${UpstreamCommit}: $head" }
if ($origin -ne $UpstreamOrigin) { throw "wifi-connect clone origin is not pinned: $origin" }
if (-not [string]::IsNullOrWhiteSpace($status)) { throw "wifi-connect source clone is dirty; refusing to use it." }

$repositoryWslPath = Convert-ToWslPath $root
$dockerCommand = New-DockerCommand $repositoryWslPath
if ($DryRun) {
  Write-Output "Dry run: source clone origin and exact HEAD verified; no clone changes or container started."
  Write-Output "wsl bash -lc $dockerCommand"
  Write-Output "Output: $(Join-Path $outputDirectory 'wifi-connect')"
  return
}

$dockerArguments = Get-WslDockerArguments
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
& wsl @dockerArguments $dockerCommand
if ($LASTEXITCODE -ne 0) {
  throw "Patched wifi-connect build failed with exit code $LASTEXITCODE"
}
Write-Output "Verified patched ELF64 AArch64 wifi-connect: $(Join-Path $outputDirectory 'wifi-connect')"
