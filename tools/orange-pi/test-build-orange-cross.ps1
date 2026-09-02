$ErrorActionPreference = "Stop"

$buildScript = Join-Path $PSScriptRoot "build-orange-cross.ps1"
$metadataModule = Join-Path $PSScriptRoot "orange-cross-metadata.psm1"
$source = [IO.File]::ReadAllText($buildScript)
$metadataSource = [IO.File]::ReadAllText($metadataModule)

foreach ($required in @(
    "docker",
    "run",
    "--rm",
    "octessera-orange-pi-cargo-registry",
    "octessera-orange-pi-cargo-git",
    "octessera-orange-pi-rustup",
    'CARGO_TARGET_DIR=$cargoTargetDirectory',
    "CargoTargetRelativePath",
    "export PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "dpkg --add-architecture arm64",
    "gcc-aarch64-linux-gnu",
    "libc6-dev-arm64-cross",
    "libasound2-dev:arm64",
    "PKG_CONFIG_ALLOW_CROSS=1",
    "PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig",
    "PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig",
    "aarch64-linux-gnu-readelf",
    "Import-Module",
    "Invoke-VerifiedOrangeBuildMetadata",
    "Remove-OrangeBuildArtifacts",
    "orange-seesaw-smoke",
    "octessera-pi",
    "--no-default-features",
    "ELF64",
    "AArch64"
  )) {
  if ($source.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
    throw "Orange cross-builder is missing required operation: $required"
  }
}

foreach ($required in @(
    "Get-FileHash",
    "ConvertTo-Json -Compress",
    "binary_sha256",
    'schema_version = $script:OrangeSchemaVersion',
    'artifact_kind = $BuildSpec.ArtifactKind',
    'runtime_ready = $false',
    "Publish-OrangeBuildMetadata",
    "Assert-OrangeBuildMetadata",
    "Remove-OrangeBuildArtifacts",
    "Invoke-VerifiedOrangeBuildMetadata",
    "WriteAllText",
    "ReadAllBytes",
    "source_commit"
  )) {
  if ($metadataSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
    throw "Orange metadata module is missing required operation: $required"
  }
}

foreach ($forbidden in @(
    "Get-FileHash",
    "ConvertTo-Json",
    "ConvertFrom-Json",
    "WriteAllText",
    "ReadAllBytes",
    "TestMetadata"
  )) {
  if ($source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
    throw "Orange cross-builder must delegate metadata operation to its module: $forbidden"
  }
}

if ($source -match "(?i)\b(ssh|scp|rsync)\b") {
  throw "Orange cross-builder must not contain board transport commands"
}
if ($source -match 'service_ready|runtime_ready\s*=\s*\$true') {
  throw "Orange cross-builder must not define deployment or service-ready output"
}
$statusCheckIndex = $source.IndexOf("status --porcelain --untracked-files=all", [StringComparison]::Ordinal)
$dryRunIndex = $source.IndexOf("if (`$DryRun)", [StringComparison]::Ordinal)
if ($statusCheckIndex -lt 0 -or $dryRunIndex -lt 0 -or $statusCheckIndex -lt $dryRunIndex -or $source.IndexOf("Authoritative Orange builds require a clean repository", [StringComparison]::Ordinal) -lt 0) {
  throw "Orange cross-builder must reject dirty source before authoritative builds while retaining dry-run support."
}

