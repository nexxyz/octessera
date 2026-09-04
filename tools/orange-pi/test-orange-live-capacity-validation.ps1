$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptPath = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
$runnerSource = [IO.File]::ReadAllText($scriptPath)
Import-Module (Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-profile-baseline-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-cross-metadata.psm1") -Force

function Invoke-StudyPrintOnly {
  param([hashtable]$Parameters)
  try {
    $global:LASTEXITCODE = 0
    $output = @(& $scriptPath @Parameters 2>&1)
    if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "study runner exited with code $LASTEXITCODE" }
    return ($output | ForEach-Object { [string]$_ }) -join "`n"
  } catch {
    throw "Study runner PrintOnly failed: $($_.Exception.Message)"
  }
}

function Assert-Contains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) { throw "Study runner output is missing: $Value" }
}

function Assert-NotContains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -ge 0) { throw "Study runner output unexpectedly contains: $Value" }
}

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected validation failure did not occur." }
}

function New-DiagnosticFixture {
  param([Parameter(Mandatory)][int]$PoolCapacity)
  $directory = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-capacity-$PoolCapacity-" + [guid]::NewGuid().ToString("N"))
  $binary = Join-Path $directory "octessera-pi"
  $metadata = "$binary.metadata.json"
  New-Item -ItemType Directory -Path $directory | Out-Null
  $spec = [pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w benchmark-voice-pools-$PoolCapacity"; ArtifactKind = "diagnostic-only" }
  $parameters = @{
    BinaryPath = $binary
    MetadataPath = $metadata
    SelectedBinary = "octessera-pi"
    SelectedTarget = "aarch64-unknown-linux-gnu"
    SelectedProfile = "release"
    BuildSpec = $spec
    SourceCommit = ("a" * 40)
  }
  [IO.File]::WriteAllBytes($binary, [byte[]](1, 2, 3, 4))
  Publish-OrangeBuildMetadata @parameters
  return [pscustomobject]@{ Directory = $directory; Binary = $binary; Metadata = $metadata }
}

foreach ($case in @(
    @{ Units = 1; Bus = 1; Global = 1; Momentary = 1; Stage = 128 },
    @{ Units = 3; Bus = 2; Global = 1; Momentary = 1; Stage = 128 },
    @{ Units = 8; Bus = 4; Global = 1; Momentary = 2; Stage = 128 },
    @{ Units = 16; Bus = 8; Global = 2; Momentary = 2; Stage = 128 },
    @{ Units = 24; Bus = 12; Global = 2; Momentary = 2; Stage = 128 },
    @{ Units = 42; Bus = 12; Global = 2; Momentary = 2; Stage = 128 },
    @{ Units = 43; Bus = 12; Global = 2; Momentary = 2; Stage = 256 },
    @{ Units = 85; Bus = 12; Global = 2; Momentary = 2; Stage = 256 }
  )) {
  $selection = Assert-OrangeLiveBenchmarkSelection -Scenario "capacity_analogue_$($case.Units)" -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 30
  if (-not $selection.IsCapacityDiagnostic -or $selection.CapacityKind -cne "analogue" -or $selection.SynthCount -ne 3 * $case.Units -or $selection.SampleCount -ne $case.Units -or $selection.RequiredPoolCapacity -ne 3 * $case.Units -or $selection.RequiredPoolStage -ne $case.Stage -or $selection.ExpectedActiveSynthVoices -ne 3 * $case.Units -or $selection.ExpectedActiveSampleVoices -ne $case.Units -or $selection.ExpectedActivePreviewSampleVoices -ne 0 -or $selection.ExpectedActiveBusFxSlots -ne $case.Bus -or $selection.ExpectedActiveGlobalFxSlots -ne $case.Global -or $selection.ExpectedActiveMomentaryFx -ne $case.Momentary -or $selection.ExpectedVoiceSteals -ne 0 -or $selection.ExpectedVoiceAdmissionDropsStart -ne 0 -or $selection.ExpectedVoiceAdmissionDropsEnd -ne 0) {
    throw "Analogue capacity selection did not retain its exact expected state: $($case.Units)"
  }
}

foreach ($case in @(
    @{ Scenario = "capacity_synth_1"; Synth = 1; Sample = 0; Required = 1 },
    @{ Scenario = "capacity_sample_256"; Synth = 0; Sample = 256; Required = 256 },
    @{ Scenario = "capacity_mixed_16_128"; Synth = 16; Sample = 128; Required = 128 }
  )) {
  $selection = Assert-OrangeLiveBenchmarkSelection -Scenario $case.Scenario -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 30
  if ($selection.SynthCount -ne $case.Synth -or $selection.SampleCount -ne $case.Sample -or $selection.RequiredPoolCapacity -ne $case.Required -or $selection.PSObject.Properties["RequiredPoolStage"]) { throw "Legacy capacity selection changed: $($case.Scenario)" }
}
foreach ($scenario in @("capacity_synth_0", "capacity_synth_01", "capacity_synth_257", "capacity_sample_999", "capacity_mixed_1_01", "capacity_mixed_1_257", "capacity_mixed_1_2_extra", "capacity_mixed_1")) {
  Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario $scenario -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 30 }
}

