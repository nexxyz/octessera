Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "board-profile.ps1")

function Assert-RaspberryLiveBenchmarkSelection {
  param(
    [ValidateSet(8, 12, 16, 24, 32)][int]$Units = 16,
    [ValidateSet("Inline", "Multicore")][string]$ExecutorMode = "Inline",
    [ValidateSet(30, 120, 180, 300)][int]$MeasureSeconds = 30,
    [switch]$ObserveCompromises
  )
  if ($ObserveCompromises -and $MeasureSeconds -ne 120) { throw "-ObserveCompromises is only valid for 120-second capacity cells." }
  $nativeExecutor = if ($ExecutorMode -ceq "Inline") { "inline" } else { "routing_tree_persistent" }
  $workerTiming = "disabled"
  $outputFrames = if ($ExecutorMode -ceq "Inline") { 128 } else { 256 }
  $alsaPeriodFrames = if ($ExecutorMode -ceq "Inline") { 32 } else { 64 }
  $internalFrames = if ($ExecutorMode -ceq "Inline") { 32 } else { 64 }
  $lookahead = if ($ExecutorMode -ceq "Inline") { 0 } else { 64 }
  $continueOnRecoveredMiss = $ObserveCompromises -and $ExecutorMode -ceq "Multicore"
  return [pscustomobject][ordered]@{
    Units = $Units
    Scenario = "capacity_analogue_$Units"
    ExecutorMode = $ExecutorMode
    NativeExecutorMode = $nativeExecutor
    WorkerTimingMode = $workerTiming
    OutputFrames = $outputFrames
    AlsaPeriodFrames = $alsaPeriodFrames
    InternalFrames = $internalFrames
    LookaheadFrames = $lookahead
    EffectiveOutputLatencyFrames = $outputFrames + $lookahead
    MeasureSeconds = $MeasureSeconds
    ObserveCompromises = [bool]$ObserveCompromises
    ContinueOnRecoveredMiss = [bool]$continueOnRecoveredMiss
    ContinueOnRecoveredMissArgument = if ($continueOnRecoveredMiss) { "--continue-on-recovered-miss" } else { "" }
  }
}

function Read-RaspberryLiveKeyValueFile {
  param([Parameter(Mandatory)][string]$Path)
  $values = @{}
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $values }
  foreach ($line in Get-Content -LiteralPath $Path -ErrorAction Stop) {
    $parts = $line -split "=", 2
    if ($parts.Count -eq 2) { $values[$parts[0]] = $parts[1] }
  }
  return $values
}

function Assert-RaspberryLiveBenchmarkReadiness {
  param(
    [Parameter(Mandatory)][pscustomobject]$Readiness,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][int]$ExpectedPid,
    [Parameter(Mandatory)][string]$ExpectedInvocation,
    [Parameter(Mandatory)][string]$ArtifactHash
  )
  Assert-RaspberryLiveUnsignedFields $Readiness @("schema_version", "pid", "requested_output_buffer_frames", "expected_alsa_buffer_frames", "expected_alsa_period_frames", "internal_block_frames", "lookahead_frames", "sample_rate", "channels", "callback_frames_min", "callback_frames_max", "callback_frame_sample_count", "invalid_callback_frame_count") "Raspberry benchmark readiness evidence"
  $checks = @(
    @([int]$Readiness.schema_version, 5),
    @([string]$Readiness.kind, "raspberry_audio_benchmark_readiness"),
    @([string]$Readiness.status, "ready"),
    @([string]$Readiness.board_profile, "raspberry-pi-zero-2w"),
    @([int]$Readiness.pid, $ExpectedPid),
    @([string]$Readiness.systemd_invocation_id, $ExpectedInvocation),
    @([string]$Readiness.artifact_sha256, $ArtifactHash),
    @([string]$Readiness.scenario, $Selection.Scenario),
    @([int]$Readiness.requested_output_buffer_frames, $Selection.OutputFrames),
    @([int]$Readiness.expected_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Readiness.expected_alsa_period_frames, $Selection.AlsaPeriodFrames),
    @([int]$Readiness.internal_block_frames, $Selection.InternalFrames),
    @([int]$Readiness.lookahead_frames, $Selection.LookaheadFrames),
    @([int]$Readiness.sample_rate, 44100),
    @([int]$Readiness.channels, 2),
    @([string]$Readiness.executor_mode, $Selection.NativeExecutorMode)
  )
  foreach ($check in $checks) { if ($check[0] -cne $check[1]) { throw "Raspberry benchmark readiness identity or geometry mismatch." } }
  if (-not [bool]$Readiness.scheduler_qualified -or -not [bool]$Readiness.post_dsp_zero) { throw "Raspberry benchmark readiness did not prove scheduler qualification and post-DSP mute." }
  if (@("F32", "I16", "U16") -notcontains [string]$Readiness.sample_format) { throw "Raspberry benchmark readiness sample format is unsupported." }
  if ([int]$Readiness.callback_frames_min -le 0 -or [int]$Readiness.callback_frames_max -lt [int]$Readiness.callback_frames_min -or [int]$Readiness.callback_frames_max -gt $Selection.OutputFrames -or [uint64]$Readiness.callback_frame_sample_count -lt 3 -or [uint64]$Readiness.invalid_callback_frame_count -ne 0) { throw "Raspberry benchmark readiness callback geometry is invalid." }
}

