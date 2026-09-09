$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$runnerPath = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
Import-Module (Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-benchmark-payloads.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-result-evidence-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-study-outcome-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-cross-metadata.psm1") -Force

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected validation failure did not occur." }
}

function New-OrangeSummaryResult {
  param([Parameter(Mandatory)][int]$MeasureSeconds)
  $callback = [pscustomobject][ordered]@{
    callback_count = 10; first_measured_callback_ns = 1; last_measured_callback_ns = 10; measured_elapsed_ns = 9
    callback_frames_min = 256; callback_frames_max = 256; callback_frame_sample_count = 10; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0
    callback_timestamp_observed = $true; worker_terminal = $false; terminal_error = $false; pre_mute_nonzero_samples = 10; post_mute_nonzero_samples = 0
    over_audio_duration_budget_count = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; rendered_frames = 2560; render_audio_duration_ns = 1000000
    render_audio_duration_ratio_p50 = 0.1; render_audio_duration_ratio_p95 = 0.1; render_audio_duration_ratio_p99 = 0.1; render_audio_duration_ratio_p99_9 = 0.1; render_audio_duration_ratio_max = 0.2
  }
  [pscustomobject][ordered]@{
    schema_version = 13; kind = "orange_audio_benchmark_result"; status = "fail"; board_profile = "orange-pi-zero-2w"; scenario = "capacity_analogue_12"
    requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 64; sample_format = "F32"; channels = 2; sample_rate = 44100; warmup_seconds = 5; measure_seconds = $MeasureSeconds
    scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; post_dsp_zero = $true; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true
    pid = 123; systemd_invocation_id = "invocation"; artifact_sha256 = ("a" * 64); callback = $callback; persistent_output_counters = $null; persistent_output_provenance = $null; detected_continuity_events = 0; profile_start = $null; profile_end = $null
    recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; executor_mode = "routing_tree_persistent"; continue_on_recovered_miss = $true; lookahead_frames = 64; effective_output_latency_frames = 320
    worker_health = "healthy"; worker_thread_name_0 = "oct-dsp-tree-0"; worker_thread_name_1 = "oct-dsp-tree-1"; joined_workers = 2; retirement_error = $null; worker_timing_mode = "disabled"; worker_timing = $null
  }
}

function Invoke-PrintOnly {
  param([hashtable]$Parameters)
  $global:LASTEXITCODE = 0
  $output = @(& $runnerPath @Parameters 2>&1)
  if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "Frame-search runner exited with code $LASTEXITCODE." }
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

function New-DiagnosticFixture {
  param([Parameter(Mandatory)][string]$Feature)
  $directory = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-frame-" + [guid]::NewGuid().ToString("N"))
  $binary = Join-Path $directory "octessera-pi"
  $metadata = "$binary.metadata.json"
  New-Item -ItemType Directory -Path $directory | Out-Null
  [IO.File]::WriteAllBytes($binary, [byte[]](1, 2, 3, 4))
  Publish-OrangeBuildMetadata -BinaryPath $binary -MetadataPath $metadata -SelectedBinary "octessera-pi" -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec ([pscustomobject]@{ Package = "octessera-pi"; Feature = $Feature; ArtifactKind = "diagnostic-only" }) -SourceCommit ("c" * 40)
  return [pscustomobject]@{ Directory = $directory; Binary = $binary; Metadata = $metadata }
}

$expectedProfiles = @{
  OI32 = @("inline", 128, 32, 32, 0, 128, "disabled", $false)
  OI64 = @("inline", 128, 32, 64, 0, 128, "disabled", $false)
  OI256 = @("inline", 256, 64, 64, 0, 256, "disabled", $false)
  OM32 = @("routing_tree_persistent", 128, 32, 32, 32, 160, "disabled", $true)
  OM64 = @("routing_tree_persistent", 256, 64, 64, 64, 320, "disabled", $true)
  OM128 = @("routing_tree_persistent", 256, 64, 128, 128, 384, "disabled", $true)
}
foreach ($profileName in $expectedProfiles.Keys) {
  $selection = Assert-OrangeFrameSearchSelection -FrameSearchProfile $profileName -FrameSearchPhase Preliminary -FrameSearchU 1
  $expected = $expectedProfiles[$profileName]
  if ($selection.ExecutorMode -cne $expected[0] -or $selection.OutputFrames -ne $expected[1] -or $selection.AlsaPeriodFrames -ne $expected[2] -or $selection.InternalFrames -ne $expected[3] -or $selection.LookaheadFrames -ne $expected[4] -or $selection.EffectiveOutputLatencyFrames -ne $expected[5] -or $selection.WorkerTimingMode -cne $expected[6] -or $selection.ContinueOnRecoveredMiss -ne $expected[7] -or $selection.RequiredPoolStage -ne 128 -or $selection.DiagnosticPoolIdentity -cne "benchmark-voice-pools-128") { throw "Frame-search profile geometry changed: $profileName" }
  if ($selection.MeasureSeconds -ne 180 -or $selection.FrameSearchU -ne 1) { throw "Preliminary frame-search selection changed: $profileName" }
}
$preliminary = Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 42
$soak = Assert-OrangeFrameSearchSelection -FrameSearchProfile OM128 -FrameSearchPhase Soak -FrameSearchU 42
if ($preliminary.MeasureSeconds -ne 180 -or $soak.MeasureSeconds -ne 600 -or $soak.ContinueOnRecoveredMiss -ne $true -or $soak.DiagnosticArtifactFeature -cne "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128") { throw "Frame-search boundary selections changed." }
if ((Assert-OrangeLiveBenchmarkSelection -FrameSearchProfile OI64 -FrameSearchPhase Preliminary -FrameSearchU 1).InternalFrames -ne 64) { throw "Frame-search live selection API did not retain OI64." }

