[CmdletBinding()]
param(
  [string]$PlanPath = "",
  [string]$StudyId = "",
  [string]$RaspberryArtifact = "",
  [string]$RaspberryMetadata = "",
  [string]$OrangeInlineArtifact = "",
  [string]$OrangeInlineMetadata = "",
  [string]$OrangeMulticoreArtifact = "",
  [string]$OrangeMulticoreMetadata = "",
  [switch]$PrintOnly,
  [switch]$Resume,
  [switch]$Detach,
  [switch]$Worker,
  [switch]$AllowServiceInterruption,
  [ValidateRange(1, 20)][int]$WallClockHours = 20
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ([string]::IsNullOrWhiteSpace($PlanPath)) { $PlanPath = Join-Path $repoRoot "docs\internal\pi-audio-capacity-frame-search.md" }
$planModule = Join-Path $PSScriptRoot "audio-capacity-frame-search-plan.psm1"
$domainModule = Join-Path $PSScriptRoot "audio-capacity-frame-search-domain.psm1"
$stateModule = Join-Path $PSScriptRoot "audio-capacity-frame-search-state.psm1"
Import-Module $stateModule -Force
Import-Module $domainModule -Force
Import-Module $planModule -Force

function Get-FrameSearchStudyDirectory {
  param([Parameter(Mandatory)][string]$Id)
  if ($Id -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$') { throw "StudyId must be a simple non-secret identifier." }
  return Join-Path $repoRoot (Join-Path "target\audio-capacity-frame-search" $Id)
}

function Assert-FrameSearchRepositoryClean {
  param([Parameter(Mandatory)][string]$RepoRoot)
  $status = @(& git -C $RepoRoot status --porcelain --untracked-files=all 2>&1)
  $exitCode = $LASTEXITCODE
  if ($exitCode -ne 0) { throw "active frame-search launch requires a clean exact-source tree. git status failed with exit code $exitCode." }
  if ($status.Count -gt 0) { throw "active frame-search launch requires a clean exact-source tree." }
}

function Save-FrameSearchState {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  Write-FrameSearchAtomicJson $StatePath $State
  Write-FrameSearchResultsMarkdown $State $ResultsPath
}

function New-FrameSearchRunRecord {
  param([Parameter(Mandatory)][object]$Schedule, [Parameter(Mandatory)][string]$Phase, [Parameter(Mandatory)][int]$Seconds, [string]$SkipReason = "")
  return [pscustomobject][ordered]@{
    Run = $Schedule.Run; Cell = $Schedule.Cell; Rep = [int]$Schedule.Rep; Epoch = [int]$Schedule.Epoch; Wave = $Schedule.Wave; Phase = $Phase; Profile = $Schedule.Profile; Board = $Schedule.Board; Mode = $Schedule.Mode; Output = [int]$Schedule.Output; Period = [int]$Schedule.Period; Internal = [int]$Schedule.Internal; Lookahead = [int]$Schedule.Lookahead; Effective = [int]$Schedule.Effective; U = [int]$Schedule.U; Sec = $Seconds; Status = if ($SkipReason) { "skipped" } else { "pending" }; SkipReason = $SkipReason; StartedUtc = ""; FinishedUtc = ""; ExitCode = $null; ProcessId = $null; StatusClass = ""; Grade = ""; Worst = $null; RepeatIncidents = $null; SilentIncidents = $null; AlsaRecoveryLogIncidents = $null; CpalStreamErrors = $null; CpalDeviceErrors = $null; CallbackOverBudget = $null; P999 = $null; MaxRatio = $null; NativeStatus = ""; NativeWorker = ""; EvidenceDirectory = ""; EvidencePath = ""; HostEvidencePath = ""; ResultPath = ""; StdoutPath = ""; StderrPath = ""; Failure = ""; StructurallyComplete = $false; OrderQuality = if ($Schedule.PSObject.Properties["OrderQuality"]) { $Schedule.OrderQuality } else { "" }; ScheduleIndex = [int]$Schedule.ScheduleIndex
  }
}

function New-FrameSearchSkippedRun {
  param([Parameter(Mandatory)][object]$Row, [Parameter(Mandatory)][int]$Rep, [Parameter(Mandatory)][string]$Wave, [Parameter(Mandatory)][string]$Reason)
  $schedule = [pscustomobject][ordered]@{ Run = "$($Row.Cell)-rep$Rep"; Cell = $Row.Cell; Rep = $Rep; Epoch = $Rep; Wave = $Wave; Profile = $Row.Profile; Board = $Row.Board; Mode = $Row.Mode; Output = $Row.Output; Period = $Row.Period; Internal = $Row.Internal; Lookahead = $Row.Lookahead; Effective = $Row.Effective; U = $Row.U; ScheduleIndex = 0; OrderQuality = "deferred" }
  return New-FrameSearchRunRecord $schedule "Preliminary" 180 $Reason
}

function Get-FrameSearchRunById {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][string]$RunId)
  $matches = @($State.runs | Where-Object { $_.Run -ceq $RunId })
  if ($matches.Count -ne 1) { throw "Frame-search run identity is not unique: $RunId" }
  return $matches[0]
}

