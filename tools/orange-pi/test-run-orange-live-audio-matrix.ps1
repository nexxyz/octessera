$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$runner = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
$matrix = Join-Path $PSScriptRoot "run-orange-live-audio-matrix.ps1"
$validation = Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1"
Import-Module $validation -Force
function Invoke-PrintOnly {
  param([Parameter(Mandatory)][string]$Path, [hashtable]$Parameters = @{})
  $global:LASTEXITCODE = 0
  $output = @(& $Path @Parameters 2>&1)
  if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "PrintOnly failed for $Path" }
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}
function Assert-Contains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) { throw "Missing expected text: $Value" }
}
function Assert-NotContains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -ge 0) { throw "Unexpected text: $Value" }
}
function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected validation failure did not occur." }
}
$scenarios = @(Get-OrangeLiveScenarioIds)
$basePlan = @(Get-OrangeLiveMatrixPlan)
if ($scenarios.Count -ne 11 -or $basePlan.Count -ne 22 -or $scenarios[0] -ne "synth_ramp_16" -or $scenarios[10] -ne "mixed_cross_slot_48_48_steal") {
  throw "The approved live scenario or base matrix order changed."
}
foreach ($tuple in @(@{ Output = 128; Engine = 32; Period = 32; Internal = 32 }, @{ Output = 256; Engine = 64; Period = 64; Internal = 64 }, @{ Output = 256; Engine = 128; Period = 64; Internal = 128 }, @{ Output = 256; Engine = 256; Period = 64; Internal = 256 }, @{ Output = 512; Engine = 128; Period = 128; Internal = 128 }, @{ Output = 1024; Engine = 256; Period = 256; Internal = 256 })) {
  $selection = Assert-OrangeLiveBenchmarkSelection -Scenario "synth_cross_slot_96_steal" -OutputFrames $tuple.Output -EngineBlockFrames $tuple.Engine -MeasureSeconds 30
  if ($selection.AlsaPeriodFrames -ne $tuple.Period -or $selection.InternalFrames -ne $tuple.Internal -or $selection.EngineBlockFrames -ne $tuple.Engine) { throw "Approved geometry tuple was not retained independently." }
}
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 32 -MeasureSeconds 30 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 512 -EngineBlockFrames 256 -MeasureSeconds 30 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 1024 -EngineBlockFrames 256 -MeasureSeconds 30 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 120 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "synth_cross_slot_96_steal" -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 120 }
$long = Assert-OrangeLiveBenchmarkSelection -Scenario "synth_cross_slot_96_steal" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds 120 -AllowLongRepeat:$true
if (-not $long.LongRepeat -or $long.InternalFrames -ne 128 -or $long.AlsaPeriodFrames -ne 64) { throw "Long-repeat selection was not classified as A/128." }
$missingActive = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-missing-" + [guid]::NewGuid().ToString("N"))
Assert-Throws {
  & $runner -Mode LiveAudioBenchmark -Scenario synth_cross_slot_96_steal -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 30 -Artifact $missingActive -Metadata "$missingActive.metadata.json" -AllowServiceInterruption
}
Assert-Throws { & $runner -Mode LiveAudioBenchmark -Scenario synth_ramp_16 -OutputFrames 256 -MeasureSeconds 30 -PrintOnly }
$distinctPrint = Invoke-PrintOnly $runner @{ Mode = "LiveAudioBenchmark"; Scenario = "synth_ramp_16"; OutputFrames = 256; EngineBlockFrames = 256; MeasureSeconds = 30; Artifact = $missingActive; Metadata = "$missingActive.metadata.json"; AllowServiceInterruption = $true; PrintOnly = $true }
Assert-Contains $distinctPrint "Live selection: individual output=256 period=64 engine=256 internal=256 scenario=synth_ramp_16 measure=30 warmup=5"
$fakeWorst = Get-OrangeLiveWorstPassingScenario @(
  [pscustomobject]@{ StatusClass = "pass"; Scenario = "synth_ramp_64"; OutputFrames = 256; EngineBlockFrames = 256; MeasureSeconds = 30; RatioP999 = 9.0; RatioMax = 9.0 },
  [pscustomobject]@{ StatusClass = "pass"; Scenario = "synth_ramp_16"; OutputFrames = 256; EngineBlockFrames = 128; MeasureSeconds = 30; RatioP999 = 1.2; RatioMax = 1.3 },
  [pscustomobject]@{ StatusClass = "pass"; Scenario = "synth_ramp_32"; OutputFrames = 256; EngineBlockFrames = 128; MeasureSeconds = 30; RatioP999 = 1.2; RatioMax = 1.4 }
)
if ($fakeWorst.Scenario -ne "synth_ramp_32") { throw "Worst-A fixture did not use max ratio as its tie breaker." }
$retrievalRoot = Join-Path ([IO.Path]::GetTempPath()) "orange-study-abcdef123456"
try {
  New-Item -ItemType Directory -Force -Path $retrievalRoot | Out-Null
  Set-Content (Join-Path $retrievalRoot "study-result.txt") "status_class=pass"
  if ((Resolve-OrangeLiveEvidenceDirectory $retrievalRoot "/tmp/expected-root") -ne (Get-Item $retrievalRoot).FullName) { throw "Direct evidence root was not resolved." }
  if ((Get-OrangeLiveRunId $retrievalRoot) -ne "abcdef123456") { throw "Direct evidence run ID was not resolved." }
  Remove-Item -LiteralPath (Join-Path $retrievalRoot "study-result.txt") -Force
  $nestedRoot = Join-Path $retrievalRoot "expected-root"
  New-Item -ItemType Directory -Force -Path $nestedRoot | Out-Null
  Set-Content (Join-Path $nestedRoot "study-result.txt") "status_class=pass"
  if ((Resolve-OrangeLiveEvidenceDirectory $retrievalRoot "/tmp/expected-root") -ne (Get-Item $nestedRoot).FullName) { throw "Nested evidence root was not resolved." }
  if ((Get-OrangeLiveRunId $nestedRoot) -ne "abcdef123456") { throw "Nested evidence run ID was not resolved from its local parent." }
  Set-Content (Join-Path $retrievalRoot "study-result.txt") "status_class=pass"
  Assert-Throws { Resolve-OrangeLiveEvidenceDirectory $retrievalRoot "/tmp/expected-root" }
  Remove-Item -LiteralPath (Join-Path $retrievalRoot "study-result.txt") -Force
  Remove-Item -LiteralPath $nestedRoot -Recurse -Force
  Assert-Throws { Resolve-OrangeLiveEvidenceDirectory $retrievalRoot "/tmp/expected-root" }
} finally {
  Remove-Item -LiteralPath $retrievalRoot -Recurse -Force -ErrorAction SilentlyContinue
}
$evidenceRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-evidence-" + [guid]::NewGuid().ToString("N"))
try {
  New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null
  $selection = Assert-OrangeLiveBenchmarkSelection -Scenario "synth_ramp_16" -OutputFrames 256 -EngineBlockFrames 256 -MeasureSeconds 30
  $readiness = [pscustomobject]@{
    schema_version = 4; kind = "orange_audio_benchmark_readiness"; status = "ready"; board_profile = "orange-pi-zero-2w"; pid = 123
    systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64); scenario = $selection.Scenario; requested_output_buffer_frames = 256
    expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 256
    callback_frames_min = 100; callback_frames_max = 100; callback_frame_sample_count = 441; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0
    sample_rate = 44100; channels = 2; sample_format = "F32"
    scheduler_qualified = $true; post_dsp_zero = $true; executor_mode = "persistent_two_workers"; worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-src-0"; worker_thread_name_1 = "oct-dsp-src-1"
  }
  $callback = [pscustomobject]@{ callback_count = 441; first_measured_callback_ns = 1; last_measured_callback_ns = 442; measured_elapsed_ns = 441; callback_frames_min = 100; callback_frames_max = 100; callback_frame_sample_count = 441; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0; callback_timestamp_observed = $true; terminal_error = $false; worker_terminal = $false; over_audio_duration_budget_count = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; pre_mute_nonzero_samples = 10; post_mute_nonzero_samples = 0; rendered_frames = 44100; render_audio_duration_ns = 1000000000; render_audio_duration_ratio_p50 = 0.5; render_audio_duration_ratio_p95 = 0.6; render_audio_duration_ratio_p99 = 0.7; render_audio_duration_ratio_p99_9 = 0.8; render_audio_duration_ratio_max = 0.9 }
  $profileStart = [pscustomobject]@{ active_synth_voices = 0; active_sample_voices = 0; active_preview_sample_voices = 0; active_momentary_fx = 0; cumulative_voice_steals = 0; cumulative_voice_admission_drops = 0 }
  $profileEnd = [pscustomobject]@{ active_synth_voices = 0; active_sample_voices = 0; active_preview_sample_voices = 0; active_momentary_fx = 0; cumulative_voice_steals = 0; cumulative_voice_admission_drops = 0 }
  $result = [pscustomobject]@{ schema_version = 5; kind = "orange_audio_benchmark_result"; status = "pass"; board_profile = "orange-pi-zero-2w"; scenario = $selection.Scenario; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 256; sample_format = "F32"; channels = 2; sample_rate = 44100; warmup_seconds = 5; measure_seconds = 30; scheduler_qualified = $true; post_dsp_zero = $true; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; pid = 123; systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64); callback = $callback; profile_start = $profileStart; profile_end = $profileEnd; recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; executor_mode = "persistent_two_workers"; worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-src-0"; worker_thread_name_1 = "oct-dsp-src-1"; joined_workers = 2; retirement_error = $null }
  $release = [pscustomobject]@{ schema_version = 2; kind = "orange_audio_benchmark_release"; status = "released"; board_profile = "orange-pi-zero-2w"; pid = 123; systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64); scenario = $selection.Scenario; expected_alsa_buffer_frames = 256; observed_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; observed_alsa_period_frames = 64 }
  $readiness | ConvertTo-Json | Set-Content (Join-Path $evidenceRoot "benchmark-readiness.json")
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  $release | ConvertTo-Json | Set-Content (Join-Path $evidenceRoot "benchmark-release.json")
  Set-Content (Join-Path $evidenceRoot "benchmark-identity.txt") "unit=u.service`nmain_pid=123`ninvocation_id=invocation"
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nSubState=dead`nResult=success`nMainPID=0`nExecMainCode=1`nExecMainStatus=0"
  Set-Content (Join-Path $evidenceRoot "study-result.txt") "interruption_started=true`nstatus_class=pass"
  Set-Content (Join-Path $evidenceRoot "service-restored-state.txt") "restore_status=0`nfinal_active=active`nfinal_enabled=enabled"
  Set-Content (Join-Path $evidenceRoot "sensor-series.txt") "sample=memory phase=startup mem_available_kb=600000`nsample=thermal phase=startup millicelsius=70000`nsample=memory phase=runtime mem_available_kb=580000`nsample=thermal phase=runtime millicelsius=75000"
  $passEvidence = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($passEvidence.StatusClass -ne "pass") { throw "Clean fake evidence did not classify as pass: $($passEvidence.StatusClass) $($passEvidence.Reason)" }
  if ($passEvidence.SensorStartupMaxThermalMillicelsius -ne 70000 -or $passEvidence.SensorRuntimeMaxThermalMillicelsius -ne 75000 -or $passEvidence.SensorMaxThermalMillicelsius -ne 75000) { throw "High startup/runtime temperatures were not retained as extrema." }
  if ([math]::Abs([double]$passEvidence.AggregateRenderAudioDurationRatio - 1.0) -gt 0.000001) { throw "Aggregate render-duration ratio was not computed from total evidence." }
  if ([math]::Abs([double](Get-OrangeLiveResultSummary -Result $result -Selection $selection).AggregateRenderAudioDurationRatio - 1.0) -gt 0.000001) { throw "Result summary did not expose the aggregate render-duration ratio." }
  $manifestAggregate = ConvertFrom-Json -InputObject (ConvertTo-OrangeLiveManifestJson -Results @($passEvidence))
  if ([math]::Abs([double]$manifestAggregate[0].AggregateRenderAudioDurationRatio - 1.0) -gt 0.000001) { throw "Manifest did not retain the aggregate render-duration ratio." }
  Assert-OrangeLiveResult -Result $result -Selection $selection
  $readiness.executor_mode = "inline"
  Assert-Throws { Assert-OrangeLiveReadiness -Readiness $readiness -Selection $selection -ExpectedPid 123 -ExpectedInvocation "invocation" -ArtifactHash ("a" * 64) }
  $readiness.executor_mode = "persistent_two_workers"
  $readiness.worker_health = "deadline_miss"
  Assert-Throws { Assert-OrangeLiveReadiness -Readiness $readiness -Selection $selection -ExpectedPid 123 -ExpectedInvocation "invocation" -ArtifactHash ("a" * 64) }
  $readiness.worker_health = "healthy"
  $readiness.PSObject.Properties.Remove("worker_thread_name_1")
  Assert-Throws { Assert-OrangeLiveReadiness -Readiness $readiness -Selection $selection -ExpectedPid 123 -ExpectedInvocation "invocation" -ArtifactHash ("a" * 64) }
  $readiness | Add-Member -NotePropertyName worker_thread_name_1 -NotePropertyValue "oct-dsp-src-1"
  $result.executor_mode = "inline"
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $result.executor_mode = "persistent_two_workers"
  $result.worker_health = "worker_exited"
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $result.worker_health = "healthy"
  $result.joined_workers = 1
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $result.joined_workers = 2
  $result.retirement_error = "retirement_failed"
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $result.retirement_error = $null
  $result.PSObject.Properties.Remove("worker_thread_name_1")
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $result | Add-Member -NotePropertyName worker_thread_name_1 -NotePropertyValue "oct-dsp-src-1"
  $profileEnd.PSObject.Properties.Remove("cumulative_voice_admission_drops")
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $profileEnd | Add-Member -NotePropertyName cumulative_voice_admission_drops -NotePropertyValue "not-a-number"
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $profileEnd.cumulative_voice_admission_drops = 1
  Assert-Throws { Assert-OrangeLiveResult -Result $result -Selection $selection }
  $profileEnd.cumulative_voice_admission_drops = 2
  $profileStart.cumulative_voice_admission_drops = 1
  $selection | Add-Member -NotePropertyName expected_admission_drops_start -NotePropertyValue 1
  $selection | Add-Member -NotePropertyName expected_admission_drops_end -NotePropertyValue 2
  Assert-OrangeLiveResult -Result $result -Selection $selection
  $selection.PSObject.Properties.Remove("expected_admission_drops_start")
  $selection.PSObject.Properties.Remove("expected_admission_drops_end")
  $profileStart.cumulative_voice_admission_drops = 0
  $profileEnd.cumulative_voice_admission_drops = 0
  $callback.rendered_frames = 44099; $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json"); if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Below-bound callback frame corruption was accepted." }
  $callback.rendered_frames = 44101; $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json"); if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Above-bound callback frame corruption was accepted." }
  $callback.rendered_frames = 0
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Zero rendered-frame aggregate evidence was accepted." }
  $callback.rendered_frames = 44100; $callback.render_audio_duration_ns = 0
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Zero render-duration aggregate evidence was accepted." }
  $callback.render_audio_duration_ns = 1000000000
  $callback.PSObject.Properties.Remove("rendered_frames")
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Missing aggregate evidence was accepted." }
  $callback | Add-Member -NotePropertyName rendered_frames -NotePropertyValue 44100
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  $zeroCodeSuccess = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($zeroCodeSuccess.StatusClass -ne "pass") { throw "Code 0 clean success was not accepted." }
  $callback.cpal_device_error_count = 1
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Pass with CPAL error was accepted." }
  $callback.cpal_device_error_count = 0; $callback.over_audio_duration_budget_count = 1
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Pass with over-budget callback was accepted." }
  $callback.over_audio_duration_budget_count = 0; $callback.pre_mute_nonzero_samples = 0
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Pass with no pre-mute activity was accepted." }
  $callback.pre_mute_nonzero_samples = 10; $callback.post_mute_nonzero_samples = 1
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Pass with post-mute activity was accepted." }
  $callback.post_mute_nonzero_samples = 0
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nSubState=dead`nResult=success`nMainPID=0`nExecMainCode=0`nExecMainStatus=1"
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Code 0 with nonzero status was accepted." }
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nSubState=dead`nResult=exit-code`nMainPID=0`nExecMainCode=0`nExecMainStatus=0"
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Code 0 with non-success Result was accepted." }
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nSubState=dead`nResult=success`nMainPID=123`nExecMainCode=0`nExecMainStatus=0"
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Code 0 with a live MainPID was accepted." }
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=active`nSubState=running`nResult=success`nMainPID=123`nExecMainCode=0`nExecMainStatus=0"
  if ((Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)).StatusClass -ne "infrastructure_failure") { throw "Code 0 with an active unit was accepted." }
  $result.status = "fail"
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  Set-Content (Join-Path $evidenceRoot "study-result.txt") "interruption_started=true`nstatus_class=measured_failure"
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nSubState=dead`nResult=success`nMainPID=0`nExecMainCode=0`nExecMainStatus=0"
  $zeroCodeFailure = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($zeroCodeFailure.StatusClass -ne "infrastructure_failure" -or $zeroCodeFailure.Reason -ne "clean measured failure did not match transient process status") { throw "Code 0 with a failed benchmark result did not reach the strict process-status rejection." }
  $result.status = "pass"
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  Set-Content (Join-Path $evidenceRoot "study-result.txt") "interruption_started=true`nstatus_class=pass"
  $release.observed_alsa_period_frames = 128
  $release | ConvertTo-Json | Set-Content (Join-Path $evidenceRoot "benchmark-release.json")
  $staleRelease = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($staleRelease.StatusClass -ne "infrastructure_failure") { throw "Mismatched release geometry was not refused." }
  $release.observed_alsa_period_frames = 64
  $release | ConvertTo-Json | Set-Content (Join-Path $evidenceRoot "benchmark-release.json")
  $callback.over_audio_duration_budget_count = 1
  $result.status = "fail"
  Set-Content (Join-Path $evidenceRoot "study-result.txt") "interruption_started=true`nstatus_class=measured_failure"
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=failed`nResult=exit-code`nExecMainCode=1`nExecMainStatus=1"
  $callback.cpal_device_error_count = 1
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  $overBudgetCpal = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($overBudgetCpal.StatusClass -ne "infrastructure_failure") { throw "Over-budget evidence with CPAL error was accepted." }
  $callback.cpal_device_error_count = 0
  $result | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $evidenceRoot "benchmark-result.json")
  $overBudget = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($overBudget.StatusClass -ne "over_budget") { throw "Clean over-budget evidence did not classify as over_budget." }
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nResult=exit-code`nExecMainCode=1`nExecMainStatus=1"
  $malformedUnit = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($malformedUnit.StatusClass -ne "infrastructure_failure") { throw "Malformed measured-failure unit state was not rejected." }
  Set-Content (Join-Path $evidenceRoot "unit-final.txt") "ActiveState=inactive`nResult=exit-code`nExecMainCode=1`nExecMainStatus=1"
  Set-Content (Join-Path $evidenceRoot "unit-stop-evidence.txt") "unit=u.service`nexplicit_stop_after_result=true`nresult_present=true"
  Set-Content (Join-Path $evidenceRoot "unit-stop-before.txt") "ActiveState=failed`nResult=exit-code`nExecMainCode=1`nExecMainStatus=1"
  $explicitStop = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($explicitStop.StatusClass -ne "over_budget") { throw "Explicit post-result unit stop evidence was not accepted." }
  Remove-Item -LiteralPath (Join-Path $evidenceRoot "unit-stop-evidence.txt"), (Join-Path $evidenceRoot "unit-stop-before.txt") -Force
  Set-Content (Join-Path $evidenceRoot "service-restored-state.txt") "restore_status=1`nfinal_active=failed`nfinal_enabled=enabled"
  $restoration = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($restoration.StatusClass -ne "restoration_failure") { throw "Restoration failure did not override benchmark evidence." }
  Remove-Item -LiteralPath (Join-Path $evidenceRoot "benchmark-result.json") -Force
  $missing = Get-OrangeLiveHostEvidence $evidenceRoot $selection ("a" * 64)
  if ($missing.StatusClass -ne "restoration_failure") { throw "Missing result was not retained as a failed evidence class." }
} finally {
  Remove-Item -LiteralPath $evidenceRoot -Recurse -Force -ErrorAction SilentlyContinue
}
$matrixOutput = Invoke-PrintOnly $matrix @{ PrintOnly = $true }
$canaryPrint = Invoke-PrintOnly $matrix @{ PrintOnly = $true; CanaryOnly = $true }
$matrixSource = Get-Content -LiteralPath $matrix -Raw
if ($matrixSource -match '&\s+\$runner') { throw "Matrix still invokes the runner in-process." }
if ($matrixSource -notmatch '(?s)(?=.*& \(Join-Path \$PSHOME "powershell.exe"\) @processArguments 2>&1)(?=.*"-NonInteractive")(?=.*"-File")(?=.*\$LASTEXITCODE)') { throw "Matrix did not preserve isolated PowerShell runner invocation." }
Assert-Contains $matrixOutput "Orange live audio matrix PrintOnly: no transport is invoked."
Assert-Contains $canaryPrint "Orange live audio matrix PrintOnly CanaryOnly: no transport is invoked."
Assert-Contains $canaryPrint "01: synth_ramp_16 output=256 internal=128 measure=30"
Assert-Contains $canaryPrint "Matrix cells: 1 total (CanaryOnly)."
if ($canaryPrint -match "02:|A120|Matrix cells: 29") { throw "CanaryOnly PrintOnly emitted more than one cell." }
if ($matrixOutput -notmatch '(?s)(?=.*01: synth_ramp_16 output=256 internal=128 measure=30)(?=.*11: mixed_cross_slot_48_48_steal output=256 internal=128 measure=30)(?=.*12: A120 scenario=<highest passing A p99.9, then max> output=256 internal=128 measure=120 warmup=5)(?=.*13: synth_ramp_16 output=512 internal=128 measure=30)(?=.*23: mixed_cross_slot_48_48_steal output=512 internal=128 measure=30)(?=.*Matrix cells: 23 total)') { throw "Live matrix PrintOnly output omitted an approved cell or count." }
$capabilitySource = Get-Content -LiteralPath $runner -Raw
$localDirectoryMarker = $capabilitySource.IndexOf('New-Item -ItemType Directory -Force -Path $localRunDirectory | Out-Null', [StringComparison]::Ordinal)
$stagingDirectoryMarker = $capabilitySource.IndexOf('Write-Output "Evidence staging directory: $localRunDirectory"', [StringComparison]::Ordinal)
if ($localDirectoryMarker -lt 0 -or $stagingDirectoryMarker -le $localDirectoryMarker) { throw "Single runner did not emit staging diagnostics immediately after local retrieval setup." }
$emptyManifest = ConvertTo-OrangeLiveManifestJson -Results @()
$oneManifest = ConvertTo-OrangeLiveManifestJson -Results @([pscustomobject]@{ StatusClass = "pass" })
if ($emptyManifest.TrimStart()[0] -ne '[' -or $oneManifest.TrimStart()[0] -ne '[') { throw "Matrix manifests were not serialized as JSON arrays." }
$emptyDecoded = ConvertFrom-Json -InputObject $emptyManifest
$oneDecoded = ConvertFrom-Json -InputObject $oneManifest
if ($emptyDecoded.Count -ne 0 -or $oneDecoded.Count -ne 1) { throw "Matrix manifest array shape was not retained." }
if ((Resolve-OrangeLiveRunnerOutcome "pass" $false) -ne "pass" -or (Resolve-OrangeLiveRunnerOutcome "pass" $true) -ne "infrastructure_failure" -or (Resolve-OrangeLiveRunnerOutcome "over_budget" $true) -ne "over_budget" -or (Resolve-OrangeLiveRunnerOutcome "measured_failure" $true) -ne "measured_failure" -or (Resolve-OrangeLiveRunnerOutcome "restoration_failure" $true) -ne "restoration_failure") { throw "Runner outcome classification changed incorrectly." }
$fakeRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-fake-runner-" + [guid]::NewGuid().ToString("N"))
$fakeEvidence = Join-Path $fakeRoot "orange-study-fedcba987654"
$fakeRunner = Join-Path $fakeRoot "fake-runner.ps1"
$fakeMatrixOutput = Join-Path $fakeRoot "matrix-output"
$oldFakeEvidence = $null
try {
  New-Item -ItemType Directory -Force -Path $fakeEvidence | Out-Null
  $fakeHostEvidence = [pscustomobject]@{ StatusClass = "over_budget"; Reason = "clean measured over-budget fixture"; Scenario = "synth_ramp_16"; OutputFrames = 256; InternalFrames = 64; MeasureSeconds = 30; RatioP999 = 1.2; RatioMax = 1.3 }
  $fakeHostEvidence | ConvertTo-Json | Set-Content (Join-Path $fakeEvidence "host-evidence.json")
  $fakeScript = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
Write-Output ("Evidence directory: " + $env:OCTESSERA_FAKE_LIVE_EVIDENCE)
throw "fake runner failure after evidence output"
'@
  [IO.File]::WriteAllText($fakeRunner, $fakeScript, (New-Object System.Text.UTF8Encoding($false)))
  $oldFakeEvidence = $env:OCTESSERA_FAKE_LIVE_EVIDENCE
  $env:OCTESSERA_FAKE_LIVE_EVIDENCE = $fakeEvidence
  $threw = $false
  try {
    & $matrix -Artifact "fake-artifact" -Metadata "fake-metadata" -OutputDirectory $fakeMatrixOutput -RunnerPath $fakeRunner -AllowMatrixServiceInterruption
  } catch { $threw = $true }
  if (-not $threw) { throw "Fake throwing runner did not stop the matrix." }
  $fakeManifestPath = @(Get-ChildItem -LiteralPath $fakeMatrixOutput -Filter "*.json" -File | Select-Object -First 1).FullName
  $fakeManifest = ConvertFrom-Json -InputObject (Get-Content -LiteralPath $fakeManifestPath -Raw)
  if ($fakeManifest.Count -ne 1 -or $fakeManifest[0].StatusClass -ne "over_budget" -or $fakeManifest[0].RunId -ne "fedcba987654" -or -not [bool]$fakeManifest[0].RunnerThrew -or [int]$fakeManifest[0].ExitCode -ne 1 -or -not (Test-Path -LiteralPath ([string]$fakeManifest[0].TranscriptPath) -PathType Leaf)) { throw "Throwing runner evidence was not retained in the matrix manifest." }
} finally {
  if ($null -eq $oldFakeEvidence) { Remove-Item Env:\OCTESSERA_FAKE_LIVE_EVIDENCE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_FAKE_LIVE_EVIDENCE = $oldFakeEvidence }
  Remove-Item -LiteralPath $fakeRoot -Recurse -Force -ErrorAction SilentlyContinue
}
$passFakeRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-pass-runner-" + [guid]::NewGuid().ToString("N"))
$passFakeEvidence = Join-Path $passFakeRoot "orange-study-0123456789ab"
$passFakeRunner = Join-Path $passFakeRoot "fake-pass-runner.ps1"
$passFakeMatrixOutput = Join-Path $passFakeRoot "matrix-output"
try {
  New-Item -ItemType Directory -Force -Path $passFakeEvidence | Out-Null
  [pscustomobject]@{ StatusClass = "pass"; Reason = "native stderr was benign"; Scenario = "synth_ramp_16"; OutputFrames = 256; InternalFrames = 64; MeasureSeconds = 30; RatioP999 = 0.8; RatioMax = 0.9 } | ConvertTo-Json | Set-Content (Join-Path $passFakeEvidence "host-evidence.json")
  $passFakeScript = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
& cmd.exe /c "echo benign native stderr 1>&2"
if ($LASTEXITCODE -ne 0) { exit 1 }
Write-Output ("Evidence staging directory: " + $env:OCTESSERA_FAKE_LIVE_EVIDENCE)
Write-Output ("Evidence directory: " + $env:OCTESSERA_FAKE_LIVE_EVIDENCE)
'@
  [IO.File]::WriteAllText($passFakeRunner, $passFakeScript, (New-Object System.Text.UTF8Encoding($false)))
  $oldPassEvidence = $env:OCTESSERA_FAKE_LIVE_EVIDENCE
  $env:OCTESSERA_FAKE_LIVE_EVIDENCE = $passFakeEvidence
  try {
    & $matrix -Artifact "fake-artifact" -Metadata "fake-metadata" -OutputDirectory $passFakeMatrixOutput -RunnerPath $passFakeRunner -AllowMatrixServiceInterruption -CanaryOnly
  } catch { throw "Native stderr success runner unexpectedly failed: $($_.Exception.Message)" }
  $passManifestPath = @(Get-ChildItem -LiteralPath $passFakeMatrixOutput -Filter "*.json" -File | Select-Object -First 1).FullName
  $passManifest = ConvertFrom-Json -InputObject (Get-Content -LiteralPath $passManifestPath -Raw)
  $passTranscriptPath = [string]$passManifest[0].TranscriptPath
  if ($passManifest.Count -ne 1 -or $passManifest[0].StatusClass -ne "pass" -or [bool]$passManifest[0].RunnerThrew -or [int]$passManifest[0].ExitCode -ne 0 -or $passManifest[0].RunId -ne "0123456789ab" -or -not (Test-Path -LiteralPath $passTranscriptPath -PathType Leaf)) { throw "Native stderr success runner did not produce a clean canary result." }
  $passTranscript = Get-Content -LiteralPath $passTranscriptPath -Raw
  Assert-Contains $passTranscript "benign native stderr"
  Assert-Contains $passTranscript "Evidence staging directory:"
} finally {
  if ($null -eq $oldPassEvidence) { Remove-Item Env:\OCTESSERA_FAKE_LIVE_EVIDENCE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_FAKE_LIVE_EVIDENCE = $oldPassEvidence }
  Remove-Item -LiteralPath $passFakeRoot -Recurse -Force -ErrorAction SilentlyContinue
}
$partialFakeRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-partial-runner-" + [guid]::NewGuid().ToString("N"))
$partialFakeEvidence = Join-Path $partialFakeRoot "orange-study-abcdefabcdef"
$partialFakeRunner = Join-Path $partialFakeRoot "fake-partial-runner.ps1"
$partialFakeMatrixOutput = Join-Path $partialFakeRoot "matrix-output"
try {
  New-Item -ItemType Directory -Force -Path $partialFakeEvidence | Out-Null
  $partialFakeScript = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
Write-Output ("Evidence staging directory: " + $env:OCTESSERA_FAKE_LIVE_EVIDENCE)
Write-Output "partial evidence diagnostic before terminal evidence"
throw "fake runner failed before terminal evidence"
'@
  [IO.File]::WriteAllText($partialFakeRunner, $partialFakeScript, (New-Object System.Text.UTF8Encoding($false)))
  $oldPartialEvidence = $env:OCTESSERA_FAKE_LIVE_EVIDENCE
  $env:OCTESSERA_FAKE_LIVE_EVIDENCE = $partialFakeEvidence
  $partialThrew = $false
  try {
    & $matrix -Artifact "fake-artifact" -Metadata "fake-metadata" -OutputDirectory $partialFakeMatrixOutput -RunnerPath $partialFakeRunner -AllowMatrixServiceInterruption -CanaryOnly
  } catch { $partialThrew = $true }
  if (-not $partialThrew) { throw "Partial staging runner did not stop the canary." }
  $partialManifestPath = @(Get-ChildItem -LiteralPath $partialFakeMatrixOutput -Filter "*.json" -File | Select-Object -First 1).FullName
  $partialManifest = ConvertFrom-Json -InputObject (Get-Content -LiteralPath $partialManifestPath -Raw)
  $partialTranscriptPath = [string]$partialManifest[0].TranscriptPath
  if ($partialManifest.Count -ne 1 -or $partialManifest[0].StatusClass -ne "infrastructure_failure" -or $partialManifest[0].EvidenceDirectory -ne $partialFakeEvidence -or $partialManifest[0].RunId -ne "abcdefabcdef" -or -not [bool]$partialManifest[0].RunnerThrew -or [int]$partialManifest[0].ExitCode -ne 1 -or -not (Test-Path -LiteralPath $partialTranscriptPath -PathType Leaf)) { throw "Partial staging evidence was not retained as infrastructure failure." }
  if ([string]$partialManifest[0].CapturedDiagnostic -notmatch "partial evidence diagnostic|fake runner failed before terminal evidence") { throw "Partial staging diagnostics were not retained." }
} finally {
  if ($null -eq $oldPartialEvidence) { Remove-Item Env:\OCTESSERA_FAKE_LIVE_EVIDENCE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_FAKE_LIVE_EVIDENCE = $oldPartialEvidence }
  Remove-Item -LiteralPath $partialFakeRoot -Recurse -Force -ErrorAction SilentlyContinue
}
$runnerArtifact = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-live-benchmark-missing"
$runnerParameters = @{ Mode = "LiveAudioBenchmark"; Scenario = "synth_cross_slot_96_steal"; OutputFrames = 256; EngineBlockFrames = 256; MeasureSeconds = 30; Artifact = $runnerArtifact; Metadata = "$runnerArtifact.metadata.json"; AllowServiceInterruption = $true; PrintOnly = $true }
$recoveryEvidenceRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-recovery-" + [guid]::NewGuid().ToString("N"))
try {
  New-Item -ItemType Directory -Force -Path $recoveryEvidenceRoot | Out-Null
  [pscustomobject]@{ StatusClass = "over_budget"; Reason = "earlier measured failure" } | ConvertTo-Json | Set-Content (Join-Path $recoveryEvidenceRoot "host-evidence.json")
  . $runner @runnerParameters | Out-Null
  Write-LiveRestorationFailureEvidence -EvidenceDirectory $recoveryEvidenceRoot -Reason "later cleanup failed" | Out-Null
  $recoveryOverride = ConvertFrom-Json -InputObject (Get-Content -LiteralPath (Join-Path $recoveryEvidenceRoot "host-evidence.json") -Raw)
  if ($recoveryOverride.StatusClass -ne "restoration_failure" -or $recoveryOverride.Reason -notmatch "later cleanup failed") { throw "Cleanup recovery failure did not override earlier measured evidence." }
} finally {
  Remove-Item -LiteralPath $recoveryEvidenceRoot -Recurse -Force -ErrorAction SilentlyContinue
}
$first = Invoke-PrintOnly $runner $runnerParameters
$second = Invoke-PrintOnly $runner $runnerParameters
if ($first -notmatch '(?s)(?=.*RuntimeMaxSec=185s)(?=.*RuntimeDirectoryPreserve=yes)(?=.*--benchmark-orange-audio)(?=.*--release-gate)(?=.*--output-frames 256 --engine-block-frames 256)(?=.*--measure-seconds 30)(?=.*sensor_abort)') { throw "Live payload omitted a required runtime or benchmark marker." }
if ([regex]::Matches($first, 'systemd-run --unit="\$unit"').Count -ne 1) { throw "Live payload did not contain exactly one transient systemd-run launch." }
if ($first -match "runtime-thermal-abort|70000|75000" -or $first -notmatch '(?s)(?=.*thermal-unreadable)(?=.*memory-unreadable)(?=.*thermal-missing)(?=.*runtime-memory-abort)(?=.*consecutive_samples)') { throw "Live payload changed its thermal or memory safety contract." }
Assert-Contains $first "validate_benchmark_worker_threads"
Assert-Contains $first "oct-dsp-src-0"
Assert-Contains $first "oct-dsp-src-1"
Assert-Contains $first "oct-src-reaper"
if ($first -match "octessera-source-reaper") { throw "Live payload retained the prior overlong reaper name." }
Assert-Contains $first 'systemctl stop "$unit"'
Assert-Contains $first "benchmark-result-final.json"
Assert-Contains $first "unit-stop-evidence"
Assert-Contains $first "service-restored-ready.json"
Assert-Contains $first "alsa-hw-params.txt"
Assert-Contains $first "release.json"
Assert-Contains $first 'benchmark_root='
Assert-Contains $first 'sudo -n rm -rf -- "$benchmark_root"'
if ($first -match "rm -rf -- '/run/octessera'") { throw "Live cleanup contained a broad /run/octessera removal." }
if ($first -match "rm -f -- '/run/octessera/candidate-ready.json'") { throw "Live cleanup attempted to remove production candidate-ready.json." }
$studyStart = $first.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
$studyEnd = $first.IndexOf("Study payload transport:", $studyStart, [StringComparison]::Ordinal)
$studyPayload = $first.Substring($studyStart, $studyEnd - $studyStart)
$captureStart = $studyPayload.IndexOf("capture_alsa_release() {", [StringComparison]::Ordinal)
$captureEnd = $studyPayload.IndexOf("validate_benchmark_progress() {", $captureStart, [StringComparison]::Ordinal)
if ($captureStart -lt 0 -or $captureEnd -lt 0) { throw "Generated capture_alsa_release fixture could not be extracted." }
$captureFunction = $studyPayload.Substring($captureStart, $captureEnd - $captureStart)
$restoreStart = $studyPayload.IndexOf("restore_service() {", [StringComparison]::Ordinal)
$onExitStart = $studyPayload.IndexOf("on_exit() {", [StringComparison]::Ordinal)
$onExit = $studyPayload.Substring($onExitStart, $studyPayload.IndexOf('test -d "$root"', $onExitStart, [StringComparison]::Ordinal) - $onExitStart)
$restore = $studyPayload.Substring($restoreStart, $onExitStart - $restoreStart)
$cleanupStart = $first.IndexOf("Cleanup payload:", [StringComparison]::Ordinal) + "Cleanup payload:`n".Length
$cleanupPayload = $first.Substring($cleanupStart)
if ($onExit -notmatch '(?s)capture_transient_evidence.*restore_service' -or $restore -notmatch '(?s)stop_benchmark_unit.*reset_failed_unit') { throw "Transient-unit reset was not ordered after evidence capture and stop." }
if ($studyPayload -notmatch '(?s)systemctl reset-failed "\$unit".*cleanup_status=1.*restore_status=1' -or $cleanupPayload -notmatch 'reset_status=1' -or $cleanupPayload -notmatch 'exit 72') { throw "Transient-unit reset failure was not retained as cleanup/restoration failure." }
if ($first -match 'reset-failed[^\r\n]*\*') { throw "Transient-unit reset used a broad wildcard." }
if ($cleanupPayload -notmatch '(?s)systemctl stop "\$unit".*?esac\s+reset_failed_unit\s+sudo -n rm -f') { throw "Standalone cleanup reset was not ordered after its exact-unit stop." }
$captureFixture = @'
set -u
root="$(mktemp -d)"
release="$root/release.json"
expected_sha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
benchmark_pid=123
benchmark_invocation=fixture-invocation
positive_number() { case "$1" in ''|0|*[!0-9]*) return 1;; *) return 0;; esac; }
sudo() { [ "$1" = -n ] && shift; case "$1" in chown|chmod) return 0;; *) "$@";; esac; }
find_dac_hw_params() {
  case "$FIXTURE_CASE" in
    missing) return 1;;
    unreadable) printf '%s\n' "$root/not-readable";;
    *) printf '%s\n' "$root/hw_params";;
  esac
}
__CAPTURE_FUNCTION__
run_case() {
  FIXTURE_CASE="$1"
  rm -f -- "$release" "$root/benchmark-release.json" "$release.tmp-$$" "$root/hw_params" "$root/not-readable"
  case "$FIXTURE_CASE" in
    exact) printf 'buffer_size: 256\nperiod_size: 64\n' > "$root/hw_params";;
    mismatch) printf 'buffer_size: 512\nperiod_size: 64\n' > "$root/hw_params";;
  esac
  if capture_alsa_release; then
    [ "$FIXTURE_CASE" = exact ] || exit 1
    grep -q '"observed_alsa_buffer_frames":256' "$release"
    grep -q '"observed_alsa_period_frames":64' "$release"
    cmp -s "$release" "$root/benchmark-release.json"
  else
    [ ! -e "$release" ] || exit 1
  fi
}
run_case exact
run_case mismatch
run_case missing
run_case unreadable
rm -rf -- "$root"
'@
$captureFixture = $captureFixture.Replace("__CAPTURE_FUNCTION__", $captureFunction)
$captureFixture | & bash -s
if ($LASTEXITCODE -ne 0) { throw "Generated capture_alsa_release execution fixtures failed." }
$threadStart = $studyPayload.IndexOf("validate_benchmark_worker_threads() {", [StringComparison]::Ordinal)
$threadEnd = $studyPayload.IndexOf("wait_for_benchmark_readiness() {", $threadStart, [StringComparison]::Ordinal)
if ($threadStart -lt 0 -or $threadEnd -lt 0) { throw "Generated worker-thread validator fixture could not be extracted." }
$threadFunction = $studyPayload.Substring($threadStart, $threadEnd - $threadStart)
$threadFixture = @'
set -u
root="$(mktemp -d)"
pid=123
mkdir -p "$root/$pid/task/1" "$root/$pid/task/2" "$root/$pid/task/3"
printf 'oct-dsp-src-0\n' > "$root/$pid/task/1/comm"
printf 'oct-dsp-src-1\n' > "$root/$pid/task/2/comm"
printf 'oct-src-reaper\n' > "$root/$pid/task/3/comm"
__THREAD_FUNCTION__
validate_benchmark_worker_threads "$pid" "$root"
printf 'octessera-sourc\n' > "$root/$pid/task/3/comm"
if validate_benchmark_worker_threads "$pid" "$root"; then exit 1; fi
printf 'oct-src-reaper\n' > "$root/$pid/task/3/comm"
mkdir -p "$root/$pid/task/4"
printf 'oct-dsp-src-0\n' > "$root/$pid/task/4/comm"
if validate_benchmark_worker_threads "$pid" "$root"; then exit 1; fi
rm -rf -- "$root"
'@
$threadFixture = $threadFixture.Replace("__THREAD_FUNCTION__", $threadFunction)
$threadFixture | & bash -s
if ($LASTEXITCODE -ne 0) { throw "Worker-thread validator did not reject the prior Linux-truncated reaper name." }
$firstRemote = [regex]::Match($first, "Remote study root: (?<root>/tmp/[^\r\n]+)").Groups["root"].Value
$secondRemote = [regex]::Match($second, "Remote study root: (?<root>/tmp/[^\r\n]+)").Groups["root"].Value
if ([string]::IsNullOrWhiteSpace($firstRemote) -or $firstRemote -eq $secondRemote) { throw "Live PrintOnly paths were not unique." }
$bashCommand = Get-Command bash -ErrorAction SilentlyContinue
$wslCommand = Get-Command wsl.exe -ErrorAction SilentlyContinue
if ($null -ne $bashCommand -and [string]$bashCommand.Source -notmatch "WindowsApps") {
  $temporary = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    $studyStart = $first.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
    $studyEnd = $first.IndexOf("Study payload transport:", $studyStart, [StringComparison]::Ordinal)
    [IO.File]::WriteAllText($temporary, $first.Substring($studyStart, $studyEnd - $studyStart), (New-Object System.Text.UTF8Encoding($false)))
    & bash -n $temporary
    if ($LASTEXITCODE -ne 0) { throw "Generated live payload failed bash -n." }
  } finally {
    Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
  }
} elseif ($null -ne $wslCommand) {
  $temporary = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    $studyStart = $first.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
    $studyEnd = $first.IndexOf("Study payload transport:", $studyStart, [StringComparison]::Ordinal)
    [IO.File]::WriteAllText($temporary, $first.Substring($studyStart, $studyEnd - $studyStart), (New-Object System.Text.UTF8Encoding($false)))
    $drive = $temporary.Substring(0, 1).ToLowerInvariant()
    $wslPath = "/mnt/$drive" + ($temporary.Substring(2) -replace "\\", "/")
    & wsl.exe bash -n $wslPath
    if ($LASTEXITCODE -ne 0) { throw "Generated live payload failed WSL bash -n." }
  } finally {
    Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
  }
}

Write-Output "Orange live benchmark matrix, selection, PrintOnly, identity, safety, release, and payload tests passed"