foreach ($phaseCase in @(@{ Phase = "Preliminary"; Seconds = 180 }, @{ Phase = "Soak"; Seconds = 600 })) {
  $om128 = Assert-OrangeFrameSearchSelection -FrameSearchProfile OM128 -FrameSearchPhase $phaseCase.Phase -FrameSearchU 12
  $payload = New-OrangeLiveBenchmarkPayloadBundle -Selection $om128 -RemoteRoot "/tmp/frame-search" -BenchmarkRoot "/run/octessera/frame-search" -HealthPath "/run/octessera/frame-health.json" -ArtifactHash ("a" * 64) -Unit "octessera-frame-search.service" -Service "octessera.service" -StartupTimeoutSeconds 20 -ReleaseTimeoutSeconds 120 -RuntimeMaxSeconds ($phaseCase.Seconds + 155) -WorkerTimingMode $om128.WorkerTimingMode -ExecutorMode $om128.ExecutorMode -ExpectedArtifactKind "diagnostic-only" -ExpectedCargoFeature $om128.DiagnosticArtifactFeature -ContinueOnRecoveredMiss:$om128.ContinueOnRecoveredMiss
  $nativeTuple = "--benchmark-orange-audio --executor routing_tree_persistent --scenario capacity_analogue_12 --output-frames 256 --engine-block-frames 128 --worker-timing disabled --warmup-seconds 5 --measure-seconds $($phaseCase.Seconds)"
  if ($payload.Study.IndexOf($nativeTuple, [StringComparison]::Ordinal) -lt 0 -or $payload.Study.IndexOf('[ "$(json_field lookahead_frames "$marker")" = 128 ]', [StringComparison]::Ordinal) -lt 0 -or $payload.Study.IndexOf('[ "$(json_field expected_alsa_period_frames "$marker")" = 64 ]', [StringComparison]::Ordinal) -lt 0 -or $payload.Study.IndexOf('if [ "$buffer" != 256 ] || [ "$period" != 64 ]', [StringComparison]::Ordinal) -lt 0 -or $payload.Study.IndexOf('[ "$(json_field effective_output_latency_frames "$result")" = 384 ]', [StringComparison]::Ordinal) -lt 0 -or $payload.Study.IndexOf('--continue-on-recovered-miss || launch_status=$?', [StringComparison]::Ordinal) -lt 0) { throw "OM128 $($phaseCase.Phase) native payload tuple was not exact." }
  if ($payload.Study.IndexOf("--worker-timing enabled", [StringComparison]::Ordinal) -ge 0 -or $payload.Study.IndexOf("--measure-seconds $($phaseCase.Seconds + 1)", [StringComparison]::Ordinal) -ge 0) { throw "OM128 $($phaseCase.Phase) native payload contradicted the selected tuple." }
}

foreach ($invalidU in @("", "0", "01", "43", "1.0", [double]1, $true, 0, 43, @("1"))) {
  Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU $invalidU }
}
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile INVALID -FrameSearchPhase Preliminary -FrameSearchU 1 }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase INVALID -FrameSearchU 1 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -FrameSearchProfile OI32 -FrameSearchU 1 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -FrameSearchPhase Preliminary -FrameSearchU 1 }
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 1 -OutputFrames 256 }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 1 -EngineBlockFrames 64 }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 1 -MeasureSeconds 600 }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 1 -ExecutorMode routing_tree_persistent }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OM128 -FrameSearchPhase Preliminary -FrameSearchU 1 -WorkerTimingMode enabled }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 1 -ContinueOnRecoveredMiss:$true }
Assert-Throws { Assert-OrangeFrameSearchSelection -FrameSearchProfile OI32 -FrameSearchPhase Preliminary -FrameSearchU 1 -AllowLongRepeat:$true }