function Get-FrameSearchArtifactForRun {
  param([Parameter(Mandatory)][object[]]$Artifacts, [Parameter(Mandatory)][object]$Run)
  $name = if ($Run.Board -ceq "Raspberry") { "Raspberry" } elseif ($Run.Mode -ceq "Inline") { "OrangeInline" } else { "OrangeMulticore" }
  $matches = @($Artifacts | Where-Object { $_.Name -ceq $name })
  if ($matches.Count -ne 1) { throw "No exact artifact identity is available for $($Run.Board) $($Run.Mode)." }
  return $matches[0]
}

function New-FrameSearchRunnerArguments {
  param([Parameter(Mandatory)][object]$Run, [Parameter(Mandatory)][object]$Artifact, [Parameter(Mandatory)][string]$OutputDirectory)
  if ($Run.Board -ceq "Raspberry") {
    return @("-Units", [string]$Run.U, "-FrameSearchProfile", $Run.Profile, "-FrameSearchPhase", $Run.Phase, "-MeasureSeconds", [string]$Run.Sec, "-Artifact", $Artifact.ArtifactPath, "-Metadata", $Artifact.MetadataPath, "-OutputDirectory", $OutputDirectory, "-AllowServiceInterruption")
  }
  return @("-Mode", "LiveAudioBenchmark", "-FrameSearchProfile", $Run.Profile, "-FrameSearchPhase", $Run.Phase, "-FrameSearchU", [string]$Run.U, "-Artifact", $Artifact.ArtifactPath, "-Metadata", $Artifact.MetadataPath, "-OutputDirectory", $OutputDirectory, "-AllowServiceInterruption")
}

function Get-FrameSearchBoardRunnerPath {
  param([Parameter(Mandatory)][object]$Run)
  if ($Run.Board -ceq "Raspberry") { return Join-Path $repoRoot "tools\pi\run-pi-live-audio-benchmark.ps1" }
  return Join-Path $repoRoot "tools\orange-pi\run-orange-capability-study.ps1"
}