foreach ($scenario in @("capacity_analogue_0", "capacity_analogue_01", "capacity_analogue_86", "capacity_analogue_18446744073709551616", "capacity_analogue_1x", "capacity_analogue_1_2")) {
  Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario $scenario -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 30 }
}
foreach ($parameters in @(
    @{ Scenario = "capacity_synth_16"; OutputFrames = 512; EngineBlockFrames = 64; MeasureSeconds = 30 },
    @{ Scenario = "capacity_synth_16"; OutputFrames = 256; EngineBlockFrames = 128; MeasureSeconds = 30 },
    @{ Scenario = "capacity_synth_16"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 120 },
    @{ Scenario = "capacity_synth_16"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 300 }
  )) {
  Assert-Throws { Assert-OrangeLiveBenchmarkSelection @parameters }
}

$baselineLiveScenarioIds = @(Get-OrangeBaselineLiveScenarioIds)
$liveMatrixPlan = @(Get-OrangeLiveMatrixPlan)
if ($baselineLiveScenarioIds.Count -ne 14 -or @($baselineLiveScenarioIds | Where-Object { $_ -like "capacity_*" }).Count -ne 0 -or @(Get-OrangeLiveScenarioIds | Where-Object { $_ -like "capacity_*" }).Count -ne 0) {
  throw "Dynamic capacity scenarios entered a historical live allowlist."
}
foreach ($scenario in @("default_envelope_24_synth_8_sample", "default_headroom_32_synth_8_sample", "default_headroom_32_synth_16_sample", "default_headroom_40_synth_16_sample", "default_headroom_48_synth_16_sample", "default_capacity_64_synth_16_sample", "default_capacity_48_synth_64_sample", "default_capacity_64_synth_64_sample")) {
  if ($baselineLiveScenarioIds -notcontains $scenario -or (Get-OrangeLiveScenarioIds) -contains $scenario -or @($liveMatrixPlan | Where-Object { $_.Scenario -eq $scenario }).Count -ne 0) { throw "Default-capacity scenario was not kept in the baseline-live allowlist: $scenario" }
}

