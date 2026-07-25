Set-StrictMode -Version Latest

$script:OrangeBoardProfile = "orange-pi-zero-2w"
$script:OrangeArtifactKind = "diagnostic-only"
$script:OrangeSchemaVersion = 2

function Get-OrangeBinarySha256 {
  param(
    [Parameter(Mandatory)][string]$BinaryPath
  )

  if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw "Copied binary is missing: $BinaryPath"
  }

  $hash = (Get-FileHash -LiteralPath $BinaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($hash -notmatch '^[0-9a-f]{64}$') {
    throw "Copied ELF SHA-256 is not canonical lowercase hex: $BinaryPath"
  }
  return $hash
}

function New-OrangeBuildMetadata {
  param(
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][string]$SelectedTarget,
    [Parameter(Mandatory)][string]$SelectedProfile,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec
  )

  return [ordered]@{
    schema_version = $script:OrangeSchemaVersion
    board_profile = $script:OrangeBoardProfile
    artifact_kind = $script:OrangeArtifactKind
    runtime_ready = $false
    binary = $SelectedBinary
    package = $BuildSpec.Package
    arch = $SelectedTarget
    cargo_feature = $BuildSpec.Feature
    profile = $SelectedProfile
    binary_sha256 = Get-OrangeBinarySha256 $BinaryPath
  }
}

function ConvertTo-OrangeBuildMetadataJson {
  param(
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][string]$SelectedTarget,
    [Parameter(Mandatory)][string]$SelectedProfile,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec
  )

  return (New-OrangeBuildMetadata `
    -BinaryPath $BinaryPath `
    -SelectedBinary $SelectedBinary `
    -SelectedTarget $SelectedTarget `
    -SelectedProfile $SelectedProfile `
    -BuildSpec $BuildSpec) | ConvertTo-Json -Compress
}

function Publish-OrangeBuildMetadata {
  param(
    [Parameter(Mandatory)][string]$MetadataPath,
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][string]$SelectedTarget,
    [Parameter(Mandatory)][string]$SelectedProfile,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec
  )

  $json = ConvertTo-OrangeBuildMetadataJson `
    -BinaryPath $BinaryPath `
    -SelectedBinary $SelectedBinary `
    -SelectedTarget $SelectedTarget `
    -SelectedProfile $SelectedProfile `
    -BuildSpec $BuildSpec
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  $temporaryMetadataPath = "$MetadataPath.tmp-$PID"
  try {
    [IO.File]::WriteAllText($temporaryMetadataPath, "$json`n", $utf8NoBom)
    Move-Item -LiteralPath $temporaryMetadataPath -Destination $MetadataPath -Force
  } finally {
    Remove-Item -LiteralPath $temporaryMetadataPath -Force -ErrorAction SilentlyContinue
  }
}

function Read-OrangeBuildMetadata {
  param(
    [Parameter(Mandatory)][string]$MetadataPath
  )

  if (-not (Test-Path -LiteralPath $MetadataPath -PathType Leaf)) {
    throw "Build finished without metadata: $MetadataPath"
  }

  $bytes = [IO.File]::ReadAllBytes($MetadataPath)
  if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
    throw "Build metadata must be BOM-less UTF-8: $MetadataPath"
  }

  try {
    return (New-Object System.Text.UTF8Encoding($false, $true)).GetString($bytes) | ConvertFrom-Json
  } catch {
    throw "Build metadata is not valid UTF-8 JSON: $MetadataPath"
  }
}

function Assert-OrangeBuildMetadata {
  param(
    [Parameter(Mandatory)][string]$MetadataPath,
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][string]$SelectedTarget,
    [Parameter(Mandatory)][string]$SelectedProfile,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec
  )

  $metadata = Read-OrangeBuildMetadata $MetadataPath
  $expected = New-OrangeBuildMetadata `
    -BinaryPath $BinaryPath `
    -SelectedBinary $SelectedBinary `
    -SelectedTarget $SelectedTarget `
    -SelectedProfile $SelectedProfile `
    -BuildSpec $BuildSpec
  $expectedProperties = @($expected.Keys)
  $properties = @($metadata.PSObject.Properties.Name)
  if ($properties.Count -ne $expectedProperties.Count) {
    throw "Build metadata has an unexpected field set: $MetadataPath"
  }
  foreach ($name in $expectedProperties) {
    if ($null -eq $metadata.PSObject.Properties[$name] -or $metadata.$name -cne $expected[$name]) {
      throw "Build metadata field '$name' is incorrect: $MetadataPath"
    }
  }
}

function Remove-OrangeBuildArtifacts {
  param(
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$MetadataPath
  )

  Remove-Item -LiteralPath $BinaryPath, $MetadataPath -Force -ErrorAction SilentlyContinue
}

function Invoke-VerifiedOrangeBuildMetadata {
  param(
    [Parameter(Mandatory)][string]$MetadataPath,
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][string]$SelectedTarget,
    [Parameter(Mandatory)][string]$SelectedProfile,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec,
    [scriptblock]$MetadataWriter
  )

  $verified = $false
  try {
    if ($null -eq $MetadataWriter) {
      Publish-OrangeBuildMetadata `
        -MetadataPath $MetadataPath `
        -BinaryPath $BinaryPath `
        -SelectedBinary $SelectedBinary `
        -SelectedTarget $SelectedTarget `
        -SelectedProfile $SelectedProfile `
        -BuildSpec $BuildSpec
    } else {
      & $MetadataWriter
    }
    Assert-OrangeBuildMetadata `
      -MetadataPath $MetadataPath `
      -BinaryPath $BinaryPath `
      -SelectedBinary $SelectedBinary `
      -SelectedTarget $SelectedTarget `
      -SelectedProfile $SelectedProfile `
      -BuildSpec $BuildSpec
    $verified = $true
  } finally {
    if (-not $verified) {
      Remove-OrangeBuildArtifacts -BinaryPath $BinaryPath -MetadataPath $MetadataPath
    }
  }
}

Export-ModuleMember -Function @(
  "Assert-OrangeBuildMetadata",
  "ConvertTo-OrangeBuildMetadataJson",
  "Invoke-VerifiedOrangeBuildMetadata",
  "New-OrangeBuildMetadata",
  "Publish-OrangeBuildMetadata",
  "Remove-OrangeBuildArtifacts"
)
