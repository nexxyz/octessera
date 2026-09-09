$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$runner = Join-Path $PSScriptRoot "run-pi-live-audio-benchmark.ps1"
$runnerSource = [IO.File]::ReadAllText($runner)
Import-Module (Join-Path $PSScriptRoot "raspberry-live-benchmark-validation.psm1") -Force

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action, [Parameter(Mandatory)][string]$Label)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected failure did not occur: $Label" }
}

function New-TestProfile {
  param([Parameter(Mandatory)][int]$Units)
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

function New-TestCounters {
  $new = { [pscustomobject][ordered]@{ rendered_quantums = 0; repeated_quantums = 0; dropped_quantums = 0; deadline_misses = 0; deadline_recoveries = 0 } }
  [pscustomobject][ordered]@{ observable = $false; warmup = (& $new); start = (& $new); end = (& $new); delta = (& $new) }
}

function New-TestResult {
  param(
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [string]$Status = "fail",
    [uint64]$RepeatIncidents = 0,
    [uint64]$RepeatedPcmFrames = 0,
    [uint64]$SilentIncidents = 0,
    [uint64]$SilentPcmFrames = 0
  )
  $multicore = $Selection.ExecutorMode -ceq "Multicore"
  $frames = $Selection.OutputFrames
  [pscustomobject][ordered]@{
    schema_version = 13
    kind = "raspberry_audio_benchmark_result"
    status = $Status
    board_profile = "raspberry-pi-zero-2w"
    artifact_sha256 = "a" * 64
    scenario = $Selection.Scenario
    requested_output_buffer_frames = $frames
    expected_alsa_buffer_frames = $frames
    expected_alsa_period_frames = $Selection.AlsaPeriodFrames
    internal_block_frames = $Selection.InternalFrames
    lookahead_frames = $Selection.LookaheadFrames
    effective_output_latency_frames = $Selection.EffectiveOutputLatencyFrames
    sample_format = "F32"
    pid = 1234
    systemd_invocation_id = "invocation"
    sample_rate = 44100
    channels = 2
    executor_mode = $Selection.NativeExecutorMode
    worker_timing_mode = "disabled"
    warmup_seconds = 5
    measure_seconds = $Selection.MeasureSeconds
    scheduler_qualified = $true
    callback_scheduling_policy = "SCHED_FIFO"
    callback_scheduling_priority = 70
    callback_scheduling_cpu = 1
    measurement_stop_acknowledged = $true
    stream_stopped = $true
    final_progress_write_succeeded = $true
    post_dsp_zero = $true
    recovered_alsa_epipe_count = $null
    recovered_alsa_epipe_observable = $false
    terminal_error = $null
    continue_on_recovered_miss = [bool]$Selection.ContinueOnRecoveredMiss
    persistent_output_provenance = [pscustomobject][ordered]@{ observable = $multicore; repeated_quantum_incidents = $RepeatIncidents; repeated_pcm_frames = $RepeatedPcmFrames; silent_quantum_incidents = $SilentIncidents; silent_pcm_frames = $SilentPcmFrames }
    worker_health = if ($multicore) { "healthy" } else { "disabled" }
    worker_thread_name_0 = if ($multicore) { "oct-dsp-tree-0" } else { "" }
    worker_thread_name_1 = if ($multicore) { "oct-dsp-tree-1" } else { "" }
    joined_workers = if ($multicore) { 2 } else { 0 }
    retirement_error = $null
    worker_timing = $null
    detected_continuity_events = 0
    profile_start = New-TestProfile $Selection.Units
    profile_end = New-TestProfile $Selection.Units
    persistent_output_counters = New-TestCounters
    callback = [pscustomobject][ordered]@{
      lifetime_callback_count = 10; callback_count = 10; first_measured_callback_ns = 1; last_measured_callback_ns = 10; measured_elapsed_ns = 9; callback_frames_min = $frames; callback_frames_max = $frames; callback_frame_sample_count = 10; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0; lifetime_callback_frames_min = $frames; lifetime_callback_frames_max = $frames; lifetime_callback_frame_sample_count = 10; lifetime_callback_frame_size_change_count = 0; lifetime_invalid_callback_frame_count = 0; rendered_frames = 10 * $frames; render_audio_duration_ns = 100; over_audio_duration_budget_count = 0; render_audio_duration_ratio_p50 = 0; render_audio_duration_ratio_p95 = 0; render_audio_duration_ratio_p99 = 0; render_audio_duration_ratio_p99_9 = 0; render_audio_duration_ratio_max = 0; callback_spacing_min_ns = 1; callback_spacing_max_ns = 1; callback_lateness_max_ns = 0; pre_mute_nonzero_samples = 10; pre_mute_peak = 1; post_mute_nonzero_samples = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; callback_timestamp_observed = $true; worker_terminal = $false; terminal_error = $false
    }
  }
}

function Write-TestEvidence {
  param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][pscustomobject]$Result,
    [string]$StudyStatus = "measured_failure",
    [int]$ExitCode = 20,
    [int]$RestoreStatus = 0
  )
  New-Item -ItemType Directory -Force -Path $Directory | Out-Null
  Set-Content -LiteralPath (Join-Path $Directory "benchmark-identity.txt") -Value "main_pid=1234`ninvocation_id=invocation" -Encoding UTF8
  $readiness = [pscustomobject][ordered]@{ schema_version = 5; kind = "raspberry_audio_benchmark_readiness"; status = "ready"; board_profile = "raspberry-pi-zero-2w"; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = "a" * 64; scenario = $Selection.Scenario; requested_output_buffer_frames = $Selection.OutputFrames; expected_alsa_buffer_frames = $Selection.OutputFrames; expected_alsa_period_frames = $Selection.AlsaPeriodFrames; internal_block_frames = $Selection.InternalFrames; lookahead_frames = $Selection.LookaheadFrames; sample_rate = 44100; channels = 2; sample_format = "F32"; scheduler_qualified = $true; post_dsp_zero = $true; executor_mode = $Selection.NativeExecutorMode; callback_frames_min = $Selection.OutputFrames; callback_frames_max = $Selection.OutputFrames; callback_frame_sample_count = 10; invalid_callback_frame_count = 0 }
  $release = [pscustomobject][ordered]@{ schema_version = 2; kind = "raspberry_audio_benchmark_release"; status = "released"; board_profile = "raspberry-pi-zero-2w"; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = "a" * 64; scenario = $Selection.Scenario; expected_alsa_buffer_frames = $Selection.OutputFrames; observed_alsa_buffer_frames = $Selection.OutputFrames; expected_alsa_period_frames = $Selection.AlsaPeriodFrames; observed_alsa_period_frames = $Selection.AlsaPeriodFrames }
  $candidate = [pscustomobject][ordered]@{ schema_version = 1; kind = "octessera_candidate_readiness"; status = "ready"; pid = 2345; systemd_invocation_id = "restored-invocation"; package_version = "0.8.2"; board_profile = "raspberry-pi-zero-2w"; ready_at_unix_ms = 1700000000000 }
  $restored = "final_active=active`nfinal_enabled=enabled`nfinal_pid=2345`nfinal_invocation_id=restored-invocation`nrestore_status=$RestoreStatus"
  $readiness | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Directory "benchmark-readiness.json") -Encoding UTF8
  $release | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Directory "benchmark-release.json") -Encoding UTF8
  $Result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $Directory "benchmark-result.json") -Encoding UTF8
  $candidate | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Directory "candidate-ready.json") -Encoding UTF8
  Set-Content -LiteralPath (Join-Path $Directory "service-restored-state.txt") -Value $restored -Encoding UTF8
  Set-Content -LiteralPath (Join-Path $Directory "study-result.txt") -Value "status_class=$StudyStatus`nstatus=$ExitCode`ninterruption_started=true" -Encoding UTF8
  Set-Content -LiteralPath (Join-Path $Directory "sensor-series.txt") -Value "raspberry_system_sample phase=startup thermal_max_millicelsius=42000 mem_available_kb=100000 throttled=0x0 current_throttled_mask=0 undervoltage=0`nraspberry_system_sample phase=runtime thermal_max_millicelsius=43000 mem_available_kb=99000 throttled=0x0 current_throttled_mask=0 undervoltage=0" -Encoding UTF8
  Set-Content -LiteralPath (Join-Path $Directory "alsa-hw-params.txt") -Value "period_size: $($Selection.AlsaPeriodFrames)`nbuffer_size: $($Selection.OutputFrames)" -Encoding UTF8
  Set-Content -LiteralPath (Join-Path $Directory "unit-journal.txt") -Value "" -Encoding UTF8
}