function Assert-RaspberryLiveBenchmarkRelease {
  param(
    [Parameter(Mandatory)][pscustomobject]$Release,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][int]$ExpectedPid,
    [Parameter(Mandatory)][string]$ExpectedInvocation,
    [Parameter(Mandatory)][string]$ArtifactHash
  )
  Assert-RaspberryLiveUnsignedFields $Release @("schema_version", "pid", "expected_alsa_buffer_frames", "observed_alsa_buffer_frames", "expected_alsa_period_frames", "observed_alsa_period_frames") "Raspberry benchmark release evidence"
  $checks = @(
    @([int]$Release.schema_version, 2),
    @([string]$Release.kind, "raspberry_audio_benchmark_release"),
    @([string]$Release.status, "released"),
    @([string]$Release.board_profile, "raspberry-pi-zero-2w"),
    @([int]$Release.pid, $ExpectedPid),
    @([string]$Release.systemd_invocation_id, $ExpectedInvocation),
    @([string]$Release.artifact_sha256, $ArtifactHash),
    @([string]$Release.scenario, $Selection.Scenario),
    @([int]$Release.expected_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Release.observed_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Release.expected_alsa_period_frames, $Selection.AlsaPeriodFrames),
    @([int]$Release.observed_alsa_period_frames, $Selection.AlsaPeriodFrames)
  )
  foreach ($check in $checks) { if ($check[0] -cne $check[1]) { throw "Raspberry benchmark release identity or ALSA geometry mismatch." } }
}

function Assert-RaspberryLiveCandidateReadiness {
  param(
    [Parameter(Mandatory)][pscustomobject]$Readiness,
    [Parameter(Mandatory)][int]$ExpectedPid,
    [Parameter(Mandatory)][string]$ExpectedInvocation
  )
  Assert-RaspberryLiveUnsignedFields $Readiness @("schema_version", "pid", "ready_at_unix_ms") "Restored Raspberry service readiness evidence"
  $checks = @(
    @([int]$Readiness.schema_version, 1),
    @([string]$Readiness.kind, "octessera_candidate_readiness"),
    @([string]$Readiness.status, "ready"),
    @([string]$Readiness.board_profile, "raspberry-pi-zero-2w"),
    @([int]$Readiness.pid, $ExpectedPid),
    @([string]$Readiness.systemd_invocation_id, $ExpectedInvocation)
  )
  foreach ($check in $checks) { if ($check[0] -cne $check[1]) { throw "Restored Raspberry service readiness identity mismatch." } }
  if ([string]::IsNullOrWhiteSpace([string]$Readiness.package_version) -or [uint64]$Readiness.ready_at_unix_ms -eq 0) { throw "Restored Raspberry service readiness marker is invalid." }
}

function Assert-RaspberryLiveRestoredCandidate {
  param([Parameter(Mandatory)][string]$EvidenceDirectory, [Parameter(Mandatory)][hashtable]$Restored)
  $candidate = Get-Content -LiteralPath (Join-Path $EvidenceDirectory "candidate-ready.json") -Raw -ErrorAction Stop | ConvertFrom-Json
  Assert-RaspberryLiveCandidateReadiness $candidate ([int]$Restored.final_pid) ([string]$Restored.final_invocation_id)
}

function Assert-RaspberryLiveSafetyEvidence {
  param([Parameter(Mandatory)][string]$EvidenceDirectory)
  $sensorPath = Join-Path $EvidenceDirectory "sensor-series.txt"
  if (-not (Test-Path -LiteralPath $sensorPath -PathType Leaf)) { throw "Raspberry live benchmark system evidence is missing." }
  $sensor = Assert-RaspberrySystemEvidence (Get-Content -LiteralPath $sensorPath -Raw -ErrorAction Stop) "Raspberry live benchmark system evidence"
  $sensorAbortPath = Join-Path $EvidenceDirectory "sensor-abort.txt"
  $sensorAbort = if (Test-Path -LiteralPath $sensorAbortPath -PathType Leaf) { (Get-Content -LiteralPath $sensorAbortPath -Raw -ErrorAction Stop).Trim() } else { "" }
  if (-not [bool]$sensor.SafetyAbort -and $sensorAbort -cne "reason=runtime-sensor-gate") { throw "Raspberry benchmark safety evidence did not contain an expected abort or runtime sensor gate." }
  $sensor
}

function Assert-RaspberryLiveAlsaEvidence {
  param([Parameter(Mandatory)][string]$EvidenceDirectory, [Parameter(Mandatory)][pscustomobject]$Selection)
  $path = Join-Path $EvidenceDirectory "alsa-hw-params.txt"
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Raspberry benchmark evidence is missing required file: alsa-hw-params.txt." }
  $content = Get-Content -LiteralPath $path -Raw -ErrorAction Stop
  if ($content -notmatch "(?m)^buffer_size\s*:\s*$($Selection.OutputFrames)\s*$" -or $content -notmatch "(?m)^period_size\s*:\s*$($Selection.AlsaPeriodFrames)\s*$") { throw "Raspberry benchmark raw ALSA evidence did not contain exact buffer_size $($Selection.OutputFrames) and period_size $($Selection.AlsaPeriodFrames) rows." }
}

function Assert-RaspberryLiveProfile {
  param([Parameter(Mandatory)][pscustomobject]$Profile, [Parameter(Mandatory)][int]$Units)
  Assert-RaspberryLiveUnsignedFields $Profile @("active_synth_voices", "active_sample_voices", "active_preview_sample_voices", "active_momentary_fx", "active_bus_fx_slots", "active_global_fx_slots", "cumulative_voice_steals", "cumulative_voice_admission_drops") "Raspberry benchmark profile evidence"
}

