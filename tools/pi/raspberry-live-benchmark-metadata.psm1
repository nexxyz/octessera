Set-StrictMode -Version Latest

$script:RaspberryLiveBenchmarkCargoFeature = "hardware-raspberry-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128"
$script:RaspberryLiveBenchmarkFields = @(
  "schema_version",
  "board_profile",
  "binary",
  "arch",
  "package_version",
  "artifact_kind",
  "profile",
  "cargo_feature",
  "source_commit",
  "binary_sha256"
)

function Get-RaspberryLiveBenchmarkCargoFeature {
  return $script:RaspberryLiveBenchmarkCargoFeature
}

function Get-RaspberryLiveBenchmarkBinarySha256 {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Raspberry live benchmark artifact was not found: $Path"
  }
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function New-RaspberryLiveBenchmarkMetadata {
  param(
    [Parameter(Mandatory)][string]$SourceCommit,
    [Parameter(Mandatory)][string]$BinaryPath,
    [string]$PackageVersion = "0.8.2"
  )
  if ($SourceCommit -notmatch '^[0-9a-f]{40}$') {
    throw "Raspberry live benchmark source_commit must be a full lowercase commit identity."
  }
  if ([string]::IsNullOrWhiteSpace($PackageVersion)) {
    throw "Raspberry live benchmark package_version must not be empty."
  }
  return [pscustomobject][ordered]@{
    schema_version = 1
    board_profile = "raspberry-pi-zero-2w"
    binary = "octessera-pi"
    arch = "aarch64-unknown-linux-gnu"
    package_version = $PackageVersion
    artifact_kind = "diagnostic-only"
    profile = "release"
    cargo_feature = $script:RaspberryLiveBenchmarkCargoFeature
    source_commit = $SourceCommit
    binary_sha256 = Get-RaspberryLiveBenchmarkBinarySha256 $BinaryPath
  }
}

function Assert-RaspberryLiveBenchmarkMetadata {
  param(
    [Parameter(Mandatory)][object]$Metadata,
    [Parameter(Mandatory)][string]$SourceCommit,
    [Parameter(Mandatory)][string]$BinaryPath
  )
  $properties = @($Metadata.PSObject.Properties)
  $names = @($properties | ForEach-Object { $_.Name })
  if ($names.Count -ne $script:RaspberryLiveBenchmarkFields.Count -or @($script:RaspberryLiveBenchmarkFields | Where-Object { $names -notcontains $_ }).Count -gt 0) {
    throw "Raspberry live benchmark metadata fields are not exact."
  }
  if ($Metadata.schema_version -ne 1 -or $Metadata.board_profile -cne "raspberry-pi-zero-2w" -or $Metadata.binary -cne "octessera-pi" -or $Metadata.arch -cne "aarch64-unknown-linux-gnu" -or $Metadata.artifact_kind -cne "diagnostic-only" -or $Metadata.profile -cne "release" -or $Metadata.cargo_feature -cne $script:RaspberryLiveBenchmarkCargoFeature) {
    throw "Raspberry live benchmark metadata identity is invalid."
  }
  if ($Metadata.source_commit -cne $SourceCommit -or $Metadata.source_commit -notmatch '^[0-9a-f]{40}$') {
    throw "Raspberry live benchmark metadata source_commit does not match the requested repository HEAD."
  }
  $hash = Get-RaspberryLiveBenchmarkBinarySha256 $BinaryPath
  if ($Metadata.binary_sha256 -cne $hash) {
    throw "Raspberry live benchmark metadata binary_sha256 does not match the selected artifact."
  }
  return $Metadata
}

function Read-RaspberryLiveBenchmarkMetadata {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Raspberry live benchmark metadata was not found: $Path"
  }
  return (Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json)
}

function Write-RaspberryLiveBenchmarkMetadata {
  param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$SourceCommit,
    [Parameter(Mandatory)][string]$BinaryPath
  )
  $metadata = New-RaspberryLiveBenchmarkMetadata -SourceCommit $SourceCommit -BinaryPath $BinaryPath
  $metadata | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Path -Encoding UTF8
}

Export-ModuleMember -Function Get-RaspberryLiveBenchmarkCargoFeature, Get-RaspberryLiveBenchmarkBinarySha256, New-RaspberryLiveBenchmarkMetadata, Assert-RaspberryLiveBenchmarkMetadata, Read-RaspberryLiveBenchmarkMetadata, Write-RaspberryLiveBenchmarkMetadata
