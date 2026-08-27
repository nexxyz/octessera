[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$Operator,

  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$Version,

  [string]$ExpectedSourceSha = "",
  [string]$ReleaseTag = "",

  [Parameter(Mandatory = $true)]
  [string]$RaspberryImage,

  [Parameter(Mandatory = $true)]
  [string]$OrangeImage,

  [string]$RaspberryChecksum = "",
  [string]$OrangeChecksum = "",
  [string]$EvidenceRoot = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-SingleLineInput {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Value,
    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  if ([string]::IsNullOrWhiteSpace($Value) -or $Value.IndexOfAny([char[]]@([char]0, "`r", "`n")) -ge 0) {
    throw "$Name must be a non-empty value without line breaks."
  }
}

function Resolve-InputFile {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  Assert-SingleLineInput $Path $Name
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Name was not found: $Path"
  }

  $item = Get-Item -LiteralPath $Path
  if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "$Name must not be a reparse point: $Path"
  }
  return $item.FullName
}

function Write-Utf8NoBom {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [string]$Content
  )

  $encoding = New-Object System.Text.UTF8Encoding($false)
  $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  try {
    $writer = New-Object IO.StreamWriter($stream, $encoding)
    try {
      $writer.Write($Content)
    } finally {
      $writer.Dispose()
    }
  } finally {
    $stream.Dispose()
  }
}

function Assert-NonReparseDirectory {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  $item = Get-Item -LiteralPath $Path
  if (-not $item.PSIsContainer) {
    throw "$Name is not a directory: $Path"
  }
  if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "$Name must not be a reparse point: $Path"
  }
  $item
}

function New-EvidenceRoot {
  param([Parameter(Mandatory = $true)][string]$Path)

  if (Test-Path -LiteralPath $Path) {
    throw "EvidenceRoot must be newly created and unused: $Path"
  }
  $parent = Split-Path -Parent $Path
  if ([string]::IsNullOrWhiteSpace($parent)) {
    throw "EvidenceRoot must have an existing parent directory: $Path"
  }
  if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
    [IO.Directory]::CreateDirectory($parent) | Out-Null
  }
  Assert-NonReparseDirectory $parent "EvidenceRoot parent" | Out-Null
  New-Item -ItemType Directory -Path $Path | Out-Null
  Assert-NonReparseDirectory $Path "EvidenceRoot" | Out-Null
  (Resolve-Path -LiteralPath $Path).Path
}

Assert-SingleLineInput $Operator "Operator"
Assert-SingleLineInput $Version "Version"

$hasExpectedSourceSha = -not [string]::IsNullOrWhiteSpace($ExpectedSourceSha)
$hasReleaseTag = -not [string]::IsNullOrWhiteSpace($ReleaseTag)
if ($hasExpectedSourceSha -eq $hasReleaseTag) {
  throw "Provide exactly one of ExpectedSourceSha or ReleaseTag."
}
if ($hasExpectedSourceSha) {
  Assert-SingleLineInput $ExpectedSourceSha "ExpectedSourceSha"
  if ($ExpectedSourceSha -notmatch '^[0-9a-fA-F]{40}$') {
    throw "ExpectedSourceSha must be a full 40-character hexadecimal git commit SHA."
  }
}
if ($hasReleaseTag) {
  Assert-SingleLineInput $ReleaseTag "ReleaseTag"
  if ($ReleaseTag -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') {
    throw "ReleaseTag must use the exact vX.Y.Z form."
  }
}

$raspberryImagePath = Resolve-InputFile $RaspberryImage "RaspberryImage"
$orangeImagePath = Resolve-InputFile $OrangeImage "OrangeImage"
$raspberryChecksumPath = $null
$orangeChecksumPath = $null
if (-not [string]::IsNullOrWhiteSpace($RaspberryChecksum)) {
  $raspberryChecksumPath = Resolve-InputFile $RaspberryChecksum "RaspberryChecksum"
}
if (-not [string]::IsNullOrWhiteSpace($OrangeChecksum)) {
  $orangeChecksumPath = Resolve-InputFile $OrangeChecksum "OrangeChecksum"
}

$repoRootOutput = @(& git rev-parse --show-toplevel)
if ($LASTEXITCODE -ne 0 -or $repoRootOutput.Count -ne 1) {
  throw "The current directory is not inside the Octessera git worktree."
}
$repoRoot = ([string]$repoRootOutput[0]).Trim()
$gitShaOutput = @(& git -C $repoRoot rev-parse HEAD)
if ($LASTEXITCODE -ne 0 -or $gitShaOutput.Count -ne 1) {
  throw "Could not read the current git SHA."
}
$gitSha = ([string]$gitShaOutput[0]).Trim()

$sourceIdentityKind = $null
$sourceIdentityValue = $null
if ($hasExpectedSourceSha) {
  $expectedSourceShaNormalized = $ExpectedSourceSha.ToLowerInvariant()
  if ($gitSha.ToLowerInvariant() -ne $expectedSourceShaNormalized) {
    throw "ExpectedSourceSha does not match the current repository HEAD."
  }
  $sourceIdentityKind = "expected-source-sha"
  $sourceIdentityValue = $expectedSourceShaNormalized
} else {
  $tagShaOutput = @(& git -C $repoRoot rev-parse --verify "refs/tags/$ReleaseTag^{commit}")
  if ($LASTEXITCODE -ne 0 -or $tagShaOutput.Count -ne 1) {
    throw "ReleaseTag was not found as an exact local git tag: $ReleaseTag"
  }
  $tagSha = ([string]$tagShaOutput[0]).Trim().ToLowerInvariant()
  if ($tagSha -ne $gitSha.ToLowerInvariant()) {
    throw "ReleaseTag does not resolve to the current repository HEAD."
  }
  $sourceIdentityKind = "release-tag"
  $sourceIdentityValue = $ReleaseTag
}