foreach ($gradeCase in @(
    @{ Seconds = 120; Incidents = 1; Grade = "Stable" }, @{ Seconds = 120; Incidents = 2; Grade = "Stretched" }, @{ Seconds = 120; Incidents = 4; Grade = "Stretched" }, @{ Seconds = 120; Incidents = 5; Grade = "Compromised" },
    @{ Seconds = 180; Incidents = 2; Grade = "Stable" }, @{ Seconds = 180; Incidents = 3; Grade = "Stretched" }, @{ Seconds = 180; Incidents = 7; Grade = "Stretched" }, @{ Seconds = 180; Incidents = 8; Grade = "Compromised" },
    @{ Seconds = 600; Incidents = 9; Grade = "Stable" }, @{ Seconds = 600; Incidents = 10; Grade = "Stretched" }, @{ Seconds = 600; Incidents = 24; Grade = "Stretched" }, @{ Seconds = 600; Incidents = 25; Grade = "Compromised" }
  )) {
  $grade = Get-OrangeLivePracticalGrade -RepeatIncidents $gradeCase.Incidents -SilentIncidents 0 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds $gradeCase.Seconds
  if ($grade -cne $gradeCase.Grade) { throw "Practical grade threshold changed for $($gradeCase.Seconds)/$($gradeCase.Incidents)." }
}
if ((Get-OrangeLivePracticalGrade -RepeatIncidents 2 -SilentIncidents 2 -AlsaRecoveryLogIncidents 0 -CpalStreamErrors 0 -CpalDeviceErrors 0 -MeasureSeconds 180) -cne "Stable") { throw "Practical grade summed incident categories instead of using the worst category." }
$soakSummarySelection = Assert-OrangeFrameSearchSelection -FrameSearchProfile OM64 -FrameSearchPhase Soak -FrameSearchU 12
$soakSummaryResult = New-OrangeSummaryResult 600
foreach ($overrunCase in @(@{ Count = 5; Expected = "measured_failure" }, @{ Count = 9; Expected = "measured_failure" }, @{ Count = 10; Expected = "over_budget" })) {
  $soakSummaryResult.callback.over_audio_duration_budget_count = $overrunCase.Count
  $soakSummaryResult.detected_continuity_events = $overrunCase.Count
  if ((Get-OrangeLiveResultSummary -Result $soakSummaryResult -Selection $soakSummarySelection).StatusClass -cne $overrunCase.Expected) { throw "600-second soak callback-overrun policy changed at $($overrunCase.Count)." }
}

$expectedExit = "Orange transport failed with exit code 20: ssh-payload"
foreach ($case in @(
    @{ Profile = "OI32"; Phase = "Preliminary"; U = 1; Continue = $false },
    @{ Profile = "OI64"; Phase = "Soak"; U = 42; Continue = $false },
    @{ Profile = "OM32"; Phase = "Preliminary"; U = 1; Continue = $true },
    @{ Profile = "OM128"; Phase = "Soak"; U = 42; Continue = $true }
  )) {
  $selection = Assert-OrangeFrameSearchSelection -FrameSearchProfile $case.Profile -FrameSearchPhase $case.Phase -FrameSearchU $case.U
  Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass measured_failure -StudyFailure $expectedExit -ContinueOnRecoveredMiss:$case.Continue
  Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass over_budget -StudyFailure $expectedExit -ContinueOnRecoveredMiss:$case.Continue
  foreach ($fatalStatus in @("safety_failure", "infrastructure_failure", "restoration_failure")) {
    Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass $fatalStatus -StudyFailure $expectedExit -ContinueOnRecoveredMiss:$case.Continue }
  }
}
$malformedSelection = ConvertFrom-Json -InputObject ($preliminary | ConvertTo-Json -Depth 8)
$malformedSelection.FrameSearchGeometry.EffectiveOutputLatencyFrames = 999
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $malformedSelection -HostStatusClass measured_failure -StudyFailure $expectedExit }
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $preliminary -HostStatusClass measured_failure -StudyFailure $expectedExit -RecoveryFailure ([Exception]::new("cleanup failed")) }

