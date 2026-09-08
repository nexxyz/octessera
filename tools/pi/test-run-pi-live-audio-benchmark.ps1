$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$runner = Join-Path $PSScriptRoot "run-pi-live-audio-benchmark.ps1"
$runnerSource = [IO.File]::ReadAllText($runner)
Import-Module (Join-Path $PSScriptRoot "raspberry-live-benchmark-metadata.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "raspberry-live-benchmark-validation.psm1") -Force

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action, [Parameter(Mandatory)][string]$Label)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected failure did not occur: $Label" }
}

function Get-TestHostEvidenceWithoutErrors {
  param([Parameter(Mandatory)][string]$EvidenceDirectory, [Parameter(Mandatory)][pscustomobject]$Selection, [Parameter(Mandatory)][string]$ArtifactHash)
  $output = @(& { Get-RaspberryLiveHostEvidence -EvidenceDirectory $EvidenceDirectory -Selection $Selection -ArtifactHash $ArtifactHash } 2>&1)
  if (@($output | Where-Object { $_ -is [System.Management.Automation.ErrorRecord] }).Count -ne 0) { throw "Raspberry host evidence emitted a raw PowerShell error." }
  $values = @($output | Where-Object { $_ -isnot [System.Management.Automation.ErrorRecord] })
  if ($values.Count -ne 1) { throw "Raspberry host evidence did not return exactly one result." }
  return $values[0]
}

function Invoke-PrintOnly {
  param([hashtable]$Parameters = @{})
  $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $runner)
  foreach ($key in $Parameters.Keys) {
    if ($Parameters[$key] -is [bool]) { if ($Parameters[$key]) { $arguments += "-$key" } }
    else { $arguments += @("-$key", [string]$Parameters[$key]) }
  }
  $output = @(& (Join-Path $PSHOME "powershell.exe") @arguments 2>&1)
  if ($LASTEXITCODE -ne 0) { throw "PrintOnly failed." }
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

$inline = Assert-RaspberryLiveBenchmarkSelection -Units 16 -ExecutorMode Inline -MeasureSeconds 30
if ($inline.Scenario -cne "capacity_analogue_16" -or $inline.NativeExecutorMode -cne "inline" -or $inline.OutputFrames -ne 256 -or $inline.AlsaPeriodFrames -ne 64 -or $inline.InternalFrames -ne 128 -or $inline.LookaheadFrames -ne 0 -or $inline.EffectiveOutputLatencyFrames -ne 256 -or $inline.WorkerTimingMode -cne "disabled" -or $inline.ContinueOnRecoveredMiss) { throw "Inline Raspberry benchmark selection changed." }
$u8 = Assert-RaspberryLiveBenchmarkSelection -Units 8 -ExecutorMode Inline -MeasureSeconds 30
if ($u8.Scenario -cne "capacity_analogue_8" -or $u8.NativeExecutorMode -cne "inline" -or $u8.OutputFrames -ne 256 -or $u8.AlsaPeriodFrames -ne 64 -or $u8.InternalFrames -ne 128 -or $u8.LookaheadFrames -ne 0 -or $u8.EffectiveOutputLatencyFrames -ne 256) { throw "U8 Raspberry benchmark selection or geometry changed." }
$multicore = Assert-RaspberryLiveBenchmarkSelection -Units 32 -ExecutorMode Multicore -MeasureSeconds 120 -ObserveCompromises
if ($multicore.Scenario -cne "capacity_analogue_32" -or $multicore.NativeExecutorMode -cne "routing_tree_persistent" -or $multicore.OutputFrames -ne 256 -or $multicore.AlsaPeriodFrames -ne 64 -or $multicore.InternalFrames -ne 128 -or $multicore.WorkerTimingMode -cne "disabled" -or $multicore.LookaheadFrames -ne 128 -or $multicore.EffectiveOutputLatencyFrames -ne 384 -or -not $multicore.ContinueOnRecoveredMiss -or $multicore.ContinueOnRecoveredMissArgument -cne "--continue-on-recovered-miss") { throw "Multicore Raspberry benchmark selection changed." }
$multicoreStrict = Assert-RaspberryLiveBenchmarkSelection -Units 32 -ExecutorMode Multicore -MeasureSeconds 120
if ($multicoreStrict.OutputFrames -ne 256 -or $multicoreStrict.AlsaPeriodFrames -ne 64 -or $multicoreStrict.InternalFrames -ne 128 -or $multicoreStrict.LookaheadFrames -ne 128 -or $multicoreStrict.EffectiveOutputLatencyFrames -ne 384 -or $multicoreStrict.ContinueOnRecoveredMiss -or $multicoreStrict.WorkerTimingMode -cne "disabled") { throw "Strict Multicore Raspberry benchmark selection changed." }
$inlineObservation = Assert-RaspberryLiveBenchmarkSelection -Units 16 -ExecutorMode Inline -MeasureSeconds 120 -ObserveCompromises
if (-not $inlineObservation.ObserveCompromises -or $inlineObservation.OutputFrames -ne 256 -or $inlineObservation.AlsaPeriodFrames -ne 64 -or $inlineObservation.InternalFrames -ne 128 -or $inlineObservation.LookaheadFrames -ne 0 -or $inlineObservation.EffectiveOutputLatencyFrames -ne 256 -or $inlineObservation.ContinueOnRecoveredMiss -or $inlineObservation.WorkerTimingMode -cne "disabled") { throw "Inline Raspberry observation selection changed." }
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 16 -ExecutorMode Inline -MeasureSeconds 30 -ObserveCompromises } "observation mode duration gate"

