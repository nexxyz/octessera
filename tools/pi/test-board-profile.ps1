$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "board-profile.ps1")

Assert-RaspberryBoardProfile "raspberry-pi-zero-2w"
Assert-OctesseraServiceName "octessera.service"
foreach ($profileCase in @(
    [pscustomobject]@{
      Profile = "raspberry-pi-zero-2w"
      Feature = "hardware-raspberry-pi-zero-2w"
    },
    [pscustomobject]@{
      Profile = "orange-pi-zero-2w"
      Feature = "hardware-orange-pi-zero-2w"
    }
  )) {
  Assert-PiBoardProfile $profileCase.Profile
  $spec = Get-PiBoardProfileSpec $profileCase.Profile
  if ($spec.ProfileId -cne $profileCase.Profile -or $spec.CargoFeature -cne $profileCase.Feature -or $spec.Binary -cne "octessera-pi" -or $spec.Architecture -cne "aarch64-unknown-linux-gnu") {
    throw "Board profile mapping is incorrect for $($profileCase.Profile)"
  }
}
foreach ($value in @("opi-zero-2w", "rpi-zero-2w", "hardware-pi", "")) {
  $rejected = $false
  try {
    Assert-PiBoardProfile $value
  } catch {
    $rejected = $true
  }
  if (-not $rejected) {
    throw "Pi cross-build accepted non-canonical profile: $value"
  }
}
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
$crossBuildScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "build-pi-cross.ps1"))
$wslCrossBuildScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "build-pi-cross-wsl.sh"))
if ($crossBuildScript -notmatch "--no-default-features --features") {
  throw "Pi PowerShell cross-build must disable default features before selecting a board feature."
}
if ($wslCrossBuildScript -notmatch "--no-default-features --features") {
  throw "Pi WSL cross-build must disable default features before selecting a board feature."
}
if ($crossBuildScript -notmatch '(?m)\$dockerfilePath\s*=\s*Join-Path \$buildContext "Dockerfile\.pi-zero"') {
  throw "Pi PowerShell cross-build must place its Dockerfile in the temporary build context."
}
if ($crossBuildScript -notmatch '(?m)Copy-Item -LiteralPath \(Join-Path \$RepoRoot "Dockerfile\.pi-zero"\) -Destination \$dockerfilePath') {
  throw "Pi PowerShell cross-build must copy the repository Dockerfile into the temporary build context."
}
if ($crossBuildScript -notmatch '(?m)& docker build\s+-f\s+\$dockerfilePath\s+-t\s+\$Image\s+\$buildContext') {
  throw "Pi PowerShell cross-build must use the temporary context and Dockerfile."
}
if ($crossBuildScript -match '(?m)& docker build .* \.\s*$') {
  throw "Pi PowerShell cross-build must not use the repository root as Docker build context."
}
if ($wslCrossBuildScript -notmatch '(?m)DOCKERFILE="\$BUILD_CONTEXT/Dockerfile\.pi-zero"') {
  throw "Pi WSL cross-build must place its Dockerfile in the temporary build context."
}
if ($wslCrossBuildScript -notmatch '(?m)cp "\$PWD/Dockerfile\.pi-zero" "\$DOCKERFILE"') {
  throw "Pi WSL cross-build must copy the repository Dockerfile into the temporary build context."
}
if ($wslCrossBuildScript -notmatch '(?m)docker build\s+-f\s+"\$DOCKERFILE"\s+-t\s+"\$IMAGE"\s+"\$BUILD_CONTEXT"') {
  throw "Pi WSL cross-build must use the temporary context and Dockerfile."
}
if ($wslCrossBuildScript -match '(?m)docker build .* \.\s*$') {
  throw "Pi WSL cross-build must not use the repository root as Docker build context."
}
if ($crossBuildScript -notmatch "Write-RaspberryBoardMetadata.*SourceCommit.*BinaryPath") {
  throw "Pi cross-build must bind Raspberry metadata to the source commit and output binary."
}
$statusCheckIndex = $crossBuildScript.IndexOf("status --porcelain --untracked-files=all", [StringComparison]::Ordinal)
$backendSwitchIndex = $crossBuildScript.IndexOf('switch ($selectedBackend)', [StringComparison]::Ordinal)
if ($statusCheckIndex -lt 0 -or $backendSwitchIndex -lt 0 -or $statusCheckIndex -gt $backendSwitchIndex -or $crossBuildScript.IndexOf("Authoritative Pi builds require a clean repository", [StringComparison]::Ordinal) -lt 0) {
  throw "Pi cross-builder must reject dirty source before starting an authoritative build."
}
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
$binaryPath = Join-Path ([IO.Path]::GetTempPath()) "octessera-board-profile-test.bin"
$swappedBinaryPath = Join-Path ([IO.Path]::GetTempPath()) "octessera-board-profile-test-swapped.bin"
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
  foreach ($profileCase in @(
      [pscustomobject]@{
        Profile = "raspberry-pi-zero-2w"
        Json = '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
      },
      [pscustomobject]@{
        Profile = "orange-pi-zero-2w"
        Json = '{"schema_version":1,"board_profile":"orange-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-orange-pi-zero-2w"}'
      }
    )) {
    Write-PiBoardMetadata -Path $metadataPath -BoardProfile $profileCase.Profile
    $json = $utf8NoBom.GetString([IO.File]::ReadAllBytes($metadataPath))
    if ($json -cne $profileCase.Json) {
      throw "Pi metadata JSON is not canonical for $($profileCase.Profile)"
    }
  }

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
  $sourceCommit = "a" * 40
  [IO.File]::WriteAllBytes($binaryPath, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7, 0x00))
  Write-RaspberryBoardMetadata $metadataPath -SourceCommit $sourceCommit -BinaryPath $binaryPath
  $buildMetadata = Read-RaspberryBoardMetadata $metadataPath
  Assert-RaspberryBuildMetadata -Metadata $buildMetadata -SourceCommit $sourceCommit -BinaryPath $binaryPath | Out-Null
  $authoritativeMetadataText = [IO.File]::ReadAllText($metadataPath)
  if ($authoritativeMetadataText -notmatch '"binary_sha256":"[0-9a-f]{64}"') { throw "Raspberry authoritative metadata omitted binary_sha256." }
  $missingBinaryHash = $authoritativeMetadataText -replace ',"binary_sha256":"[0-9a-f]{64}"', ''
  [IO.File]::WriteAllText($metadataPath, $missingBinaryHash, $utf8NoBom)
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "missing Raspberry artifact hash"
  $malformedBinaryHash = $authoritativeMetadataText -replace '"binary_sha256":"[0-9a-f]{64}"', ('"binary_sha256":"' + ("g" * 64) + '"')
  [IO.File]::WriteAllText($metadataPath, $malformedBinaryHash, $utf8NoBom)
  Assert-Rejected { Read-RaspberryBoardMetadata $metadataPath | Out-Null } "malformed Raspberry artifact hash"
  Write-RaspberryBoardMetadata $metadataPath -SourceCommit $sourceCommit -BinaryPath $binaryPath
  [IO.File]::WriteAllBytes($binaryPath, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7, 0x01))
  Assert-Rejected { Assert-RaspberryBuildMetadata -Metadata (Read-RaspberryBoardMetadata $metadataPath) -SourceCommit $sourceCommit -BinaryPath $binaryPath | Out-Null } "changed Raspberry artifact bytes"
  [IO.File]::WriteAllBytes($binaryPath, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7, 0x00))
  [IO.File]::WriteAllBytes($swappedBinaryPath, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7, 0x01))
  Assert-Rejected { Assert-RaspberryBuildMetadata -Metadata (Read-RaspberryBoardMetadata $metadataPath) -SourceCommit $sourceCommit -BinaryPath $swappedBinaryPath | Out-Null } "swapped Raspberry artifact"
  $mismatchedBuildMetadata = [regex]::Replace([IO.File]::ReadAllText($metadataPath), '"source_commit":"[0-9a-f]{40}"', ('"source_commit":"' + ("b" * 40) + '"'))
  [IO.File]::WriteAllText($metadataPath, $mismatchedBuildMetadata, $utf8NoBom)
  Assert-Rejected { Assert-RaspberryBuildMetadata -Metadata (Read-RaspberryBoardMetadata $metadataPath) -SourceCommit $sourceCommit -BinaryPath $binaryPath | Out-Null } "mismatched Raspberry build commit"
  $staleBuildMetadata = [regex]::Replace($mismatchedBuildMetadata, '"source_commit":"[0-9a-f]{40}"', ('"source_commit":"' + ("c" * 40) + '"'))
  [IO.File]::WriteAllText($metadataPath, $staleBuildMetadata, $utf8NoBom)
  Assert-Rejected { Assert-RaspberryBuildMetadata -Metadata (Read-RaspberryBoardMetadata $metadataPath) -SourceCommit $sourceCommit -BinaryPath $binaryPath | Out-Null } "stale Raspberry artifact metadata"
  Write-RaspberryBoardMetadata $metadataPath

  $validSystemEvidence = @(
    "raspberry_system_sample phase=startup thermal_max_millicelsius=50000 mem_available_kb=600000 throttled=0x0 current_throttled_mask=0 undervoltage=0",
    "raspberry_system_sample phase=runtime thermal_max_millicelsius=60000 mem_available_kb=580000 throttled=0x0 current_throttled_mask=0 undervoltage=0"
  ) -join "`n"
  $systemSummary = Assert-RaspberrySystemEvidence $validSystemEvidence
  if ($systemSummary.StartupSampleCount -ne 1 -or $systemSummary.RuntimeSampleCount -ne 1) { throw "Valid Raspberry system evidence was not summarized." }
  Assert-Rejected { Assert-RaspberrySystemEvidence ($validSystemEvidence.Split("`n")[0]) } "missing runtime thermal evidence"
  Assert-Rejected { Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("thermal_max_millicelsius=60000", "thermal_max_millicelsius=bad")) } "malformed thermal evidence"
  Assert-Rejected { Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("throttled=0x0 current_throttled_mask=0", "throttled=0x4 current_throttled_mask=4")) } "active throttling evidence"
  Assert-Rejected { Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("throttled=0x0 current_throttled_mask=0 undervoltage=0", "throttled=0x1 current_throttled_mask=1 undervoltage=1")) } "undervoltage evidence"
  Assert-Rejected { Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("thermal_max_millicelsius=50000", "thermal_max_millicelsius=70000")) } "startup temperature limit"
  Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("thermal_max_millicelsius=60000", "thermal_max_millicelsius=70000")) | Out-Null
  Assert-Rejected { Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("thermal_max_millicelsius=60000", "thermal_max_millicelsius=75000")) } "runtime temperature limit"
  Assert-RaspberrySystemEvidence ($validSystemEvidence.Replace("throttled=0x0 current_throttled_mask=0", "throttled=0x10000 current_throttled_mask=0")) | Out-Null

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
  Remove-Item -LiteralPath $metadataPath, $binaryPath, $swappedBinaryPath -Force -ErrorAction SilentlyContinue
}

Write-Output "PowerShell Raspberry board profile validation passed"
