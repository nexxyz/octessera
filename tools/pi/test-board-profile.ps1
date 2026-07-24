$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "board-profile.ps1")

Assert-RaspberryBoardProfile "raspberry-pi-zero-2w"
Assert-OctesseraServiceName "octessera.service"
try {
  Assert-OctesseraServiceName "other.service"
  throw "Pi tooling accepted a non-default service name"
} catch {
  if ($_.Exception.Message -like "Pi tooling accepted*") {
    throw
  }
}
foreach ($value in @(
    "orange-pi-zero-2w",
    "opi-zero-2w",
    "rpi-zero-2w",
    "pi-zero-2w",
    "hardware-rpi-zero-2w",
    "hardware-pi"
  )) {
  try {
    Assert-RaspberryBoardProfile $value
    throw "Raspberry tooling accepted non-canonical profile: $value"
  } catch {
    if ($_.Exception.Message -like "Raspberry tooling accepted*") {
      throw
    }
  }
}

$deployScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "deploy-pi-fast.ps1"))
$remoteDeployScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "deploy-pi-fast-remote.sh"))
$candidateCheckIndex = $remoteDeployScript.IndexOf('"$STAGING_RELEASE/octessera-pi" --print-build-metadata', [StringComparison]::Ordinal)
$activationIndex = $remoteDeployScript.LastIndexOf('ACTIVATED=1', [StringComparison]::Ordinal)
$serviceIndex = $remoteDeployScript.LastIndexOf('systemctl restart "$SERVICE"', [StringComparison]::Ordinal)
if ($candidateCheckIndex -lt 0) {
  throw "Fast deployment does not validate candidate build metadata"
}
if ($candidateCheckIndex -lt 0 -or $activationIndex -lt 0 -or $candidateCheckIndex -ge $activationIndex -or $activationIndex -ge $serviceIndex) {
  throw "Fast deployment metadata validation must precede activation and service restart"
}
if ($deployScript.IndexOf("ConvertTo-PosixShellSingleQuoted", [StringComparison]::Ordinal) -lt 0 -or $deployScript.IndexOf("deploy-pi-fast-lock.py", [StringComparison]::Ordinal) -lt 0 -or $deployScript.IndexOf("sudo python3", [StringComparison]::Ordinal) -lt 0) {
  throw "Fast deployment is missing POSIX path quoting"
}

$metadataPath = Join-Path ([IO.Path]::GetTempPath()) "octessera-board-profile-test.json"
$utf8NoBom = New-Object System.Text.UTF8Encoding($false, $true)

function Write-TestJson {
  param([string]$Json)

  [IO.File]::WriteAllText($metadataPath, $Json, $utf8NoBom)
}

function Assert-Rejected {
  param(
    [scriptblock]$Action,
    [string]$Label
  )

  $rejected = $false
  try {
    & $Action
  } catch {
    $rejected = $true
  }
  if (-not $rejected) {
    throw "Expected metadata test case to be rejected: $Label"
  }
}