function Invoke-FrameSearchRun {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object]$Run, [Parameter(Mandatory)][object[]]$Artifacts, [Parameter(Mandatory)][string]$StudyDirectory, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  if ($Run.Status -ne "pending") { return }
  $artifact = Get-FrameSearchArtifactForRun $Artifacts $Run
  $runDirectory = Join-Path $StudyDirectory (Join-Path "runs" $Run.Run)
  $runnerOutput = Join-Path $runDirectory "runner-output"
  $stdoutPath = Join-Path $runDirectory "stdout.txt"; $stderrPath = Join-Path $runDirectory "stderr.txt"
  $Run.Status = "running"; $Run.StartedUtc = [datetime]::UtcNow.ToString("o"); $Run.StdoutPath = $stdoutPath; $Run.StderrPath = $stderrPath
  Save-FrameSearchState $State $StatePath $ResultsPath
  try {
    $runner = Get-FrameSearchBoardRunnerPath $Run
    $arguments = New-FrameSearchRunnerArguments $Run $artifact $runnerOutput
    $child = Invoke-FrameSearchChildProcess $runner $arguments $stdoutPath $stderrPath
    $Run.ProcessId = [int]$child.ProcessId
    $Run.ExitCode = [int]$child.ExitCode
    Assert-FrameSearchChildExit $child.ExitCode
    $matches = [regex]::Matches($child.Stdout, '(?m)^Evidence directory:\s*(.+?)\s*$')
    if ($matches.Count -ne 1) { throw "Board runner did not emit exactly one final evidence directory." }
    $evidenceDirectory = $matches[0].Groups[1].Value.Trim()
    if (-not (Test-Path -LiteralPath $evidenceDirectory -PathType Container)) { throw "Board runner evidence directory was not found: $evidenceDirectory" }
    $expectedBoard = if ($Run.Board -ceq "Raspberry") { "raspberry-pi-zero-2w" } else { "orange-pi-zero-2w" }
    $evidence = Assert-FrameSearchHostEvidence $evidenceDirectory $Run $artifact.Sha256 $expectedBoard
    $Run.Status = "completed"; $Run.StatusClass = $evidence.StatusClass; $Run.Grade = $evidence.Grade; $Run.Worst = $evidence.Worst; $Run.RepeatIncidents = $evidence.RepeatIncidents; $Run.SilentIncidents = $evidence.SilentIncidents; $Run.AlsaRecoveryLogIncidents = $evidence.AlsaRecoveryLogIncidents; $Run.CpalStreamErrors = $evidence.CpalStreamErrors; $Run.CpalDeviceErrors = $evidence.CpalDeviceErrors; $Run.CallbackOverBudget = $evidence.CallbackOverBudget; $Run.P999 = $evidence.P999; $Run.MaxRatio = $evidence.MaxRatio; $Run.NativeStatus = $evidence.NativeStatus; $Run.NativeWorker = $evidence.NativeWorker; $Run.EvidenceDirectory = $evidence.EvidenceDirectory; $Run.EvidencePath = $evidence.EvidencePath; $Run.HostEvidencePath = $evidence.HostEvidencePath; $Run.ResultPath = $evidence.ResultPath; $Run.StructurallyComplete = $true; $Run.FinishedUtc = [datetime]::UtcNow.ToString("o")
  } catch {
    $Run.Status = "failed"; $Run.Failure = $_.Exception.Message; $Run.FinishedUtc = [datetime]::UtcNow.ToString("o"); Save-FrameSearchState $State $StatePath $ResultsPath; throw
  }
  Save-FrameSearchState $State $StatePath $ResultsPath
}

function Mark-FrameSearchSeedSkips {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][string]$Wave, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  $pairs = Get-FrameSearchPairObservations @($State.runs)
  foreach ($row in $Rows) {
    if (@($State.runs | Where-Object { $_.Cell -ceq $row.Cell }).Count -gt 0) { continue }
    $reason = Get-FrameSearchSeedSkipReason $row @($State.runs) $pairs
    if ($reason) {
      $State.runs += ,(New-FrameSearchSkippedRun $row 1 $Wave $reason); $State.runs += ,(New-FrameSearchSkippedRun $row 2 $Wave $reason); $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $row.Profile; Reason = "$($row.Cell): $reason" })
    }
  }
  Save-FrameSearchState $State $StatePath $ResultsPath
}

