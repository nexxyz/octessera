[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$Operator,

  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$Version,

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
  [IO.File]::WriteAllText($Path, $Content, $encoding)
}

Assert-SingleLineInput $Operator "Operator"
Assert-SingleLineInput $Version "Version"

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

if ([string]::IsNullOrWhiteSpace($EvidenceRoot)) {
  $stamp = [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssZ")
  $EvidenceRoot = Join-Path (Get-Location).Path (Join-Path "artifacts\fat" $stamp)
} elseif (-not [IO.Path]::IsPathRooted($EvidenceRoot)) {
  $EvidenceRoot = Join-Path (Get-Location).Path $EvidenceRoot
}

if (Test-Path -LiteralPath $EvidenceRoot -PathType Leaf) {
  throw "EvidenceRoot is an existing file: $EvidenceRoot"
}
New-Item -ItemType Directory -Path $EvidenceRoot -Force | Out-Null

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
  evidenceRoot = (Resolve-Path -LiteralPath $EvidenceRoot).Path
  boards = @(
    [ordered]@{ board = "raspberry-pi-zero-2w"; image = $assetRecords[0] }
    [ordered]@{ board = "orange-pi-zero-2w"; image = $assetRecords[1] }
  )
}

$destructiveCommands = @"
PRINT ONLY. This helper does not flash, reboot, shut down, alter a board, or run a restore.

Raspberry Pi card flash: use Raspberry Pi Imager with $([IO.Path]::GetFileName($raspberryImagePath)).
Orange Pi card flash: use an image flasher with $([IO.Path]::GetFileName($orangeImagePath)).
Both operations destroy the selected card contents. Confirm the target card twice.

Preferred instrument actions: System > Reboot; System > Shutdown.
Administrative commands below are printed for review only, not executed by this helper:
sudo systemctl reboot
sudo systemctl poweroff

Data restore is also destructive and requires physical confirmation. Existing documented shape:
curl -f -X POST --data-binary @octessera-user-data.oct -H "X-Octessera-Transfer-Code: TRANSFER_CODE" "http://192.168.42.1:8081/restore"
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
Write-Output "Hashed exact image and supplied checksum assets; no board or card was touched."
Write-Output ""
Write-Output $destructiveCommands.TrimStart()
