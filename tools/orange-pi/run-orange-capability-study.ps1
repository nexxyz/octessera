[CmdletBinding()]
param(
  [ValidateSet("PassiveBaseline", "ProfileBaseline", "Dsp64", "Dsp256", "LiveCandidate", "LiveAudioBenchmark")]
  [string]$Mode = "PassiveBaseline",
  [ValidateSet("full", "overload", "soak", "fx-limits")]
  [string]$ProfileMode = "soak",
  [string]$Artifact = "",
  [string]$Metadata = "",
  [ValidateRange(30, 1800)]
  [int]$TimeoutSeconds = 900,
  [ValidateRange(5, 300)]
  [int]$LiveSeconds = 30,
  [string]$Scenario = "",
  [ValidateSet(128, 256, 512, 1024)]
  [int]$OutputFrames = 256,
  [ValidateSet(32, 64, 128, 256)]
  [int]$EngineBlockFrames = 0,
  [ValidateSet(64, 128, 256)]
  [int]$ProfileMeasureFrames = 0,
  [ValidateSet(30, 120, 300)]
  [int]$MeasureSeconds = 30,
  [ValidateSet("enabled", "disabled")]
  [string]$WorkerTimingMode = "enabled",
  [ValidateRange(1, 120)]
  [int]$ReleaseTimeoutSeconds = 120,
  [ValidateRange(5, 60)]
  [int]$StartupTimeoutSeconds = 20,
  [string]$OutputDirectory = "",
  [switch]$AllowServiceInterruption,
  [switch]$AllowLongRepeat,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
if (@("enabled", "disabled") -cnotcontains $WorkerTimingMode) { throw "WorkerTimingMode must be exactly enabled or disabled." }

$target = "octessera@192.168.0.217"
$service = "octessera.service"
$transport = Join-Path $PSScriptRoot "with-orange-ssh.ps1"
$metadataModule = Join-Path $PSScriptRoot "orange-cross-metadata.psm1"
$payloadModule = Join-Path $PSScriptRoot "orange-capability-study-payloads.psm1"
$livePayloadModule = Join-Path $PSScriptRoot "orange-live-benchmark-payloads.psm1"
$liveValidationModule = Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1"
$baselineValidationModule = Join-Path $PSScriptRoot "orange-profile-baseline-validation.psm1"
$defaultArtifact = Join-Path $PSScriptRoot "..\..\target\orange-pi-cross\octessera-pi"
$defaultOutput = Join-Path $PSScriptRoot "..\..\target\orange-pi-study"
$artifactRequired = $Mode -ne "PassiveBaseline"
$activeMode = $artifactRequired

if ([string]::IsNullOrWhiteSpace($Artifact)) { $Artifact = $defaultArtifact }
if ([string]::IsNullOrWhiteSpace($Metadata)) { $Metadata = "$Artifact.metadata.json" }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = $defaultOutput }
if ($activeMode -and -not $AllowServiceInterruption -and -not ($PrintOnly -and $Mode -eq "ProfileBaseline")) {
  throw "$Mode requires -AllowServiceInterruption; no service will be interrupted without it."
}

Import-Module $metadataModule -Force
Import-Module $payloadModule -Force
Import-Module $liveValidationModule -Force
Import-Module $baselineValidationModule -Force
$liveSelection = $null
$baselineSelection = $null
if ($Mode -eq "ProfileBaseline") {
  $baselineSelection = Assert-OrangeProfileBaselineSelection `
    -Scenario $Scenario `
    -InternalFrames $EngineBlockFrames `
    -MeasureFrames $ProfileMeasureFrames
}
if ($Mode -eq "LiveAudioBenchmark") {
  Import-Module $livePayloadModule -Force
  if ($EngineBlockFrames -eq 0) { throw "LiveAudioBenchmark requires -EngineBlockFrames (64, 128, or 256)." }
  $liveSelection = Assert-OrangeLiveBenchmarkSelection `
    -Scenario $Scenario `
    -OutputFrames $OutputFrames `
    -EngineBlockFrames $EngineBlockFrames `
    -MeasureSeconds $MeasureSeconds `
    -AllowLongRepeat:$AllowLongRepeat
}

