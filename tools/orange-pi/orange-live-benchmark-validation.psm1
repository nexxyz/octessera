Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "orange-profile-baseline-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-worker-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-worker-timing-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-result-evidence-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-capacity-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-sensor-validation.psm1") -Force
$script:OrangeLiveScenarioIds = @("synth_ramp_16", "synth_ramp_32", "synth_ramp_64", "sample_ramp_64", "mixed_ramp_16_16", "mixed_ramp_32_32", "bus_heavy_6_bus_fx_2_global", "momentary_combined", "synth_cross_slot_96_steal", "sample_cross_slot_96_steal", "mixed_cross_slot_48_48_steal")
function Get-OrangeLiveScenarioIds {
  return @($script:OrangeLiveScenarioIds)
}
function Assert-OrangeLiveBenchmarkSelection {
  param(
    [Parameter(Mandatory)][string]$Scenario,
    [Parameter(Mandatory)][int]$OutputFrames,
    [Parameter(Mandatory)][ValidateSet(32, 64, 128, 256)][int]$EngineBlockFrames,
    [Parameter(Mandatory)][int]$MeasureSeconds,
    [bool]$AllowLongRepeat = $false
  )
  $capacityScenario = ConvertFrom-OrangeCapacityScenario $Scenario
  if ($null -ne $capacityScenario) {
    return Assert-OrangeCapacityBenchmarkSelection -Scenario $Scenario -CapacityScenario $capacityScenario -OutputFrames $OutputFrames -EngineBlockFrames $EngineBlockFrames -MeasureSeconds $MeasureSeconds -AllowLongRepeat:$AllowLongRepeat
  }
  if ($script:OrangeLiveScenarioIds -notcontains $Scenario -and (Get-OrangeBaselineLiveScenarioIds) -notcontains $Scenario) {
    throw "LiveAudioBenchmark scenario is not an approved live baseline ID: $Scenario"
  }
  $alsaPeriodFrames = @{ 128 = 32; 256 = 64; 512 = 128; 1024 = 256 }[$OutputFrames]
  if ($null -eq $alsaPeriodFrames) { throw "LiveAudioBenchmark output frames must be 128, 256, 512, or 1024." }
  if (@(30, 120, 180, 300) -notcontains $MeasureSeconds) {
    throw "LiveAudioBenchmark measure seconds must be 30, 120, 180, or 300."
  }
  $approvedTuples = @("128/32", "256/64", "256/128", "256/256", "512/128", "1024/256")
  $approvedTuple = $approvedTuples -contains "$OutputFrames/$EngineBlockFrames"
  if (-not $approvedTuple) {
    throw "LiveAudioBenchmark geometry tuple is not approved: output=$OutputFrames engine=$EngineBlockFrames."
  }
  if ($OutputFrames -eq 1024 -and @("synth_cross_slot_96_steal", "mixed_cross_slot_48_48_steal", "synth_cross_slot_32_no_steal", "mixed_16_synth_32_sample") -notcontains $Scenario) {
    throw "LiveAudioBenchmark output 1024 is limited to the synth and mixed steal scenarios."
  }
  if ($MeasureSeconds -eq 120) {
    if (-not $AllowLongRepeat) {
      throw "A 120-second run requires the explicit -AllowLongRepeat consent."
    }
    if ($OutputFrames -ne 256 -or $EngineBlockFrames -ne 128) {
      throw "A 120-second repeat must use the selected A scenario at output=256 and engine=128."
    }
  }
  if ($AllowLongRepeat -and $MeasureSeconds -ne 120) {
    throw "-AllowLongRepeat is only valid for a 120-second A repeat."
  }
  $matrixClass = if ($OutputFrames -eq 256 -and $EngineBlockFrames -eq 128) { "A" } elseif ($OutputFrames -eq 512) { "B" } else { "individual" }
  return [pscustomobject]@{
    Scenario = $Scenario
    OutputFrames = $OutputFrames
    AlsaPeriodFrames = $alsaPeriodFrames
    EngineBlockFrames = $EngineBlockFrames
    InternalFrames = $EngineBlockFrames
    MeasureSeconds = $MeasureSeconds
    WarmupSeconds = 5
    MatrixClass = $matrixClass
    LongRepeat = $MeasureSeconds -eq 120
  }
}
function Get-OrangeLiveMatrixPlan {
  $plan = @()
  foreach ($scenario in $script:OrangeLiveScenarioIds) {
    $plan += Assert-OrangeLiveBenchmarkSelection -Scenario $scenario -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 30
  }
  foreach ($scenario in $script:OrangeLiveScenarioIds) {
    $plan += Assert-OrangeLiveBenchmarkSelection -Scenario $scenario -OutputFrames 512 -EngineBlockFrames 128 -MeasureSeconds 30
  }
  return $plan
}
function Resolve-OrangeLiveEvidenceDirectory {
  param(
    [Parameter(Mandatory)][string]$LocalRunDirectory,
    [Parameter(Mandatory)][string]$RemoteRoot
  )
  $direct = Test-Path -LiteralPath (Join-Path $LocalRunDirectory "study-result.txt") -PathType Leaf
  $remoteLeaf = Split-Path -Leaf $RemoteRoot
  $nested = @(Get-ChildItem -LiteralPath $LocalRunDirectory -Directory -ErrorAction SilentlyContinue | Where-Object {
      $_.Name -ceq $remoteLeaf -and (Test-Path -LiteralPath (Join-Path $_.FullName "study-result.txt") -PathType Leaf)
    })
  if ($direct -and $nested.Count -eq 0) { return (Get-Item -LiteralPath $LocalRunDirectory).FullName }
  if (-not $direct -and $nested.Count -eq 1) { return $nested[0].FullName }
  throw "Live benchmark evidence retrieval was missing or ambiguous: $LocalRunDirectory"
}
function Resolve-OrangeLiveRunnerOutcome {
  param(
    [Parameter(Mandatory)][string]$EvidenceStatusClass,
    [Parameter(Mandatory)][bool]$RunnerThrew
  )
  if ($RunnerThrew -and $EvidenceStatusClass -eq "pass") { return "infrastructure_failure" }
  return $EvidenceStatusClass
}
function Get-OrangeLiveRunId {
  param([Parameter(Mandatory)][string]$EvidenceDirectory)
  $evidence = Get-Item -LiteralPath $EvidenceDirectory -ErrorAction Stop
  $runDirectory = if ($evidence.Name -like "orange-study-*") { $evidence } else { Get-Item -LiteralPath $evidence.Parent.FullName -ErrorAction Stop }
  if ($runDirectory.Name -notmatch '^orange-study-(?<runid>[0-9a-f]+)$') {
    throw "Live benchmark evidence directory does not carry an exact local run ID."
  }
  return $Matches.runid
}
function ConvertTo-OrangeLiveManifestJson {
  param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Results)
  return (ConvertTo-Json -InputObject @($Results) -Depth 8)
}
function Get-OrangeLiveWorstPassingScenario {
  param([Parameter(Mandatory)][object[]]$Results)
  $passing = @($Results | Where-Object {
      $_.StatusClass -eq "pass" -and
      $_.OutputFrames -eq 256 -and
      $_.EngineBlockFrames -eq 128 -and
      $_.MeasureSeconds -eq 30
    })
  if ($passing.Count -eq 0) {
    throw "No passing A scenario is available for the required 120-second repeat."
  }
  return $passing |
    Sort-Object -Property @(
      @{ Expression = { [double]$_.RatioP999 }; Descending = $true },
      @{ Expression = { [double]$_.RatioMax }; Descending = $true },
      @{ Expression = { [array]::IndexOf($script:OrangeLiveScenarioIds, $_.Scenario) }; Descending = $false }
    ) |
    Select-Object -First 1
}
function Get-OrangeLiveResultSummary {
  param(
    [Parameter(Mandatory)][pscustomobject]$Result,
    [Parameter(Mandatory)][pscustomobject]$Selection
  )
  Assert-OrangeLiveResultFieldNames -Result $Result
  $callback = $Result.callback
  $terminal = $null -ne $Result.terminal_error -and -not [string]::IsNullOrWhiteSpace([string]$Result.terminal_error)
  $callbackErrors = [uint64]$callback.cpal_device_error_count + [uint64]$callback.cpal_stream_error_count
  $detectedContinuityEvents = Get-OrangeLiveStrictInteger -Value $Result.detected_continuity_events -Path "detected_continuity_events"
  $measured = [uint64]$callback.callback_count -gt 0 -and -not $terminal -and -not [bool]$callback.terminal_error
  $muteProof = [uint64]$callback.pre_mute_nonzero_samples -gt 0 -and [uint64]$callback.post_mute_nonzero_samples -eq 0
  $complete = $measured -and $callbackErrors -eq 0
  $maxCallbackBudgetOverruns = switch ($Selection.MeasureSeconds) { 30 { 0 }; 120 { 0 }; 180 { 0 }; 300 { 5 }; default { throw "Unsupported live benchmark duration: $($Selection.MeasureSeconds) seconds." } }
  $statusClass = if (-not $measured) {
    "infrastructure_failure"
  } elseif ($callbackErrors -gt 0) {
    "infrastructure_failure"
  } elseif ([string]$Result.status -eq "pass") {
    if ([uint64]$callback.over_audio_duration_budget_count -gt $maxCallbackBudgetOverruns -or -not $muteProof -or ($Selection.MeasureSeconds -eq 180 -and ($detectedContinuityEvents -ne 0 -or [uint64]$callback.over_audio_duration_budget_count -ne 0 -or [uint64]$callback.cpal_device_error_count -ne 0 -or [uint64]$callback.cpal_stream_error_count -ne 0))) { "infrastructure_failure" } else { "pass" }
  } elseif ([uint64]$callback.over_audio_duration_budget_count -gt $maxCallbackBudgetOverruns) {
    if ($complete -and $muteProof) { "over_budget" } else { "infrastructure_failure" }
  } elseif ($complete -and $muteProof) {
    "measured_failure"
  } else {
    "infrastructure_failure"
  }
  return [pscustomobject]@{
    StatusClass = $statusClass
    Scenario = $Selection.Scenario
    OutputFrames = $Selection.OutputFrames
    AlsaPeriodFrames = $Selection.AlsaPeriodFrames
    EngineBlockFrames = $Selection.EngineBlockFrames
    InternalFrames = $Selection.InternalFrames
    MeasureSeconds = $Selection.MeasureSeconds
    RatioP50 = [double]$callback.render_audio_duration_ratio_p50
    RatioP95 = [double]$callback.render_audio_duration_ratio_p95
    RatioP99 = [double]$callback.render_audio_duration_ratio_p99
    RatioP999 = [double]$callback.render_audio_duration_ratio_p99_9
    RatioMax = [double]$callback.render_audio_duration_ratio_max
    AggregateRenderAudioDurationRatio = Get-OrangeLiveAggregateRenderAudioDurationRatio $Result
    OverBudget = [uint64]$callback.over_audio_duration_budget_count
    CallbackErrors = $callbackErrors
  }
}
function Assert-OrangeLiveReadiness {
  param(
    [Parameter(Mandatory)][pscustomobject]$Readiness,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][int]$ExpectedPid,
    [Parameter(Mandatory)][string]$ExpectedInvocation,
    [Parameter(Mandatory)][string]$ArtifactHash
  )
  if ([int]$Readiness.schema_version -ne 4 -or [string]$Readiness.kind -cne "orange_audio_benchmark_readiness" -or [string]$Readiness.status -cne "ready") {
    throw "Live benchmark readiness schema or status is invalid."
  }
  $checks = @(
    @([string]$Readiness.board_profile, "orange-pi-zero-2w"),
    @([int]$Readiness.pid, $ExpectedPid),
    @([string]$Readiness.systemd_invocation_id, $ExpectedInvocation),
    @([string]$Readiness.artifact_sha256, $ArtifactHash),
    @([string]$Readiness.scenario, $Selection.Scenario),
    @([int]$Readiness.requested_output_buffer_frames, $Selection.OutputFrames),
    @([int]$Readiness.expected_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Readiness.expected_alsa_period_frames, $Selection.AlsaPeriodFrames),
    @([int]$Readiness.sample_rate, 44100),
    @([int]$Readiness.channels, 2),
    @([int]$Readiness.internal_block_frames, $Selection.InternalFrames)
  )
  foreach ($check in $checks) {
    if ($check[0] -cne $check[1]) { throw "Live benchmark readiness identity or geometry mismatch." }
  }
  if (@("F32", "I16", "U16") -notcontains [string]$Readiness.sample_format) {
    throw "Live benchmark readiness sample format is unsupported."
  }
  if (-not [bool]$Readiness.scheduler_qualified -or -not [bool]$Readiness.post_dsp_zero) {
    throw "Live benchmark readiness did not prove scheduler qualification and post-DSP mute."
  }
  Assert-OrangeWorkerEvidence -Evidence $Readiness
  if ([int]$Readiness.callback_frames_min -le 0 -or [int]$Readiness.callback_frames_max -lt [int]$Readiness.callback_frames_min -or [int]$Readiness.callback_frames_max -gt $Selection.OutputFrames -or [uint64]$Readiness.callback_frame_sample_count -lt 3 -or [uint64]$Readiness.invalid_callback_frame_count -ne 0) {
    throw "Live benchmark readiness callback batch evidence is invalid."
  }
}
function Assert-OrangeLiveRelease {
  param(
    [Parameter(Mandatory)][pscustomobject]$Release,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][int]$ExpectedPid,
    [Parameter(Mandatory)][string]$ExpectedInvocation,
    [Parameter(Mandatory)][string]$ArtifactHash
  )
  $checks = @(
    @([int]$Release.schema_version, 2),
    @([string]$Release.kind, "orange_audio_benchmark_release"),
    @([string]$Release.status, "released"),
    @([string]$Release.board_profile, "orange-pi-zero-2w"),
    @([int]$Release.pid, $ExpectedPid),
    @([string]$Release.systemd_invocation_id, $ExpectedInvocation),
    @([string]$Release.artifact_sha256, $ArtifactHash),
    @([string]$Release.scenario, $Selection.Scenario),
    @([int]$Release.expected_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Release.observed_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Release.expected_alsa_period_frames, $Selection.AlsaPeriodFrames),
    @([int]$Release.observed_alsa_period_frames, $Selection.AlsaPeriodFrames)
  )
  foreach ($check in $checks) {
    if ($check[0] -cne $check[1]) { throw "Live benchmark release identity or ALSA geometry mismatch." }
  }
}
function Read-OrangeLiveKeyValueFile {
  param([Parameter(Mandatory)][string]$Path)
  $values = @{}
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $values }
  foreach ($line in Get-Content -LiteralPath $Path) {
    $parts = $line -split "=", 2
    if ($parts.Count -eq 2) { $values[$parts[0]] = $parts[1] }
  }
  return $values
}
function Assert-OrangeAdmissionDropEvidence {
  param(
    [Parameter(Mandatory)][pscustomobject]$Result,
    [Parameter(Mandatory)][pscustomobject]$Selection
  )
  $expectedStart = Get-OrangeExpectedAdmissionDrops $Selection "expected_admission_drops_start"
  $expectedEnd = Get-OrangeExpectedAdmissionDrops $Selection "expected_admission_drops_end"
  if ($expectedEnd -lt $expectedStart) { throw "Live benchmark expected admission-drop end is below start." }
  $startProperty = $Result.PSObject.Properties["profile_start"]
  $endProperty = $Result.PSObject.Properties["profile_end"]
  if ($null -eq $startProperty -or $null -eq $endProperty -or $null -eq $startProperty.Value -or $null -eq $endProperty.Value) { throw "Live benchmark profile admission-drop evidence is required." }
  $start = Get-OrangeRequiredNonNegativeInteger $startProperty.Value "cumulative_voice_admission_drops" "profile_start"
  $end = Get-OrangeRequiredNonNegativeInteger $endProperty.Value "cumulative_voice_admission_drops" "profile_end"
  if ($start -ne $expectedStart -or $end -ne $expectedEnd -or $end - $start -ne $expectedEnd - $expectedStart) { throw "Live benchmark admission-drop evidence does not reconcile with expected start/end values." }
}