$testHash = "a" * 64
if ((Get-RaspberryLivePracticalGrade -RepeatIncidents 1 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0) -cne "Stable" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 0 -SilentIncidents 2 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0) -cne "Stretched" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 0 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 5 -CpalDeviceErrors 0) -cne "Compromised") { throw "Raspberry practical grade thresholds changed." }
function New-TestProfile {
  param([int]$Units = 16)
  [pscustomobject][ordered]@{
    active_synth_voices = 3 * $Units
    active_sample_voices = $Units
    active_preview_sample_voices = 0
    active_momentary_fx = [math]::Min([math]::Ceiling($Units / 4), 2)
    active_bus_fx_slots = [math]::Min([math]::Ceiling($Units / 2), 12)
    active_global_fx_slots = [math]::Min([math]::Ceiling($Units / 8), 2)
    cumulative_voice_steals = 0
    cumulative_voice_admission_drops = 0
  }
}
$u8Profile = New-TestProfile 8
if ($u8Profile.active_synth_voices -ne 24 -or $u8Profile.active_sample_voices -ne 8 -or $u8Profile.active_bus_fx_slots -ne 4 -or $u8Profile.active_global_fx_slots -ne 1 -or $u8Profile.active_momentary_fx -ne 2) { throw "U8 Raspberry benchmark workload changed." }
function New-TestCounters {
  param([Parameter(Mandatory)][pscustomobject]$Selection)
  $zero = [pscustomobject][ordered]@{ rendered_quantums = 0; repeated_quantums = 0; dropped_quantums = 0; deadline_misses = 0; deadline_recoveries = 0 }
  [pscustomobject][ordered]@{ observable = $Selection.ExecutorMode -ceq "Multicore"; warmup = $zero; start = $zero; end = $zero; delta = $zero }
}
function New-TestCallback {
  param([Parameter(Mandatory)][pscustomobject]$Selection, [uint64]$OverrunCount = 0, [uint64]$CpalDeviceErrors = 0, [uint64]$CpalStreamErrors = 0)
  $frames = $Selection.OutputFrames
  [pscustomobject][ordered]@{
    lifetime_callback_count = 10; callback_count = 10; first_measured_callback_ns = 1; last_measured_callback_ns = 10; measured_elapsed_ns = 9; callback_frames_min = $frames; callback_frames_max = $frames; callback_frame_sample_count = 10; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0; lifetime_callback_frames_min = $frames; lifetime_callback_frames_max = $frames; lifetime_callback_frame_sample_count = 10; lifetime_callback_frame_size_change_count = 0; lifetime_invalid_callback_frame_count = 0; rendered_frames = 10 * $frames; render_audio_duration_ns = 100; render_audio_duration_ratio_p50 = 0; render_audio_duration_ratio_p95 = 0; render_audio_duration_ratio_p99 = 0; render_audio_duration_ratio_p99_9 = 0; render_audio_duration_ratio_max = 0; over_audio_duration_budget_count = $OverrunCount; callback_spacing_min_ns = 1; callback_spacing_max_ns = 1; callback_lateness_max_ns = 0; callback_timestamp_observed = $true; pre_mute_nonzero_samples = 10; pre_mute_peak = 1; post_mute_nonzero_samples = 0; cpal_device_error_count = $CpalDeviceErrors; cpal_stream_error_count = $CpalStreamErrors; worker_terminal = $false; terminal_error = $false
  }
}
function New-TestWorkerTiming {
  [pscustomobject][ordered]@{
    workers = @(
      [pscustomobject][ordered]@{ sequence = 7; render_ns = 10; dispatch_to_finish_ns = 20; cpu_start = 2; cpu_end = 2; finished = $true },
      [pscustomobject][ordered]@{ sequence = 7; render_ns = 11; dispatch_to_finish_ns = 25; cpu_start = 3; cpu_end = 3; finished = $true }
    )
    coordinator = [pscustomobject][ordered]@{ sequence = 7; deadline_ns = 100; dispatch_to_deadline_start_ns = 10; dispatch_to_deadline_elapsed_ns = $null; in_flight_mask = 0; completed_mask = 3; first_parity = 0; dispatch_to_first_ns = 20; dispatch_to_both_ns = 25; reduction_ns = 4; coordinator_remainder_ns = 5; engine_block_total_ns = 40; callback_total_ns = 50; failed = $false; frozen = $true }
    late_after_deadline_ns = $null
    cpu_endpoint_changed = $false
  }
}
$workerTiming = New-TestWorkerTiming
$validationModule = Get-Module raspberry-live-benchmark-validation
if (-not (& $validationModule { param($Timing) Test-RaspberryLiveWorkerTimingClean $Timing } $workerTiming)) { throw "Frozen Raspberry worker timing evidence was not accepted as clean." }
$workerTiming.coordinator.frozen = $false
if (& $validationModule { param($Timing) Test-RaspberryLiveWorkerTimingClean $Timing } $workerTiming) { throw "Unfrozen Raspberry worker timing evidence was accepted as clean." }
function New-TestResult {
  param([string]$Status = "pass", [uint64]$OverrunCount = 0, [pscustomobject]$Selection = $inline, [uint64]$RepeatIncidents = 0, [uint64]$RepeatedPcmFrames = 0, [uint64]$SilentIncidents = 0, [uint64]$SilentPcmFrames = 0, [uint64]$CpalDeviceErrors = 0, [uint64]$CpalStreamErrors = 0)
  $isMulticore = $Selection.ExecutorMode -ceq "Multicore"
  [pscustomobject][ordered]@{
    schema_version = 13; kind = "raspberry_audio_benchmark_result"; status = $Status; board_profile = "raspberry-pi-zero-2w"; artifact_sha256 = $testHash; scenario = $Selection.Scenario; requested_output_buffer_frames = $Selection.OutputFrames; expected_alsa_buffer_frames = $Selection.OutputFrames; expected_alsa_period_frames = $Selection.AlsaPeriodFrames; internal_block_frames = $Selection.InternalFrames; lookahead_frames = $Selection.LookaheadFrames; effective_output_latency_frames = $Selection.EffectiveOutputLatencyFrames; sample_format = "F32"; pid = 1234; systemd_invocation_id = "invocation"; sample_rate = 44100; channels = 2; executor_mode = $Selection.NativeExecutorMode; worker_timing_mode = $Selection.WorkerTimingMode; warmup_seconds = 5; measure_seconds = $Selection.MeasureSeconds; scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; post_dsp_zero = $true; recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; continue_on_recovered_miss = [bool]$Selection.ContinueOnRecoveredMiss; persistent_output_provenance = [pscustomobject][ordered]@{ observable = $isMulticore; repeated_quantum_incidents = $RepeatIncidents; repeated_pcm_frames = $RepeatedPcmFrames; silent_quantum_incidents = $SilentIncidents; silent_pcm_frames = $SilentPcmFrames }; worker_health = if ($isMulticore) { "healthy" } else { "disabled" }; worker_thread_name_0 = if ($isMulticore) { "oct-dsp-tree-0" } else { "" }; worker_thread_name_1 = if ($isMulticore) { "oct-dsp-tree-1" } else { "" }; joined_workers = if ($isMulticore) { 2 } else { 0 }; retirement_error = $null; worker_timing = $null; detected_continuity_events = $OverrunCount; profile_start = New-TestProfile $Selection.Units; profile_end = New-TestProfile $Selection.Units; persistent_output_counters = New-TestCounters $Selection; callback = New-TestCallback $Selection $OverrunCount $CpalDeviceErrors $CpalStreamErrors
  }
}
function Write-TestEvidence {
  param([Parameter(Mandatory)][string]$Directory, [Parameter(Mandatory)][pscustomobject]$Result, [Parameter(Mandatory)][string]$StudyStatus, [string]$RestoreStatus = "0", [pscustomobject]$Selection = $inline, [string]$Journal = "")
  New-Item -ItemType Directory -Force -Path $Directory | Out-Null
  Set-Content (Join-Path $Directory "benchmark-identity.txt") "main_pid=1234`ninvocation_id=invocation" -Encoding UTF8
  $readiness = [pscustomobject][ordered]@{ schema_version = 5; kind = "raspberry_audio_benchmark_readiness"; status = "ready"; board_profile = "raspberry-pi-zero-2w"; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = $testHash; scenario = $Selection.Scenario; requested_output_buffer_frames = $Selection.OutputFrames; expected_alsa_buffer_frames = $Selection.OutputFrames; expected_alsa_period_frames = $Selection.AlsaPeriodFrames; internal_block_frames = $Selection.InternalFrames; lookahead_frames = $Selection.LookaheadFrames; sample_rate = 44100; channels = 2; sample_format = "F32"; scheduler_qualified = $true; post_dsp_zero = $true; executor_mode = $Selection.NativeExecutorMode; worker_health = if ($Selection.ExecutorMode -ceq "Multicore") { "healthy" } else { "disabled" }; worker_thread_name_0 = if ($Selection.ExecutorMode -ceq "Multicore") { "oct-dsp-tree-0" } else { "" }; worker_thread_name_1 = if ($Selection.ExecutorMode -ceq "Multicore") { "oct-dsp-tree-1" } else { "" }; callback_frames_min = $Selection.OutputFrames; callback_frames_max = $Selection.OutputFrames; callback_frame_sample_count = 10; invalid_callback_frame_count = 0 }
  $release = [pscustomobject][ordered]@{ schema_version = 2; kind = "raspberry_audio_benchmark_release"; status = "released"; board_profile = "raspberry-pi-zero-2w"; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = $testHash; scenario = $Selection.Scenario; expected_alsa_buffer_frames = $Selection.OutputFrames; observed_alsa_buffer_frames = $Selection.OutputFrames; expected_alsa_period_frames = $Selection.AlsaPeriodFrames; observed_alsa_period_frames = $Selection.AlsaPeriodFrames }
  $candidate = [pscustomobject][ordered]@{ schema_version = 1; kind = "octessera_candidate_readiness"; status = "ready"; pid = 2345; systemd_invocation_id = "restored-invocation"; package_version = "0.8.2"; board_profile = "raspberry-pi-zero-2w"; ready_at_unix_ms = 1700000000000 }
  $restored = "final_active=active`nfinal_enabled=enabled`nfinal_pid=2345`nfinal_invocation_id=restored-invocation`nrestore_status=$RestoreStatus"
  $readiness | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory "benchmark-readiness.json") -Encoding UTF8
  $release | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory "benchmark-release.json") -Encoding UTF8
  $Result | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Directory "benchmark-result.json") -Encoding UTF8
  $candidate | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory "candidate-ready.json") -Encoding UTF8
  Set-Content (Join-Path $Directory "service-restored-state.txt") $restored -Encoding UTF8
  Set-Content (Join-Path $Directory "study-result.txt") "status_class=$StudyStatus`ninterruption_started=true" -Encoding UTF8
  Set-Content (Join-Path $Directory "sensor-series.txt") "raspberry_system_sample phase=startup thermal_max_millicelsius=42000 mem_available_kb=100000 throttled=0x0 current_throttled_mask=0 undervoltage=0`nraspberry_system_sample phase=runtime thermal_max_millicelsius=43000 mem_available_kb=99000 throttled=0x0 current_throttled_mask=0 undervoltage=0" -Encoding UTF8
  Set-Content (Join-Path $Directory "alsa-hw-params.txt") "period_size: $($Selection.AlsaPeriodFrames)`nbuffer_size: $($Selection.OutputFrames)" -Encoding UTF8
  Set-Content (Join-Path $Directory "unit-journal.txt") $Journal -Encoding UTF8
}

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-raspberry-validation-" + [guid]::NewGuid().ToString("N"))
try {
  $failedResult = New-TestResult -Status fail -OverrunCount 5
  if (Test-RaspberryLiveBenchmarkClean $failedResult $inline) { throw "Non-clean Raspberry evidence was accepted as clean." }
  Assert-RaspberryLiveBenchmarkResult $failedResult $inline $testHash 1234 "invocation"
  $measuredRoot = Join-Path $testRoot "measured"
  Write-TestEvidence $measuredRoot $failedResult "measured_failure"
  $measured = Get-RaspberryLiveHostEvidence $measuredRoot $inline $testHash
  if ($measured.StatusClass -cne "measured_failure" -or $measured.PracticalGrade -cne "Stable" -or $measured.RepeatIncidents -ne 0 -or $measured.SilentIncidents -ne 0 -or $measured.AlsaRecoveryLogIncidents -ne 0) { throw "Structurally valid non-clean evidence was not measured_failure with the expected practical evidence." }
  $inlineObservationResult = New-TestResult -Status fail -Selection $inlineObservation -OverrunCount 5
  $inlineObservationRoot = Join-Path $testRoot "inline-observation"
  Write-TestEvidence $inlineObservationRoot $inlineObservationResult "measured_failure" "0" $inlineObservation
  $inlineObservationEvidence = Get-TestHostEvidenceWithoutErrors $inlineObservationRoot $inlineObservation $testHash
  if ($inlineObservationEvidence.StatusClass -cne "measured_failure" -or $inlineObservationEvidence.PracticalGrade -cne "Stable") { throw "Inline compromised observation did not remain completed measured evidence." }
  $multicoreResult = New-TestResult -Status fail -Selection $multicore -RepeatIncidents 2 -RepeatedPcmFrames 128 -SilentIncidents 0 -SilentPcmFrames 0
  Assert-RaspberryLiveBenchmarkResult $multicoreResult $multicore $testHash 1234 "invocation"
  $numericEvidence = [pscustomobject]@{ value = 0 }
  $invalidNumericIndex = 0
  foreach ($invalidNumeric in @("5", [double]5.0, [decimal]5.5, -1, $true, $null)) {
    $numericEvidence.value = $invalidNumeric
    Assert-Throws { & $validationModule { param($Evidence) Assert-RaspberryLiveUnsignedFields $Evidence @("value") "numeric evidence" } $numericEvidence } "invalid unsigned numeric evidence $invalidNumericIndex"
    $invalidNumericIndex++
  }
  $numericEvidence.value = [uint64]::MaxValue
  & $validationModule { param($Evidence) Assert-RaspberryLiveUnsignedFields $Evidence @("value") "numeric evidence" } $numericEvidence
  $numericEvidence.value = [decimal]([uint64]::MaxValue)
  & $validationModule { param($Evidence) Assert-RaspberryLiveUnsignedFields $Evidence @("value") "numeric evidence" } $numericEvidence
  $invalidSchema = $multicoreResult | ConvertTo-Json -Depth 10 | ConvertFrom-Json
  $invalidSchema.schema_version = 12
  Assert-Throws { Assert-RaspberryLiveBenchmarkResult $invalidSchema $multicore $testHash 1234 "invocation" } "schema 13 result"
  $invalidContinuation = $multicoreResult | ConvertTo-Json -Depth 10 | ConvertFrom-Json
  $invalidContinuation.continue_on_recovered_miss = $false
  Assert-Throws { Assert-RaspberryLiveBenchmarkResult $invalidContinuation $multicore $testHash 1234 "invocation" } "continuation identity"
  $invalidProvenance = $multicoreResult | ConvertTo-Json -Depth 10 | ConvertFrom-Json
  $invalidProvenance.persistent_output_provenance.observable = $false
  Assert-Throws { Assert-RaspberryLiveBenchmarkResult $invalidProvenance $multicore $testHash 1234 "invocation" } "persistent output provenance observability"
  foreach ($invalidPair in @(
    @{ Incidents = 0; Frames = 1 },
    @{ Incidents = 3; Frames = 2 }
  )) {
    $invalidPairResult = $multicoreResult | ConvertTo-Json -Depth 10 | ConvertFrom-Json
    $invalidPairResult.persistent_output_provenance.repeated_quantum_incidents = $invalidPair.Incidents
    $invalidPairResult.persistent_output_provenance.repeated_pcm_frames = $invalidPair.Frames
    Assert-Throws { Assert-RaspberryLiveBenchmarkResult $invalidPairResult $multicore $testHash 1234 "invocation" } "provenance incident/frame pair"
  }
  $invalidInlineProvenance = $failedResult | ConvertTo-Json -Depth 10 | ConvertFrom-Json
  $invalidInlineProvenance.persistent_output_provenance.repeated_quantum_incidents = 1
  Assert-Throws { Assert-RaspberryLiveBenchmarkResult $invalidInlineProvenance $inline $testHash 1234 "invocation" } "unobservable inline provenance"
  $invalidEpipe = $multicoreResult | ConvertTo-Json -Depth 10 | ConvertFrom-Json
  $invalidEpipe.recovered_alsa_epipe_observable = $true
  Assert-Throws { Assert-RaspberryLiveBenchmarkResult $invalidEpipe $multicore $testHash 1234 "invocation" } "unobservable EPIPE claim"
  $multicoreRoot = Join-Path $testRoot "multicore"
  Write-TestEvidence $multicoreRoot $multicoreResult "measured_failure" "0" $multicore "snd_pcm_recover: underrun occurred`nALSA lib pcm.c:123:(_snd_pcm_recover_with_limit) underrun occurred"
  $multicoreEvidence = Get-RaspberryLiveHostEvidence $multicoreRoot $multicore $testHash
  if ($multicoreEvidence.StatusClass -cne "measured_failure" -or $multicoreEvidence.PracticalGrade -cne "Stretched" -or $multicoreEvidence.RepeatIncidents -ne 2 -or $multicoreEvidence.RepeatedPcmFrames -ne 128 -or $multicoreEvidence.AlsaRecoveryLogIncidents -ne 2) { throw "Multicore observation evidence or conservative ALSA log count changed." }
  $deadlineResult = New-TestResult -Status fail -Selection $multicore
  $deadlineResult.worker_health = "deadline_miss"
  $deadlineRoot = Join-Path $testRoot "deadline-miss"
  Write-TestEvidence $deadlineRoot $deadlineResult "measured_failure" "0" $multicore
  $deadlineEvidence = Get-TestHostEvidenceWithoutErrors $deadlineRoot $multicore $testHash
  if ($deadlineEvidence.StatusClass -cne "measured_failure") { throw "A continued Multicore deadline miss was not retained as completed measured evidence." }
  $strictDeadlineResult = New-TestResult -Status fail -Selection $multicoreStrict
  $strictDeadlineResult.worker_health = "deadline_miss"
  $strictDeadlineRoot = Join-Path $testRoot "strict-deadline-miss"
  Write-TestEvidence $strictDeadlineRoot $strictDeadlineResult "measured_failure" "0" $multicoreStrict
  $strictDeadlineEvidence = Get-TestHostEvidenceWithoutErrors $strictDeadlineRoot $multicoreStrict $testHash
  if ($strictDeadlineEvidence.StatusClass -cne "infrastructure_failure") { throw "A non-continued Multicore deadline miss was accepted as measured evidence." }
  $terminalCases = @(
    @{ Name = "terminal-callback"; Mutate = { param($Result) $Result.callback.worker_terminal = $true } },
    @{ Name = "terminal-error"; Mutate = { param($Result) $Result.terminal_error = "worker terminated" } },
    @{ Name = "terminal-worker"; Mutate = { param($Result) $Result.worker_health = "worker_exited" } },
    @{ Name = "invalid-stream"; Mutate = { param($Result) $Result.stream_stopped = $false } }
  )
  foreach ($terminalCase in $terminalCases) {
    $terminalResult = New-TestResult -Status fail -Selection $multicore
    & $terminalCase.Mutate $terminalResult
    $terminalRoot = Join-Path $testRoot $terminalCase.Name
    Write-TestEvidence $terminalRoot $terminalResult "measured_failure" "0" $multicore
    $terminalEvidence = Get-TestHostEvidenceWithoutErrors $terminalRoot $multicore $testHash
    if ($terminalEvidence.StatusClass -cne "infrastructure_failure") { throw "Terminal or invalid lifecycle evidence was accepted as measured evidence: $($terminalCase.Name)." }
  }
  $gradeCases = @(
    @{ Name = "stable"; Result = New-TestResult -Selection $multicore; StudyStatus = "pass"; Journal = ""; Expected = "Stable" },
    @{ Name = "stretched"; Result = New-TestResult -Status fail -Selection $multicore -RepeatIncidents 2 -RepeatedPcmFrames 128 -CpalDeviceErrors 2; StudyStatus = "measured_failure"; Journal = "snd_pcm_recover: underrun occurred`nnot an ALSA recovery"; Expected = "Stretched" },
    @{ Name = "compromised"; Result = New-TestResult -Status fail -Selection $multicore -SilentIncidents 5 -SilentPcmFrames 320 -CpalStreamErrors 5; StudyStatus = "measured_failure"; Journal = "snd_pcm_recover: underrun occurred`nALSA lib pcm.c:123:(_snd_pcm_recover_with_limit) underrun occurred`nALSA lib pcm.c:124:(_snd_pcm_recover_with_limit) underrun occurred`nALSA lib pcm.c:125:(_snd_pcm_recover_with_limit) underrun occurred`nALSA lib pcm.c:126:(_snd_pcm_recover_with_limit) underrun occurred"; Expected = "Compromised" }
  )
  foreach ($gradeCase in $gradeCases) {
    $gradeRoot = Join-Path $testRoot $gradeCase.Name
    if ($gradeCase.Name -ceq "stable" -and -not (Test-RaspberryLiveBenchmarkClean $gradeCase.Result $multicore)) { throw "Synthetic stable Multicore result was not clean." }
    Write-TestEvidence $gradeRoot $gradeCase.Result $gradeCase.StudyStatus "0" $multicore $gradeCase.Journal
    $gradeEvidence = Get-RaspberryLiveHostEvidence $gradeRoot $multicore $testHash
    if ($gradeEvidence.PracticalGrade -cne $gradeCase.Expected) { throw "Raspberry practical grade changed for $($gradeCase.Name)." }
  }
  $compromisedObservation = Get-TestHostEvidenceWithoutErrors (Join-Path $testRoot "compromised") $multicore $testHash
  if ($compromisedObservation.StatusClass -cne "measured_failure" -or $compromisedObservation.PracticalGrade -cne "Compromised") { throw "Compromised observation was not retained as completed measured evidence." }
  foreach ($studyStatus in @("measured_failure", "pass")) {
    $retainedRoot = Join-Path $testRoot ("retained-" + $studyStatus)
    $retainedResult = if ($studyStatus -ceq "measured_failure") { $failedResult } else { New-TestResult }
    Write-TestEvidence $retainedRoot $retainedResult $studyStatus
    Copy-Item -LiteralPath (Join-Path $retainedRoot "benchmark-result.json") -Destination (Join-Path $retainedRoot "runtime-result.json")
    Remove-Item -LiteralPath (Join-Path $retainedRoot "runtime-result.json")
    $retained = Get-RaspberryLiveHostEvidence $retainedRoot $inline $testHash
    if ($retained.StatusClass -cne $studyStatus) { throw "Retained $studyStatus evidence changed after runtime result cleanup." }
  }
  $preStreamRoot = Join-Path $testRoot "pre-stream"
  $preStreamResult = New-TestResult -Status fail
  $preStreamResult.callback.callback_count = 0
  $preStreamResult.callback.callback_frame_sample_count = 0
  Write-TestEvidence $preStreamRoot $preStreamResult "measured_failure"
  $preStream = Get-RaspberryLiveHostEvidence $preStreamRoot $inline $testHash
  if ($preStream.StatusClass -cne "infrastructure_failure") { throw "An incomplete failed benchmark was accepted as measured evidence." }
  $statusMismatchRoot = Join-Path $testRoot "status-mismatch"
  Write-TestEvidence $statusMismatchRoot $failedResult "pass"
  $statusMismatch = Get-RaspberryLiveHostEvidence $statusMismatchRoot $inline $testHash
  if ($statusMismatch.StatusClass -cne "infrastructure_failure") { throw "A status disagreement inherited restoration failure." }
  $restorationRoot = Join-Path $testRoot "restoration"
  Write-TestEvidence $restorationRoot (New-TestResult) "pass" "1"
  $restoration = Get-RaspberryLiveHostEvidence $restorationRoot $inline $testHash
  if ($restoration.StatusClass -cne "restoration_failure") { throw "Restoration failure was not highest precedence." }
  $candidateRoot = Join-Path $testRoot "candidate-mismatch"
  Write-TestEvidence $candidateRoot (New-TestResult) "pass"
  $candidate = Get-Content (Join-Path $candidateRoot "candidate-ready.json") -Raw | ConvertFrom-Json
  $candidate.pid = 9999
  $candidate | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $candidateRoot "candidate-ready.json") -Encoding UTF8
  $candidateMismatch = Get-RaspberryLiveHostEvidence $candidateRoot $inline $testHash
  if ($candidateMismatch.StatusClass -cne "restoration_failure") { throw "Candidate identity mismatch was not a restoration failure." }
  $safetyRoot = Join-Path $testRoot "safety"
  Write-TestEvidence $safetyRoot $failedResult "safety_failure"
  Set-Content (Join-Path $safetyRoot "sensor-series.txt") "raspberry_system_sample phase=startup thermal_max_millicelsius=42000 mem_available_kb=100000 throttled=0x0 current_throttled_mask=0 undervoltage=0`nraspberry_system_sample phase=runtime thermal_max_millicelsius=43000 mem_available_kb=99000 throttled=0x1 current_throttled_mask=1 undervoltage=1`nraspberry_system_abort phase=runtime reason=undervoltage" -Encoding UTF8
  $safety = Get-RaspberryLiveHostEvidence $safetyRoot $inline $testHash
  if ($safety.StatusClass -cne "safety_failure") { throw "Safety failure was overwritten by result status." }
  $gateRoot = Join-Path $testRoot "runtime-gate"
  Write-TestEvidence $gateRoot $failedResult "safety_failure"
  Set-Content (Join-Path $gateRoot "sensor-abort.txt") "reason=runtime-sensor-gate" -Encoding UTF8
  $gate = Get-RaspberryLiveHostEvidence $gateRoot $inline $testHash
  if ($gate.StatusClass -cne "safety_failure") { throw "A runtime sensor gate was not retained as safety failure." }
  $malformedRoot = Join-Path $testRoot "malformed-safety"
  Write-TestEvidence $malformedRoot $failedResult "safety_failure"
  Set-Content (Join-Path $malformedRoot "sensor-series.txt") "raspberry_system_error phase=runtime reason=thermal_missing" -Encoding UTF8
  $malformed = Get-RaspberryLiveHostEvidence $malformedRoot $inline $testHash
  if ($malformed.StatusClass -cne "infrastructure_failure") { throw "Malformed sensor evidence was accepted as safety failure." }
  $missingRoot = Join-Path $testRoot "missing-safety"
  Write-TestEvidence $missingRoot $failedResult "safety_failure"
  Remove-Item (Join-Path $missingRoot "sensor-series.txt")
  $missingOutput = @(& { Get-RaspberryLiveHostEvidence $missingRoot $inline $testHash } 2>&1)
  if (@($missingOutput | Where-Object { $_ -is [System.Management.Automation.ErrorRecord] }).Count -ne 0) { throw "Missing sensor evidence emitted a raw PowerShell error." }
  $missing = @($missingOutput | Where-Object { $_ -isnot [System.Management.Automation.ErrorRecord] })[0]
  if ($missing.StatusClass -cne "infrastructure_failure" -or $missing.Reason -cne "Raspberry benchmark evidence is missing required file: sensor-series.txt.") { throw "Missing sensor evidence was not classified deterministically as infrastructure failure." }
  $alsaMismatchRoot = Join-Path $testRoot "alsa-mismatch"
  Write-TestEvidence $alsaMismatchRoot (New-TestResult) "pass"
  Set-Content (Join-Path $alsaMismatchRoot "alsa-hw-params.txt") "period_size: 32`nbuffer_size: 256" -Encoding UTF8
  $alsaMismatch = Get-TestHostEvidenceWithoutErrors $alsaMismatchRoot $inline $testHash
  if ($alsaMismatch.StatusClass -cne "infrastructure_failure" -or $alsaMismatch.Reason -ne "Raspberry benchmark raw ALSA evidence did not contain exact buffer_size 256 and period_size 64 rows.") { throw "Raw ALSA evidence was not validated exactly." }
  $preSudoRoot = Join-Path $testRoot "pre-sudo"
  New-Item -ItemType Directory -Force -Path $preSudoRoot | Out-Null
  Set-Content (Join-Path $preSudoRoot "study-result.txt") "mode=LiveAudioBenchmark`nstatus_class=infrastructure_failure`ninterruption_started=false`nreason=operator-sudo-authorization-unavailable" -Encoding UTF8
  $preSudo = Get-TestHostEvidenceWithoutErrors $preSudoRoot $inline $testHash
  if ($preSudo.StatusClass -cne "infrastructure_failure" -or $preSudo.Reason -cne "operator-sudo-authorization-unavailable") { throw "Pre-interruption sudo failure was not returned with its recorded reason." }
  foreach ($artifactName in @("study-result.txt", "service-restored-state.txt", "benchmark-identity.txt", "benchmark-result.json", "benchmark-readiness.json", "benchmark-release.json", "sensor-series.txt", "alsa-hw-params.txt", "unit-journal.txt", "candidate-ready.json")) {
    $missingArtifactRoot = Join-Path $testRoot ("missing-post-" + $artifactName.Replace(".", "-"))
    Write-TestEvidence $missingArtifactRoot (New-TestResult) "pass"
    Remove-Item -LiteralPath (Join-Path $missingArtifactRoot $artifactName)
    $missingArtifact = Get-TestHostEvidenceWithoutErrors $missingArtifactRoot $inline $testHash
    $expectedStatus = if ($artifactName -ceq "candidate-ready.json") { "restoration_failure" } else { "infrastructure_failure" }
    if ($missingArtifact.StatusClass -cne $expectedStatus -or $missingArtifact.Reason -cne "Raspberry benchmark evidence is missing required file: $artifactName.") { throw "Missing post-interruption artifact was not classified deterministically: $artifactName." }
  }
  $combinedRoot = Join-Path $testRoot "combined-safety-restoration"
  Write-TestEvidence $combinedRoot $failedResult "safety_failure"
  Set-Content (Join-Path $combinedRoot "sensor-series.txt") "raspberry_system_sample phase=startup thermal_max_millicelsius=42000 mem_available_kb=100000 throttled=0x0 current_throttled_mask=0 undervoltage=0`nraspberry_system_sample phase=runtime thermal_max_millicelsius=43000 mem_available_kb=99000 throttled=0x1 current_throttled_mask=1 undervoltage=1`nraspberry_system_abort phase=runtime reason=undervoltage" -Encoding UTF8
  $combinedCandidate = Get-Content (Join-Path $combinedRoot "candidate-ready.json") -Raw | ConvertFrom-Json
  $combinedCandidate.systemd_invocation_id = "stale-invocation"
  $combinedCandidate | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $combinedRoot "candidate-ready.json") -Encoding UTF8
  $combined = Get-RaspberryLiveHostEvidence $combinedRoot $inline $testHash
  if ($combined.StatusClass -cne "restoration_failure") { throw "Candidate mismatch did not override a safety abort." }
} finally {
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$binaryPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-binary-" + [guid]::NewGuid().ToString("N"))
try {
  [IO.File]::WriteAllText($binaryPath, "diagnostic binary")
  $sourceCommit = "0123456789abcdef0123456789abcdef01234567"
  $metadata = New-RaspberryLiveBenchmarkMetadata -SourceCommit $sourceCommit -BinaryPath $binaryPath
  Assert-RaspberryLiveBenchmarkMetadata -Metadata $metadata -SourceCommit $sourceCommit -BinaryPath $binaryPath | Out-Null
  if ($metadata.artifact_kind -cne "diagnostic-only" -or $metadata.cargo_feature -cne (Get-RaspberryLiveBenchmarkCargoFeature)) { throw "Diagnostic metadata contract changed." }
  $metadata.artifact_kind = "release"
  Assert-Throws { Assert-RaspberryLiveBenchmarkMetadata -Metadata $metadata -SourceCommit $sourceCommit -BinaryPath $binaryPath } "non-diagnostic artifact metadata"
} finally {
  Remove-Item -LiteralPath $binaryPath -Force -ErrorAction SilentlyContinue
}

$printOnly = Invoke-PrintOnly @{ Units = 16; ExecutorMode = "Inline"; MeasureSeconds = 30; PrintOnly = $true }
if ($printOnly -notmatch "no transport is invoked" -or $printOnly -notmatch "U16 scenario=capacity_analogue_16 executor=Inline output=256 period=64 internal=128 lookahead=0 worker-timing=disabled continue-on-recovered-miss=False measure=30 label=30-second screen") { throw "Inline PrintOnly output changed." }
$u8PrintOnly = Invoke-PrintOnly @{ Units = 8; ExecutorMode = "Inline"; MeasureSeconds = 30; PrintOnly = $true }
if ($u8PrintOnly -notmatch "U8 scenario=capacity_analogue_8 executor=Inline output=256 period=64 internal=128 lookahead=0 worker-timing=disabled continue-on-recovered-miss=False measure=30 label=30-second screen") { throw "U8 PrintOnly output changed." }
$multicorePrintOnly = Invoke-PrintOnly @{ Units = 32; ExecutorMode = "Multicore"; MeasureSeconds = 120; ObserveCompromises = $true; PrintOnly = $true }
if ($multicorePrintOnly -notmatch "U32 scenario=capacity_analogue_32 executor=Multicore output=256 period=64 internal=128 lookahead=128 worker-timing=disabled continue-on-recovered-miss=True measure=120 label=120-second repeat") { throw "Multicore PrintOnly output changed." }
Assert-Throws { & $runner -Units 16 } "missing explicit interruption consent"
if ($runnerSource.IndexOf("ObserveCompromises", [StringComparison]::Ordinal) -lt 0 -or $runnerSource.IndexOf("completed observation", [StringComparison]::Ordinal) -lt 0 -or $runnerSource.IndexOf('StatusClass -ne "pass" -and -not ($ObserveCompromises', [StringComparison]::Ordinal) -lt 0) { throw "Raspberry observation mode does not retain completed compromised runs." }

foreach ($required in @(
  "with-pi-ssh.ps1",
  "systemd-run",
  "sndrpihifiberry/pcm0p/sub0/hw_params",
  "vcgencmd get_throttled",
  "restore_service",
  "restoration_ready=false",
  "json_field kind",
  "json_field status",
  "diagnostic-only"
)) {
  if ($runnerSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Runner is missing required contract: $required" }
}
foreach ($required in @("__OUTPUT_FRAMES__", "__ALSA_PERIOD_FRAMES__", "__INTERNAL_FRAMES__", "__CONTINUE_ON_RECOVERED_MISS__")) {
  if ($runnerSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Runner is missing executor-owned geometry or continuation contract: $required" }
}
if ($runnerSource -match '--output-frames 256|--engine-block-frames 128|\[ "\$buffer" = 256 \]|\[ "\$period" = 64 \]') { throw "Raspberry runner retains hard-coded benchmark geometry." }
if ($runnerSource -match "with-orange-ssh|orange_audio_benchmark|fallback") { throw "Raspberry runner contains Orange or fallback behavior." }
$preflightIndex = $runnerSource.IndexOf("if ! sudo -n -v >/dev/null 2>&1", [StringComparison]::Ordinal)
$serviceReadIndex = $runnerSource.IndexOf('initial_active="$(sudo -n systemctl', [StringComparison]::Ordinal)
if ($preflightIndex -lt 0 -or $serviceReadIndex -lt 0 -or $preflightIndex -ge $serviceReadIndex) { throw "Raspberry runner does not preflight sudo before service-state reads." }
foreach ($required in @(
  "reason=operator-sudo-authorization-unavailable",
  "interruption_started=false",
  'privileged_status=`$?',
  'unprivileged_status=`$?',
  '[ "`$privileged_status" -eq 0 ] && [ "`$unprivileged_status" -eq 0 ]'
)) {
  if ($runnerSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Raspberry runner is missing required failure-path contract: $required" }
}
$cleanupStart = $runnerSource.IndexOf('$cleanupContents', [StringComparison]::Ordinal)
$cleanupEnd = $runnerSource.IndexOf('$cleanupPath =', $cleanupStart, [StringComparison]::Ordinal)
if ($cleanupStart -lt 0 -or $cleanupEnd -le $cleanupStart) { throw "Raspberry runner cleanup payload was not found." }
$cleanupSource = $runnerSource.Substring($cleanupStart, $cleanupEnd - $cleanupStart)
if ($cleanupSource -notmatch '(?s)set \+e.*sudo -n rm -rf -- /run/octessera/raspberry-live-\$runId.*rm -rf --.*privileged_status.*unprivileged_status') { throw "Raspberry cleanup does not attempt both exact roots after one failure." }
if ($cleanupSource -match 'rm -rf[^\r\n]*\*') { throw "Raspberry cleanup uses wildcard deletion." }
if ($runnerSource.Contains('$hostEvidence.StatusClass = "restoration_failure"')) { throw "Cleanup failure is incorrectly classified as restoration failure." }
if ($runnerSource -match '\$hostEvidence = if \(Test-Path') { throw "Raspberry runner synthesizes missing host evidence." }
$stopServiceIndex = $runnerSource.IndexOf('sudo -n systemctl stop "$service"', [StringComparison]::Ordinal)
$runtimeDirectoryIndex = $runnerSource.IndexOf('sudo -n install -d -o pi -g pi -m 0755 /run/octessera', [StringComparison]::Ordinal)
$benchmarkDirectoryIndex = $runnerSource.IndexOf('sudo -n install -d -o pi -g pi -m 0750 "$benchmark_root"', [StringComparison]::Ordinal)
$systemdRunIndex = $runnerSource.IndexOf('sudo -n systemd-run --unit="$unit"', [StringComparison]::Ordinal)
if ($stopServiceIndex -lt 0 -or $runtimeDirectoryIndex -lt 0 -or $benchmarkDirectoryIndex -lt 0 -or $systemdRunIndex -lt 0 -or $stopServiceIndex -ge $runtimeDirectoryIndex -or $runtimeDirectoryIndex -ge $benchmarkDirectoryIndex -or $benchmarkDirectoryIndex -ge $systemdRunIndex) { throw "Raspberry runtime directory recreation is not ordered after service stop and before systemd-run." }
foreach ($required in @(
  'sudo -n install -o pi -g pi -m 0640 -- "$candidate_readiness" "$candidate_copy"',
  'reset-failed "$unit"',
  'unit=octessera-raspberry-live-$runId.service'
)) {
  if ($runnerSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Raspberry runner is missing lifecycle ownership contract: $required" }
}
$onExitIndex = $runnerSource.IndexOf('on_exit() {', [StringComparison]::Ordinal)
$captureStatusIndex = $runnerSource.IndexOf('sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=SubState', $onExitIndex, [StringComparison]::Ordinal)
$captureJournalIndex = $runnerSource.IndexOf('sudo -n journalctl -u "$unit"', $onExitIndex, [StringComparison]::Ordinal)
$evidenceCopyIndex = $runnerSource.IndexOf('[ -r "$readiness" ] && cp', $onExitIndex, [StringComparison]::Ordinal)
$resetCallIndex = $runnerSource.IndexOf('  reset_transient_unit', $onExitIndex, [StringComparison]::Ordinal)
$restoreIndex = $runnerSource.IndexOf('  if [ "$interruption_started" = true ]', $onExitIndex, [StringComparison]::Ordinal)
if ($onExitIndex -lt 0 -or $captureStatusIndex -lt 0 -or $captureJournalIndex -lt 0 -or $evidenceCopyIndex -lt 0 -or $resetCallIndex -lt 0 -or $restoreIndex -lt 0 -or $captureStatusIndex -ge $captureJournalIndex -or $captureJournalIndex -ge $evidenceCopyIndex -or $evidenceCopyIndex -ge $resetCallIndex -or $resetCallIndex -ge $restoreIndex) { throw "Raspberry transient-unit evidence, reset, and restoration order changed." }
if ($runnerSource -match 'reset-failed[^\r\n]*\*') { throw "Raspberry transient-unit reset used a wildcard." }
if ($runnerSource.IndexOf('"0x$hex"', [StringComparison]::Ordinal) -lt 0 -or $runnerSource.IndexOf('"$throttled" "$mask"', [StringComparison]::Ordinal) -ge 0) { throw "Raspberry sensor output does not retain the exact normalized throttled value." }
if ($runnerSource.IndexOf('sudo -n install -o pi -g pi -m 0640 -- "$path" "$root/alsa-hw-params.txt"', [StringComparison]::Ordinal) -lt 0) { throw "Raspberry ALSA raw evidence is not installed with explicit ownership and mode." }
$classificationIndex = $runnerSource.IndexOf('local class=infrastructure_failure', [StringComparison]::Ordinal)
$restoreCallIndex = $runnerSource.IndexOf('if [ "$interruption_started" = true ]; then restore_service', [StringComparison]::Ordinal)
$restorationOverrideIndex = $runnerSource.IndexOf('[ "$restore_status" -ne 0 ] && class=restoration_failure', [StringComparison]::Ordinal)
if ($classificationIndex -lt 0 -or $restoreCallIndex -lt 0 -or $restorationOverrideIndex -lt 0 -or $classificationIndex -ge $restoreCallIndex -or $restoreCallIndex -ge $restorationOverrideIndex) { throw "Raspberry result classification is not retained before restoration and overridden afterward." }
foreach ($required in @(
  '[ "$status" -eq 20 ]',
  '[ "$status" -eq 0 ]',
  'retained_result="$root/benchmark-result.json"',
  '$retrievalFailure = $null',
  'catch { $retrievalFailure = $_ }',
  '$hostEvidence.StatusClass = "infrastructure_failure"; $hostEvidence.Reason = $retrievalFailure.Exception.Message'
)) {
  if ($runnerSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Raspberry runner is missing retained evidence failure contract: $required" }
}
if ($runnerSource -match '(?s)\$null -ne \$retrievalFailure.*?\$hostEvidence\.StatusClass -ceq "restoration_failure"') { throw "Raspberry retrieval failure can override restoration failure." }
$terminalStart = $runnerSource.IndexOf('wait_for_terminal() {', [StringComparison]::Ordinal)
$terminalResultIndex = $runnerSource.IndexOf('if [ -r "$result" ] && ! sudo -n systemctl is-active --quiet "$unit"', $terminalStart, [StringComparison]::Ordinal)
$terminalPidIndex = $runnerSource.IndexOf('pid="$(unit_pid)"', $terminalStart, [StringComparison]::Ordinal)
$terminalInvocationIndex = $runnerSource.IndexOf('[ "$(unit_invocation)" = "$benchmark_invocation" ]', $terminalStart, [StringComparison]::Ordinal)
if ($terminalStart -lt 0 -or $terminalResultIndex -lt 0 -or $terminalPidIndex -lt 0 -or $terminalInvocationIndex -lt 0 -or $terminalResultIndex -ge $terminalPidIndex -or $terminalPidIndex -ge $terminalInvocationIndex) { throw "Raspberry terminal result collection does not precede live identity checks." }

$bash = Get-Command bash -ErrorAction SilentlyContinue
$wsl = Get-Command wsl.exe -ErrorAction SilentlyContinue
$payloadMatch = [regex]::Match($runnerSource, '(?s)\$body = @''(.*?)''@')
if (-not $payloadMatch.Success) { throw "Raspberry runner payload was not found." }
$payloadTemplate = $payloadMatch.Groups[1].Value
function Resolve-TestPayload {
  param([Parameter(Mandatory)][string]$Template, [Parameter(Mandatory)][pscustomobject]$Selection)
  return $Template.Replace("__SCENARIO__", $Selection.Scenario).Replace("__EXECUTOR__", $Selection.NativeExecutorMode).Replace("__OUTPUT_FRAMES__", [string]$Selection.OutputFrames).Replace("__INTERNAL_FRAMES__", [string]$Selection.InternalFrames).Replace("__WORKER_TIMING__", $Selection.WorkerTimingMode).Replace("__CONTINUE_ON_RECOVERED_MISS__", $Selection.ContinueOnRecoveredMissArgument).Replace("__ALSA_PERIOD_FRAMES__", [string]$Selection.AlsaPeriodFrames)
}
$inlinePayload = Resolve-TestPayload $payloadTemplate $inline
$multicorePayload = Resolve-TestPayload $payloadTemplate $multicore
if ($inlinePayload -notmatch '--executor inline --scenario capacity_analogue_16 --output-frames 256 --engine-block-frames 128 --worker-timing disabled\s+--warmup-seconds' -or $inlinePayload -match '--continue-on-recovered-miss') { throw "Inline Raspberry command geometry or continuation changed." }
if ($multicorePayload -notmatch '--executor routing_tree_persistent --scenario capacity_analogue_32 --output-frames 256 --engine-block-frames 128 --worker-timing disabled\s+--continue-on-recovered-miss --warmup-seconds') { throw "Multicore Raspberry command geometry or continuation changed." }
if (($null -ne $bash -and [string]$bash.Source -notmatch "WindowsApps") -or $null -ne $wsl) {
  $payloadPath = [IO.Path]::GetTempFileName()
  try {
    [IO.File]::WriteAllText($payloadPath, $payloadMatch.Groups[1].Value)
    if ($null -ne $bash -and [string]$bash.Source -notmatch "WindowsApps") {
      & bash -n $payloadPath
    } else {
      $drive = $payloadPath.Substring(0, 1).ToLowerInvariant()
      $wslPath = "/mnt/$drive" + ($payloadPath.Substring(2) -replace "\\", "/")
      & wsl.exe bash -n $wslPath
    }
    if ($LASTEXITCODE -ne 0) { throw "Generated Raspberry live benchmark payload failed bash -n." }
  } finally {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
}

Write-Output "Raspberry live audio benchmark selection, metadata, consent, isolation, safety, and runner tests passed"