function Quote-PowerShellValue {
  param([Parameter(Mandatory)][string]$Value)
  return "'" + $Value.Replace("'", "''") + "'"
}

function Get-StudyArtifactHash {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Orange runtime-candidate binary was not found: $Path"
  }
  $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($hash -notmatch '^[0-9a-f]{64}$') {
    throw "Orange runtime-candidate SHA-256 is not canonical lowercase hex: $Path"
  }
  return $hash
}

function Assert-StudyArtifact {
  param(
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$MetadataPath
  )
  $buildSpec = [pscustomobject]@{
    Package = "octessera-pi"
    Feature = "hardware-orange-pi-zero-2w"
    ArtifactKind = "runtime-candidate"
  }
  Assert-OrangeBuildMetadata `
    -MetadataPath $MetadataPath `
    -BinaryPath $BinaryPath `
    -SelectedBinary "octessera-pi" `
    -SelectedTarget "aarch64-unknown-linux-gnu" `
    -SelectedProfile "release" `
    -BuildSpec $buildSpec
}

function Write-PayloadFile {
  param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Contents,
    [Parameter(Mandatory)][string]$RunId
  )
  $path = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-study-$RunId-$Name.sh"
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [IO.File]::WriteAllText($path, "$Contents`n", $utf8NoBom)
  return $path
}

function Write-LiveRestorationFailureEvidence {
  param(
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [Parameter(Mandatory)][string]$Reason
  )
  $path = Join-Path $EvidenceDirectory "host-evidence.json"
  $evidence = [ordered]@{}
  if (Test-Path -LiteralPath $path -PathType Leaf) {
    try {
      $existing = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
      foreach ($property in $existing.PSObject.Properties) { $evidence[$property.Name] = $property.Value }
    } catch { $evidence = [ordered]@{} }
  }
  $evidence.StatusClass = "restoration_failure"
  $evidence.Reason = "Host cleanup/recovery failed: $Reason"
  $evidence.RecoveryFailure = $Reason
  $evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $path -Encoding UTF8
  return $path
}

function Format-TransportCommand {
  param(
    [Parameter(Mandatory)][string]$Command,
    [Parameter(Mandatory)][string[]]$Arguments
  )
  return "& $(Quote-PowerShellValue $transport) $(Quote-PowerShellValue $Command) " + (($Arguments | ForEach-Object { Quote-PowerShellValue $_ }) -join " ")
}

function Invoke-OrangeTransport {
  param(
    [Parameter(Mandatory)][string]$Command,
    [Parameter(Mandatory)][string[]]$Arguments
  )
  if ($PrintOnly) {
    Write-Output (Format-TransportCommand $Command $Arguments)
    return
  }
  & $transport $Command @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "Orange transport failed with exit code ${LASTEXITCODE}: $Command"
  }
}

$runId = [guid]::NewGuid().ToString("N")
$artifactExists = Test-Path -LiteralPath $Artifact -PathType Leaf
$artifactHash = "artifact-sha256"
if ($artifactRequired -and $artifactExists) {
  $artifactHash = Get-StudyArtifactHash $Artifact
  if (Test-Path -LiteralPath $Metadata -PathType Leaf) {
    Assert-StudyArtifact $Artifact $Metadata
  } elseif (-not $PrintOnly) {
    throw "Orange runtime-candidate metadata sidecar was not found: $Metadata"
  }
} elseif ($artifactRequired -and -not $PrintOnly) {
  throw "A release Orange octessera-pi artifact is required for ${Mode}: $Artifact"
}

