Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "..\performance\performance-baseline-json.psm1") -Force

$RaspberryPiZero2WProfileId = "raspberry-pi-zero-2w"
$OrangePiZero2WProfileId = "orange-pi-zero-2w"
$RaspberryPiZero2WSchemaVersion = 1
$RaspberryPiZero2WBinary = "octessera-pi"
$RaspberryPiZero2WArchitecture = "aarch64-unknown-linux-gnu"
$RaspberryPiZero2WRuntimeArchitecture = "aarch64"
$RaspberryPiZero2WCargoFeature = "hardware-raspberry-pi-zero-2w"
$OrangePiZero2WCargoFeature = "hardware-orange-pi-zero-2w"
$PiBinary = $RaspberryPiZero2WBinary
$PiArchitecture = $RaspberryPiZero2WArchitecture
$RaspberryBoardMetadataFields = @(
  "schema_version",
  "board_profile",
  "binary",
  "arch",
  "cargo_feature"
)
$RaspberryRuntimeMetadataFields = @(
  "schema_version",
  "board_profile",
  "binary",
  "arch",
  "package_version"
)

function Assert-PiSourceCommit {
  param([Parameter(Mandatory)][string]$SourceCommit)
  if ($SourceCommit -notmatch '^[0-9a-f]{40}$') {
    throw "Raspberry build metadata source_commit must be a full lowercase commit identity."
  }
}

function Get-RaspberryBinarySha256 {
  param([Parameter(Mandatory)][string]$BinaryPath)
  if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw "Raspberry build metadata binary was not found: $BinaryPath"
  }
  $hash = (Get-FileHash -LiteralPath $BinaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($hash -notmatch '^[0-9a-f]{64}$') {
    throw "Raspberry build metadata binary SHA-256 is not canonical lowercase hex: $BinaryPath"
  }
  $hash
}

function Get-PiBoardProfileSpec {
  param([string]$BoardProfile)

  switch -CaseSensitive ($BoardProfile) {
    $RaspberryPiZero2WProfileId {
      return [pscustomobject][ordered]@{
        ProfileId = $RaspberryPiZero2WProfileId
        CargoFeature = $RaspberryPiZero2WCargoFeature
        Binary = $PiBinary
        Architecture = $PiArchitecture
      }
    }
    $OrangePiZero2WProfileId {
      return [pscustomobject][ordered]@{
        ProfileId = $OrangePiZero2WProfileId
        CargoFeature = $OrangePiZero2WCargoFeature
        Binary = $PiBinary
        Architecture = $PiArchitecture
      }
    }
    default {
      throw "Pi cross-build accepts only '$RaspberryPiZero2WProfileId' or '$OrangePiZero2WProfileId'; got '$BoardProfile'."
    }
  }
}

function Assert-PiBoardProfile {
  param([string]$BoardProfile)

  Get-PiBoardProfileSpec $BoardProfile | Out-Null
}

function New-PiBoardMetadata {
  param(
    [string]$BoardProfile,
    [string]$Binary = $PiBinary
  )

  $spec = Get-PiBoardProfileSpec $BoardProfile
  if ($Binary -cne $spec.Binary) {
    throw "Pi board metadata binary must be $($spec.Binary); got '$Binary'."
  }
  [pscustomobject][ordered]@{
    schema_version = 1
    board_profile = $spec.ProfileId
    binary = $spec.Binary
    arch = $spec.Architecture
    cargo_feature = $spec.CargoFeature
  }
}

function Get-PiBoardMetadataJson {
  param([string]$BoardProfile)

  $metadata = New-PiBoardMetadata $BoardProfile
  '{"schema_version":1,"board_profile":"' + $metadata.board_profile + '","binary":"' + $metadata.binary + '","arch":"' + $metadata.arch + '","cargo_feature":"' + $metadata.cargo_feature + '"}'
}

function Write-PiBoardMetadata {
  param(
    [string]$Path,
    [string]$BoardProfile
  )

  $json = Get-PiBoardMetadataJson $BoardProfile
  $encoding = New-Object System.Text.UTF8Encoding($false)
  [IO.File]::WriteAllText($Path, $json, $encoding)
}