function Test-RaspberryLiveUnsignedClrInteger {
  param([object]$Value)
  if ($Value -is [bool] -or $null -eq $Value) { return $false }
  $typeName = $Value.GetType().FullName
  if ($typeName -eq "System.Decimal") {
    $bits = [decimal]::GetBits([decimal]$Value)
    return ($bits[3] -band 0x00FF0000) -eq 0 -and [decimal]$Value -ge 0 -and [decimal]$Value -le [decimal]([uint64]::MaxValue)
  }
  if (@("System.Byte", "System.UInt16", "System.UInt32", "System.UInt64") -contains $typeName) { return $true }
  if (@("System.SByte", "System.Int16", "System.Int32", "System.Int64") -contains $typeName) { return [int64]$Value -ge 0 }
  return $false
}

function Assert-RaspberryLiveUnsignedFields {
  param([Parameter(Mandatory)][pscustomobject]$Value, [Parameter(Mandatory)][string[]]$Names, [Parameter(Mandatory)][string]$Context)
  foreach ($name in $Names) {
    $property = $Value.PSObject.Properties[$name]
    if ($null -eq $property -or -not (Test-RaspberryLiveUnsignedClrInteger $property.Value)) { throw "$Context is structurally invalid for $name." }
  }
}

function Assert-RaspberryLiveBooleanFields {
  param([Parameter(Mandatory)][pscustomobject]$Value, [Parameter(Mandatory)][string[]]$Names, [Parameter(Mandatory)][string]$Context)
  foreach ($name in $Names) {
    $property = $Value.PSObject.Properties[$name]
    if ($null -eq $property -or $property.Value -isnot [bool]) { throw "$Context is structurally invalid for $name." }
  }
}

function Assert-RaspberryLiveNullableUnsignedFields {
  param([Parameter(Mandatory)][pscustomobject]$Value, [Parameter(Mandatory)][string[]]$Names, [Parameter(Mandatory)][string]$Context)
  foreach ($name in $Names) {
    $property = $Value.PSObject.Properties[$name]
    if ($null -eq $property -or ($null -ne $property.Value -and -not (Test-RaspberryLiveUnsignedClrInteger $property.Value))) { throw "$Context is structurally invalid for $name." }
  }
}

function Assert-RaspberryLiveStringFields {
  param([Parameter(Mandatory)][pscustomobject]$Value, [Parameter(Mandatory)][string[]]$Names, [Parameter(Mandatory)][string]$Context)
  foreach ($name in $Names) {
    $property = $Value.PSObject.Properties[$name]
    if ($null -eq $property -or $property.Value -isnot [string]) { throw "$Context is structurally invalid for $name." }
  }
}

function Assert-RaspberryLiveNullableStringFields {
  param([Parameter(Mandatory)][pscustomobject]$Value, [Parameter(Mandatory)][string[]]$Names, [Parameter(Mandatory)][string]$Context)
  foreach ($name in $Names) {
    $property = $Value.PSObject.Properties[$name]
    if ($null -eq $property -or ($null -ne $property.Value -and $property.Value -isnot [string])) { throw "$Context is structurally invalid for $name." }
  }
}

function Assert-RaspberryLiveCallback {
  param([Parameter(Mandatory)][pscustomobject]$Callback)
  Assert-RaspberryLiveUnsignedFields $Callback @(
    "lifetime_callback_count", "callback_count", "first_measured_callback_ns", "last_measured_callback_ns", "measured_elapsed_ns", "callback_frames_min", "callback_frames_max", "callback_frame_sample_count", "callback_frame_size_change_count", "invalid_callback_frame_count", "lifetime_callback_frames_min", "lifetime_callback_frames_max", "lifetime_callback_frame_sample_count", "lifetime_callback_frame_size_change_count", "lifetime_invalid_callback_frame_count", "rendered_frames", "render_audio_duration_ns", "over_audio_duration_budget_count", "callback_spacing_min_ns", "callback_spacing_max_ns", "callback_lateness_max_ns", "pre_mute_nonzero_samples", "post_mute_nonzero_samples", "cpal_device_error_count", "cpal_stream_error_count"
  ) "Raspberry benchmark callback evidence"
  foreach ($name in @("render_audio_duration_ratio_p50", "render_audio_duration_ratio_p95", "render_audio_duration_ratio_p99", "render_audio_duration_ratio_p99_9", "render_audio_duration_ratio_max", "pre_mute_peak")) {
    $property = $Callback.PSObject.Properties[$name]
    if ($null -eq $property -or [string]$property.Value -notmatch '^[0-9]+(\.[0-9]+)?$') { throw "Raspberry benchmark callback evidence is structurally invalid for $name." }
  }
  Assert-RaspberryLiveBooleanFields $Callback @("callback_timestamp_observed", "worker_terminal", "terminal_error") "Raspberry benchmark callback evidence"
}

function Assert-RaspberryLiveOutputCounters {
  param([Parameter(Mandatory)][pscustomobject]$Counters)
  Assert-RaspberryLiveBooleanFields $Counters @("observable") "Raspberry benchmark output-counter evidence"
  foreach ($phase in @("warmup", "start", "end", "delta")) {
    $snapshot = $Counters.PSObject.Properties[$phase]
    if ($null -eq $snapshot -or $null -eq $snapshot.Value) { throw "Raspberry benchmark output-counter evidence is structurally invalid for $phase." }
    Assert-RaspberryLiveUnsignedFields $snapshot.Value @("rendered_quantums", "repeated_quantums", "dropped_quantums", "deadline_misses", "deadline_recoveries") "Raspberry benchmark output-counter evidence"
  }
}

