$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$runner = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
$matrix = Join-Path $PSScriptRoot "run-orange-live-audio-matrix.ps1"
$validation = Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1"
Import-Module $validation -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-payload-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-worker-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-worker-timing-validation.psm1") -Force

function Invoke-PrintOnly {
  param([Parameter(Mandatory)][string]$Path, [hashtable]$Parameters = @{})
  $global:LASTEXITCODE = 0
  $output = @(& $Path @Parameters 2>&1)
  if ($LASTEXITCODE -ne 0) { throw "PrintOnly failed for $Path" }
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

function Assert-Contains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) { throw "Missing expected text: $Value" }
}

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected validation failure did not occur." }
}

function Assert-NotContains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -ge 0) { throw "Unexpected text: $Value" }
}

function New-RoutingResult {
  param([Parameter(Mandatory)][pscustomobject]$Selection)
  $snapshot = [pscustomobject]@{ active_synth_voices = 0; active_sample_voices = 0; active_preview_sample_voices = 0; active_momentary_fx = 0; active_bus_fx_slots = 0; active_global_fx_slots = 0; cumulative_voice_steals = 0; cumulative_voice_admission_drops = 0 }
  $callback = [pscustomobject]@{ callback_count = 441; first_measured_callback_ns = 1; last_measured_callback_ns = 442; measured_elapsed_ns = 441; callback_frames_min = 100; callback_frames_max = 100; callback_frame_sample_count = 441; invalid_callback_frame_count = 0; callback_timestamp_observed = $true; terminal_error = $false; worker_terminal = $false; over_audio_duration_budget_count = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; pre_mute_nonzero_samples = 10; post_mute_nonzero_samples = 0; rendered_frames = 44100; render_audio_duration_ns = 1000000000; render_audio_duration_ratio_p50 = 0.5; render_audio_duration_ratio_p95 = 0.6; render_audio_duration_ratio_p99 = 0.7; render_audio_duration_ratio_p99_9 = 0.8; render_audio_duration_ratio_max = 0.9 }
  $timing = [pscustomobject]@{ workers = @([pscustomobject]@{ sequence = 7; render_ns = 10; dispatch_to_finish_ns = 20; cpu_start = 2; cpu_end = 2; finished = $true }, [pscustomobject]@{ sequence = 7; render_ns = 11; dispatch_to_finish_ns = 25; cpu_start = 3; cpu_end = 3; finished = $true }); coordinator = [pscustomobject]@{ sequence = 7; deadline_ns = 100; dispatch_to_deadline_start_ns = 10; dispatch_to_deadline_elapsed_ns = $null; in_flight_mask = 0; completed_mask = 3; first_parity = 0; dispatch_to_first_ns = 20; dispatch_to_both_ns = 25; reduction_ns = 4; coordinator_remainder_ns = 5; engine_block_total_ns = 40; callback_total_ns = 50; failed = $false; frozen = $true }; late_after_deadline_ns = $null; cpu_endpoint_changed = $false }
  $persistent = [pscustomobject]@{ observable = $true; warmup = [pscustomobject]@{ rendered_quantums = 3; repeated_quantums = 1; dropped_quantums = 0; deadline_misses = 1; deadline_recoveries = 1 }; start = [pscustomobject]@{ rendered_quantums = 5; repeated_quantums = 1; dropped_quantums = 1; deadline_misses = 2; deadline_recoveries = 1 }; end = [pscustomobject]@{ rendered_quantums = 7; repeated_quantums = 2; dropped_quantums = 3; deadline_misses = 4; deadline_recoveries = 4 }; delta = [pscustomobject]@{ rendered_quantums = 2; repeated_quantums = 1; dropped_quantums = 2; deadline_misses = 2; deadline_recoveries = 3 } }
  return [pscustomobject]@{
    schema_version = 12; kind = "orange_audio_benchmark_result"; status = "pass"; board_profile = "orange-pi-zero-2w"; scenario = $Selection.Scenario
    requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 128
    sample_format = "F32"; channels = 2; sample_rate = 44100; warmup_seconds = 5; measure_seconds = 30
    scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; post_dsp_zero = $true
    measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; pid = 123; systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64)
    callback = $callback; persistent_output_counters = $persistent; detected_continuity_events = 3; profile_start = $snapshot; profile_end = $snapshot
    recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; executor_mode = "routing_tree_persistent"
    lookahead_frames = 128; effective_output_latency_frames = 384; worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-tree-0"; worker_thread_name_1 = "oct-dsp-tree-1"; joined_workers = 2; retirement_error = $null; worker_timing_mode = "enabled"; worker_timing = $timing
  }
}

