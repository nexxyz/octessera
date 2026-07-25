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
    "export PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "dpkg --add-architecture arm64",
    "gcc-aarch64-linux-gnu",
    "libc6-dev-arm64-cross",
    "aarch64-linux-gnu-readelf",
    "Import-Module",
    "Invoke-VerifiedOrangeBuildMetadata",
    "Remove-OrangeBuildArtifacts",
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
    'artifact_kind = $script:OrangeArtifactKind',
    'runtime_ready = $false',
    "Publish-OrangeBuildMetadata",
    "Assert-OrangeBuildMetadata",
    "Remove-OrangeBuildArtifacts",
    "Invoke-VerifiedOrangeBuildMetadata",
    "WriteAllText",
    "ReadAllBytes"
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
if ($source -match 'octessera-pi.*hardware-orange-pi-zero-2w') {
  throw "Orange cross-builder must not define an Orange runtime build"
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

$dev = Invoke-DryRun @{ Profile = "dev" }
if ($dev -notmatch "target/aarch64-unknown-linux-gnu/debug/orange-oled-smoke") {
  throw "Dev profile dry run did not use Cargo's debug artifact directory"
}

foreach ($parameters in @(
    @{ Binary = "octessera-pi" },
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

$testDirectory = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-metadata-test-$PID-$([guid]::NewGuid().ToString('N'))"
$testBinary = Join-Path $testDirectory "orange-oled-smoke"
$testMetadata = "$testBinary.metadata.json"
$testSpec = [pscustomobject]@{ Package = "octessera-hal"; Feature = "orange-pi-zero-2w" }
$metadataParameters = @{
  BinaryPath = $testBinary
  SelectedBinary = "orange-oled-smoke"
  SelectedTarget = "aarch64-unknown-linux-gnu"
  SelectedProfile = "pi-dev"
  BuildSpec = $testSpec
}
$publicationParameters = @{ MetadataPath = $testMetadata } + $metadataParameters

New-Item -ItemType Directory -Path $testDirectory | Out-Null
try {
  [IO.File]::WriteAllBytes($testBinary, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7))
  $json = ConvertTo-OrangeBuildMetadataJson @metadataParameters
  $expectedHash = (Get-FileHash -LiteralPath $testBinary -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($json -notmatch ('"binary_sha256":"' + $expectedHash + '"')) {
    throw "Orange metadata serialization did not bind the binary hash"
  }
  Publish-OrangeBuildMetadata @publicationParameters
  Assert-OrangeBuildMetadata @publicationParameters
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false, $true)
  $publishedBytes = [IO.File]::ReadAllBytes($testMetadata)
  if ($publishedBytes.Length -ge 3 -and $publishedBytes[0] -eq 0xEF -and $publishedBytes[1] -eq 0xBB -and $publishedBytes[2] -eq 0xBF) {
    throw "Orange metadata publication unexpectedly wrote a UTF-8 BOM"
  }
  if ($utf8NoBom.GetString($publishedBytes) -cne "$json`n") {
    throw "Orange metadata publication changed the canonical serialized JSON"
  }

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
    throw "Orange metadata tamper test unexpectedly passed"
  }
  if ((Test-Path -LiteralPath $testBinary) -or (Test-Path -LiteralPath $testMetadata)) {
    throw "Orange metadata verification failure did not clean temporary artifacts"
  }
} finally {
  Remove-Item -LiteralPath $testDirectory -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange Pi cross-builder host, dry-run, and metadata tests passed"