function Test-RaspberryLiveBenchmarkMeasurementComplete {
  param([Parameter(Mandatory)][pscustomobject]$Result)
  $callback = $Result.callback
  $callbackComplete = [uint64]$callback.callback_count -gt 0 -and [uint64]$callback.callback_frames_min -gt 0 -and [uint64]$callback.callback_frames_max -ge [uint64]$callback.callback_frames_min -and [uint64]$callback.callback_frames_max -le [uint64]$Result.requested_output_buffer_frames -and [uint64]$callback.callback_frame_sample_count -eq [uint64]$callback.callback_count -and [uint64]$callback.invalid_callback_frame_count -eq 0 -and [bool]$callback.callback_timestamp_observed -and -not [bool]$callback.worker_terminal -and -not [bool]$callback.terminal_error -and [uint64]$callback.post_mute_nonzero_samples -eq 0
  $lifecycleComplete = [bool]$Result.scheduler_qualified -and [bool]$Result.measurement_stop_acknowledged -and [bool]$Result.stream_stopped -and [bool]$Result.final_progress_write_succeeded -and [bool]$Result.post_dsp_zero -and $null -eq $Result.terminal_error -and $null -eq $Result.retirement_error
  if ([string]$Result.executor_mode -ceq "inline") {
    $workerComplete = [string]$Result.worker_health -ceq "disabled" -and [string]$Result.worker_thread_name_0 -eq "" -and [string]$Result.worker_thread_name_1 -eq "" -and [int]$Result.joined_workers -eq 0
  } else {
    $healthy = [string]$Result.worker_health -ceq "healthy"
    $allowedDeadlineMiss = [string]$Result.executor_mode -ceq "routing_tree_persistent" -and [bool]$Result.continue_on_recovered_miss -and [string]$Result.worker_health -ceq "deadline_miss"
    $workerComplete = [string]$Result.worker_thread_name_0 -ceq "oct-dsp-tree-0" -and [string]$Result.worker_thread_name_1 -ceq "oct-dsp-tree-1" -and [int]$Result.joined_workers -eq 2 -and ($healthy -or $allowedDeadlineMiss)
  }
  return $callbackComplete -and $lifecycleComplete -and $workerComplete
}

function Test-RaspberryLiveProfileClean {
  param([Parameter(Mandatory)][pscustomobject]$Profile, [Parameter(Mandatory)][int]$Units)
  $expected = @{
    active_synth_voices = 3 * $Units
    active_sample_voices = $Units
    active_preview_sample_voices = 0
    active_momentary_fx = [math]::Min([math]::Ceiling($Units / 4), 2)
    active_bus_fx_slots = [math]::Min([math]::Ceiling($Units / 2), 12)
    active_global_fx_slots = [math]::Min([math]::Ceiling($Units / 8), 2)
    cumulative_voice_steals = 0
    cumulative_voice_admission_drops = 0
  }
  return @($expected.Keys | Where-Object { [uint64]$Profile.$_ -ne [uint64]$expected[$_] }).Count -eq 0
}

function Assert-RaspberryLiveWorkerTiming {
  param([Parameter(Mandatory)][pscustomobject]$Timing)
  foreach ($name in @("workers", "coordinator", "late_after_deadline_ns", "cpu_endpoint_changed")) { if ($null -eq $Timing.PSObject.Properties[$name]) { throw "Raspberry benchmark worker timing evidence is structurally invalid." } }
  if ($null -eq $Timing.workers -or $Timing.workers.Count -ne 2 -or $null -eq $Timing.coordinator) { throw "Raspberry benchmark worker timing evidence is structurally invalid." }
  Assert-RaspberryLiveNullableUnsignedFields $Timing @("late_after_deadline_ns") "Raspberry benchmark worker timing evidence"
  Assert-RaspberryLiveBooleanFields $Timing @("cpu_endpoint_changed") "Raspberry benchmark worker timing evidence"
  foreach ($worker in $Timing.workers) {
    Assert-RaspberryLiveNullableUnsignedFields $worker @("sequence", "render_ns", "dispatch_to_finish_ns", "cpu_start", "cpu_end") "Raspberry benchmark worker timing evidence"
    Assert-RaspberryLiveBooleanFields $worker @("finished") "Raspberry benchmark worker timing evidence"
  }
  Assert-RaspberryLiveNullableUnsignedFields $Timing.coordinator @("sequence", "deadline_ns", "dispatch_to_deadline_start_ns", "dispatch_to_deadline_elapsed_ns", "in_flight_mask", "completed_mask", "first_parity", "dispatch_to_first_ns", "dispatch_to_both_ns", "reduction_ns", "coordinator_remainder_ns", "engine_block_total_ns", "callback_total_ns") "Raspberry benchmark worker timing evidence"
  Assert-RaspberryLiveBooleanFields $Timing.coordinator @("failed", "frozen") "Raspberry benchmark worker timing evidence"
}

function Test-RaspberryLiveWorkerTimingClean {
  param([Parameter(Mandatory)][pscustomobject]$Timing)
  if ($null -ne $Timing.late_after_deadline_ns -or [bool]$Timing.cpu_endpoint_changed) { return $false }
  if ($null -eq $Timing.coordinator.sequence -or [bool]$Timing.coordinator.failed -or -not [bool]$Timing.coordinator.frozen -or [int]$Timing.coordinator.completed_mask -ne 3) { return $false }
  foreach ($index in 0..1) {
    $worker = $Timing.workers[$index]
    if (-not [bool]$worker.finished -or [int]$worker.cpu_start -ne (2 + $index) -or [int]$worker.cpu_end -ne (2 + $index) -or $null -eq $worker.dispatch_to_finish_ns) { return $false }
  }
  return $true
}