function Invoke-DryRun {
  param([hashtable]$Parameters)

  $output = @(& $buildScript @Parameters -DryRun 2>&1)
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

$default = Invoke-DryRun @{}
foreach ($expected in @(
    "no Docker container was started",
    "target/orange-pi-cross/orange-oled-smoke",
    "orange-oled-smoke.metadata.json",
    "--profile.*pi-dev",
    "-p.*octessera-hal",
    "--features.*orange-pi-zero-2w"
  )) {
  if ($default -notmatch $expected) {
    throw "Default Orange cross-builder dry run is missing: $expected"
  }
}

$hal = Invoke-DryRun @{ Profile = "release" }
foreach ($expected in @(
    "target/orange-pi-cross/orange-oled-smoke",
    "--profile.*release",
    "-p.*octessera-hal",
    "--features.*orange-pi-zero-2w"
  )) {
  if ($hal -notmatch $expected) {
    throw "HAL Orange cross-builder dry run is missing: $expected"
  }
}

$seesaw = Invoke-DryRun @{ Binary = "orange-seesaw-smoke" }
foreach ($expected in @(
    "target/orange-pi-cross/orange-seesaw-smoke",
    "orange-seesaw-smoke.metadata.json",
    "-p.*octessera-hal",
    "--features.*orange-pi-zero-2w"
  )) {
  if ($seesaw -notmatch $expected) {
    throw "Seesaw Orange cross-builder is missing: $expected"
  }
}

$candidate = Invoke-DryRun @{ Binary = "octessera-pi"; Profile = "release" }
foreach ($expected in @(
    "target/orange-pi-cross/octessera-pi",
    "octessera-pi.metadata.json",
    "-p.*octessera-pi",
    "--features.*hardware-orange-pi-zero-2w",
    "--no-default-features"
  )) {
  if ($candidate -notmatch $expected) {
    throw "Orange runtime-candidate dry run is missing: $expected"
  }
}

$dev = Invoke-DryRun @{ Profile = "dev" }
if ($dev -notmatch "/work/target/orange-cross-cargo/aarch64-unknown-linux-gnu/debug/orange-oled-smoke") {
  throw "Dev profile dry run did not use Cargo's debug artifact directory"
}

foreach ($dryRun in @($default, $hal, $seesaw, $candidate, $dev)) {
  if ($dryRun -match "target/release" -or $dryRun -notmatch "/work/target/orange-cross-cargo/aarch64-unknown-linux-gnu") {
    throw "Orange cross-builder dry run used a shared release target or omitted the dedicated Cargo target directory."
  }
}
if ($default -notmatch "CARGO_TARGET_DIR=/work/target/orange-cross-cargo") {
  throw "Orange cross-builder dry run did not export the dedicated Cargo target directory."
}
if ($default -notmatch "'/work/target/orange-cross-cargo/aarch64-unknown-linux-gnu/pi-dev/orange-oled-smoke'" -or $default -notmatch "'/work/target/orange-pi-cross/orange-oled-smoke'") {
  throw "Orange cross-builder did not keep source and output paths quoted and canonical."
}

foreach ($parameters in @(
    @{ Profile = "release;touch" },
    @{ Image = "rust:bookworm;touch" },
    @{ Target = "x86_64-unknown-linux-gnu" }
  )) {
  $rejected = $false
  try {
    Invoke-DryRun $parameters | Out-Null
  } catch {
    $rejected = $true
  }
  if (-not $rejected) {
    throw "Orange cross-builder accepted unsafe or unsupported arguments: $($parameters | ConvertTo-Json -Compress)"
  }
}

$metadataCases = @(
  "orange-oled-smoke",
  "orange-seesaw-smoke"
)
$testSpec = [pscustomobject]@{ Package = "octessera-hal"; Feature = "orange-pi-zero-2w"; ArtifactKind = "diagnostic-only" }
$sourceCommit = "a" * 40
foreach ($binaryName in $metadataCases) {
  $testDirectory = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-metadata-test-$PID-$([guid]::NewGuid().ToString('N'))"
  $testBinary = Join-Path $testDirectory $binaryName
  $testMetadata = "$testBinary.metadata.json"
  $metadataParameters = @{
    BinaryPath = $testBinary
    SelectedBinary = $binaryName
    SelectedTarget = "aarch64-unknown-linux-gnu"
    SelectedProfile = "pi-dev"
    BuildSpec = $testSpec
    SourceCommit = $sourceCommit
  }
  $publicationParameters = @{ MetadataPath = $testMetadata } + $metadataParameters

  New-Item -ItemType Directory -Path $testDirectory | Out-Null
  try {
    [IO.File]::WriteAllBytes($testBinary, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7))
    $json = ConvertTo-OrangeBuildMetadataJson @metadataParameters
    $expectedHash = (Get-FileHash -LiteralPath $testBinary -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($json -notmatch ('"binary_sha256":"' + $expectedHash + '"') -or $json -notmatch ('"source_commit":"' + $sourceCommit + '"')) {
      throw "Orange metadata serialization did not bind the binary hash: $binaryName"
    }
    Publish-OrangeBuildMetadata @publicationParameters
    Assert-OrangeBuildMetadata @publicationParameters
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false, $true)
    $publishedBytes = [IO.File]::ReadAllBytes($testMetadata)
    if ($publishedBytes.Length -ge 3 -and $publishedBytes[0] -eq 0xEF -and $publishedBytes[1] -eq 0xBB -and $publishedBytes[2] -eq 0xBF) {
      throw "Orange metadata publication unexpectedly wrote a UTF-8 BOM: $binaryName"
    }
    if ($utf8NoBom.GetString($publishedBytes) -cne "$json`n") {
      throw "Orange metadata publication changed the canonical serialized JSON: $binaryName"
    }
    $mismatched = [regex]::Replace([IO.File]::ReadAllText($testMetadata), '"source_commit":"[0-9a-f]{40}"', ('"source_commit":"' + ("b" * 40) + '"'))
    [IO.File]::WriteAllText($testMetadata, $mismatched, (New-Object System.Text.UTF8Encoding($false)))
    $mismatchRejected = $false
    try { Assert-OrangeBuildMetadata @publicationParameters } catch { $mismatchRejected = $true }
    if (-not $mismatchRejected) { throw "Orange metadata accepted a mismatched source commit: $binaryName" }
    Publish-OrangeBuildMetadata @publicationParameters

    $tamperedWriter = {
      Publish-OrangeBuildMetadata @publicationParameters
      $fakeHash = "0" * 64
      $tampered = [regex]::Replace(
        [IO.File]::ReadAllText($testMetadata),
        '"binary_sha256":"[0-9a-f]{64}"',
        ('"binary_sha256":"' + $fakeHash + '"')
      )
      [IO.File]::WriteAllText($testMetadata, $tampered, (New-Object System.Text.UTF8Encoding($false)))
    }
    $tamperedParameters = @{} + $publicationParameters
    $tamperedParameters.MetadataWriter = $tamperedWriter
    $failed = $false
    try {
      Invoke-VerifiedOrangeBuildMetadata @tamperedParameters
    } catch {
      $failed = $true
    }
    if (-not $failed) {
      throw "Orange metadata tamper test unexpectedly passed: $binaryName"
    }
    if ((Test-Path -LiteralPath $testBinary) -or (Test-Path -LiteralPath $testMetadata)) {
      throw "Orange metadata verification failure did not clean temporary artifacts: $binaryName"
    }
  } finally {
    Remove-Item -LiteralPath $testDirectory -Recurse -Force -ErrorAction SilentlyContinue
  }
}