try {
  Write-RaspberryBoardMetadata $metadataPath
  $bytes = [IO.File]::ReadAllBytes($metadataPath)
  if ($bytes.Length -lt 3 -or $bytes[0] -eq 0xEF -or $bytes[1] -eq 0xBB -or $bytes[2] -eq 0xBF) {
    throw "Raspberry metadata must be BOM-less UTF-8"
  }
  $json = $utf8NoBom.GetString($bytes)
  $expectedJson = '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
  if ($json -cne $expectedJson) {
    throw "Raspberry metadata JSON is not the strict canonical byte representation"
  }
  Read-RaspberryBoardMetadata $metadataPath | Out-Null

  $validFields = [ordered]@{
    schema_version = 1
    board_profile = "raspberry-pi-zero-2w"
    binary = "octessera-pi"
    arch = "aarch64-unknown-linux-gnu"
    cargo_feature = "hardware-raspberry-pi-zero-2w"
  }
  foreach ($field in $validFields.Keys) {
    $missing = [ordered]@{}
    foreach ($name in $validFields.Keys) {
      if ($name -cne $field) {
        $missing[$name] = $validFields[$name]
      }
    }
    Write-TestJson ($missing | ConvertTo-Json -Compress)
    Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "missing $field"
  }

  $invalidValues = [ordered]@{
    schema_version = 2
    board_profile = "orange-pi-zero-2w"
    binary = "other-binary"
    arch = "x86_64-unknown-linux-gnu"
    cargo_feature = "hardware-pi"
  }
  foreach ($field in $invalidValues.Keys) {
    $invalid = [ordered]@{}
    foreach ($name in $validFields.Keys) {
      $invalid[$name] = if ($name -ceq $field) { $invalidValues[$name] } else { $validFields[$name] }
    }
    Write-TestJson ($invalid | ConvertTo-Json -Compress)
    Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "invalid $field"
  }
  $invalidTypes = [ordered]@{
    schema_version = "1"
    board_profile = 1
    binary = 1
    arch = 1
    cargo_feature = 1
  }
  foreach ($field in $invalidTypes.Keys) {
    $invalid = [ordered]@{}
    foreach ($name in $validFields.Keys) {
      $invalid[$name] = if ($name -ceq $field) { $invalidTypes[$name] } else { $validFields[$name] }
    }
    Write-TestJson ($invalid | ConvertTo-Json -Compress)
    Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "wrong type $field"
  }

  Write-TestJson '{"schema_version":"1","board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "schema_version string"
  Write-TestJson '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w","extra":true}'
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "unexpected field"
  Write-TestJson '{"schema_version":1,"Schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "field name casing"
  Write-TestJson '{"schema_version":1,"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "duplicate field"
  Write-TestJson '{"schema_version":1,"schema\u005fversion":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "escaped duplicate field"
  foreach ($malformed in @("", "{", "null", "[]", "not-json")) {
    Write-TestJson $malformed
    Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "malformed JSON '$malformed'"
  }

  $expectedMetadata = New-RaspberryBoardMetadata
  $runtimeFields = [ordered]@{
    schema_version = 1
    board_profile = "raspberry-pi-zero-2w"
    binary = "octessera-pi"
    arch = "aarch64"
    package_version = "0.7.0"
  }
  $runtimeJson = $runtimeFields | ConvertTo-Json -Compress
  $runtimeMetadata = ConvertFrom-StrictJsonText $runtimeJson "runtime metadata"
  Compare-RaspberryRuntimeMetadata $runtimeMetadata $expectedMetadata | Out-Null
  foreach ($field in $runtimeFields.Keys) {
    $missing = [ordered]@{}
    foreach ($name in $runtimeFields.Keys) {
      if ($name -cne $field) {
        $missing[$name] = $runtimeFields[$name]
      }
    }
    $missingJson = $missing | ConvertTo-Json -Compress
    Assert-Rejected {
      $missingMetadata = ConvertFrom-StrictJsonText $missingJson "runtime metadata"
      Assert-RaspberryRuntimeMetadata $missingMetadata | Out-Null
    } "runtime missing $field"
  }
  $runtimeInvalidValues = [ordered]@{
    schema_version = 2
    board_profile = "orange-pi-zero-2w"
    binary = "other-binary"
    arch = "x86_64"
    package_version = ""
  }
  foreach ($field in $runtimeInvalidValues.Keys) {
    $invalid = [ordered]@{}
    foreach ($name in $runtimeFields.Keys) {
      $invalid[$name] = if ($name -ceq $field) { $runtimeInvalidValues[$name] } else { $runtimeFields[$name] }
    }
    $invalidJson = $invalid | ConvertTo-Json -Compress
    Assert-Rejected {
      $invalidMetadata = ConvertFrom-StrictJsonText $invalidJson "runtime metadata"
      Assert-RaspberryRuntimeMetadata $invalidMetadata | Out-Null
    } "runtime invalid $field"
  }

  [IO.File]::WriteAllText($metadataPath, $expectedJson, (New-Object System.Text.UTF8Encoding($true)))
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "UTF-8 BOM"
  [IO.File]::WriteAllBytes($metadataPath, [byte[]](0x7B, 0xFF, 0x7D))
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "invalid UTF-8"
  Assert-Rejected { Write-RaspberryBoardMetadata $metadataPath -Binary "wrong" } "writer binary"
  Assert-Rejected { Write-RaspberryBoardMetadata $metadataPath -CargoFeature "hardware-pi" } "writer cargo feature"
} finally {
  Remove-Item -LiteralPath $metadataPath -Force -ErrorAction SilentlyContinue
}

Write-Output "PowerShell Raspberry board profile validation passed"