function Assert-RaspberryLiveBenchmarkResult {
  param(
    [Parameter(Mandatory)][pscustomobject]$Result,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$ArtifactHash,
    [Parameter(Mandatory)][int]$ExpectedPid,
    [Parameter(Mandatory)][string]$ExpectedInvocation
  )
  Assert-RaspberryLiveUnsignedFields $Result @("schema_version", "requested_output_buffer_frames", "expected_alsa_buffer_frames", "expected_alsa_period_frames", "internal_block_frames", "lookahead_frames", "effective_output_latency_frames", "pid", "sample_rate", "channels", "warmup_seconds", "measure_seconds") "Raspberry benchmark result evidence"
  $checks = @(
    @([int]$Result.schema_version, 13),
    @([string]$Result.kind, "raspberry_audio_benchmark_result"),
    @([string]$Result.board_profile, "raspberry-pi-zero-2w"),
    @([string]$Result.artifact_sha256, $ArtifactHash),
    @([string]$Result.scenario, $Selection.Scenario),
    @([int]$Result.requested_output_buffer_frames, $Selection.OutputFrames),
    @([int]$Result.expected_alsa_buffer_frames, $Selection.OutputFrames),
    @([int]$Result.expected_alsa_period_frames, $Selection.AlsaPeriodFrames),
    @([int]$Result.internal_block_frames, $Selection.InternalFrames),
    @([int]$Result.lookahead_frames, $Selection.LookaheadFrames),
    @([int]$Result.effective_output_latency_frames, $Selection.EffectiveOutputLatencyFrames),
    @([int]$Result.pid, $ExpectedPid),
    @([string]$Result.systemd_invocation_id, $ExpectedInvocation),
    @([int]$Result.sample_rate, 44100),
    @([int]$Result.channels, 2),
    @([string]$Result.executor_mode, $Selection.NativeExecutorMode),
    @([string]$Result.worker_timing_mode, $Selection.WorkerTimingMode),
    @([int]$Result.warmup_seconds, 5),
    @([int]$Result.measure_seconds, $Selection.MeasureSeconds)
  )
  foreach ($check in $checks) { if ($check[0] -cne $check[1]) { throw "Raspberry benchmark result identity or geometry mismatch." } }
  if (@("pass", "fail") -notcontains [string]$Result.status) { throw "Raspberry benchmark result status is invalid." }
  if ($null -ne $Result.PSObject.Properties["inline_underfill"]) { throw "Raspberry benchmark result must not report inline underfill evidence." }
  Assert-RaspberryLiveCallback $Result.callback
  Assert-RaspberryLiveOutputCounters $Result.persistent_output_counters
  if ([bool]$Result.persistent_output_counters.observable -ne ($Selection.NativeExecutorMode -cne "inline")) { throw "Raspberry benchmark output-counter observability does not match the executor." }
  Assert-RaspberryLiveBooleanFields $Result @("continue_on_recovered_miss") "Raspberry benchmark continuation evidence"
  if ([bool]$Result.continue_on_recovered_miss -ne [bool]$Selection.ContinueOnRecoveredMiss) { throw "Raspberry benchmark continuation identity does not match the selected observation mode." }
  $provenance = $Result.PSObject.Properties["persistent_output_provenance"]
  if ($null -eq $provenance -or $null -eq $provenance.Value) { throw "Raspberry benchmark persistent output provenance evidence is missing." }
  Assert-RaspberryLiveBooleanFields $provenance.Value @("observable") "Raspberry benchmark persistent output provenance evidence"
  Assert-RaspberryLiveUnsignedFields $provenance.Value @("repeated_quantum_incidents", "repeated_pcm_frames", "silent_quantum_incidents", "silent_pcm_frames") "Raspberry benchmark persistent output provenance evidence"
  if ([bool]$provenance.Value.observable -ne ($Selection.ExecutorMode -ceq "Multicore")) { throw "Raspberry benchmark persistent output provenance observability does not match the executor." }
  $unobservableProvenance = @("repeated_quantum_incidents", "repeated_pcm_frames", "silent_quantum_incidents", "silent_pcm_frames") | Where-Object { [uint64]$provenance.Value.$_ -ne 0 }
  if (-not [bool]$provenance.Value.observable -and @($unobservableProvenance).Count -gt 0) { throw "Unobservable Raspberry benchmark persistent output provenance must be zero." }
  foreach ($pair in @(@("repeated_quantum_incidents", "repeated_pcm_frames"), @("silent_quantum_incidents", "silent_pcm_frames"))) {
    $incidents = [uint64]$provenance.Value.($pair[0])
    $frames = [uint64]$provenance.Value.($pair[1])
    if (($incidents -eq 0) -ne ($frames -eq 0) -or $incidents -gt $frames) { throw "Raspberry benchmark persistent output provenance is internally inconsistent." }
  }
  Assert-RaspberryLiveUnsignedFields $Result @("detected_continuity_events", "joined_workers") "Raspberry benchmark result evidence"
  Assert-RaspberryLiveStringFields $Result @("kind", "board_profile", "artifact_sha256", "scenario", "sample_format", "executor_mode", "worker_health", "worker_thread_name_0", "worker_thread_name_1", "worker_timing_mode") "Raspberry benchmark result evidence"
  Assert-RaspberryLiveNullableStringFields $Result @("systemd_invocation_id", "callback_scheduling_policy", "terminal_error", "retirement_error") "Raspberry benchmark result evidence"
  Assert-RaspberryLiveNullableUnsignedFields $Result @("recovered_alsa_epipe_count", "callback_scheduling_priority", "callback_scheduling_cpu") "Raspberry benchmark result evidence"
  Assert-RaspberryLiveBooleanFields $Result @("scheduler_qualified", "post_dsp_zero", "measurement_stop_acknowledged", "stream_stopped", "final_progress_write_succeeded", "recovered_alsa_epipe_observable") "Raspberry benchmark lifecycle evidence"
  if ($null -ne $Result.recovered_alsa_epipe_count -or [bool]$Result.recovered_alsa_epipe_observable) { throw "Raspberry benchmark result made an invalid recovered ALSA EPIPE claim." }
  if (($Selection.WorkerTimingMode -ceq "disabled" -and $null -ne $Result.worker_timing) -or ($Selection.WorkerTimingMode -ceq "enabled" -and $null -eq $Result.worker_timing)) { throw "Raspberry benchmark worker timing evidence does not match the selected timing mode." }
  if ([bool]$Result.scheduler_qualified -and ([string]$Result.callback_scheduling_policy -cne "SCHED_FIFO" -or [int]$Result.callback_scheduling_priority -ne 70 -or [int]$Result.callback_scheduling_cpu -ne 1)) { throw "Raspberry benchmark callback scheduling evidence is invalid." }
  Assert-RaspberryLiveProfile $Result.profile_start $Selection.Units
  Assert-RaspberryLiveProfile $Result.profile_end $Selection.Units
  $callback = $Result.callback
  if ($Selection.NativeExecutorMode -ceq "inline") {
    if ([string]$Result.worker_health -cne "disabled" -or [string]$Result.worker_thread_name_0 -ne "" -or [string]$Result.worker_thread_name_1 -ne "" -or [int]$Result.joined_workers -ne 0 -or $null -ne $Result.worker_timing) { throw "Raspberry Inline worker evidence is invalid." }
  } else {
    $preStreamFailure = [string]$Result.status -ceq "fail" -and $null -ne $Result.terminal_error -and [string]$Result.worker_health -ceq "disabled" -and [string]$Result.worker_thread_name_0 -eq "" -and [string]$Result.worker_thread_name_1 -eq "" -and [int]$Result.joined_workers -eq 0
    $persistentHealth = @("healthy", "deadline_miss", "dispatch_failed", "completion_failed", "worker_exited", "invalid_block") -contains [string]$Result.worker_health
    if (-not $preStreamFailure -and ([string]$Result.worker_thread_name_0 -cne "oct-dsp-tree-0" -or [string]$Result.worker_thread_name_1 -cne "oct-dsp-tree-1" -or -not $persistentHealth -or [int]$Result.joined_workers -ne 2)) { throw "Raspberry Multicore worker lifecycle evidence is invalid." }
    if ($null -ne $Result.worker_timing) { Assert-RaspberryLiveWorkerTiming $Result.worker_timing }
  }
}