$expectedProfiles = @{
  RI64 = [pscustomobject]@{ ExecutorMode = "Inline"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 256 }
  RI128 = [pscustomobject]@{ ExecutorMode = "Inline"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 256 }
  RI512 = [pscustomobject]@{ ExecutorMode = "Inline"; OutputFrames = 512; AlsaPeriodFrames = 128; InternalFrames = 128; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 512 }
  RM64 = [pscustomobject]@{ ExecutorMode = "Multicore"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 64; EffectiveOutputLatencyFrames = 320 }
  RM128 = [pscustomobject]@{ ExecutorMode = "Multicore"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 128; EffectiveOutputLatencyFrames = 384 }
  RM256 = [pscustomobject]@{ ExecutorMode = "Multicore"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 256; LookaheadFrames = 256; EffectiveOutputLatencyFrames = 512 }
}
if ($runnerSource -notmatch 'ValidateSet\("RI64", "RI128", "RI512", "RM64", "RM128", "RM256"\)') { throw "Raspberry runner frame-search profile ValidateSet changed." }
if ($runnerSource -notmatch 'ValidateSet\("Preliminary", "Soak"\)') { throw "Raspberry runner frame-search phase ValidateSet changed." }

foreach ($name in $expectedProfiles.Keys) {
  $units = if ($name -ceq "RI64") { 1 } elseif ($name -ceq "RM256") { 42 } else { 12 }
  $expected = $expectedProfiles[$name]
  foreach ($phase in @("Preliminary", "Soak")) {
    $seconds = if ($phase -ceq "Preliminary") { 180 } else { 600 }
    $selection = Assert-RaspberryLiveBenchmarkSelection -Units $units -FrameSearchProfile $name -FrameSearchPhase $phase -MeasureSeconds $seconds
    if ($selection.FrameSearchProfile -cne $name -or $selection.FrameSearchPhase -cne $phase -or $selection.Units -ne $units -or $selection.FrameSearchU -ne $units -or $selection.ExecutorMode -cne $expected.ExecutorMode -or $selection.NativeExecutorMode -cne $(if ($expected.ExecutorMode -ceq "Inline") { "inline" } else { "routing_tree_persistent" }) -or $selection.OutputFrames -ne $expected.OutputFrames -or $selection.AlsaPeriodFrames -ne $expected.AlsaPeriodFrames -or $selection.InternalFrames -ne $expected.InternalFrames -or $selection.LookaheadFrames -ne $expected.LookaheadFrames -or $selection.EffectiveOutputLatencyFrames -ne $expected.EffectiveOutputLatencyFrames -or $selection.MeasureSeconds -ne $seconds -or $selection.PhysicalRuns -ne 1 -or $selection.GeometryIdentity -cne "output=$($expected.OutputFrames),period=$($expected.AlsaPeriodFrames),internal=$($expected.InternalFrames),lookahead=$($expected.LookaheadFrames),effective=$($expected.EffectiveOutputLatencyFrames)" -or $selection.FrameSearchGeometry.OutputFrames -ne $expected.OutputFrames -or $selection.FrameSearchGeometry.AlsaPeriodFrames -ne $expected.AlsaPeriodFrames -or $selection.FrameSearchGeometry.InternalFrames -ne $expected.InternalFrames -or $selection.FrameSearchGeometry.LookaheadFrames -ne $expected.LookaheadFrames -or $selection.FrameSearchGeometry.EffectiveOutputLatencyFrames -ne $expected.EffectiveOutputLatencyFrames) { throw "Raspberry frame-search selection changed for $name $phase." }
    if ($selection.ContinueOnRecoveredMiss -ne ($expected.ExecutorMode -ceq "Multicore")) { throw "Raspberry frame-search continuation changed for $name." }
  }
}