if ([string]::IsNullOrWhiteSpace($EvidenceRoot)) {
  $stamp = [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssZ")
  $EvidenceRoot = Join-Path (Get-Location).Path (Join-Path "artifacts\fat" $stamp)
} elseif (-not [IO.Path]::IsPathRooted($EvidenceRoot)) {
  $EvidenceRoot = Join-Path (Get-Location).Path $EvidenceRoot
}

$EvidenceRoot = New-EvidenceRoot $EvidenceRoot

$createdUtc = [DateTime]::UtcNow.ToString("o")
$assets = @(
  [pscustomobject]@{ Board = "raspberry-pi-zero-2w"; Kind = "image"; Path = $raspberryImagePath }
  [pscustomobject]@{ Board = "orange-pi-zero-2w"; Kind = "image"; Path = $orangeImagePath }
)
if ($null -ne $raspberryChecksumPath) {
  $assets += [pscustomobject]@{ Board = "raspberry-pi-zero-2w"; Kind = "checksum"; Path = $raspberryChecksumPath }
}
if ($null -ne $orangeChecksumPath) {
  $assets += [pscustomobject]@{ Board = "orange-pi-zero-2w"; Kind = "checksum"; Path = $orangeChecksumPath }
}

$assetRecords = @()
foreach ($asset in $assets) {
  $item = Get-Item -LiteralPath $asset.Path
  $hash = (Get-FileHash -LiteralPath $asset.Path -Algorithm SHA256).Hash.ToLowerInvariant()
  $assetRecords += [pscustomobject]@{
    Board = $asset.Board
    Kind = $asset.Kind
    FileName = $item.Name
    Path = $item.FullName
    SizeBytes = $item.Length
    SHA256 = $hash
  }
}

$hashLines = @(
  "board`tkind`tfilename`tsize_bytes`tsha256`tpath"
)
foreach ($record in $assetRecords) {
  $hashLines += "$($record.Board)`t$($record.Kind)`t$($record.FileName)`t$($record.SizeBytes)`t$($record.SHA256)`t$($record.Path)"
}

$session = [ordered]@{
  schema = 1
  createdUtc = $createdUtc
  operator = $Operator
  version = $Version
  gitSha = $gitSha
  sourceIdentity = [ordered]@{
    kind = $sourceIdentityKind
    value = $sourceIdentityValue
    commitSha = $gitSha
  }
  evidenceRoot = $EvidenceRoot
  boards = @(
    [ordered]@{ board = "raspberry-pi-zero-2w"; image = $assetRecords[0] }
    [ordered]@{ board = "orange-pi-zero-2w"; image = $assetRecords[1] }
  )
}

$destructiveCommands = @"
PRINT ONLY. This evidence-preparation script does not flash, reboot, shut down, alter a board, or run a restore.

Raspberry Pi card flash: use Raspberry Pi Imager with $([IO.Path]::GetFileName($raspberryImagePath)).
Orange Pi card flash: use an image flasher with $([IO.Path]::GetFileName($orangeImagePath)).
Both operations destroy the selected card contents. Confirm the target card twice.

Preferred instrument actions: System > Reboot; System > Shutdown.
Administrative commands below are printed for review only, not executed by this evidence-preparation script:
sudo systemctl reboot
sudo systemctl poweroff

Data restore is also destructive and requires physical confirmation. Existing documented shape:
curl -f -X POST --data-binary @octessera-user-data.oct -H "X-Octessera-Transfer-Code: TRANSFER_CODE" "http://<regular-wlan0-ip>:8081/restore"
"@

Write-Utf8NoBom (Join-Path $EvidenceRoot "00-session.json") ($session | ConvertTo-Json -Depth 8)
Write-Utf8NoBom (Join-Path $EvidenceRoot "00-git-sha.txt") "$gitSha`n"
Write-Utf8NoBom (Join-Path $EvidenceRoot "00-version.txt") "$Version`n"
Write-Utf8NoBom (Join-Path $EvidenceRoot "00-operator.txt") "$Operator`n"
Write-Utf8NoBom (Join-Path $EvidenceRoot "00-created-utc.txt") "$createdUtc`n"
Write-Utf8NoBom (Join-Path $EvidenceRoot "00-image-hashes.tsv") ($hashLines -join "`n")
Write-Utf8NoBom (Join-Path $EvidenceRoot "00-destructive-commands.txt") $destructiveCommands.TrimStart()

Write-Output "Evidence folder created: $EvidenceRoot"
Write-Output "Recorded git SHA: $gitSha"
Write-Output "Recorded source identity: $sourceIdentityValue ($sourceIdentityKind)"
Write-Output "Hashed exact image and supplied checksum assets; no board or card was touched."
Write-Output ""
Write-Output $destructiveCommands.TrimStart()