function Test-RaspberryLiveBenchmarkClean {
  param([Parameter(Mandatory)][pscustomobject]$Result, [Parameter(Mandatory)][pscustomobject]$Selection)
  $callback = $Result.callback
  $counters = $Result.persistent_output_counters
  $callbackClean = [uint64]$callback.callback_count -gt 0 -and [uint64]$callback.callback_frames_min -gt 0 -and [uint64]$callback.callback_frames_max -ge [uint64]$callback.callback_frames_min -and [uint64]$callback.callback_frames_max -le $Selection.OutputFrames -and [uint64]$callback.callback_frame_sample_count -eq [uint64]$callback.callback_count -and [uint64]$callback.invalid_callback_frame_count -eq 0 -and [uint64]$callback.over_audio_duration_budget_count -eq 0 -and [uint64]$callback.cpal_device_error_count -eq 0 -and [uint64]$callback.cpal_stream_error_count -eq 0 -and [uint64]$Result.detected_continuity_events -eq 0 -and [uint64]$callback.post_mute_nonzero_samples -eq 0 -and [bool]$callback.callback_timestamp_observed -and -not [bool]$callback.worker_terminal -and -not [bool]$callback.terminal_error -and [bool]$Result.post_dsp_zero
  $counterClean = [bool]$counters.observable -eq ($Selection.NativeExecutorMode -cne "inline")
  foreach ($name in @("warmup", "start", "end", "delta")) {
    $snapshot = $counters.$name
    $counterClean = $counterClean -and [uint64]$snapshot.repeated_quantums -eq 0 -and [uint64]$snapshot.dropped_quantums -eq 0 -and [uint64]$snapshot.deadline_misses -eq 0 -and [uint64]$snapshot.deadline_recoveries -eq 0
  }
  $profilesClean = (Test-RaspberryLiveProfileClean $Result.profile_start $Selection.Units) -and (Test-RaspberryLiveProfileClean $Result.profile_end $Selection.Units)
  $lifecycleClean = [bool]$Result.scheduler_qualified -and [bool]$Result.measurement_stop_acknowledged -and [bool]$Result.stream_stopped -and [bool]$Result.final_progress_write_succeeded -and $null -eq $Result.retirement_error -and $null -eq $Result.terminal_error
  if ($Selection.NativeExecutorMode -ceq "inline") {
    $workersClean = [string]$Result.worker_health -ceq "disabled" -and [int]$Result.joined_workers -eq 0
  } else {
    $timingClean = if ($Selection.WorkerTimingMode -ceq "disabled") { $null -eq $Result.worker_timing } else { $null -ne $Result.worker_timing -and (Test-RaspberryLiveWorkerTimingClean $Result.worker_timing) }
    $workersClean = [string]$Result.worker_health -ceq "healthy" -and [int]$Result.joined_workers -eq 2 -and $timingClean
  }
  return $callbackClean -and $counterClean -and $profilesClean -and $lifecycleClean -and $workersClean
}

function Assert-RaspberryLiveEvidenceFiles {
  param(
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [Parameter(Mandatory)][string[]]$Names
  )
  foreach ($name in $Names) {
    if (-not (Test-Path -LiteralPath (Join-Path $EvidenceDirectory $name) -PathType Leaf)) { throw "Raspberry benchmark evidence is missing required file: $name." }
  }
}