function Invoke-FrameSearchPreliminaryWave {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][object[]]$Artifacts, [Parameter(Mandatory)][string]$Wave, [Parameter(Mandatory)][string]$StudyDirectory, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  Mark-FrameSearchSeedSkips $State $Rows $Wave $StatePath $ResultsPath
  $candidateRows = @()
  foreach ($row in $Rows) {
    $cellRuns = @($State.runs | Where-Object { $_.Cell -ceq $row.Cell })
    if ($cellRuns.Count -eq 0 -or @($cellRuns | Where-Object { $_.Status -eq "pending" }).Count -gt 0) { $candidateRows += ,$row }
  }
  foreach ($row in @($State.deferred_rows | Where-Object { $_.Cell -notin @($Rows | ForEach-Object { $_.Cell }) })) {
    if (@($candidateRows | Where-Object { $_.Cell -ceq $row.Cell }).Count -eq 0 -and @($State.runs | Where-Object { $_.Cell -ceq $row.Cell }).Count -eq 0) { $candidateRows += ,$row }
  }
  if ($candidateRows.Count -eq 0) { return }
  if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) {
    foreach ($row in $candidateRows) { $cellRuns = @($State.runs | Where-Object { $_.Cell -ceq $row.Cell }); if ($cellRuns.Count -eq 0) { $State.runs += ,(New-FrameSearchSkippedRun $row 1 $Wave "wall-clock reserve reached before preliminary/adaptive work"); $State.runs += ,(New-FrameSearchSkippedRun $row 2 $Wave "wall-clock reserve reached before preliminary/adaptive work") } else { foreach ($run in @($cellRuns | Where-Object { $_.Status -eq "pending" })) { $run.Status = "skipped"; $run.SkipReason = "wall-clock reserve reached before preliminary/adaptive work" } } }
    Save-FrameSearchState $State $StatePath $ResultsPath
    return
  }
  if ($candidateRows.Count -eq 1) {
    $row = $candidateRows[0]
    if (@($State.deferred_rows | Where-Object { $_.Cell -ceq $row.Cell }).Count -eq 0) { $State.deferred_rows += ,$row }
    $State.decisions += ,([pscustomobject][ordered]@{ Kind = "deferred"; Profile = $row.Profile; Reason = "$($row.Cell): singleton batch deferred until another active cell is available" })
    Save-FrameSearchState $State $StatePath $ResultsPath
    return
  }
  $schedule = @(New-FrameSearchPhysicalSchedule $candidateRows $Wave)
  $scheduledCells = @($schedule | Select-Object -ExpandProperty Cell -Unique)
  $State.deferred_rows = @($State.deferred_rows | Where-Object { $scheduledCells -notcontains $_.Cell })
  foreach ($scheduled in $schedule) {
    if (@($State.runs | Where-Object { $_.Run -ceq $scheduled.Run }).Count -eq 0) { $State.runs += ,(New-FrameSearchRunRecord $scheduled "Preliminary" 180) }
    $run = Get-FrameSearchRunById $State $scheduled.Run
    if ($run.Status -eq "pending") { if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) { $run.Status = "skipped"; $run.SkipReason = "wall-clock reserve reached before preliminary/adaptive work"; Save-FrameSearchState $State $StatePath $ResultsPath; continue }; Invoke-FrameSearchRun $State $run $Artifacts $StudyDirectory $StatePath $ResultsPath }
  }
}

function Add-FrameSearchAdaptiveRows {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][string]$Wave, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  $existing = @($State.runs | Select-Object -ExpandProperty Cell -Unique)
  $rows = @($Rows | Where-Object { $existing -notcontains $_.Cell })
  if ($rows.Count -eq 0) { return @() }
  $remainingPhysical = 64 - @($State.runs | Where-Object { $_.Wave -like "A*" }).Count
  if ($remainingPhysical -lt 2) { foreach ($row in $rows) { $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $row.Profile; Reason = "$($row.Cell): adaptive physical-run budget exhausted" }) }; Save-FrameSearchState $State $StatePath $ResultsPath; return @() }
  $maxRows = [math]::Floor($remainingPhysical / 2); if ($rows.Count -gt $maxRows) { foreach ($row in @($rows | Select-Object -Skip $maxRows)) { $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $row.Profile; Reason = "$($row.Cell): adaptive physical-run budget exhausted" }) }; $rows = @($rows | Select-Object -First $maxRows) }
  if ($rows.Count -eq 1) {
    $row = $rows[0]
    if (@($State.deferred_rows | Where-Object { $_.Cell -ceq $row.Cell }).Count -eq 0) { $State.deferred_rows += ,$row }
    $State.decisions += ,([pscustomobject][ordered]@{ Kind = "deferred"; Profile = $row.Profile; Reason = "$($row.Cell): singleton adaptive batch deferred" })
    Save-FrameSearchState $State $StatePath $ResultsPath
    return @()
  }
  $schedule = New-FrameSearchPhysicalSchedule $rows $Wave
  $nextScheduleIndex = 1
  $existingScheduleIndexes = @($State.runs | Where-Object { $null -ne $_.PSObject.Properties["ScheduleIndex"] } | ForEach-Object { [int]$_.ScheduleIndex })
  if ($existingScheduleIndexes.Count -gt 0) { $nextScheduleIndex = 1 + [int]($existingScheduleIndexes | Measure-Object -Maximum).Maximum }
  foreach ($scheduled in $schedule) {
    $run = New-FrameSearchRunRecord $scheduled "Preliminary" 180
    $run.ScheduleIndex = $nextScheduleIndex
    $nextScheduleIndex++
    $State.runs += ,$run
  }
  $scheduledCells = @($rows | ForEach-Object { $_.Cell })
  $State.deferred_rows = @($State.deferred_rows | Where-Object { $scheduledCells -notcontains $_.Cell })
  $State.adaptive_rows += @($rows)
  Save-FrameSearchState $State $StatePath $ResultsPath
  return $schedule
}

