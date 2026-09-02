[CmdletBinding()]
param(
  [string]$Artifact = "",
  [string]$Metadata = "",
  [string]$OutputDirectory = "",
  [ValidateRange(1, 120)]
  [int]$ReleaseTimeoutSeconds = 120,
  [ValidateRange(5, 60)]
  [int]$StartupTimeoutSeconds = 20,
  [string]$RunnerPath = "",
  [switch]$AllowMatrixServiceInterruption,
  [switch]$CanaryOnly,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$runner = if ([string]::IsNullOrWhiteSpace($RunnerPath)) { Join-Path $PSScriptRoot "run-orange-capability-study.ps1" } else { $RunnerPath }
$validationModule = Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1"
$defaultOutput = Join-Path $PSScriptRoot "..\..\target\orange-pi-study"
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = $defaultOutput }
Import-Module $validationModule -Force

function Write-MatrixManifest {
  param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][object[]]$Results
  )
  $json = ConvertTo-OrangeLiveManifestJson -Results @($Results)
  $temporary = "$Path.tmp-$PID"
  [IO.File]::WriteAllText($temporary, "$json`n", (New-Object System.Text.UTF8Encoding($false)))
  Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Write-PrintOnlyPlan {
  if ($CanaryOnly) {
    $cell = @(Get-OrangeLiveMatrixPlan | Where-Object { $_.Scenario -eq "synth_ramp_16" -and $_.OutputFrames -eq 256 } | Select-Object -First 1)
    Write-Output "Orange live audio matrix PrintOnly CanaryOnly: no transport is invoked."
    Write-Output ("01: {0} output={1} internal={2} measure={3}" -f $cell[0].Scenario, $cell[0].OutputFrames, $cell[0].InternalFrames, $cell[0].MeasureSeconds)
    Write-Output "Matrix cells: 1 total (CanaryOnly)."
    return
  }
  Write-Output "Orange live audio matrix PrintOnly: no transport is invoked."
  $index = 1
  $plan = @(Get-OrangeLiveMatrixPlan)
  foreach ($cell in @($plan | Where-Object { $_.OutputFrames -eq 256 })) {
    Write-Output ("{0:D2}: {1} output={2} internal={3} measure={4}" -f $index, $cell.Scenario, $cell.OutputFrames, $cell.InternalFrames, $cell.MeasureSeconds)
    $index++
  }
  Write-Output ("{0:D2}: A120 scenario=<highest passing A p99.9, then max> output=256 internal=128 measure=120 warmup=5" -f $index)
  $index++
  $afterLong = @($plan | Where-Object { $_.OutputFrames -ne 256 })
  foreach ($cell in $afterLong) {
    Write-Output ("{0:D2}: {1} output={2} internal={3} measure={4}" -f $index, $cell.Scenario, $cell.OutputFrames, $cell.InternalFrames, $cell.MeasureSeconds)
    $index++
  }
  Write-Output "Matrix cells: 23 total (11 A + 1 selected A120 + 11 B)."
}