$remoteRoot = "/tmp/octessera-orange-study-$artifactHash-$runId"
$remoteBinary = "$remoteRoot/octessera-pi"
$remoteMetadata = "$remoteBinary.metadata.json"
$healthPath = "/run/octessera/candidate-health-$artifactHash-$runId.json"
$remoteUnit = "octessera-study-$runId.service"
$payloadBundle = if ($Mode -eq "LiveAudioBenchmark") {
  $benchmarkRoot = "/run/octessera/orange-live-$runId"
  $runtimeMaxSeconds = $ReleaseTimeoutSeconds + 5 + $liveSelection.MeasureSeconds + 30
  New-OrangeLiveBenchmarkPayloadBundle `
    -Selection $liveSelection `
    -RemoteRoot $remoteRoot `
    -BenchmarkRoot $benchmarkRoot `
    -HealthPath $healthPath `
    -ArtifactHash $artifactHash `
    -Unit $remoteUnit `
    -Service $service `
    -StartupTimeoutSeconds $StartupTimeoutSeconds `
    -ReleaseTimeoutSeconds $ReleaseTimeoutSeconds `
    -RuntimeMaxSeconds $runtimeMaxSeconds `
    -WorkerTimingMode $WorkerTimingMode
} else {
  New-OrangeCapabilityStudyPayloadBundle `
    -Mode $Mode `
    -ProfileMode $ProfileMode `
    -TimeoutSeconds $TimeoutSeconds `
    -LiveSeconds $LiveSeconds `
    -StartupTimeoutSeconds $StartupTimeoutSeconds `
    -RemoteRoot $remoteRoot `
    -HealthPath $healthPath `
    -ArtifactHash $artifactHash `
    -Unit $remoteUnit `
    -Service $service `
    -ActiveMode $activeMode `
    -ArtifactRequired $artifactRequired `
    -Scenario $(if ($null -ne $baselineSelection) { $baselineSelection.Scenario } else { "" }) `
    -InternalFrames $(if ($null -ne $baselineSelection) { $baselineSelection.InternalFrames } else { 0 }) `
    -MeasureFrames $(if ($null -ne $baselineSelection) { $baselineSelection.MeasureFrames } else { 0 })
}
$payloadPaths = @()
$studyFailure = $null
$recoveryFailure = $null
$preparePath = $null
$studyPath = $null
$cleanupPath = $null
$resolvedEvidenceDirectory = $null