function Assert-RaspberryBoardProfile {
  param([string]$BoardProfile)

  if ($BoardProfile -ceq $OrangePiZero2WProfileId) {
    throw "Orange Pi profile '$OrangePiZero2WProfileId' is not supported by Raspberry Pi tooling; use the separate Armbian workflow."
  }
  if ($BoardProfile -cne $RaspberryPiZero2WProfileId) {
    throw "Raspberry Pi tooling accepts only '$RaspberryPiZero2WProfileId'; got '$BoardProfile'."
  }
}

function Assert-OctesseraServiceName {
  param([string]$Service)

  if ($Service -cne "octessera.service") {
    throw "Pi tooling supports only the managed service name octessera.service; got '$Service'."
  }
}

function New-RaspberryBoardMetadata {
  param(
    [string]$SourceCommit = "",
    [string]$BinaryPath = ""
  )
  if (-not [string]::IsNullOrWhiteSpace($SourceCommit)) { Assert-PiSourceCommit $SourceCommit }
  if ([string]::IsNullOrWhiteSpace($SourceCommit) -and -not [string]::IsNullOrWhiteSpace($BinaryPath)) {
    throw "Raspberry build metadata BinaryPath requires SourceCommit."
  }
  if (-not [string]::IsNullOrWhiteSpace($SourceCommit) -and [string]::IsNullOrWhiteSpace($BinaryPath)) {
    throw "Authoritative Raspberry build metadata requires BinaryPath."
  }
  $metadata = [ordered]@{
    schema_version = $RaspberryPiZero2WSchemaVersion
    board_profile = $RaspberryPiZero2WProfileId
    binary = $RaspberryPiZero2WBinary
    arch = $RaspberryPiZero2WArchitecture
    cargo_feature = $RaspberryPiZero2WCargoFeature
  }
  if (-not [string]::IsNullOrWhiteSpace($SourceCommit)) {
    $metadata.source_commit = $SourceCommit
    $metadata.binary_sha256 = Get-RaspberryBinarySha256 $BinaryPath
  }
  return [pscustomobject]$metadata
}

function Assert-JsonObjectFields {
  param(
    [object]$Metadata,
    [string[]]$Fields,
    [string]$Context
  )

  if ($null -eq $Metadata -or $Metadata -is [array] -or $Metadata -is [string] -or $Metadata -is [ValueType]) {
    throw "$Context must be a JSON object."
  }

  $properties = @($Metadata.PSObject.Properties)
  $propertyNames = @($properties | ForEach-Object { $_.Name })
  if ($propertyNames.Count -ne $Fields.Count) {
    throw "$Context must contain exactly: $($Fields -join ', ')."
  }
  foreach ($field in $Fields) {
    if (-not ($propertyNames -ccontains $field)) {
      throw "$Context is missing required field '$field'."
    }
  }
  foreach ($propertyName in $propertyNames) {
    if (-not ($Fields -ccontains $propertyName)) {
      throw "$Context contains unexpected field '$propertyName'."
    }
  }
}

function Assert-RaspberryBoardMetadata {
  param(
    [object]$Metadata,
    [string]$Context = "Raspberry board metadata"
  )

  $hasSourceCommit = $null -ne $Metadata.PSObject.Properties["source_commit"]
  $hasBinarySha256 = $null -ne $Metadata.PSObject.Properties["binary_sha256"]
  if ($hasSourceCommit -ne $hasBinarySha256) {
    throw "$Context authoritative fields source_commit and binary_sha256 must appear together."
  }
  $fields = if ($hasSourceCommit) { $RaspberryBoardMetadataFields + @("source_commit", "binary_sha256") } else { $RaspberryBoardMetadataFields }
  Assert-JsonObjectFields $Metadata $fields $Context
  if ($Metadata.schema_version -isnot [int] -and $Metadata.schema_version -isnot [long]) {
    throw "$Context schema_version must be integer $RaspberryPiZero2WSchemaVersion."
  }
  if ([long]$Metadata.schema_version -ne $RaspberryPiZero2WSchemaVersion) {
    throw "$Context schema_version must be $RaspberryPiZero2WSchemaVersion."
  }
  Assert-RaspberryBoardProfile ([string]$Metadata.board_profile)
  if ($Metadata.binary -isnot [string] -or $Metadata.binary -cne $RaspberryPiZero2WBinary) {
    throw "$Context binary must be $RaspberryPiZero2WBinary."
  }
  if ($Metadata.arch -isnot [string] -or $Metadata.arch -cne $RaspberryPiZero2WArchitecture) {
    throw "$Context arch must be $RaspberryPiZero2WArchitecture."
  }
  if ($Metadata.cargo_feature -isnot [string] -or $Metadata.cargo_feature -cne $RaspberryPiZero2WCargoFeature) {
    throw "$Context cargo_feature must be $RaspberryPiZero2WCargoFeature."
  }
  if ($hasSourceCommit) {
    if ($Metadata.source_commit -isnot [string]) { throw "$Context source_commit must be a string." }
    Assert-PiSourceCommit $Metadata.source_commit
    if ($Metadata.binary_sha256 -isnot [string] -or $Metadata.binary_sha256 -notmatch '^[0-9a-f]{64}$') { throw "$Context binary_sha256 must be 64 lowercase hexadecimal characters." }
  }
  $Metadata
}

