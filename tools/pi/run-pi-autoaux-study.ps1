[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Target,
  [string]$Artifact = "",
  [string]$Metadata = "",
  [ValidateRange(5, 300)][int]$LiveSeconds = 30,
  [string]$OutputDirectory = "",
  [switch]$AllowServiceInterruption,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "..\deployment-target.ps1")
Assert-DeploymentTarget $Target | Out-Null
. (Join-Path $PSScriptRoot "board-profile.ps1")
Assert-OctesseraServiceName "octessera.service"
if (-not $PrintOnly -and -not $AllowServiceInterruption) { throw "AutoAux live study requires -AllowServiceInterruption." }

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ([string]::IsNullOrWhiteSpace($Artifact)) { $Artifact = Join-Path $repoRoot "target\pi-cross\octessera-pi" }
if ([string]::IsNullOrWhiteSpace($Metadata)) { $Metadata = "$Artifact.metadata.json" }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $repoRoot "target\pi-autoaux-study" }
if (Test-Path -LiteralPath $Artifact -PathType Leaf) {
  $buildMetadata = Read-RaspberryBoardMetadata $Metadata
  $SourceCommit = [string]$buildMetadata.source_commit
  Assert-RaspberryBuildMetadata -Metadata $buildMetadata -SourceCommit $SourceCommit -BinaryPath $Artifact | Out-Null
} elseif ($PrintOnly) {
  $SourceCommit = (& git -C $repoRoot rev-parse HEAD).Trim()
  if ($LASTEXITCODE -ne 0) { throw "Could not resolve the repository source checkpoint." }
} else {
  throw "Raspberry candidate artifact is missing: $Artifact"
}

$hash = if (Test-Path -LiteralPath $Artifact -PathType Leaf) { (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant() } else { "artifact-sha256" }
$runId = [guid]::NewGuid().ToString("N")
$remoteRoot = "/tmp/octessera-pi-autoaux-$runId"
$remoteBinary = "$remoteRoot/octessera-pi"
$remoteMetadata = "$remoteBinary.metadata.json"
$payload = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "pi-autoaux-study-payload.sh"))
$payload = $payload.Replace("__RUN_ID__", $runId).Replace("__LIVE_SECONDS__", [string]$LiveSeconds).Replace("__ARTIFACT_HASH__", $hash).Replace("__SOURCE_COMMIT__", $SourceCommit).Replace("__REMOTE_ROOT__", $remoteRoot)
$payloadPath = Join-Path ([IO.Path]::GetTempPath()) "octessera-pi-autoaux-$runId.sh"
[IO.File]::WriteAllText($payloadPath, $payload, (New-Object System.Text.UTF8Encoding($false)))
$transport = Join-Path $PSScriptRoot "with-rpi-ssh.ps1"
$evidencePath = Join-Path $OutputDirectory "pi-autoaux-$runId"
function Invoke-StudyTransport {
  param([string]$Command, [string[]]$Arguments)
  & $transport -Command $Command -Target $Target -ArgumentList $Arguments
  if ($LASTEXITCODE -ne 0) { throw "Raspberry transport failed ($LASTEXITCODE): $Command" }
}

try {
  if ($PrintOnly) {
    Write-Output "Target=$Target SourceCommit=$SourceCommit ArtifactSha256=$hash LiveSeconds=$LiveSeconds"
    Write-Output "Normal-runtime AutoAux candidate; timing-probe mode is not used."
    Write-Output "Payload: $payloadPath"
    return
  }
  if (-not $AllowServiceInterruption) { throw "AutoAux live study requires -AllowServiceInterruption." }
  Invoke-StudyTransport "ssh" @($Target, "mkdir -m 700 '$remoteRoot'")
  Invoke-StudyTransport "scp" @($Artifact, "$Target`:$remoteBinary")
  Invoke-StudyTransport "scp" @($Metadata, "$Target`:$remoteMetadata")
  $studyFailure = $null
  try { Invoke-StudyTransport "ssh-payload" @($payloadPath) } catch { $studyFailure = $_ }
  New-Item -ItemType Directory -Force -Path $evidencePath | Out-Null
  try { Invoke-StudyTransport "scp" @("-r", "$Target`:$remoteRoot/evidence/.", $evidencePath) } catch { if ($null -eq $studyFailure) { $studyFailure = $_ } }
  if ($null -ne $studyFailure) { throw "Study failed: $studyFailure. Evidence: $evidencePath" }
  Invoke-StudyTransport "ssh" @($Target, "rm -rf -- '$remoteRoot'")
} finally {
  Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
}
Write-Output "Raspberry AutoAux study completed; inspect candidate stderr, UI/runtime profile, and kernel/audio journals in $evidencePath."