function Get-RaspberryLiveAlsaRecoveryLogIncidents {
  param([Parameter(Mandatory)][string]$EvidenceDirectory)
  $path = Join-Path $EvidenceDirectory "unit-journal.txt"
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Raspberry benchmark evidence is missing required file: unit-journal.txt." }
  $content = Get-Content -LiteralPath $path -Raw -ErrorAction Stop
  return [uint64](@($content -split "`r?`n" | Where-Object { $_ -match "snd_pcm_recover.*underrun occurred" -or $_ -match "ALSA lib pcm\.c:[0-9]+:\([^)]*\) underrun occurred" }).Count)
}

function Get-RaspberryLivePracticalGrade {
  param(
    [Parameter(Mandatory)][uint64]$RepeatIncidents,
    [Parameter(Mandatory)][uint64]$SilentIncidents,
    [Parameter(Mandatory)][uint64]$AlsaRecoveryLogIncidents,
    [Parameter(Mandatory)][uint64]$CpalStreamErrors,
    [Parameter(Mandatory)][uint64]$CpalDeviceErrors
  )
  $worst = [uint64](@($RepeatIncidents, $SilentIncidents, $AlsaRecoveryLogIncidents, $CpalStreamErrors, $CpalDeviceErrors) | Measure-Object -Maximum).Maximum
  if ($worst -ge 5) { return "Compromised" }
  if ($worst -ge 2) { return "Stretched" }
  return "Stable"
}