function Invoke-LiveCell {
  param(
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$CellKey,
    [Parameter(Mandatory)][string]$MatrixRunId
  )
  $processArguments = @(
    "-NoLogo"
    "-NoProfile"
    "-NonInteractive"
    "-ExecutionPolicy"
    "Bypass"
    "-File"
    $runner
    "-Mode"
    "LiveAudioBenchmark"
    "-WorkerTimingMode"
    "enabled"
    "-ExecutorMode"
    "persistent_two_workers"
    "-Artifact"
    $Artifact
    "-Metadata"
    $Metadata
    "-OutputDirectory"
    $OutputDirectory
    "-Scenario"
    $Selection.Scenario
    "-OutputFrames"
    [string]$Selection.OutputFrames
    "-EngineBlockFrames"
    [string]$Selection.InternalFrames
    "-MeasureSeconds"
    [string]$Selection.MeasureSeconds
    "-StartupTimeoutSeconds"
    [string]$StartupTimeoutSeconds
    "-ReleaseTimeoutSeconds"
    [string]$ReleaseTimeoutSeconds
    "-AllowServiceInterruption"
  )
  if ($Selection.LongRepeat) { $processArguments += "-AllowLongRepeat" }
  $output = New-Object 'System.Collections.Generic.List[object]'
  $exitCode = 0
  $runnerThrew = $false
  $previousErrorActionPreference = $ErrorActionPreference
  $transcriptPath = Join-Path $OutputDirectory "orange-live-audio-matrix-$MatrixRunId-$CellKey.log"
  try {
    try {
      $ErrorActionPreference = "Continue"
      & (Join-Path $PSHOME "powershell.exe") @processArguments 2>&1 | ForEach-Object { [void]$output.Add($_) }
      $exitCode = [int]$LASTEXITCODE
      $runnerThrew = $exitCode -ne 0
    } catch {
      $exitCode = 1
      $runnerThrew = $true
      [void]$output.Add($_)
    } finally {
      $ErrorActionPreference = $previousErrorActionPreference
    }
  } finally {
    $transcriptLines = @($output | ForEach-Object { [string]$_ })
    [IO.File]::WriteAllLines($transcriptPath, $transcriptLines, (New-Object System.Text.UTF8Encoding($false)))
  }
  $capturedDiagnostic = @($output | ForEach-Object { [string]$_ } | Select-Object -Last 20) -join [Environment]::NewLine
  $evidenceLine = @($output | ForEach-Object { [string]$_ } | Where-Object { $_ -like "Evidence directory: *" } | Select-Object -Last 1)
  $stagingLine = @($output | ForEach-Object { [string]$_ } | Where-Object { $_ -like "Evidence staging directory: *" } | Select-Object -Last 1)
  $hasStrictEvidence = $evidenceLine.Count -gt 0
  $directoryLine = if ($hasStrictEvidence) { $evidenceLine[0] } elseif ($stagingLine.Count -gt 0) { $stagingLine[0] } else { $null }
  if ($null -eq $directoryLine) {
    return [pscustomobject]@{
      CellKey = $CellKey
      Scenario = $Selection.Scenario
      OutputFrames = $Selection.OutputFrames
      AlsaPeriodFrames = $Selection.AlsaPeriodFrames
      EngineBlockFrames = $Selection.EngineBlockFrames
      InternalFrames = $Selection.InternalFrames
      MeasureSeconds = $Selection.MeasureSeconds
      StatusClass = "infrastructure_failure"
      Reason = "single-scenario runner did not publish an evidence directory"
      ExitCode = $exitCode
      RunnerThrew = $runnerThrew
      TranscriptPath = $transcriptPath
      CapturedDiagnostic = $capturedDiagnostic
      AggregateRenderAudioDurationRatio = $null
    }
  }
  $prefix = if ($hasStrictEvidence) { "Evidence directory: " } else { "Evidence staging directory: " }
  $evidenceDirectory = ([string]$directoryLine).Substring($prefix.Length).Trim()
  $runId = $null
  try { $runId = Get-OrangeLiveRunId $evidenceDirectory } catch {
    return [pscustomobject]@{
      CellKey = $CellKey
      Scenario = $Selection.Scenario
      OutputFrames = $Selection.OutputFrames
      AlsaPeriodFrames = $Selection.AlsaPeriodFrames
      EngineBlockFrames = $Selection.EngineBlockFrames
      InternalFrames = $Selection.InternalFrames
      MeasureSeconds = $Selection.MeasureSeconds
      StatusClass = "infrastructure_failure"
      Reason = "single-scenario runner evidence directory had no valid run ID: $($_.Exception.Message)"
      ExitCode = $exitCode
      RunnerThrew = $runnerThrew
      EvidenceDirectory = $evidenceDirectory
      TranscriptPath = $transcriptPath
      CapturedDiagnostic = $capturedDiagnostic
      AggregateRenderAudioDurationRatio = $null
    }
  }
  if (-not $hasStrictEvidence) {
    return [pscustomobject]@{
      RunId = $runId
      CellKey = $CellKey
      Scenario = $Selection.Scenario
      OutputFrames = $Selection.OutputFrames
      AlsaPeriodFrames = $Selection.AlsaPeriodFrames
      EngineBlockFrames = $Selection.EngineBlockFrames
      InternalFrames = $Selection.InternalFrames
      MeasureSeconds = $Selection.MeasureSeconds
      StatusClass = "infrastructure_failure"
      Reason = "single-scenario runner published staging diagnostics without terminal evidence"
      ExitCode = $exitCode
      RunnerThrew = $runnerThrew
      EvidenceDirectory = $evidenceDirectory
      TranscriptPath = $transcriptPath
      CapturedDiagnostic = $capturedDiagnostic
      AggregateRenderAudioDurationRatio = $null
    }
  }
  $hostEvidencePath = Join-Path $evidenceDirectory "host-evidence.json"
  if (-not (Test-Path -LiteralPath $hostEvidencePath -PathType Leaf)) {
    return [pscustomobject]@{
      CellKey = $CellKey
      Scenario = $Selection.Scenario
      OutputFrames = $Selection.OutputFrames
      AlsaPeriodFrames = $Selection.AlsaPeriodFrames
      EngineBlockFrames = $Selection.EngineBlockFrames
      InternalFrames = $Selection.InternalFrames
      MeasureSeconds = $Selection.MeasureSeconds
      StatusClass = "infrastructure_failure"
      Reason = "single-scenario runner did not publish host evidence"
      ExitCode = $exitCode
      RunnerThrew = $runnerThrew
      EvidenceDirectory = $evidenceDirectory
      RunId = $runId
      TranscriptPath = $transcriptPath
      CapturedDiagnostic = $capturedDiagnostic
      AggregateRenderAudioDurationRatio = $null
    }
  }
  $evidence = Get-Content -LiteralPath $hostEvidencePath -Raw | ConvertFrom-Json
  $properties = [ordered]@{ RunId = $runId; CellKey = $CellKey; ExitCode = $exitCode; EvidenceDirectory = $evidenceDirectory }
  foreach ($property in $evidence.PSObject.Properties) { $properties[$property.Name] = $property.Value }
  if (-not $properties.Contains("AggregateRenderAudioDurationRatio")) { $properties.AggregateRenderAudioDurationRatio = $null }
  $properties.RunnerThrew = $runnerThrew
  $properties.TranscriptPath = $transcriptPath
  $properties.CapturedDiagnostic = $capturedDiagnostic
  $properties.StatusClass = Resolve-OrangeLiveRunnerOutcome `
    -EvidenceStatusClass ([string]$properties.StatusClass) `
    -RunnerThrew:$runnerThrew
  if ($runnerThrew -and [string]$evidence.StatusClass -eq "pass") { $properties.Reason = "single-scenario runner threw after publishing apparently passing evidence" }
  return [pscustomobject]$properties
}