$stage128 = New-DiagnosticFixture 128
$stage256 = New-DiagnosticFixture 256
try {
  $legacyPrint = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_mixed_16_128"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 180; Artifact = $stage128.Binary; Metadata = $stage128.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $legacyPrint "Live selection: diagnostic output=256 period=64 engine=64 internal=64 scenario=capacity_mixed_16_128 measure=180"
  Assert-Contains $legacyPrint "Live artifact identity: artifact_kind=diagnostic-only cargo_feature=hardware-orange-pi-zero-2w benchmark-voice-pools-128"
  Assert-Contains $legacyPrint "Diagnostic pool identity: benchmark-voice-pools-128 requested-synth=16 requested-sample=128 required-capacity=128"
  Assert-NotContains $legacyPrint "required-pool-stage="
  Assert-Contains $legacyPrint '"artifact_kind":"diagnostic-only"'
  Assert-Contains $legacyPrint '"cargo_feature":"hardware-orange-pi-zero-2w benchmark-voice-pools-128"'
  Assert-NotContains $legacyPrint '"artifact_kind":"runtime-candidate"'
  $capacityGateIndex = $legacyPrint.IndexOf('"artifact_kind":"diagnostic-only"', [StringComparison]::Ordinal)
  $capacityStopIndex = $legacyPrint.IndexOf('systemctl stop "$service"', [StringComparison]::Ordinal)
  if ($capacityGateIndex -lt 0 -or $capacityStopIndex -lt 0 -or $capacityGateIndex -ge $capacityStopIndex) { throw "Dynamic remote identity validation was not before production interruption." }
  $legacyPrintWithoutArtifact = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_synth_16"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = (Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-capacity-missing"); Metadata = (Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-capacity-missing.metadata.json"); AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $legacyPrintWithoutArtifact "Live artifact identity: artifact_kind=diagnostic-only cargo_feature=hardware-orange-pi-zero-2w benchmark-voice-pools-128"
  Assert-Contains $legacyPrintWithoutArtifact "Diagnostic pool identity: benchmark-voice-pools-128 requested-synth=16 requested-sample=0 required-capacity=16"
  $exact42 = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_analogue_42"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage128.Binary; Metadata = $stage128.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $exact42 "Live artifact identity: artifact_kind=diagnostic-only cargo_feature=hardware-orange-pi-zero-2w benchmark-voice-pools-128"
  Assert-Contains $exact42 "Diagnostic pool identity: benchmark-voice-pools-128 requested-synth=126 requested-sample=42 required-capacity=126 required-pool-stage=128"
  $exact43 = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_analogue_43"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage256.Binary; Metadata = $stage256.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $exact43 "Diagnostic pool identity: benchmark-voice-pools-256 requested-synth=129 requested-sample=43 required-capacity=129 required-pool-stage=256"
  $exact85 = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_analogue_85"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage256.Binary; Metadata = $stage256.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $exact85 "Diagnostic pool identity: benchmark-voice-pools-256 requested-synth=255 requested-sample=85 required-capacity=255 required-pool-stage=256"
  $legacyCrossStage = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_mixed_16_128"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage256.Binary; Metadata = $stage256.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $legacyCrossStage "required-capacity=128"
  Assert-NotContains $legacyCrossStage "required-pool-stage="
  Assert-Throws { Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_analogue_43"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage128.Binary; Metadata = $stage128.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
  Assert-Throws { Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_analogue_42"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage256.Binary; Metadata = $stage256.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
  Assert-Throws { Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_synth_256"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $stage128.Binary; Metadata = $stage128.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
  $artifactValidationIndex = $runnerSource.IndexOf('$artifactIdentity = Assert-StudyArtifact', [StringComparison]::Ordinal)
  $transportIndex = $runnerSource.IndexOf('Invoke-OrangeTransport "ssh-payload"', [StringComparison]::Ordinal)
  if ($artifactValidationIndex -lt 0 -or $transportIndex -lt 0 -or $artifactValidationIndex -ge $transportIndex) { throw "Capacity artifact validation was not placed before Orange board transport." }
} finally {
  Remove-Item -LiteralPath $stage128.Directory, $stage256.Directory -Recurse -Force -ErrorAction SilentlyContinue
}

$candidate = New-DiagnosticFixture 64
try {
  $candidateSpec = [pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w"; ArtifactKind = "runtime-candidate" }
  $candidateParameters = @{
    BinaryPath = $candidate.Binary
    MetadataPath = $candidate.Metadata
    SelectedBinary = "octessera-pi"
    SelectedTarget = "aarch64-unknown-linux-gnu"
    SelectedProfile = "release"
    BuildSpec = $candidateSpec
    SourceCommit = ("b" * 40)
  }
  Remove-Item -LiteralPath $candidate.Metadata -Force
  Publish-OrangeBuildMetadata @candidateParameters
  Assert-Throws { Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_synth_1"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $candidate.Binary; Metadata = $candidate.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
  Assert-Throws { Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveAudioBenchmark"; Scenario = "capacity_analogue_1"; OutputFrames = 256; EngineBlockFrames = 64; MeasureSeconds = 30; Artifact = $candidate.Binary; Metadata = $candidate.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
} finally {
  Remove-Item -LiteralPath $candidate.Directory -Recurse -Force -ErrorAction SilentlyContinue
}

function New-AnalogueProfileSnapshot {
  param([Parameter(Mandatory)][pscustomobject]$Selection)
  return [pscustomobject]@{
    active_synth_voices = $Selection.ExpectedActiveSynthVoices
    active_sample_voices = $Selection.ExpectedActiveSampleVoices
    active_preview_sample_voices = $Selection.ExpectedActivePreviewSampleVoices
    active_momentary_fx = $Selection.ExpectedActiveMomentaryFx
    active_bus_fx_slots = $Selection.ExpectedActiveBusFxSlots
    active_global_fx_slots = $Selection.ExpectedActiveGlobalFxSlots
    cumulative_voice_steals = $Selection.ExpectedVoiceSteals
    cumulative_voice_admission_drops = $Selection.ExpectedVoiceAdmissionDropsStart
  }
}

function New-AnalogueResult {
  param([Parameter(Mandatory)][pscustomobject]$Selection)
  $profileStart = New-AnalogueProfileSnapshot $Selection
  $profileEnd = New-AnalogueProfileSnapshot $Selection
  $callback = [pscustomobject]@{ callback_count = 441; first_measured_callback_ns = 1; last_measured_callback_ns = 442; measured_elapsed_ns = 441; callback_frames_min = 100; callback_frames_max = 100; callback_frame_sample_count = 441; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0; callback_timestamp_observed = $true; terminal_error = $false; worker_terminal = $false; over_audio_duration_budget_count = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; pre_mute_nonzero_samples = 10; post_mute_nonzero_samples = 0; rendered_frames = 44100; render_audio_duration_ns = 1000000000; render_audio_duration_ratio_p50 = 0.5; render_audio_duration_ratio_p95 = 0.6; render_audio_duration_ratio_p99 = 0.7; render_audio_duration_ratio_p99_9 = 0.8; render_audio_duration_ratio_max = 0.9 }
  $counters = [pscustomobject]@{ rendered_quantums = 7; repeated_quantums = 2; dropped_quantums = 3; deadline_misses = 4; deadline_recoveries = 4 }
  $persistent = [pscustomobject]@{ observable = $true; warmup = [pscustomobject]@{ rendered_quantums = 3; repeated_quantums = 1; dropped_quantums = 0; deadline_misses = 1; deadline_recoveries = 1 }; start = [pscustomobject]@{ rendered_quantums = 5; repeated_quantums = 1; dropped_quantums = 1; deadline_misses = 2; deadline_recoveries = 1 }; end = $counters; delta = [pscustomobject]@{ rendered_quantums = 2; repeated_quantums = 1; dropped_quantums = 2; deadline_misses = 2; deadline_recoveries = 3 } }
  $workerTiming = [pscustomobject]@{ workers = @([pscustomobject]@{ sequence = 7; render_ns = 10; dispatch_to_finish_ns = 20; cpu_start = 2; cpu_end = 2; finished = $true }, [pscustomobject]@{ sequence = 7; render_ns = 11; dispatch_to_finish_ns = 25; cpu_start = 3; cpu_end = 3; finished = $true }); coordinator = [pscustomobject]@{ sequence = 7; deadline_ns = 100; dispatch_to_deadline_start_ns = 10; dispatch_to_deadline_elapsed_ns = $null; in_flight_mask = 0; completed_mask = 3; first_parity = 0; dispatch_to_first_ns = 20; dispatch_to_both_ns = 25; reduction_ns = 4; coordinator_remainder_ns = 5; engine_block_total_ns = 40; callback_total_ns = 50; failed = $false; frozen = $true }; late_after_deadline_ns = $null; cpu_endpoint_changed = $false }
  return [pscustomobject]@{ schema_version = 11; kind = "orange_audio_benchmark_result"; status = "pass"; board_profile = "orange-pi-zero-2w"; scenario = $Selection.Scenario; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 64; sample_format = "F32"; channels = 2; sample_rate = 44100; warmup_seconds = 5; measure_seconds = 30; scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; post_dsp_zero = $true; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; pid = 123; systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64); callback = $callback; persistent_output_counters = $persistent; detected_continuity_events = 3; profile_start = $profileStart; profile_end = $profileEnd; recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; executor_mode = "persistent_two_workers"; worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-src-0"; worker_thread_name_1 = "oct-dsp-src-1"; joined_workers = 2; retirement_error = $null; worker_timing_mode = "enabled"; worker_timing = $workerTiming }
}

$analogueSelection = Assert-OrangeLiveBenchmarkSelection -Scenario "capacity_analogue_24" -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 30
$analogueResult = New-AnalogueResult $analogueSelection
Assert-OrangeLiveResult -Result $analogueResult -Selection $analogueSelection
foreach ($profileName in @("profile_start", "profile_end")) {
  foreach ($field in @("active_synth_voices", "active_sample_voices", "active_preview_sample_voices", "active_momentary_fx", "active_bus_fx_slots", "active_global_fx_slots", "cumulative_voice_steals", "cumulative_voice_admission_drops")) {
    $invalid = New-AnalogueResult $analogueSelection
    $snapshot = $invalid.$profileName
    $snapshot.$field = [uint64]$snapshot.$field + 1
    Assert-Throws { Assert-OrangeLiveResult -Result $invalid -Selection $analogueSelection }
    $invalid = New-AnalogueResult $analogueSelection
    $snapshot = $invalid.$profileName
    $snapshot.$field = "not-an-integer"
    Assert-Throws { Assert-OrangeLiveResult -Result $invalid -Selection $analogueSelection }
  }
}

Write-Output "Orange live capacity selection, stage, profile, and artifact validation tests passed"