$inlineFixture = New-DiagnosticFixture "hardware-orange-pi-zero-2w benchmark-voice-pools-128"
$routingFixture = New-DiagnosticFixture "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128"
$wrongInlineFixture = New-DiagnosticFixture "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128"
$wrongRoutingFixture = New-DiagnosticFixture "hardware-orange-pi-zero-2w benchmark-voice-pools-128"
try {
  $inlineOutput = Invoke-PrintOnly @{ Mode = "LiveAudioBenchmark"; FrameSearchProfile = "OI32"; FrameSearchPhase = "Preliminary"; FrameSearchU = 1; Artifact = $inlineFixture.Binary; Metadata = $inlineFixture.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  if ($inlineOutput -notmatch "Frame-search: profile=OI32 phase=Preliminary U=1 geometry=output=128,period=32,internal=32,lookahead=0,effective=128" -or $inlineOutput -notmatch "cargo_feature=hardware-orange-pi-zero-2w benchmark-voice-pools-128") { throw "Inline frame-search identity or evidence output changed." }
  $routingOutput = Invoke-PrintOnly @{ Mode = "LiveAudioBenchmark"; FrameSearchProfile = "OM128"; FrameSearchPhase = "Soak"; FrameSearchU = 42; Artifact = $routingFixture.Binary; Metadata = $routingFixture.Metadata; AllowServiceInterruption = $true; PrintOnly = $true }
  if ($routingOutput -notmatch "Frame-search: profile=OM128 phase=Soak U=42 geometry=output=256,period=64,internal=128,lookahead=128,effective=384" -or $routingOutput -notmatch "cargo_feature=hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128" -or $routingOutput -notmatch "--continue-on-recovered-miss") { throw "Routing frame-search identity or continuation output changed." }
  Assert-Throws { Invoke-PrintOnly @{ Mode = "LiveAudioBenchmark"; FrameSearchProfile = "OI32"; FrameSearchPhase = "Preliminary"; FrameSearchU = 1; Artifact = $wrongInlineFixture.Binary; Metadata = $wrongInlineFixture.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
  Assert-Throws { Invoke-PrintOnly @{ Mode = "LiveAudioBenchmark"; FrameSearchProfile = "OM128"; FrameSearchPhase = "Soak"; FrameSearchU = 42; Artifact = $wrongRoutingFixture.Binary; Metadata = $wrongRoutingFixture.Metadata; AllowServiceInterruption = $true; PrintOnly = $true } | Out-Null }
} finally {
  Remove-Item -LiteralPath $inlineFixture.Directory, $routingFixture.Directory, $wrongInlineFixture.Directory, $wrongRoutingFixture.Directory -Recurse -Force -ErrorAction SilentlyContinue
}

$evidenceDirectory = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-frame-evidence-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $evidenceDirectory | Out-Null
try {
  Set-Content -LiteralPath (Join-Path $evidenceDirectory "study-result.txt") -Value @("status_class=infrastructure_failure", "interruption_started=false")
  $frameEvidence = Get-OrangeLiveHostEvidence -EvidenceDirectory $evidenceDirectory -Selection $preliminary -ArtifactHash ("a" * 64)
  if ($frameEvidence.FrameSearchProfile -cne "OI32" -or $frameEvidence.FrameSearchPhase -cne "Preliminary" -or $frameEvidence.FrameSearchU -ne 42 -or $frameEvidence.FrameSearchGeometry.OutputFrames -ne 128) { throw "Frame-search host evidence omitted profile, phase, or geometry." }
  Set-Content -LiteralPath (Join-Path $evidenceDirectory "study-result.txt") -Value @("status_class=measured_failure", "interruption_started=false")
  Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-result.json") -Value "{}"
  Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-readiness.json") -Value "{}"
  $malformedEvidence = Get-OrangeLiveHostEvidence -EvidenceDirectory $evidenceDirectory -Selection $preliminary -ArtifactHash ("a" * 64)
  if ($malformedEvidence.StatusClass -cne "infrastructure_failure") { throw "Malformed frame-search evidence was not fatal." }
  Set-Content -LiteralPath (Join-Path $evidenceDirectory "study-result.txt") -Value @("status_class=measured_failure", "interruption_started=true")
  Set-Content -LiteralPath (Join-Path $evidenceDirectory "service-restored-state.txt") -Value @("restore_status=1", "final_active=inactive", "final_enabled=disabled")
  $restorationEvidence = Get-OrangeLiveHostEvidence -EvidenceDirectory $evidenceDirectory -Selection $preliminary -ArtifactHash ("a" * 64)
  if ($restorationEvidence.StatusClass -cne "restoration_failure") { throw "Restoration failure did not take precedence." }
} finally {
  Remove-Item -LiteralPath $evidenceDirectory -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange frame-search selection, identity, outcome, grading, evidence, and fail-closed tests passed"