function Assert-RaspberryBuildMetadata {
  param(
    [Parameter(Mandatory)][object]$Metadata,
    [Parameter(Mandatory)][string]$SourceCommit,
    [Parameter(Mandatory)][string]$BinaryPath,
    [string]$Context = "Raspberry build metadata"
  )
  Assert-RaspberryBoardMetadata $Metadata $Context | Out-Null
  Assert-PiSourceCommit $SourceCommit
  if ($null -eq $Metadata.PSObject.Properties["source_commit"] -or $Metadata.source_commit -cne $SourceCommit) {
    throw "$Context source_commit does not match the requested repository HEAD."
  }
  $binarySha256 = Get-RaspberryBinarySha256 $BinaryPath
  if ($Metadata.binary_sha256 -cne $binarySha256) {
    throw "$Context binary_sha256 does not match the selected local artifact."
  }
  $Metadata
}

function Assert-RaspberrySystemEvidence {
  param(
    [Parameter(Mandatory)][string]$Text,
    [string]$Context = "Raspberry system evidence"
  )
  $startupCount = 0
  $runtimeCount = 0
  $maximumTemperature = 0
  $maximumThrottlingMask = 0
  foreach ($line in @($Text -split "`r?`n")) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    if ($line.StartsWith("raspberry_system_error", [StringComparison]::Ordinal) -or $line.StartsWith("raspberry_system_abort", [StringComparison]::Ordinal)) {
      throw "$Context reported invalid or unsafe native system state."
    }
    if (-not $line.StartsWith("raspberry_system_sample", [StringComparison]::Ordinal)) { continue }
    if ($line -notmatch '^raspberry_system_sample phase=(startup|runtime) thermal_max_millicelsius=([0-9]+) mem_available_kb=([0-9]+) throttled=(0x[0-9A-Fa-f]+) current_throttled_mask=([0-9]+) undervoltage=(0|1)$') {
      throw "$Context contains a malformed thermal sample."
    }
    $phase = $Matches[1]
    $temperature = [uint64]$Matches[2]
    $throttled = $Matches[4]
    $currentMask = [uint64]$Matches[5]
    $undervoltage = [uint64]$Matches[6]
    try { $reportedMask = [Convert]::ToUInt64($throttled.Substring(2), 16) -band 15 } catch { throw "$Context contains a malformed throttling mask." }
    if ($reportedMask -ne $currentMask -or (($currentMask -band 1) -ne $undervoltage)) { throw "$Context contains inconsistent throttling evidence." }
    if (($currentMask -band 1) -ne 0 -or $undervoltage -ne 0) { throw "$Context reported active undervoltage." }
    if ($phase -ceq "startup") { $startupCount++ } else { $runtimeCount++ }
    if ($temperature -gt $maximumTemperature) { $maximumTemperature = $temperature }
    if ($currentMask -gt $maximumThrottlingMask) { $maximumThrottlingMask = $currentMask }
  }
  if ($startupCount -lt 1 -or $runtimeCount -lt 1) { throw "$Context is missing startup or continuous runtime thermal evidence." }
  return [pscustomobject]@{ StartupSampleCount = $startupCount; RuntimeSampleCount = $runtimeCount; MaximumTemperatureMillicelsius = $maximumTemperature; MaximumCurrentThrottlingMask = $maximumThrottlingMask }
}