$domainSelection = Assert-RaspberryFrameSearchSelection -FrameSearchProfile RM128 -FrameSearchPhase Preliminary -FrameSearchU 1
if ($domainSelection.FrameSearchProfile -cne "RM128" -or $domainSelection.FrameSearchU -ne 1 -or $domainSelection.FrameSearchGeometry.LookaheadFrames -ne 128) { throw "Raspberry frame-search domain selection API changed." }
$derivedSoak = Assert-RaspberryLiveBenchmarkSelection -Units 42 -FrameSearchProfile RM256 -FrameSearchPhase Soak
if ($derivedSoak.MeasureSeconds -ne 600) { throw "Raspberry frame-search phase did not derive its exact duration." }
Assert-Throws { Assert-RaspberryFrameSearchSelection -FrameSearchProfile RI64 -FrameSearchPhase Preliminary -FrameSearchU 1 -OutputFrames 512 } "frame-search output geometry mismatch"
Assert-Throws { Assert-RaspberryFrameSearchSelection -FrameSearchProfile RM64 -FrameSearchPhase Preliminary -FrameSearchU 1 -InternalFrames 128 } "frame-search internal geometry mismatch"

Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 12 -FrameSearchProfile RI64 -MeasureSeconds 180 } "missing frame-search phase"
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 12 -FrameSearchPhase Preliminary -MeasureSeconds 180 } "missing frame-search profile"
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 12 -FrameSearchProfile RI64 -FrameSearchPhase Preliminary -MeasureSeconds 600 } "preliminary duration"
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 12 -FrameSearchProfile RI64 -FrameSearchPhase Soak -MeasureSeconds 180 } "soak duration"
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 12 -FrameSearchProfile RI64 -FrameSearchPhase Preliminary -MeasureSeconds 180 -ExecutorMode Multicore } "frame-search executor mismatch"
foreach ($invalidUnits in @(0, 43, "12", [double]12.5)) {
  Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units $invalidUnits -FrameSearchProfile RI64 -FrameSearchPhase Preliminary -MeasureSeconds 180 } "invalid frame-search Units $invalidUnits"
}
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 0 -ExecutorMode Inline -MeasureSeconds 30 } "legacy zero Units"
Assert-Throws { Assert-RaspberryLiveBenchmarkSelection -Units 43 -ExecutorMode Inline -MeasureSeconds 30 } "legacy out-of-range Units"

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-raspberry-frame-search-" + [guid]::NewGuid().ToString("N"))
try {
  $preliminary = Assert-RaspberryLiveBenchmarkSelection -Units 1 -FrameSearchProfile RI128 -FrameSearchPhase Preliminary -MeasureSeconds 180
  $failed = New-TestResult $preliminary
  $retainedRoot = Join-Path $testRoot "retained"
  Write-TestEvidence $retainedRoot $preliminary $failed
  $retained = Get-RaspberryLiveHostEvidence $retainedRoot $preliminary ("a" * 64)
  if ($retained.StatusClass -cne "measured_failure" -or $retained.FrameSearchProfile -cne "RI128" -or $retained.FrameSearchPhase -cne "Preliminary" -or $retained.FrameSearchU -ne 1 -or $retained.FrameSearchGeometry.InternalFrames -ne 128 -or $retained.GeometryIdentity -cne $preliminary.GeometryIdentity -or $retained.PracticalGrade -cne "Stable") { throw "A valid frame-search exit20 observation was not retained with exact host identity." }
  $wrongExitRoot = Join-Path $testRoot "wrong-exit"
  Write-TestEvidence $wrongExitRoot $preliminary $failed -ExitCode 19
  $wrongExit = Get-RaspberryLiveHostEvidence $wrongExitRoot $preliminary ("a" * 64)
  if ($wrongExit.StatusClass -cne "infrastructure_failure") { throw "A non-exit20 frame-search failure was retained." }

  foreach ($secondsAndExpected in @(@(120, "Stable"), @(180, "Stable"), @(600, "Stable"))) {
    $seconds = $secondsAndExpected[0]
    $grade = if ($seconds -eq 120) { Get-RaspberryLivePracticalGrade -RepeatIncidents 1 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds $seconds } elseif ($seconds -eq 180) { Get-RaspberryLivePracticalGrade -RepeatIncidents 2 -SilentIncidents 2 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds $seconds } else { Get-RaspberryLivePracticalGrade -RepeatIncidents 9 -SilentIncidents 9 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds $seconds }
    if ($grade -cne $secondsAndExpected[1]) { throw "Raspberry duration grade did not use the worst incident category for $seconds seconds." }
  }
  if ((Get-RaspberryLivePracticalGrade -RepeatIncidents 2 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 180) -cne "Stable" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 3 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 180) -cne "Stretched" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 8 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 180) -cne "Compromised" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 9 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 600) -cne "Stable" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 10 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 600) -cne "Stretched" -or (Get-RaspberryLivePracticalGrade -RepeatIncidents 25 -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 600) -cne "Compromised") { throw "Raspberry frame-search duration grade thresholds changed." }

  $restorationRoot = Join-Path $testRoot "restoration"
  Write-TestEvidence $restorationRoot $preliminary $failed -RestoreStatus 1
  if ((Get-RaspberryLiveHostEvidence $restorationRoot $preliminary ("a" * 64)).StatusClass -cne "restoration_failure") { throw "Restoration failure did not take precedence over a retained frame-search observation." }
  $identityRoot = Join-Path $testRoot "identity"
  Write-TestEvidence $identityRoot $preliminary $failed
  $identityResult = Get-Content -LiteralPath (Join-Path $identityRoot "benchmark-result.json") -Raw | ConvertFrom-Json
  $identityResult.artifact_sha256 = "b" * 64
  $identityResult | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $identityRoot "benchmark-result.json") -Encoding UTF8
  if ((Get-RaspberryLiveHostEvidence $identityRoot $preliminary ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Identity failure was not fatal." }
  $terminalRoot = Join-Path $testRoot "terminal"
  $terminal = New-TestResult $preliminary
  $terminal.callback.worker_terminal = $true
  Write-TestEvidence $terminalRoot $preliminary $terminal
  if ((Get-RaspberryLiveHostEvidence $terminalRoot $preliminary ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Terminal failure was not fatal." }
  $malformedRoot = Join-Path $testRoot "malformed"
  Write-TestEvidence $malformedRoot $preliminary $failed
  $malformed = Get-Content -LiteralPath (Join-Path $malformedRoot "benchmark-result.json") -Raw | ConvertFrom-Json
  $malformed.schema_version = 12
  $malformed | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $malformedRoot "benchmark-result.json") -Encoding UTF8
  if ((Get-RaspberryLiveHostEvidence $malformedRoot $preliminary ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Malformed failure was not fatal." }
  $safetyRoot = Join-Path $testRoot "safety"
  Write-TestEvidence $safetyRoot $preliminary $failed "safety_failure" 75
  Set-Content -LiteralPath (Join-Path $safetyRoot "sensor-series.txt") -Value "raspberry_system_sample phase=startup thermal_max_millicelsius=42000 mem_available_kb=100000 throttled=0x0 current_throttled_mask=0 undervoltage=0`nraspberry_system_sample phase=runtime thermal_max_millicelsius=43000 mem_available_kb=99000 throttled=0x1 current_throttled_mask=1 undervoltage=1`nraspberry_system_abort phase=runtime reason=undervoltage" -Encoding UTF8
  if ((Get-RaspberryLiveHostEvidence $safetyRoot $preliminary ("a" * 64)).StatusClass -ne "safety_failure") { throw "Safety failure was not fatal." }
} finally {
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$printArguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $runner, "-Units", "42", "-FrameSearchProfile", "RM256", "-FrameSearchPhase", "Soak", "-PrintOnly")
$printOutput = @(& (Join-Path $PSHOME "powershell.exe") @printArguments 2>&1)
$printText = ($printOutput | ForEach-Object { [string]$_ }) -join "`n"
if ($LASTEXITCODE -ne 0 -or $printText -notmatch "U42 profile=RM256 phase=Soak scenario=capacity_analogue_42 executor=Multicore output=256 period=64 internal=256 lookahead=256 effective=512 worker-timing=disabled continue-on-recovered-miss=True measure=600 label=600-second soak") { throw "Raspberry frame-search PrintOnly contract changed." }
if ($printText -notmatch "Frame-search: profile=RM256 phase=Soak U=42 geometry=output=256,period=64,internal=256,lookahead=256,effective=512") { throw "Raspberry frame-search identity output changed." }

Write-Output "Raspberry frame-search profiles, canonical Units, durations, retention, grading, fatal precedence, and host identity tests passed"
