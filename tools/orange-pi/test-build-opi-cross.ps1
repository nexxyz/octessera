$ErrorActionPreference = "Stop"

$buildScript = Join-Path $PSScriptRoot "build-opi-cross.ps1"
$metadataModule = Join-Path $PSScriptRoot "orange-cross-metadata.psm1"
$source = [IO.File]::ReadAllText($buildScript)

if ($source -match "(?i)\b(ssh|scp|rsync)\b") {
  throw "Orange cross-builder must not contain board transport commands"
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
$profiling = Invoke-DryRun @{ Binary = "octessera-pi"; Profile = "release"; Profiling = $true }
foreach ($expected in @(
    "target/orange-pi-cross-diagnostics/profiling/octessera-pi",
    "--features.*hardware-orange-pi-zero-2w profiling",
    "--no-default-features"
  )) {
  if ($profiling -notmatch $expected) {
    throw "Orange profiling dry run is missing: $expected"
  }
}
if ($profiling -match "target/orange-pi-cross/octessera-pi") {
  throw "Orange profiling build selected the production candidate output path."
}
foreach ($binary in @("orange-oled-smoke", "orange-seesaw-smoke")) {
  $rejected = $false
  try { Invoke-DryRun @{ Binary = $binary; Profiling = $true } | Out-Null } catch { $rejected = $true }
  if (-not $rejected) { throw "Orange cross-builder accepted a profiling build for a HAL smoke binary." }
}

foreach ($expected in @(
    "target/orange-pi-cross/octessera-pi",
    "-p.*octessera-pi",
    "--features.*hardware-orange-pi-zero-2w",
    "--no-default-features"
  )) {
  if ($candidate -notmatch $expected) {
    throw "Orange runtime-candidate dry run is missing: $expected"
  }
}
if ($candidate -match "metadata" -or $profiling -match "metadata") {
  throw "Orange octessera-pi builds must not write a metadata sidecar."
}

$dev = Invoke-DryRun @{ Profile = "dev" }
if ($dev -notmatch "/work/target/orange-cross-cargo/aarch64-unknown-linux-gnu/debug/orange-oled-smoke") {
  throw "Dev profile dry run did not use Cargo's debug artifact directory"
}

foreach ($dryRun in @($default, $hal, $seesaw, $candidate, $profiling, $dev)) {
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

Import-Module $metadataModule -Force
$metadataDirectory = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-metadata-test-$PID-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $metadataDirectory | Out-Null
try {
  foreach ($case in @(
      @{ Binary = "orange-oled-smoke"; Spec = [pscustomobject]@{ Package = "octessera-hal"; Feature = "orange-pi-zero-2w"; ArtifactKind = "diagnostic-only" } }
    )) {
    $binaryPath = Join-Path $metadataDirectory $case.Binary
    [IO.File]::WriteAllBytes($binaryPath, [byte[]](0x7F, 0x45, 0x4C, 0x46, 0x02, 0xB7))
    Publish-OrangeBuildMetadata -MetadataPath "$binaryPath.metadata.json" -BinaryPath $binaryPath -SelectedBinary $case.Binary `
      -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec $case.Spec -SourceCommit ("a" * 40)
    $hash = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = '{"schema_version":2,"board_profile":"orange-pi-zero-2w","artifact_kind":"' + $case.Spec.ArtifactKind +
      '","runtime_ready":false,"binary":"' + $case.Binary + '","package":"' + $case.Spec.Package +
      '","arch":"aarch64-unknown-linux-gnu","cargo_feature":"' + $case.Spec.Feature + '","profile":"release","binary_sha256":"' + $hash +
      '","source_commit":"' + ("a" * 40) + '"}' + "`n"
    if ([IO.File]::ReadAllText("$binaryPath.metadata.json") -cne $expected) {
      throw "Orange metadata sidecar is not the schema-2 contract: $($case.Binary)"
    }
  }
} finally {
  Remove-Item -LiteralPath $metadataDirectory -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange Pi cross-builder host, dry-run, and metadata tests passed"