function Write-RaspberryBoardMetadata {
  param(
    [string]$Path,
    [string]$Binary = "octessera-pi",
    [string]$CargoFeature = "hardware-raspberry-pi-zero-2w",
    [string]$SourceCommit = "",
    [string]$BinaryPath = ""
  )

  $metadata = New-RaspberryBoardMetadata -SourceCommit $SourceCommit -BinaryPath $BinaryPath
  $metadata.binary = $Binary
  $metadata.cargo_feature = $CargoFeature
  Assert-RaspberryBoardMetadata ([pscustomobject]$metadata) | Out-Null
  $json = Get-RaspberryBoardMetadataJson -SourceCommit $SourceCommit -BinaryPath $BinaryPath
  $encoding = New-Object System.Text.UTF8Encoding($false)
  [IO.File]::WriteAllText($Path, $json, $encoding)
}

function Get-RaspberryBoardMetadataJson {
  param(
    [string]$SourceCommit = "",
    [string]$BinaryPath = ""
  )
  $metadata = New-RaspberryBoardMetadata -SourceCommit $SourceCommit -BinaryPath $BinaryPath
  Assert-RaspberryBoardMetadata $metadata | Out-Null
  $json = '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"'
  if (-not [string]::IsNullOrWhiteSpace($SourceCommit)) {
    $json += ',"source_commit":"' + $SourceCommit + '","binary_sha256":"' + $metadata.binary_sha256 + '"'
  }
  $json + '}'
}

function Read-RaspberryBoardMetadata {
  param([string]$Path)

  if (-not (Test-Path -LiteralPath $Path)) {
    throw "Missing board metadata: $Path"
  }
  $json = Read-StrictUtf8Text $Path "Raspberry board metadata"
  $metadata = ConvertFrom-StrictJsonText $json "Raspberry board metadata"
  Assert-RaspberryBoardMetadata $metadata "Raspberry board metadata '$Path'"
}

function Assert-RaspberryRuntimeMetadata {
  param(
    [object]$Metadata,
    [string]$Context = "Raspberry runtime metadata"
  )

  Assert-JsonObjectFields $Metadata $RaspberryRuntimeMetadataFields $Context
  if ($Metadata.schema_version -isnot [int] -and $Metadata.schema_version -isnot [long]) {
    throw "$Context schema_version must be integer $RaspberryPiZero2WSchemaVersion."
  }
  if ([long]$Metadata.schema_version -ne $RaspberryPiZero2WSchemaVersion) {
    throw "$Context schema_version must be $RaspberryPiZero2WSchemaVersion."
  }
  Assert-RaspberryBoardProfile ([string]$Metadata.board_profile)
  if ($Metadata.binary -isnot [string] -or $Metadata.binary -cne $RaspberryPiZero2WBinary) {
    throw "$Context binary must be $RaspberryPiZero2WBinary."
  }
  if ($Metadata.arch -isnot [string] -or $Metadata.arch -cne $RaspberryPiZero2WRuntimeArchitecture) {
    throw "$Context arch must be $RaspberryPiZero2WRuntimeArchitecture."
  }
  if ($Metadata.package_version -isnot [string] -or [string]::IsNullOrWhiteSpace($Metadata.package_version)) {
    throw "$Context package_version must be a non-empty string."
  }
  $Metadata
}

function Compare-RaspberryRuntimeMetadata {
  param(
    [object]$RuntimeMetadata,
    [object]$ExpectedMetadata
  )

  Assert-RaspberryRuntimeMetadata $RuntimeMetadata | Out-Null
  Assert-RaspberryBoardMetadata $ExpectedMetadata | Out-Null
  if ([long]$RuntimeMetadata.schema_version -ne [long]$ExpectedMetadata.schema_version) {
    throw "Candidate runtime metadata schema_version does not match local metadata."
  }
  if ($RuntimeMetadata.board_profile -cne $ExpectedMetadata.board_profile) {
    throw "Candidate runtime metadata board_profile does not match local metadata."
  }
  if ($RuntimeMetadata.binary -cne $ExpectedMetadata.binary) {
    throw "Candidate runtime metadata binary does not match local metadata."
  }
  $expectedArchitecture = ([string]$ExpectedMetadata.arch).Split("-", 2)[0]
  if ($RuntimeMetadata.arch -cne $expectedArchitecture) {
    throw "Candidate runtime metadata arch does not match local metadata."
  }
  $RuntimeMetadata
}