function Assert-OrangeLiveResult {
  param(
    [Parameter(Mandatory)][pscustomobject]$Result,
    [Parameter(Mandatory)][pscustomobject]$Selection
  )
  Assert-OrangeLiveResultFieldNames -Result $Result
  $callback = $Result.callback
  $executorProperty = $Result.PSObject.Properties["executor_mode"]
  if ($null -eq $executorProperty -or $executorProperty.Value -isnot [string] -or @("inline", "persistent_two_workers") -cnotcontains $executorProperty.Value) { throw "Live benchmark executor mode is missing or invalid." }
  $schedulingPolicy = $Result.PSObject.Properties["callback_scheduling_policy"]
  $schedulingPriority = $Result.PSObject.Properties["callback_scheduling_priority"]
  $schedulingCpu = $Result.PSObject.Properties["callback_scheduling_cpu"]
  $expectedPriority = 70
  if ($null -eq $schedulingPolicy -or $schedulingPolicy.Value -isnot [string] -or $schedulingPolicy.Value -cne "SCHED_FIFO" -or $null -eq $schedulingPriority -or $schedulingPriority.Value -isnot [byte] -and $schedulingPriority.Value -isnot [int16] -and $schedulingPriority.Value -isnot [uint16] -and $schedulingPriority.Value -isnot [int32] -and $schedulingPriority.Value -isnot [uint32] -and $schedulingPriority.Value -isnot [int64] -and $schedulingPriority.Value -isnot [uint64] -or [int]$schedulingPriority.Value -ne $expectedPriority) { throw "Live benchmark effective scheduling evidence is invalid." }
  if ($null -eq $schedulingCpu) { throw "Live benchmark callback CPU evidence is missing." }
  if ((Get-OrangeLiveStrictInteger -Value $schedulingCpu.Value -Path "callback_scheduling_cpu") -ne 1) { throw "Live benchmark callback CPU evidence is invalid." }
  $checks = @(
    @((Get-OrangeLiveStrictInteger -Value $Result.schema_version -Path "schema_version"), 11),
    @([string]$Result.kind, "orange_audio_benchmark_result"),
    @([string]$Result.board_profile, "orange-pi-zero-2w"),
    @([string]$Result.scenario, $Selection.Scenario),
    @([int]$Result.requested_output_buffer_frames, $Selection.OutputFrames),
    @([int]$Result.expected_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Result.expected_alsa_period_frames, $Selection.AlsaPeriodFrames),
    @([int]$Result.sample_rate, 44100),
    @([int]$Result.channels, 2),
    @([int]$Result.internal_block_frames, $Selection.InternalFrames),
    @([int]$Result.warmup_seconds, 5),
    @([int]$Result.measure_seconds, $Selection.MeasureSeconds)
  )
  if (@("pass", "fail") -notcontains [string]$Result.status) { throw "Live benchmark result status is invalid." }
  foreach ($check in $checks) {
    if ($check[0] -cne $check[1]) { throw "Live benchmark result contract mismatch." }
  }
  $profileStart = Assert-OrangeLiveProfileSnapshot -Snapshot $Result.profile_start -Path "profile_start"
  $profileEnd = Assert-OrangeLiveProfileSnapshot -Snapshot $Result.profile_end -Path "profile_end"
  if ($Selection.PSObject.Properties["CapacityKind"] -and [string]$Selection.CapacityKind -ceq "analogue") {
    Assert-OrangeAnalogueProfileEvidence -Selection $Selection -ProfileStart $profileStart -ProfileEnd $profileEnd
  }
  if (@("F32", "I16", "U16") -notcontains [string]$Result.sample_format) { throw "Live benchmark result sample format is unsupported." }
  if (-not [bool]$Result.scheduler_qualified -or -not [bool]$Result.measurement_stop_acknowledged -or -not [bool]$Result.stream_stopped -or -not [bool]$Result.final_progress_write_succeeded) {
    throw "Live benchmark result did not complete the required finalization contract."
  }
  if ([string]$Result.status -ceq "pass" -and $executorProperty.Value -ceq "persistent_two_workers" -and [string]$Result.worker_health -cne "healthy") { throw "A passing live benchmark must report healthy persistent workers." }
  Assert-OrangeWorkerEvidence -Evidence $Result -RequireShutdown:$true -AllowTerminalHealth
  Assert-OrangeWorkerTimingEvidence -Result $Result
  $persistentOutputProperty = $Result.PSObject.Properties["persistent_output_counters"]
  $persistentOutput = if ($null -ne $persistentOutputProperty) { Assert-OrangeLivePersistentOutputEvidence -Evidence $persistentOutputProperty.Value -ExecutorMode $executorProperty.Value } else { throw "Live benchmark persistent output counter evidence is missing." }
  $detectedContinuityEvents = Get-OrangeLiveStrictInteger -Value $Result.detected_continuity_events -Path "detected_continuity_events"
  $callbackOverruns = Get-OrangeLiveStrictInteger -Value $callback.over_audio_duration_budget_count -Path "callback.over_audio_duration_budget_count"
  $callbackDeviceErrors = Get-OrangeLiveStrictInteger -Value $callback.cpal_device_error_count -Path "callback.cpal_device_error_count"
  $callbackStreamErrors = Get-OrangeLiveStrictInteger -Value $callback.cpal_stream_error_count -Path "callback.cpal_stream_error_count"
  $carryIn = $persistentOutput.start.deadline_misses -gt $persistentOutput.start.deadline_recoveries -and ($persistentOutput.delta.repeated_quantums -gt 0 -or $persistentOutput.delta.dropped_quantums -gt 0)
  $counterContinuityEvents = $persistentOutput.delta.deadline_misses
  if ($carryIn -and $counterContinuityEvents -lt [uint64]::MaxValue) { $counterContinuityEvents++ }
  $expectedContinuityEvents = if ($callbackOverruns -gt $counterContinuityEvents) { $callbackOverruns } else { $counterContinuityEvents }
  if ($detectedContinuityEvents -ne $expectedContinuityEvents) { throw "Live benchmark detected continuity event evidence is inconsistent." }
  if ([string]$Result.status -ceq "pass" -and $Selection.MeasureSeconds -eq 180 -and ($detectedContinuityEvents -ne 0 -or $callbackOverruns -ne 0 -or $callbackDeviceErrors -ne 0 -or $callbackStreamErrors -ne 0)) { throw "A passing 180-second benchmark must have zero callback continuity events." }
  $recoveredCount = $Result.PSObject.Properties["recovered_alsa_epipe_count"]
  $recoveredObservable = $Result.PSObject.Properties["recovered_alsa_epipe_observable"]
  if ($null -eq $recoveredCount -or $null -eq $recoveredObservable -or $null -ne $recoveredCount.Value -or $recoveredObservable.Value -isnot [bool] -or $recoveredObservable.Value) {
    throw "Live benchmark result made an invalid recovered ALSA EPIPE claim."
  }
  Assert-OrangeAdmissionDropEvidence $Result $Selection
  Get-OrangeLiveAggregateRenderAudioDurationRatio $Result | Out-Null
  $workerTerminal = $callback.PSObject.Properties["worker_terminal"]
  if ($null -eq $workerTerminal -or [bool]$workerTerminal.Value) {
    throw "Live benchmark callback worker terminal evidence is invalid."
  }
  if ([uint64]$callback.callback_count -eq 0 -or [uint64]$callback.first_measured_callback_ns -eq 0 -or [uint64]$callback.last_measured_callback_ns -lt [uint64]$callback.first_measured_callback_ns -or [uint64]$callback.measured_elapsed_ns -ne ([uint64]$callback.last_measured_callback_ns - [uint64]$callback.first_measured_callback_ns) -or -not [bool]$callback.callback_timestamp_observed -or [uint32]$callback.callback_frames_min -le 0 -or [uint32]$callback.callback_frames_max -lt [uint32]$callback.callback_frames_min -or [uint32]$callback.callback_frames_max -gt $Selection.OutputFrames -or [uint64]$callback.callback_frame_sample_count -ne [uint64]$callback.callback_count -or [uint64]$callback.invalid_callback_frame_count -ne 0) {
    throw "Live benchmark callback timing or geometry evidence is incomplete."
  }
}
function Get-OrangeLiveHostEvidence {
  param(
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$ArtifactHash
  )
  $identity = Read-OrangeLiveKeyValueFile (Join-Path $EvidenceDirectory "benchmark-identity.txt")
  $restored = Read-OrangeLiveKeyValueFile (Join-Path $EvidenceDirectory "service-restored-state.txt")
  $study = Read-OrangeLiveKeyValueFile (Join-Path $EvidenceDirectory "study-result.txt")
  $remoteStatusClass = if ($study.ContainsKey("status_class")) { [string]$study.status_class } else { "" }
  $unit = Read-OrangeLiveKeyValueFile (Join-Path $EvidenceDirectory "unit-final.txt")
  $unitStop = Read-OrangeLiveKeyValueFile (Join-Path $EvidenceDirectory "unit-stop-evidence.txt")
  $unitStopBefore = Read-OrangeLiveKeyValueFile (Join-Path $EvidenceDirectory "unit-stop-before.txt")
  $sensorAbortPath = Join-Path $EvidenceDirectory "sensor-abort.txt"
  $resultPath = Join-Path $EvidenceDirectory "benchmark-result.json"
  $readinessPath = Join-Path $EvidenceDirectory "benchmark-readiness.json"
  $releasePath = Join-Path $EvidenceDirectory "benchmark-release.json"
  $result = $null
  $readiness = $null
  $aggregateRatio = $null
  $sensor = Get-OrangeLiveSensorEvidence (Join-Path $EvidenceDirectory "sensor-series.txt")
  $statusClass = "infrastructure_failure"
  $reason = "missing benchmark result or readiness evidence"
  if ((Test-Path -LiteralPath $resultPath -PathType Leaf) -and (Test-Path -LiteralPath $readinessPath -PathType Leaf)) {
    try {
      $result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
      $readiness = Get-Content -LiteralPath $readinessPath -Raw | ConvertFrom-Json
      $aggregateRatio = Get-OrangeLiveAggregateRenderAudioDurationRatio $result
      Assert-OrangeLiveReadiness `
        -Readiness $readiness `
        -Selection $Selection `
        -ExpectedPid ([int]$identity.main_pid) `
        -ExpectedInvocation ([string]$identity.invocation_id) `
        -ArtifactHash $ArtifactHash
      if (-not (Test-Path -LiteralPath $releasePath -PathType Leaf)) { throw "benchmark release evidence is missing" }
      $release = Get-Content -LiteralPath $releasePath -Raw | ConvertFrom-Json
      Assert-OrangeLiveRelease `
        -Release $release `
        -Selection $Selection `
        -ExpectedPid ([int]$identity.main_pid) `
        -ExpectedInvocation ([string]$identity.invocation_id) `
        -ArtifactHash $ArtifactHash
      Assert-OrangeLiveResult -Result $result -Selection $Selection
      if ([int]$result.pid -ne [int]$identity.main_pid -or [string]$result.systemd_invocation_id -cne [string]$identity.invocation_id -or [string]$result.artifact_sha256 -cne $ArtifactHash) {
        throw "benchmark result identity mismatch"
      }
      if (-not [bool]$result.post_dsp_zero -or [uint64]$result.callback.post_mute_nonzero_samples -ne 0) {
        throw "benchmark result did not prove post-DSP mute"
      }
      $validSuccessExecMainCode = @("0", "1", "exited") -contains [string]$unit.ExecMainCode
      $validFailureExecMainCode = @("1", "exited") -contains [string]$unit.ExecMainCode
      if ([string]$result.status -eq "pass") {
        $cleanSuccess = [string]$unit.ActiveState -eq "inactive" -and [string]$unit.SubState -eq "dead" -and [string]$unit.Result -eq "success" -and [int]$unit.MainPID -eq 0 -and $validSuccessExecMainCode -and [int]$unit.ExecMainStatus -eq 0
        if (-not $cleanSuccess) {
          throw "successful benchmark result did not match transient process status"
        }
      } else {
        $cleanFailure = [string]$unit.ActiveState -eq "failed" -and [string]$unit.Result -eq "exit-code" -and $validFailureExecMainCode -and [int]$unit.ExecMainStatus -eq 1
        $explicitFailureStop = [string]$unit.ActiveState -eq "inactive" -and [string]$unit.Result -eq "exit-code" -and $validFailureExecMainCode -and [int]$unit.ExecMainStatus -eq 1 -and $unitStop.explicit_stop_after_result -eq "true" -and $unitStop.result_present -eq "true" -and $unitStop.unit -eq $identity.unit -and $unitStopBefore.Result -eq "exit-code" -and $unitStopBefore.ExecMainStatus -eq "1" -and @("1", "exited") -contains $unitStopBefore.ExecMainCode
        if (-not $cleanFailure -and -not $explicitFailureStop) {
          throw "clean measured failure did not match transient process status"
        }
      }
      $summary = Get-OrangeLiveResultSummary -Result $result -Selection $Selection
      $statusClass = $summary.StatusClass
      $reason = "result identity and process status validated"
    } catch {
      $reason = $_.Exception.Message
    }
  }
  if ($study.interruption_started -eq "true" -and ($restored.restore_status -ne "0" -or $restored.final_active -ne "active" -or $restored.final_enabled -ne "enabled")) {
    $statusClass = "restoration_failure"
    $reason = "production service restoration did not pass the stable active/enabled gate"
  } elseif ($remoteStatusClass -eq "restoration_failure") {
    $statusClass = "restoration_failure"
    $reason = "remote study reported restoration failure"
  } elseif ((Test-Path -LiteralPath $sensorAbortPath -PathType Leaf) -or $remoteStatusClass -eq "safety_failure") {
    $statusClass = "safety_failure"
    $reason = "remote safety evidence contains a sensor abort"
  } elseif ($remoteStatusClass -eq "infrastructure_failure") {
    $statusClass = "infrastructure_failure"
    $reason = "remote study reported infrastructure failure"
  } elseif (@("pass", "measured_failure") -notcontains $remoteStatusClass -and $null -ne $result) {
    $statusClass = "infrastructure_failure"
    $reason = "remote study status class was missing or contradictory"
  }
  if ($null -ne $result -and (($remoteStatusClass -eq "pass" -and [string]$result.status -ne "pass") -or ($remoteStatusClass -eq "measured_failure" -and [string]$result.status -eq "pass"))) {
    $statusClass = "infrastructure_failure"
    $reason = "remote study status and retained benchmark result disagree"
  }
  if (@("pass", "measured_failure", "over_budget") -contains $statusClass) {
    if ((Test-Path -LiteralPath $sensorAbortPath -PathType Leaf) -or -not $sensor.CoolingEvidenceValid -or -not $sensor.FrequencyEvidenceValid -or $sensor.StartupSampleCount -lt 1 -or $sensor.RuntimeSampleCount -lt 1 -or $null -eq $sensor.StartupMaxThermalMillicelsius -or $null -eq $sensor.StartupMinMemAvailableKb -or $null -eq $sensor.RuntimeMaxThermalMillicelsius -or $null -eq $sensor.RuntimeMinMemAvailableKb) {
      $statusClass = "infrastructure_failure"
      $reason = if (-not $sensor.CoolingEvidenceValid) { "passing evidence contained incomplete or malformed cooling-device state" } elseif (-not $sensor.FrequencyEvidenceValid) { "passing evidence contained malformed frequency data" } else { "passing evidence did not retain startup/runtime sensor samples and extrema" }
    }
  }
  return [pscustomobject]@{
    StatusClass = $statusClass
    Reason = $reason
    Scenario = $Selection.Scenario
    OutputFrames = $Selection.OutputFrames
    AlsaPeriodFrames = $Selection.AlsaPeriodFrames
    EngineBlockFrames = $Selection.EngineBlockFrames
    InternalFrames = $Selection.InternalFrames
    MeasureSeconds = $Selection.MeasureSeconds
    AggregateRenderAudioDurationRatio = $aggregateRatio
    RatioP50 = if ($null -ne $result) { [double]$result.callback.render_audio_duration_ratio_p50 } else { 0.0 }
    RatioP95 = if ($null -ne $result) { [double]$result.callback.render_audio_duration_ratio_p95 } else { 0.0 }
    RatioP99 = if ($null -ne $result) { [double]$result.callback.render_audio_duration_ratio_p99 } else { 0.0 }
    RatioP999 = if ($null -ne $result) { [double]$result.callback.render_audio_duration_ratio_p99_9 } else { 0.0 }
    RatioMax = if ($null -ne $result) { [double]$result.callback.render_audio_duration_ratio_max } else { 0.0 }
    OverBudget = if ($null -ne $result) { [uint64]$result.callback.over_audio_duration_budget_count } else { 0 }
    CallbackErrors = if ($null -ne $result) { [uint64]$result.callback.cpal_device_error_count + [uint64]$result.callback.cpal_stream_error_count } else { 0 }
    ArtifactSha256 = $ArtifactHash
    BenchmarkOutcome = if ($null -ne $result) { [string]$result.status } else { "" }
    StudyStatusClass = if ($study.ContainsKey("status_class")) { [string]$study.status_class } else { "" }
    SensorMaxThermalMillicelsius = $sensor.MaxThermalMillicelsius
    SensorMinMemAvailableKb = $sensor.MinMemAvailableKb
    SensorStartupSampleCount = $sensor.StartupSampleCount
    SensorRuntimeSampleCount = $sensor.RuntimeSampleCount
    SensorStartupMaxThermalMillicelsius = $sensor.StartupMaxThermalMillicelsius
    SensorStartupMinMemAvailableKb = $sensor.StartupMinMemAvailableKb
    SensorRuntimeMaxThermalMillicelsius = $sensor.RuntimeMaxThermalMillicelsius
    SensorRuntimeMinMemAvailableKb = $sensor.RuntimeMinMemAvailableKb
    SensorCoolingObserved = $sensor.CoolingObserved
    SensorMaxCoolingState = $sensor.MaxCoolingState
    SensorMinFrequencyKhz = $sensor.MinFrequencyKhz
    SensorCoolingEvidenceValid = $sensor.CoolingEvidenceValid
    SensorFrequencyEvidenceValid = $sensor.FrequencyEvidenceValid
    SensorAbortPath = $sensorAbortPath
    ResultPath = if ($null -ne $result) { $resultPath } else { "" }
    ReadinessPath = if ($null -ne $readiness) { $readinessPath } else { "" }
    ReleasePath = if (Test-Path -LiteralPath $releasePath -PathType Leaf) { $releasePath } else { "" }
    InitialReadinessPath = Join-Path $EvidenceDirectory "service-initial-candidate-ready.json"
    RestoredReadinessPath = Join-Path $EvidenceDirectory "service-restored-ready.json"
    StudyStatusPath = Join-Path $EvidenceDirectory "study-result.txt"
    SensorSeriesPath = Join-Path $EvidenceDirectory "sensor-series.txt"
    UnitStatusPath = Join-Path $EvidenceDirectory "unit-final.txt"
  }
}
Export-ModuleMember -Function @("Assert-OrangeLiveBenchmarkSelection", "Assert-OrangeLiveRelease", "Assert-OrangeLiveReadiness", "Assert-OrangeLiveResult", "ConvertFrom-OrangeCapacityScenario", "ConvertTo-OrangeLiveManifestJson", "Get-OrangeLiveAggregateRenderAudioDurationRatio", "Get-OrangeLiveMatrixPlan", "Get-OrangeLiveHostEvidence", "Get-OrangeLiveResultSummary", "Get-OrangeLiveScenarioIds", "Get-OrangeLiveSensorEvidence", "Get-OrangeLiveWorstPassingScenario", "Get-OrangeLiveRunId", "Resolve-OrangeLiveEvidenceDirectory", "Resolve-OrangeLiveRunnerOutcome")