function Invoke-FrameSearchPendingAdaptiveRuns {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object[]]$Artifacts, [Parameter(Mandatory)][string]$StudyDirectory, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  $pending = @(Get-FrameSearchPendingAdaptiveRuns $State)
  foreach ($run in $pending) {
    if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) {
      $State.adaptive_reserve_reached = $true
      foreach ($remaining in @($pending | Where-Object { $_.Status -ceq "pending" })) {
        $remaining.Status = "skipped"
        $remaining.SkipReason = "wall-clock reserve reached before adaptive physical run"
        $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $remaining.Profile; Reason = "$($remaining.Cell): wall-clock reserve reached before adaptive physical run" })
      }
      Save-FrameSearchState $State $StatePath $ResultsPath
      return
    }
    Invoke-FrameSearchRun $State $run $Artifacts $StudyDirectory $StatePath $ResultsPath
  }
}

function Invoke-FrameSearchCampaign {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object]$Plan, [Parameter(Mandatory)][object[]]$Artifacts, [Parameter(Mandatory)][string]$StudyDirectory, [Parameter(Mandatory)][string]$StatePath, [Parameter(Mandatory)][string]$ResultsPath)
  if ($State.status -ceq "recovery_required") { return }
  foreach ($wave in @("W1", "W2", "W3", "W4")) {
    $rows = @($Plan.SeedQueue | Where-Object { $_.Wave -ceq $wave })
    Invoke-FrameSearchPreliminaryWave $State $rows $Artifacts $wave $StudyDirectory $StatePath $ResultsPath
    if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) { $State.adaptive_reserve_reached = $true; Save-FrameSearchState $State $StatePath $ResultsPath; break }
  }
  if (-not (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc))) {
    Invoke-FrameSearchPendingAdaptiveRuns $State $Artifacts $StudyDirectory $StatePath $ResultsPath
  }
  if (-not (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) -and -not $State.adaptive_reserve_reached) {
    $adaptiveWave = 1
    while ($true) {
      if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) { $State.adaptive_reserve_reached = $true; Save-FrameSearchState $State $StatePath $ResultsPath; break }
      $pairs = Get-FrameSearchPairObservations @($State.runs)
      $blockedCells = @($State.runs | Where-Object { $_.Status -eq "skipped" } | ForEach-Object { $_.Cell }) + @($State.deferred_rows | ForEach-Object { $_.Cell })
      $adaptive = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) $pairs -BlockedCells $blockedCells
      $State.analyses = @($adaptive.Analyses)
      $State.decisions += @($adaptive.Decisions)
      Save-FrameSearchState $State $StatePath $ResultsPath
      if ($adaptive.Rows.Count -eq 0) { break }
      $wave = "A$adaptiveWave"; $scheduledBatch = @(Add-FrameSearchAdaptiveRows $State $adaptive.Rows $wave $StatePath $ResultsPath)
      if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) {
        $State.adaptive_reserve_reached = $true
        foreach ($scheduled in $scheduledBatch) { $run = Get-FrameSearchRunById $State $scheduled.Run; if ($run.Status -eq "pending") { $run.Status = "skipped"; $run.SkipReason = "wall-clock reserve reached before adaptive physical run"; $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $run.Profile; Reason = "$($run.Cell): wall-clock reserve reached before adaptive physical run" }) } }
        Save-FrameSearchState $State $StatePath $ResultsPath
        break
      }
      if ($scheduledBatch.Count -eq 0) { break }
      foreach ($scheduled in $scheduledBatch) {
        $run = Get-FrameSearchRunById $State $scheduled.Run
        if ($run.Status -ne "pending") { continue }
        if (Test-FrameSearchReserveWindow ([datetime]$State.deadline_utc)) {
          $run.Status = "skipped"
          $run.SkipReason = "wall-clock reserve reached before adaptive physical run"
          $State.adaptive_reserve_reached = $true
          $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $run.Profile; Reason = "$($run.Cell): wall-clock reserve reached before adaptive physical run" })
          continue
        }
        Invoke-FrameSearchRun $State $run $Artifacts $StudyDirectory $StatePath $ResultsPath
      }
      if ($State.adaptive_reserve_reached) { Save-FrameSearchState $State $StatePath $ResultsPath; break }
      $adaptiveWave++
    }
  }
  $pairs = Get-FrameSearchPairObservations @($State.runs)
  $blockedCells = @($State.runs | Where-Object { $_.Status -eq "skipped" } | ForEach-Object { $_.Cell }) + @($State.deferred_rows | ForEach-Object { $_.Cell })
  $retainedProfiles = @($State.analyses | ForEach-Object { $_.Profile })
  $finalAnalyses = @(Get-FrameSearchProfileAnalyses (Get-FrameSearchProfileDefinitions | Where-Object { $retainedProfiles -contains $_.Profile }) $pairs -BlockedCells $blockedCells)
  foreach ($analysis in @($State.analyses)) {
    $match = @($finalAnalyses | Where-Object { $_.Profile -ceq $analysis.Profile })[0]
    if ($null -ne $match -and $analysis.BudgetBlocked) { $match.BudgetBlocked = $true; $match.Resolution = "inconclusive"; $match.UnresolvedReasons += "required probes were suppressed by the adaptive budget" }
  }
  $State.analyses = @($finalAnalyses)
  if ($State.adaptive_reserve_reached) {
    foreach ($run in @($State.runs | Where-Object { $_.Wave -like "A*" -and $_.Status -eq "pending" })) { $run.Status = "skipped"; $run.SkipReason = "wall-clock reserve reached before adaptive physical run"; $State.decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $run.Profile; Reason = "$($run.Cell): wall-clock reserve reached before adaptive physical run" }) }
    $State.status = "inconclusive"; Clear-FrameSearchPublicationState $State | Out-Null
    $State.decisions += ,([pscustomobject][ordered]@{ Kind = "inconclusive"; Profile = ""; Reason = "wall-clock reserve reached before adaptive campaign completion" })
    Save-FrameSearchState $State $StatePath $ResultsPath
    return
  }
  $readiness = Get-FrameSearchCampaignReadiness $State.analyses $State.deferred_rows
  if (-not $readiness.Ready) {
    $State.status = if (@($State.runs | Where-Object { $_.Status -eq "recovery_required" }).Count -gt 0) { "recovery_required" } else { "inconclusive" }
    Clear-FrameSearchPublicationState $State | Out-Null
    $State.decisions += ,([pscustomobject][ordered]@{ Kind = "inconclusive"; Profile = ""; Reason = $readiness.Reason })
    Save-FrameSearchState $State $StatePath $ResultsPath
    return
  }
  $provisional = @(Get-FrameSearchWinnerSet (Get-FrameSearchProfileDefinitions) $pairs $State.analyses)
  $State.provisional_winners = @($provisional); $State.winners = @(); $State.gains = @()
  if ($provisional.Count -ne 4) {
    $State.status = if (@($State.runs | Where-Object { $_.Status -eq "recovery_required" }).Count -gt 0) { "recovery_required" } else { "inconclusive" }
    $State.decisions += ,([pscustomobject][ordered]@{ Kind = "inconclusive"; Profile = ""; Reason = "All four board/mode groups did not produce a validated preliminary winner." })
    Save-FrameSearchState $State $StatePath $ResultsPath
    return
  }
  $State.phase = "Soak"; Save-FrameSearchState $State $StatePath $ResultsPath
  $soakFailure = $false
  foreach ($winner in $provisional) {
    $soakId = "SOAK-" + $winner.Board.Substring(0, 1) + $winner.Mode.Substring(0, 1)
    $profile = Get-FrameSearchProfileDefinition $winner.Profile
    $schedule = [pscustomobject][ordered]@{ Run = $soakId; Cell = $soakId; Rep = 1; Epoch = 1; Wave = "SOAK"; Profile = $winner.Profile; Board = $winner.Board; Mode = $winner.Mode; Output = $profile.Output; Period = $profile.Period; Internal = $profile.Internal; Lookahead = $profile.Lookahead; Effective = $profile.Effective; U = $winner.U; ScheduleIndex = 0; OrderQuality = "single soak" }
    if (@($State.runs | Where-Object { $_.Run -ceq $soakId }).Count -eq 0) { $State.runs += ,(New-FrameSearchRunRecord $schedule "Soak" 600) }
    $run = Get-FrameSearchRunById $State $soakId
    if ($run.Status -eq "pending") { Invoke-FrameSearchRun $State $run $Artifacts $StudyDirectory $StatePath $ResultsPath }
    if ($run.Status -ne "completed" -or @("Stable", "Stretched") -notcontains $run.Grade) { $soakFailure = $true; $State.decisions += ,([pscustomobject][ordered]@{ Kind = "validation_failure"; Profile = $winner.Profile; Reason = "$soakId 600-second soak did not pass Stable/Stretched validation." }) }
    $State.soaks = @($State.runs | Where-Object { $_.Wave -ceq "SOAK" } | ForEach-Object { [pscustomobject][ordered]@{ Board = $_.Board; Mode = $_.Mode; Profile = $_.Profile; U = $_.U; Grade = $_.Grade; Worst = $_.Worst; Geometry = "output=$($_.Output),period=$($_.Period),internal=$($_.Internal),lookahead=$($_.Lookahead),effective=$($_.Effective)"; EvidenceDirectory = $_.EvidenceDirectory; HostEvidencePath = $_.HostEvidencePath; ResultPath = $_.ResultPath } }); Save-FrameSearchState $State $StatePath $ResultsPath
  }
  if ($soakFailure) { $State.status = "validation_failed"; Clear-FrameSearchPublicationState $State | Out-Null } else { $State.status = "completed"; $State.winners = @($provisional); $State.gains = @(Get-FrameSearchGainResults $provisional) }
  Save-FrameSearchState $State $StatePath $ResultsPath
}