$selection = Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 30 -ExecutorMode "routing_tree_persistent"
if ($selection.ExecutorMode -cne "routing_tree_persistent" -or $selection.WorkerTimingMode -cne "enabled" -or $selection.LookaheadFrames -ne 128 -or $selection.EffectiveOutputLatencyFrames -ne 384) { throw "Routing-tree selection identity was not retained." }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 512 -EngineBlockFrames 128 -MeasureSeconds 30 -ExecutorMode "routing_tree_persistent" }
$persistentDefault = Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 30 -ExecutorMode "persistent_two_workers"
$persistentDisabled = Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 30 -ExecutorMode "persistent_two_workers" -WorkerTimingMode "disabled"
if ($persistentDefault.WorkerTimingMode -cne "enabled" -or $persistentDisabled.WorkerTimingMode -cne "disabled") { throw "Two-wave worker timing selection did not preserve default or explicit mode." }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 30 -ExecutorMode "inline" -WorkerTimingMode "enabled" }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 30 -ExecutorMode "routing_tree_persistent" -WorkerTimingMode "disabled" }
$routingWorkers = [pscustomobject]@{ executor_mode = "routing_tree_persistent"; worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-tree-0"; worker_thread_name_1 = "oct-dsp-tree-1"; joined_workers = 2; retirement_error = $null }
Assert-OrangeWorkerEvidence -Evidence $routingWorkers -RequireShutdown:$true
$wrongWorkers = $routingWorkers.PSObject.Copy()
$wrongWorkers.worker_thread_name_0 = "oct-dsp-src-0"
Assert-Throws { Assert-OrangeWorkerEvidence -Evidence $wrongWorkers }
$routingResult = New-RoutingResult $selection
Assert-OrangeLiveResult -Result $routingResult -Selection $selection
$disabledResult = ConvertFrom-Json -InputObject ($routingResult | ConvertTo-Json -Depth 8)
$disabledResult.executor_mode = "persistent_two_workers"; $disabledResult.lookahead_frames = 0; $disabledResult.effective_output_latency_frames = 256; $disabledResult.worker_thread_name_0 = "oct-dsp-src-0"; $disabledResult.worker_thread_name_1 = "oct-dsp-src-1"; $disabledResult.worker_timing_mode = "disabled"; $disabledResult.worker_timing = $null
Assert-OrangeLiveResult -Result $disabledResult -Selection $persistentDisabled
$wrongRoutingTiming = ConvertFrom-Json -InputObject ($routingResult | ConvertTo-Json -Depth 8)
$wrongRoutingTiming.worker_timing_mode = "disabled"; $wrongRoutingTiming.worker_timing = $null
Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $wrongRoutingTiming }
$wrongRoutingResult = ConvertFrom-Json -InputObject ($routingResult | ConvertTo-Json -Depth 8)
$wrongRoutingResult.effective_output_latency_frames = 385
Assert-Throws { Assert-OrangeLiveResult -Result $wrongRoutingResult -Selection $selection }
$wrongRoutingResult = ConvertFrom-Json -InputObject ($routingResult | ConvertTo-Json -Depth 8)
$wrongRoutingResult.persistent_output_counters.observable = $false
Assert-Throws { Assert-OrangeLiveResult -Result $wrongRoutingResult -Selection $selection }
$wrongRoutingResult = ConvertFrom-Json -InputObject ($routingResult | ConvertTo-Json -Depth 8)
$wrongRoutingResult.worker_thread_name_0 = "oct-dsp-src-0"
Assert-Throws { Assert-OrangeLiveResult -Result $wrongRoutingResult -Selection $selection }
$wrongRoutingResult = ConvertFrom-Json -InputObject ($routingResult | ConvertTo-Json -Depth 8)
$wrongRoutingResult.joined_workers = 1
Assert-Throws { Assert-OrangeLiveResult -Result $wrongRoutingResult -Selection $selection }
$routingReadiness = [pscustomobject]@{ schema_version = 5; kind = "orange_audio_benchmark_readiness"; status = "ready"; board_profile = "orange-pi-zero-2w"; pid = 123; systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64); scenario = "synth_ramp_16"; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; sample_rate = 44100; channels = 2; internal_block_frames = 128; sample_format = "F32"; scheduler_qualified = $true; post_dsp_zero = $true; callback_frames_min = 100; callback_frames_max = 100; callback_frame_sample_count = 3; invalid_callback_frame_count = 0; executor_mode = "routing_tree_persistent"; lookahead_frames = 128; worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-tree-0"; worker_thread_name_1 = "oct-dsp-tree-1" }
Assert-OrangeLiveReadiness -Readiness $routingReadiness -Selection $selection -ExpectedPid 123 -ExpectedInvocation "invocation" -ArtifactHash ("a" * 64)