$candidateDirectory = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-candidate-metadata-test-$PID-$([guid]::NewGuid().ToString('N'))"
$candidateBinary = Join-Path $candidateDirectory "octessera-pi"
$candidateMetadata = "$candidateBinary.metadata.json"
$candidateSpec = [pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w"; ArtifactKind = "runtime-candidate" }
$candidateParameters = @{
  BinaryPath = $candidateBinary
  MetadataPath = $candidateMetadata
  SelectedBinary = "octessera-pi"
  SelectedTarget = "aarch64-unknown-linux-gnu"
  SelectedProfile = "release"
  BuildSpec = $candidateSpec
  SourceCommit = $sourceCommit
}
New-Item -ItemType Directory -Path $candidateDirectory | Out-Null
try {
  [IO.File]::WriteAllBytes($candidateBinary, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7, 0x00))
  $candidateJsonParameters = @{} + $candidateParameters
  $candidateJsonParameters.Remove("MetadataPath")
  $candidateJson = ConvertTo-OrangeBuildMetadataJson @candidateJsonParameters
  if ($candidateJson -notmatch '"artifact_kind":"runtime-candidate"' -or $candidateJson -notmatch '"runtime_ready":false') {
    throw "Orange runtime-candidate metadata identity was not serialized"
  }
  Publish-OrangeBuildMetadata @candidateParameters
  Assert-OrangeBuildMetadata @candidateParameters
  $staleCandidate = [regex]::Replace([IO.File]::ReadAllText($candidateMetadata), '"source_commit":"[0-9a-f]{40}"', ('"source_commit":"' + ("c" * 40) + '"'))
  [IO.File]::WriteAllText($candidateMetadata, $staleCandidate, (New-Object System.Text.UTF8Encoding($false)))
  $staleRejected = $false
  try { Assert-OrangeBuildMetadata @candidateParameters } catch { $staleRejected = $true }
  if (-not $staleRejected) { throw "Orange metadata accepted a stale runtime candidate with valid hash and board fields" }
  Publish-OrangeBuildMetadata @candidateParameters
  $candidateTamperedWriter = {
    Publish-OrangeBuildMetadata @candidateParameters
    $candidateTampered = [regex]::Replace(
      [IO.File]::ReadAllText($candidateMetadata),
      '"binary_sha256":"[0-9a-f]{64}"',
      ('"binary_sha256":"' + ("0" * 64) + '"')
    )
    [IO.File]::WriteAllText($candidateMetadata, $candidateTampered, (New-Object System.Text.UTF8Encoding($false)))
  }
  $candidateVerifyParameters = @{} + $candidateParameters
  $candidateVerifyParameters.MetadataWriter = $candidateTamperedWriter
  $candidateFailed = $false
  try { Invoke-VerifiedOrangeBuildMetadata @candidateVerifyParameters } catch { $candidateFailed = $true }
  if (-not $candidateFailed -or (Test-Path -LiteralPath $candidateBinary) -or (Test-Path -LiteralPath $candidateMetadata)) {
    throw "Orange runtime-candidate metadata tamper contract failed"
  }
} finally {
  Remove-Item -LiteralPath $candidateDirectory -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange Pi cross-builder host, dry-run, and metadata tests passed"