function Get-RaspberryLiveHostEvidence {
  param(
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$ArtifactHash
  )
  $status = "infrastructure_failure"
  $reason = "missing Raspberry benchmark evidence"
  $result = $null
  $sensor = $null
  $repeatIncidents = $null
  $repeatedPcmFrames = $null
  $silentIncidents = $null
  $silentPcmFrames = $null
  $alsaRecoveryLogIncidents = $null
  $practicalGrade = $null
  try {
    Assert-RaspberryLiveEvidenceFiles $EvidenceDirectory @("study-result.txt")
    $study = Read-RaspberryLiveKeyValueFile (Join-Path $EvidenceDirectory "study-result.txt")
    if ($study.status_class -ceq "infrastructure_failure" -and $study.interruption_started -ceq "false") {
      if ([string]::IsNullOrWhiteSpace([string]$study.reason)) { throw "Raspberry benchmark pre-interruption infrastructure evidence did not record a reason." }
      return [pscustomobject][ordered]@{ StatusClass = "infrastructure_failure"; Reason = [string]$study.reason; Scenario = $Selection.Scenario; Units = $Selection.Units; ExecutorMode = $Selection.ExecutorMode; OutputFrames = $Selection.OutputFrames; AlsaPeriodFrames = $Selection.AlsaPeriodFrames; InternalFrames = $Selection.InternalFrames; LookaheadFrames = $Selection.LookaheadFrames; ArtifactSha256 = $ArtifactHash; RepeatIncidents = $null; RepeatedPcmFrames = $null; SilentIncidents = $null; SilentPcmFrames = $null; AlsaRecoveryLogIncidents = $null; PracticalGrade = $null; ResultPath = ""; ReadinessPath = ""; ReleasePath = ""; SensorSeriesPath = "" }
    }
    Assert-RaspberryLiveEvidenceFiles $EvidenceDirectory @("service-restored-state.txt")
    $restored = Read-RaspberryLiveKeyValueFile (Join-Path $EvidenceDirectory "service-restored-state.txt")
    $restorationFailed = $restored.Count -gt 0 -and ($restored.restore_status -ne "0" -or $restored.final_active -ne "active" -or $restored.final_enabled -ne "enabled")
    if ($restorationFailed) {
      $status = "restoration_failure"
      $reason = "Raspberry benchmark service restoration failed."
    } elseif ($restored.Count -gt 0 -and $study.status_class -ceq "safety_failure") {
      $status = "restoration_failure"
      Assert-RaspberryLiveEvidenceFiles $EvidenceDirectory @("candidate-ready.json")
      Assert-RaspberryLiveRestoredCandidate $EvidenceDirectory $restored
      $status = "infrastructure_failure"
      Assert-RaspberryLiveEvidenceFiles $EvidenceDirectory @("sensor-series.txt")
      $sensor = Assert-RaspberryLiveSafetyEvidence $EvidenceDirectory
      $status = "safety_failure"
      $reason = "Raspberry benchmark safety gate aborted the study."
    }
    if ($status -eq "infrastructure_failure") {
      Assert-RaspberryLiveEvidenceFiles $EvidenceDirectory @("benchmark-identity.txt", "benchmark-result.json", "benchmark-readiness.json", "benchmark-release.json", "sensor-series.txt", "alsa-hw-params.txt", "unit-journal.txt")
      $identity = Read-RaspberryLiveKeyValueFile (Join-Path $EvidenceDirectory "benchmark-identity.txt")
      $result = Get-Content -LiteralPath (Join-Path $EvidenceDirectory "benchmark-result.json") -Raw -ErrorAction Stop | ConvertFrom-Json
      $readiness = Get-Content -LiteralPath (Join-Path $EvidenceDirectory "benchmark-readiness.json") -Raw -ErrorAction Stop | ConvertFrom-Json
      $release = Get-Content -LiteralPath (Join-Path $EvidenceDirectory "benchmark-release.json") -Raw -ErrorAction Stop | ConvertFrom-Json
      Assert-RaspberryLiveBenchmarkReadiness $readiness $Selection ([int]$identity.main_pid) ([string]$identity.invocation_id) $ArtifactHash
      Assert-RaspberryLiveBenchmarkRelease $release $Selection ([int]$identity.main_pid) ([string]$identity.invocation_id) $ArtifactHash
      Assert-RaspberryLiveBenchmarkResult $result $Selection $ArtifactHash ([int]$identity.main_pid) ([string]$identity.invocation_id)
      $sensor = Assert-RaspberrySystemEvidence (Get-Content -LiteralPath (Join-Path $EvidenceDirectory "sensor-series.txt") -Raw -ErrorAction Stop) "Raspberry live benchmark system evidence"
      Assert-RaspberryLiveAlsaEvidence $EvidenceDirectory $Selection
      $provenance = $result.persistent_output_provenance
      $repeatIncidents = [uint64]$provenance.repeated_quantum_incidents
      $repeatedPcmFrames = [uint64]$provenance.repeated_pcm_frames
      $silentIncidents = [uint64]$provenance.silent_quantum_incidents
      $silentPcmFrames = [uint64]$provenance.silent_pcm_frames
      $alsaRecoveryLogIncidents = Get-RaspberryLiveAlsaRecoveryLogIncidents $EvidenceDirectory
      $practicalGrade = Get-RaspberryLivePracticalGrade -RepeatIncidents $repeatIncidents -SilentIncidents $silentIncidents -AlsaRecoveryLogIncidents $alsaRecoveryLogIncidents -CpalStreamErrors ([uint64]$result.callback.cpal_stream_error_count) -CpalDeviceErrors ([uint64]$result.callback.cpal_device_error_count)
      $restorationFailed = $restored.Count -eq 0 -or $restored.restore_status -ne "0" -or $restored.final_active -ne "active" -or $restored.final_enabled -ne "enabled"
      if ($restorationFailed) { throw "Raspberry benchmark service restoration failed." }
      $status = "restoration_failure"
      Assert-RaspberryLiveEvidenceFiles $EvidenceDirectory @("candidate-ready.json")
      Assert-RaspberryLiveRestoredCandidate $EvidenceDirectory $restored
      $status = "infrastructure_failure"
      if ($study.status_class -ceq "infrastructure_failure") { $status = "infrastructure_failure"; $reason = "Raspberry benchmark infrastructure evidence failed." }
      elseif ($study.status_class -ceq "pass" -and [string]$result.status -ceq "pass" -and (Test-RaspberryLiveBenchmarkClean $result $Selection)) { $status = "pass" }
      elseif ($study.status_class -ceq "measured_failure" -and [string]$result.status -ceq "fail" -and (Test-RaspberryLiveBenchmarkMeasurementComplete $result)) { $status = "measured_failure"; $reason = if ($Selection.ObserveCompromises) { "Raspberry benchmark completed observation with PracticalGrade=$practicalGrade; ALSA recovery count is a conservative whole-run unit journal count, not phase-exact EPIPE observability." } else { "Raspberry benchmark produced structurally valid non-clean measurement evidence." } }
      else { throw "Raspberry benchmark study and result status disagree." }
      if ($status -eq "pass") { $reason = "Raspberry benchmark identity, geometry, worker, callback, sensor, and restoration evidence validated; ALSA recovery count is a conservative whole-run unit journal count, not phase-exact EPIPE observability." }
    }
    return [pscustomobject][ordered]@{ StatusClass = $status; Reason = $reason; Scenario = $Selection.Scenario; Units = $Selection.Units; ExecutorMode = $Selection.ExecutorMode; OutputFrames = $Selection.OutputFrames; AlsaPeriodFrames = $Selection.AlsaPeriodFrames; InternalFrames = $Selection.InternalFrames; LookaheadFrames = $Selection.LookaheadFrames; ArtifactSha256 = $ArtifactHash; RepeatIncidents = $repeatIncidents; RepeatedPcmFrames = $repeatedPcmFrames; SilentIncidents = $silentIncidents; SilentPcmFrames = $silentPcmFrames; AlsaRecoveryLogIncidents = $alsaRecoveryLogIncidents; PracticalGrade = $practicalGrade; ResultPath = Join-Path $EvidenceDirectory "benchmark-result.json"; ReadinessPath = Join-Path $EvidenceDirectory "benchmark-readiness.json"; ReleasePath = Join-Path $EvidenceDirectory "benchmark-release.json"; SensorSeriesPath = Join-Path $EvidenceDirectory "sensor-series.txt"; Sensor = $sensor }
  } catch {
    $reason = $_.Exception.Message
  }
  return [pscustomobject][ordered]@{ StatusClass = $status; Reason = $reason; Scenario = $Selection.Scenario; Units = $Selection.Units; ExecutorMode = $Selection.ExecutorMode; OutputFrames = $Selection.OutputFrames; AlsaPeriodFrames = $Selection.AlsaPeriodFrames; InternalFrames = $Selection.InternalFrames; LookaheadFrames = $Selection.LookaheadFrames; ArtifactSha256 = $ArtifactHash; RepeatIncidents = $null; RepeatedPcmFrames = $null; SilentIncidents = $null; SilentPcmFrames = $null; AlsaRecoveryLogIncidents = $null; PracticalGrade = $null; ResultPath = ""; ReadinessPath = ""; ReleasePath = ""; SensorSeriesPath = "" }
}

Export-ModuleMember -Function Assert-RaspberryLiveBenchmarkSelection, Assert-RaspberryLiveBenchmarkReadiness, Assert-RaspberryLiveBenchmarkRelease, Assert-RaspberryLiveCandidateReadiness, Assert-RaspberryLiveBenchmarkResult, Test-RaspberryLiveBenchmarkClean, Get-RaspberryLiveHostEvidence, Read-RaspberryLiveKeyValueFile, Get-RaspberryLivePracticalGrade, Get-RaspberryLiveAlsaRecoveryLogIncidents