function Write-FrameSearchPrintOnly {
  param([Parameter(Mandatory)][object]$Plan, [Parameter(Mandatory)][object[]]$Artifacts)
  Write-Output "Frame-search PrintOnly: no board transport is invoked."
  Write-Output "Plan: $($Plan.Path)"
  Write-Output "Plan SHA256: $($Plan.Sha256)"
  Write-Output "Candidate profiles: $($Plan.CandidateProfiles.Count); seed paired cells: $($Plan.SeedQueue.Count); physical seed runs: $($Plan.SeedQueue.Count * 2)"
  foreach ($wave in @("W1", "W2", "W3", "W4")) { Write-Output "Wave ${wave}: $(@($Plan.SeedQueue | Where-Object { $_.Wave -ceq $wave }).Count) paired cells" }
  Write-Output "Artifact expectations:"
  foreach ($artifact in $Artifacts) { $hash = if ([string]::IsNullOrWhiteSpace($artifact.Sha256)) { "unresolved" } else { $artifact.Sha256 }; $commit = if ([string]::IsNullOrWhiteSpace($artifact.MetadataSourceCommit)) { "unresolved" } else { $artifact.MetadataSourceCommit }; Write-Output "  $($artifact.Name): path=$($artifact.ArtifactPath) metadata=$($artifact.MetadataPath) board=$($artifact.BoardProfile) cargo_feature=$($artifact.CargoFeature) hash=$hash source_commit=$commit" }
  Write-Output "Normalized seed schedule:"
  foreach ($row in @($Plan.SeedQueue | Sort-Object Wave, Profile, U)) { Write-Output "$($row.Cell) wave=$($row.Wave) board=$($row.Board) mode=$($row.Mode) profile=$($row.Profile) U=$($row.U) sec=$($row.Sec) reps=$($row.Reps) state=$($row.State) condition=$($row.Condition)" }
}