try {
  $preparePath = Write-PayloadFile "prepare" $payloadBundle.Prepare $runId
  $studyPath = Write-PayloadFile "study" $payloadBundle.Study $runId
  $cleanupPath = Write-PayloadFile "cleanup" $payloadBundle.Cleanup $runId
  $payloadPaths += @($preparePath, $studyPath, $cleanupPath)

  if ($PrintOnly) {
    Write-Output "PrintOnly: no Orange transport is invoked."
    if ($artifactRequired -and $artifactExists -and (Test-Path -LiteralPath $Metadata -PathType Leaf)) {
      Write-Output "Local release metadata: verified"
    } elseif ($artifactRequired) {
      Write-Output "Local release metadata: required before a non-print run"
    }
    Write-Output "Fixed target: $target"
    Write-Output "Remote study root: $remoteRoot"
    Write-Output "Candidate health path: $healthPath"
    if ($Mode -eq "LiveAudioBenchmark") {
      Write-Output "Live selection: $($liveSelection.MatrixClass) output=$($liveSelection.OutputFrames) period=$($liveSelection.AlsaPeriodFrames) engine=$($liveSelection.EngineBlockFrames) internal=$($liveSelection.InternalFrames) scenario=$($liveSelection.Scenario) measure=$($liveSelection.MeasureSeconds) warmup=5 worker-timing=$WorkerTimingMode"
      Write-Output "Live release path: $benchmarkRoot/release.json"
      Write-Output "Live readiness path: $benchmarkRoot/readiness.json"
      Write-Output "Live progress path: $benchmarkRoot/progress.json"
      Write-Output "Live result path: $benchmarkRoot/result.json"
    }
    if ($Mode -eq "ProfileBaseline") {
      Write-Output "Profile baseline selection: scenario=$($baselineSelection.Scenario) internal=$($baselineSelection.InternalFrames) measure=$($baselineSelection.MeasureFrames) warmup=2 observations=4096"
    }
    Write-Output "Prepare payload:"
    Write-Output $payloadBundle.Prepare
    Write-Output "Prepare payload transport:"
    Write-Output (Format-TransportCommand "ssh-payload" @($target, $preparePath))
    Write-Output "Study payload:"
    Write-Output $payloadBundle.Study
    Write-Output "Study payload transport:"
    Write-Output (Format-TransportCommand "ssh-payload" @($target, $studyPath))
    if ($artifactRequired) {
      Write-Output (Format-TransportCommand "scp" @($Artifact, "$target`:$remoteBinary"))
      Write-Output (Format-TransportCommand "scp" @($Metadata, "$target`:$remoteMetadata"))
    }
    Write-Output "Retrieve payload transport:"
    Write-Output (Format-TransportCommand "scp" @("-r", "$target`:$remoteRoot/.", $OutputDirectory))
    Write-Output "Cleanup payload:"
    Write-Output $payloadBundle.Cleanup
    Write-Output (Format-TransportCommand "ssh-payload" @($target, $cleanupPath))
  } else {
    Invoke-OrangeTransport "ssh-payload" @($target, $preparePath)
    if ($artifactRequired) {
      Invoke-OrangeTransport "scp" @($Artifact, "$target`:$remoteBinary")
      Invoke-OrangeTransport "scp" @($Metadata, "$target`:$remoteMetadata")
    }
    Invoke-OrangeTransport "ssh-payload" @($target, $studyPath)
  }
} catch {
  $studyFailure = $_
} finally {
  if (-not $PrintOnly) {
    $localRunDirectory = Join-Path $OutputDirectory "orange-study-$runId"
    New-Item -ItemType Directory -Force -Path $localRunDirectory | Out-Null
    Write-Output "Evidence staging directory: $localRunDirectory"
    try {
      Invoke-OrangeTransport "scp" @("-r", "$target`:$remoteRoot/.", $localRunDirectory)
    } catch {
      if ($null -eq $studyFailure) { $studyFailure = $_ }
    }
    if ($null -ne $cleanupPath) {
      try {
        Invoke-OrangeTransport "ssh-payload" @($target, $cleanupPath)
      } catch {
        if ($null -eq $recoveryFailure) { $recoveryFailure = $_ }
      }
    }
    if ($Mode -eq "LiveAudioBenchmark") {
      $resolvedEvidenceDirectory = $null
      try {
        $resolvedEvidenceDirectory = Resolve-OrangeLiveEvidenceDirectory `
          -LocalRunDirectory $localRunDirectory `
          -RemoteRoot $remoteRoot
        Write-Output "Evidence directory: $resolvedEvidenceDirectory"
      } catch {
        if ($null -eq $studyFailure) { $studyFailure = $_ }
      }
      if ($null -ne $resolvedEvidenceDirectory) {
        try {
          $hostEvidence = Get-OrangeLiveHostEvidence `
            -EvidenceDirectory $resolvedEvidenceDirectory `
            -Selection $liveSelection `
            -ArtifactHash $artifactHash
          $hostEvidencePath = Join-Path $resolvedEvidenceDirectory "host-evidence.json"
          $hostEvidence | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $hostEvidencePath -Encoding UTF8
          if ($hostEvidence.StatusClass -ne "pass" -and $null -eq $studyFailure) {
            $studyFailure = "Live benchmark status class was $($hostEvidence.StatusClass): $($hostEvidence.Reason)"
          }
        } catch {
          if ($null -eq $studyFailure) { $studyFailure = $_ }
        }
      }
      if ($null -ne $recoveryFailure) {
        $recoveryReason = [string]$recoveryFailure.Exception.Message
        if ([string]::IsNullOrWhiteSpace($recoveryReason)) { $recoveryReason = [string]$recoveryFailure }
        $overrideDirectory = if ($null -ne $resolvedEvidenceDirectory) { $resolvedEvidenceDirectory } else { $localRunDirectory }
        Write-LiveRestorationFailureEvidence -EvidenceDirectory $overrideDirectory -Reason $recoveryReason | Out-Null
      }
    } else {
      Write-Output "Evidence directory: $localRunDirectory"
    }
  }
  foreach ($payloadPath in $payloadPaths) {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
}

if ($null -ne $recoveryFailure) {
  throw $recoveryFailure
}
if ($null -ne $studyFailure) {
  throw $studyFailure
}
if (-not $PrintOnly) {
  Write-Output "Orange capability study completed: $Mode"
}
