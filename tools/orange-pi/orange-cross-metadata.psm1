Set-StrictMode -Version Latest

function Publish-OrangeBuildMetadata {
  param(
    [Parameter(Mandatory)][string]$MetadataPath,
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$SelectedBinary,
    [Parameter(Mandatory)][string]$SelectedTarget,
    [Parameter(Mandatory)][string]$SelectedProfile,
    [Parameter(Mandatory)][pscustomobject]$BuildSpec,
    [Parameter(Mandatory)][string]$SourceCommit
  )

  if ($SourceCommit -notmatch '^[0-9a-f]{40}$') {
    throw "Orange build metadata source_commit must be a full lowercase commit identity."
  }
  $json = [ordered]@{
    schema_version = 2
    board_profile = "orange-pi-zero-2w"
    artifact_kind = $BuildSpec.ArtifactKind
    runtime_ready = $false
    binary = $SelectedBinary
    package = $BuildSpec.Package
    arch = $SelectedTarget
    cargo_feature = $BuildSpec.Feature
    profile = $SelectedProfile
    binary_sha256 = (Get-FileHash -LiteralPath $BinaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    source_commit = $SourceCommit
  } | ConvertTo-Json -Compress
  [IO.File]::WriteAllText($MetadataPath, "$json`n", (New-Object System.Text.UTF8Encoding($false)))
}

function Remove-OrangeBuildArtifacts {
  param(
    [Parameter(Mandatory)][string]$BinaryPath,
    [string]$MetadataPath
  )

  Remove-Item -LiteralPath @($BinaryPath, $MetadataPath | Where-Object { $_ }) -Force -ErrorAction SilentlyContinue
}

Export-ModuleMember -Function @("Publish-OrangeBuildMetadata", "Remove-OrangeBuildArtifacts")