if ($PrintOnly) {
  Write-PrintOnlyPlan
  exit 0
}
if (-not $AllowMatrixServiceInterruption) {
  throw "Active matrix execution requires the separate -AllowMatrixServiceInterruption consent."
}
if ([string]::IsNullOrWhiteSpace($Artifact) -or [string]::IsNullOrWhiteSpace($Metadata)) {
  throw "Active matrix execution requires both -Artifact and -Metadata."
}

$matrixRunId = [guid]::NewGuid().ToString("N")
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$manifestPath = Join-Path $OutputDirectory "orange-live-audio-matrix-$matrixRunId.json"
$results = @()
try {
  if ($CanaryOnly) {
    $canaryCell = @(Get-OrangeLiveMatrixPlan | Where-Object { $_.Scenario -eq "synth_ramp_16" -and $_.OutputFrames -eq 256 } | Select-Object -First 1)
    if ($canaryCell.Count -ne 1) { throw "Approved A/synth_ramp_16 canary cell was not found." }
    $canaryKey = "A-$($canaryCell[0].Scenario)"
    $canaryResult = Invoke-LiveCell $canaryCell[0] $canaryKey $matrixRunId
    $results += $canaryResult
    Write-MatrixManifest $manifestPath $results
    if ($canaryResult.StatusClass -ne "pass") { throw "Matrix stopped at ${canaryKey}: $($canaryResult.StatusClass) $($canaryResult.Reason)" }
  } else {
    foreach ($cell in @(Get-OrangeLiveMatrixPlan | Where-Object { $_.OutputFrames -eq 256 })) {
      $key = "A-$($cell.Scenario)"
      $result = Invoke-LiveCell $cell $key $matrixRunId
      $results += $result
      Write-MatrixManifest $manifestPath $results
      if ($result.StatusClass -ne "pass") { throw "Matrix stopped at ${key}: $($result.StatusClass) $($result.Reason)" }
    }

    $worst = Get-OrangeLiveWorstPassingScenario $results
    $longSelection = Assert-OrangeLiveBenchmarkSelection `
      -Scenario $worst.Scenario `
      -OutputFrames 256 `
      -EngineBlockFrames 128 `
      -MeasureSeconds 120 `
      -AllowLongRepeat:$true
    $longResult = Invoke-LiveCell $longSelection "A120-$($longSelection.Scenario)" $matrixRunId
    $results += $longResult
    Write-MatrixManifest $manifestPath $results
    if ($longResult.StatusClass -ne "pass") { throw "Matrix stopped at A120: $($longResult.StatusClass) $($longResult.Reason)" }

    foreach ($cell in @(Get-OrangeLiveMatrixPlan | Where-Object { $_.OutputFrames -eq 512 })) {
      $key = "B-$($cell.Scenario)"
      $result = Invoke-LiveCell $cell $key $matrixRunId
      $results += $result
      Write-MatrixManifest $manifestPath $results
      if ($result.StatusClass -ne "pass") { throw "Matrix stopped at ${key}: $($result.StatusClass) $($result.Reason)" }
    }
  }
} finally {
  Write-MatrixManifest $manifestPath $results
}
Write-Output "Orange live audio matrix completed: $manifestPath"
