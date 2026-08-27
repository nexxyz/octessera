$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$scriptPath = Join-Path $PSScriptRoot "prepare-evidence.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-fat-evidence-test-" + [guid]::NewGuid().ToString("N"))
$imageDirectory = Join-Path $testRoot "inputs"
$imageRaspberry = Join-Path $imageDirectory "raspberry.img"
$imageOrange = Join-Path $imageDirectory "orange.img"
$checksum = Join-Path $imageDirectory "SHA256SUMS.txt"
$sourceSha = (& git -C $repositoryRoot rev-parse HEAD).Trim()

function Invoke-Preparation {
  param([string]$EvidenceRoot)

  $output = @()
  $exitCode = 0
  try {
    $output = @(& $scriptPath `
      -Operator "test operator" `
      -Version "0.0.0-test" `
      -ExpectedSourceSha $sourceSha `
      -RaspberryImage $imageRaspberry `
      -OrangeImage $imageOrange `
      -RaspberryChecksum $checksum `
      -EvidenceRoot $EvidenceRoot 2>&1)
    $exitCode = $LASTEXITCODE
  } catch {
    $output += $_
    $exitCode = 1
  }
  [pscustomobject]@{
    ExitCode = $exitCode
    Output = ($output -join "`n")
  }
}

try {
  New-Item -ItemType Directory -Path $imageDirectory | Out-Null
  [IO.File]::WriteAllBytes($imageRaspberry, [byte[]](1, 2, 3))
  [IO.File]::WriteAllBytes($imageOrange, [byte[]](4, 5, 6))
  [IO.File]::WriteAllText($checksum, "test checksum`n")

  $createdRoot = Join-Path $testRoot "created"
  $first = Invoke-Preparation $createdRoot
  if ($first.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $createdRoot -PathType Container)) {
    throw "Initial evidence preparation failed: $($first.Output)"
  }
  foreach ($name in @(
      "00-session.json",
      "00-git-sha.txt",
      "00-version.txt",
      "00-operator.txt",
      "00-created-utc.txt",
      "00-image-hashes.tsv",
      "00-destructive-commands.txt"
    )) {
    if (-not (Test-Path -LiteralPath (Join-Path $createdRoot $name) -PathType Leaf)) {
      throw "Evidence preparation did not create $name"
    }
  }
  if (((Get-Item -LiteralPath $createdRoot).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "Evidence root is a reparse point"
  }
  if ((Get-Content -LiteralPath (Join-Path $createdRoot "00-operator.txt") -Raw) -cne "test operator`n") {
    throw "Evidence file content was not written exactly once"
  }
  if ($first.Output -notmatch "http://<regular-wlan0-ip>:8081/restore" -or $first.Output -match "192\.168\.42\.1:8081") {
    throw "Print-only restore reminder has the wrong network placeholder"
  }

  $reused = Invoke-Preparation $createdRoot
  if ($reused.ExitCode -eq 0 -or $reused.Output -notmatch "newly created and unused") {
    throw "Existing evidence root was reused"
  }

  $existing = Join-Path $testRoot "existing"
  New-Item -ItemType Directory -Path $existing | Out-Null
  $rejected = Invoke-Preparation $existing
  if ($rejected.ExitCode -eq 0 -or $rejected.Output -notmatch "newly created and unused") {
    throw "Pre-existing empty evidence root was accepted"
  }

  $source = [IO.File]::ReadAllText($scriptPath)
  if ($source -match "New-Item[^\r\n]*-Force" -or $source -notmatch "CreateNew") {
    throw "Evidence preparation does not enforce create-new destinations and files"
  }
  if ($source -notmatch "PRINT ONLY") {
    throw "Evidence preparation lost its print-only safety statement"
  }
} finally {
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "FAT evidence preparation safety tests passed"