$plan = Read-FrameSearchPlan $PlanPath
$sourceCommit = Get-FrameSearchRepoCommit $repoRoot
$definitions = Get-FrameSearchArtifactDefinitions $RaspberryArtifact $RaspberryMetadata $OrangeInlineArtifact $OrangeInlineMetadata $OrangeMulticoreArtifact $OrangeMulticoreMetadata
if ($PrintOnly) { Write-FrameSearchPrintOnly $plan (Get-FrameSearchArtifactSet $definitions $sourceCommit); exit 0 }
Assert-FrameSearchRepositoryClean $repoRoot
if (-not $AllowServiceInterruption) { throw "Active frame-search execution requires explicit -AllowServiceInterruption." }
if ($Detach -and -not $Worker) {
  if ([string]::IsNullOrWhiteSpace($StudyId)) { $StudyId = "frame-search-$([datetime]::UtcNow.ToString('yyyyMMddTHHmmssZ'))" }
  $studyDirectory = Get-FrameSearchStudyDirectory $StudyId
  if ((Test-Path -LiteralPath (Join-Path $studyDirectory "study-state.json") -PathType Leaf) -and -not $Resume) { throw "Existing frame-search state requires explicit -Resume: $studyDirectory" }
  $command = New-FrameSearchDetachedCommand $MyInvocation.MyCommand.Path $StudyId $plan.Path $definitions $WallClockHours -Resume:$Resume -AllowServiceInterruption:$AllowServiceInterruption
  New-Item -ItemType Directory -Force -Path $studyDirectory | Out-Null
  try { $process = Start-Process -FilePath $command.FilePath -ArgumentList $command.ArgumentString -RedirectStandardOutput (Join-Path $studyDirectory "launcher.stdout.txt") -RedirectStandardError (Join-Path $studyDirectory "launcher.stderr.txt") -PassThru -WindowStyle Hidden } catch { Write-FrameSearchSentinel $studyDirectory "failed" $_.Exception.Message; throw }
  Write-Output "Detached frame-search study started: $StudyId"
  Write-Output "Study directory: $studyDirectory"
  Write-Output "PID: $($process.Id)"
  exit 0
}
if ([string]::IsNullOrWhiteSpace($StudyId)) { $StudyId = "frame-search-$([datetime]::UtcNow.ToString('yyyyMMddTHHmmssZ'))" }
$studyDirectory = Get-FrameSearchStudyDirectory $StudyId
$statePath = Join-Path $studyDirectory "study-state.json"; $resultsPath = Join-Path $studyDirectory "study-results.md"
$state = $null
New-Item -ItemType Directory -Force -Path $studyDirectory | Out-Null
$studyLock = Enter-FrameSearchStudyLock $studyDirectory
try {
  if ($Worker) { Write-FrameSearchSentinel $studyDirectory "running" "worker_pid=$PID"; [IO.File]::WriteAllText((Join-Path $studyDirectory "pid"), "$PID`n", (New-Object Text.UTF8Encoding($false))) }
  $artifacts = Get-FrameSearchArtifactSet $definitions $sourceCommit -RequireFiles
  if (Test-Path -LiteralPath $statePath -PathType Leaf) {
    if (-not $Resume) { throw "Existing frame-search state requires explicit -Resume: $statePath" }
    $state = Read-FrameSearchState $statePath; Assert-FrameSearchResumeIdentity $state $StudyId $plan $sourceCommit $artifacts
    if ([string]$state.status -ne "running") { throw "Frame-search state is not resumable after status '$($state.status)'." }
    $state = Reconcile-FrameSearchState $state $artifacts
    Save-FrameSearchState $state $statePath $resultsPath
  } else {
    if ($Resume) { throw "-Resume requires existing frame-search state: $statePath" }
    $state = New-FrameSearchState $StudyId $plan $sourceCommit $artifacts ([datetime]::UtcNow) $WallClockHours
    Save-FrameSearchState $state $statePath $resultsPath
  }

  Invoke-FrameSearchCampaign $state $plan $artifacts $studyDirectory $statePath $resultsPath
  Remove-Item -LiteralPath (Join-Path $studyDirectory "running") -Force -ErrorAction SilentlyContinue
  $finalSentinel = switch ($state.status) { "completed" { "completed"; break }; "inconclusive" { "inconclusive"; break }; "recovery_required" { "recovery_required"; break }; "validation_failed" { "failed"; break }; default { "failed" } }
  Write-FrameSearchSentinel $studyDirectory $finalSentinel "status=$($state.status)"
  Write-Output "Frame-search study $StudyId finished with status $($state.status)."
} catch {
  $failure = $_
  if ($null -ne $state) {
    Clear-FrameSearchPublicationState $state | Out-Null
    $state.status = "failed"
    $state.decisions += ,([pscustomobject][ordered]@{ Kind = "fatal"; Profile = ""; Reason = $failure.Exception.Message })
    try { Save-FrameSearchState $state $statePath $resultsPath } catch { }
  }
  Remove-Item -LiteralPath (Join-Path $studyDirectory "running") -Force -ErrorAction SilentlyContinue
  Write-FrameSearchSentinel $studyDirectory "failed" $failure.Exception.Message
  throw $failure
} finally {
  Exit-FrameSearchStudyLock $studyLock
}