$runnerParameters = @{ Mode = "LiveAudioBenchmark"; Scenario = "synth_ramp_16"; OutputFrames = 256; EngineBlockFrames = 128; MeasureSeconds = 30; ExecutorMode = "routing_tree_persistent"; Artifact = (Join-Path ([IO.Path]::GetTempPath()) "octessera-routing-tree-missing"); Metadata = (Join-Path ([IO.Path]::GetTempPath()) "octessera-routing-tree-missing.metadata.json"); AllowServiceInterruption = $true; PrintOnly = $true }
$routingPrint = Invoke-PrintOnly $runner $runnerParameters
Assert-Contains $routingPrint "executor=routing_tree_persistent lookahead=128 effective-latency=384"
Assert-Contains $routingPrint '--executor routing_tree_persistent'
Assert-Contains $routingPrint '--worker-timing enabled'
Assert-Contains $routingPrint '"artifact_kind":"diagnostic-only"'
Assert-Contains $routingPrint '"cargo_feature":"hardware-orange-pi-zero-2w routing-tree-benchmark"'
Assert-Contains $routingPrint 'json_field schema_version "$marker")" = 5'
$validationSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1") -Raw
Assert-Contains $validationSource 'schema_version -Path "schema_version"), 12'

$studyStart = $routingPrint.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
$studyEnd = $routingPrint.IndexOf("Study payload transport:", $studyStart, [StringComparison]::Ordinal)
$routingPayload = $routingPrint.Substring($studyStart, $studyEnd - $studyStart)
$ordinaryParameters = $runnerParameters.Clone(); $ordinaryParameters.ExecutorMode = "persistent_two_workers"
$ordinaryPrint = Invoke-PrintOnly $runner $ordinaryParameters
$ordinaryStart = $ordinaryPrint.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
$ordinaryEnd = $ordinaryPrint.IndexOf("Study payload transport:", $ordinaryStart, [StringComparison]::Ordinal)
$ordinaryPayload = $ordinaryPrint.Substring($ordinaryStart, $ordinaryEnd - $ordinaryStart)
$disabledParameters = $runnerParameters.Clone(); $disabledParameters.ExecutorMode = "persistent_two_workers"; $disabledParameters.WorkerTimingMode = "disabled"
$disabledPrint = Invoke-PrintOnly $runner $disabledParameters
Assert-Contains $disabledPrint "worker-timing=disabled executor=persistent_two_workers lookahead=0 effective-latency=256"
$disabledStart = $disabledPrint.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
$disabledEnd = $disabledPrint.IndexOf("Study payload transport:", $disabledStart, [StringComparison]::Ordinal)
$disabledPayload = $disabledPrint.Substring($disabledStart, $disabledEnd - $disabledStart)
Assert-Contains $disabledPayload '--worker-timing disabled'
Assert-Contains $disabledPayload 'worker_timing_mode "$result")" = disabled'
$disabledPlan = @(Get-OrangeLiveMatrixPlan -ExecutorMode "persistent_two_workers" -WorkerTimingMode "disabled")
if ($disabledPlan.Count -ne 22 -or @($disabledPlan | Where-Object { $_.WorkerTimingMode -ne "disabled" }).Count -ne 0) { throw "Explicit disabled two-wave timing was not retained through the matrix plan." }
$disabledMatrixPrint = Invoke-PrintOnly $matrix @{ PrintOnly = $true; ExecutorMode = "persistent_two_workers"; WorkerTimingMode = "disabled" }
Assert-Contains $disabledMatrixPrint "Matrix cells: 23 total (11 A + 1 selected A120 + 11 B)."
$inlineParameters = $runnerParameters.Clone(); $inlineParameters.ExecutorMode = "inline"; $inlineParameters.WorkerTimingMode = "disabled"
$inlinePrint = Invoke-PrintOnly $runner $inlineParameters
$inlineStart = $inlinePrint.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
$inlineEnd = $inlinePrint.IndexOf("Study payload transport:", $inlineStart, [StringComparison]::Ordinal)
$inlinePayload = $inlinePrint.Substring($inlineStart, $inlineEnd - $inlineStart)
Assert-OrangeGeneratedLivePayloadSyntax -Payload $routingPayload
Assert-NotContains $routingPayload.Substring($routingPayload.IndexOf("validate_benchmark_readiness() {", [StringComparison]::Ordinal), $routingPayload.IndexOf("validate_benchmark_worker_evidence() {", [StringComparison]::Ordinal) - $routingPayload.IndexOf("validate_benchmark_readiness() {", [StringComparison]::Ordinal)) "effective_output_latency_frames"
Assert-NotContains $routingPayload.Substring($routingPayload.IndexOf("validate_benchmark_progress() {", [StringComparison]::Ordinal), $routingPayload.IndexOf("validate_benchmark_result() {", [StringComparison]::Ordinal) - $routingPayload.IndexOf("validate_benchmark_progress() {", [StringComparison]::Ordinal)) "effective_output_latency_frames"
Assert-Contains $routingPayload "oct-dsp-tree-0"
Assert-Contains $routingPayload "oct-dsp-tree-1"
Assert-Contains $routingPayload "lookahead_frames"
Assert-Contains $routingPayload "effective_output_latency_frames"
Assert-Contains $routingPayload 'validate_benchmark_result()'
Assert-Contains $routingPayload '[ "$(json_field schema_version "$result")" = 12 ]'
Assert-Contains $routingPayload 'worker_timing_mode "$result")" = enabled'
Assert-OrangeGeneratedWorkerTaskAudit -PersistentPayload $ordinaryPayload -InlinePayload $inlinePayload -RoutingPayload $routingPayload

$matrixPrint = Invoke-PrintOnly $matrix @{ PrintOnly = $true; ExecutorMode = "routing_tree_persistent" }
if ($matrixPrint -match 'output=512|output=1024' -or $matrixPrint -notmatch '(?s)(?=.*01: synth_ramp_16)(?=.*11: mixed_cross_slot_48_48_steal)(?=.*12: A120)(?=.*Matrix cells: 12 total \(11 A \+ 1 selected A120\))') { throw "Routing-tree matrix did not retain exactly 11 A cells plus A120." }

Write-Output "Orange routing-tree executor, geometry, worker, payload, and matrix tests passed"
