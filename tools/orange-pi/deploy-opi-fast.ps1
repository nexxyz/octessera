[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Target,
  [string]$LocalBinary,
  [string]$LocalMetadata,
  [switch]$Restore
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "..\deployment-target.ps1")
Assert-DeploymentTarget $Target | Out-Null
if (-not $Target.StartsWith("octessera@", [StringComparison]::Ordinal)) {
  throw "Orange development deployment requires octessera@host."
}
if ($Restore) {
  if ($PSBoundParameters.ContainsKey("LocalBinary") -or $PSBoundParameters.ContainsKey("LocalMetadata")) {
    throw "-Restore does not accept a binary or metadata."
  }
} elseif ([string]::IsNullOrWhiteSpace($LocalBinary)) {
  throw "-LocalBinary is required for Orange development deployment."
}

$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "with-opi-ssh.ps1")).Path
$remoteScript = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "deploy-opi-fast-remote.sh")).Path
$source = ""
$hash = ""
$stage = ""
if (-not $Restore) {
  $binary = (Resolve-Path -LiteralPath $LocalBinary -ErrorAction Stop).Path
  if ((Split-Path -Leaf $binary) -cne "octessera-pi" -or
    ((Get-Item -LiteralPath $binary).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "LocalBinary must be a regular octessera-pi file."
  }
  $metadata = if ($PSBoundParameters.ContainsKey("LocalMetadata")) { $LocalMetadata } else { "$binary.metadata.json" }
  if (-not (Test-Path -LiteralPath $metadata -PathType Leaf) -or
    ((Get-Item -LiteralPath $metadata).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "Adjacent Orange metadata must be a regular file."
  }
  $metadata = (Resolve-Path -LiteralPath $metadata).Path
  if ($metadata -cne "$binary.metadata.json") { throw "Only the unchanged adjacent metadata sidecar is supported." }
  $header = New-Object byte[] 20
  $stream = [IO.File]::OpenRead($binary)
  try {
    $headerLength = $stream.Read($header, 0, $header.Length)
  } finally {
    $stream.Dispose()
  }
  if ($headerLength -ne 20 -or $header[0] -ne 127 -or $header[1] -ne 69 -or $header[2] -ne 76 -or $header[3] -ne 70 -or
    $header[4] -ne 2 -or $header[5] -ne 1 -or $header[6] -ne 1 -or $header[18] -ne 183 -or $header[19] -ne 0) {
    throw "LocalBinary must be ELF64 little-endian AArch64."
  }
  $root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
  $source = (& git -C $root rev-parse HEAD 2>$null | Out-String).Trim()
  if ($LASTEXITCODE -ne 0 -or $source -cnotmatch '^[0-9a-f]{40}$') { throw "Could not resolve repository HEAD." }
  Import-Module (Join-Path $PSScriptRoot "orange-cross-metadata.psm1") -Force
  $spec = [pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w"; ArtifactKind = "runtime-candidate" }
  Assert-OrangeBuildMetadata -MetadataPath $metadata -BinaryPath $binary -SelectedBinary "octessera-pi" `
    -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec $spec -SourceCommit $source
  $expectedJson = ConvertTo-OrangeBuildMetadataJson -BinaryPath $binary -SelectedBinary "octessera-pi" `
    -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec $spec -SourceCommit $source
  if ([IO.File]::ReadAllText($metadata) -cne "$expectedJson`n") { throw "Orange sidecar is not the unmodified builder output." }
  $hash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
  $stage = "/tmp/octessera-dev-$([guid]::NewGuid().ToString('N'))"
}

function Invoke-OrangePayload {
  param([string]$Action)
  $payloadPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-dev-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    $prefix = "ACTION='$Action'`nSOURCE='$source'`nHASH='$hash'`nSTAGE='$stage'`n"
    $contents = $prefix + [IO.File]::ReadAllText($remoteScript)
    [IO.File]::WriteAllText($payloadPath, $contents, (New-Object System.Text.UTF8Encoding($false)))
    & $transport ssh-payload -Target $Target $payloadPath
    if ($LASTEXITCODE -ne 0) { throw "Orange $Action payload failed (exit $LASTEXITCODE)." }
  } finally {
    if (Test-Path -LiteralPath $payloadPath) { Remove-Item -LiteralPath $payloadPath -Force }
  }
}

if ($Restore) {
  Invoke-OrangePayload "restore"
  return
}

Invoke-OrangePayload "preflight"
try {
  & $transport scp -Target $Target $binary "${Target}:$stage/octessera-pi"
  if ($LASTEXITCODE -ne 0) { throw "Orange binary staging failed (exit $LASTEXITCODE)." }
  & $transport scp -Target $Target $metadata "${Target}:$stage/octessera-pi.metadata.json"
  if ($LASTEXITCODE -ne 0) { throw "Orange metadata staging failed (exit $LASTEXITCODE)." }
  Invoke-OrangePayload "activate"
} finally {
  Invoke-OrangePayload "cleanup"
}
