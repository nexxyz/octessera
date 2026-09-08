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
if ($inline.Scenario -cne "capacity_analogue_16" -or $inline.NativeExecutorMode -cne "inline" -or $inline.LookaheadFrames -ne 0) { throw "Inline Raspberry benchmark selection changed." }
$multicore = Assert-RaspberryLiveBenchmarkSelection -Units 32 -ExecutorMode Multicore -MeasureSeconds 120
if ($multicore.Scenario -cne "capacity_analogue_32" -or $multicore.NativeExecutorMode -cne "routing_tree_persistent" -or $multicore.WorkerTimingMode -cne "enabled" -or $multicore.LookaheadFrames -ne 128) { throw "Multicore Raspberry benchmark selection changed." }

$testHash = "a" * 64
function New-TestProfile {
  param([int]$Units = 16)
  [pscustomobject][ordered]@{
    active_synth_voices = 3 * $Units
    active_sample_voices = $Units
    active_preview_sample_voices = 0
    active_momentary_fx = 2
    active_bus_fx_slots = 8
    active_global_fx_slots = 2
    cumulative_voice_steals = 0
    cumulative_voice_admission_drops = 0
  }
}
function New-TestCounters {
  $zero = [pscustomobject][ordered]@{ rendered_quantums = 0; repeated_quantums = 0; dropped_quantums = 0; deadline_misses = 0; deadline_recoveries = 0 }
  [pscustomobject][ordered]@{ observable = $false; warmup = $zero; start = $zero; end = $zero; delta = $zero }
}
function New-TestCallback {
  param([uint64]$OverrunCount = 0)
  [pscustomobject][ordered]@{
    lifetime_callback_count = 10; callback_count = 10; first_measured_callback_ns = 1; last_measured_callback_ns = 10; measured_elapsed_ns = 9; callback_frames_min = 256; callback_frames_max = 256; callback_frame_sample_count = 10; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0; lifetime_callback_frames_min = 256; lifetime_callback_frames_max = 256; lifetime_callback_frame_sample_count = 10; lifetime_callback_frame_size_change_count = 0; lifetime_invalid_callback_frame_count = 0; rendered_frames = 2560; render_audio_duration_ns = 100; render_audio_duration_ratio_p50 = 0; render_audio_duration_ratio_p95 = 0; render_audio_duration_ratio_p99 = 0; render_audio_duration_ratio_p99_9 = 0; render_audio_duration_ratio_max = 0; over_audio_duration_budget_count = $OverrunCount; callback_spacing_min_ns = 1; callback_spacing_max_ns = 1; callback_lateness_max_ns = 0; callback_timestamp_observed = $true; pre_mute_nonzero_samples = 10; pre_mute_peak = 1; post_mute_nonzero_samples = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; worker_terminal = $false; terminal_error = $false
  }
}
function New-TestResult {
  param([string]$Status = "pass", [uint64]$OverrunCount = 0)
  [pscustomobject][ordered]@{
    schema_version = 12; kind = "raspberry_audio_benchmark_result"; status = $Status; board_profile = "raspberry-pi-zero-2w"; artifact_sha256 = $testHash; scenario = "capacity_analogue_16"; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 128; lookahead_frames = 0; effective_output_latency_frames = 256; sample_format = "F32"; pid = 1234; systemd_invocation_id = "invocation"; sample_rate = 44100; channels = 2; executor_mode = "inline"; worker_timing_mode = "disabled"; warmup_seconds = 5; measure_seconds = 30; scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; post_dsp_zero = $true; recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; worker_health = "disabled"; worker_thread_name_0 = ""; worker_thread_name_1 = ""; joined_workers = 0; retirement_error = $null; worker_timing = $null; detected_continuity_events = 0; profile_start = New-TestProfile; profile_end = New-TestProfile; persistent_output_counters = New-TestCounters; callback = New-TestCallback $OverrunCount
  }
}
function Write-TestEvidence {
  param([Parameter(Mandatory)][string]$Directory, [Parameter(Mandatory)][pscustomobject]$Result, [Parameter(Mandatory)][string]$StudyStatus, [string]$RestoreStatus = "0")
  New-Item -ItemType Directory -Force -Path $Directory | Out-Null
  Set-Content (Join-Path $Directory "benchmark-identity.txt") "main_pid=1234`ninvocation_id=invocation" -Encoding UTF8
  $readiness = [pscustomobject][ordered]@{ schema_version = 5; kind = "raspberry_audio_benchmark_readiness"; status = "ready"; board_profile = "raspberry-pi-zero-2w"; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = $testHash; scenario = "capacity_analogue_16"; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 128; lookahead_frames = 0; sample_rate = 44100; channels = 2; sample_format = "F32"; scheduler_qualified = $true; post_dsp_zero = $true; executor_mode = "inline"; worker_health = "disabled"; worker_thread_name_0 = ""; worker_thread_name_1 = ""; callback_frames_min = 256; callback_frames_max = 256; callback_frame_sample_count = 10; invalid_callback_frame_count = 0 }
  $release = [pscustomobject][ordered]@{ schema_version = 2; kind = "raspberry_audio_benchmark_release"; status = "released"; board_profile = "raspberry-pi-zero-2w"; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = $testHash; scenario = "capacity_analogue_16"; expected_alsa_buffer_frames = 256; observed_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; observed_alsa_period_frames = 64 }
  $candidate = [pscustomobject][ordered]@{ schema_version = 1; kind = "octessera_candidate_readiness"; status = "ready"; pid = 2345; systemd_invocation_id = "restored-invocation"; package_version = "0.8.2"; board_profile = "raspberry-pi-zero-2w"; ready_at_unix_ms = 1700000000000 }
  $restored = "final_active=active`nfinal_enabled=enabled`nfinal_pid=2345`nfinal_invocation_id=restored-invocation`nrestore_status=$RestoreStatus"
  $readiness | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory "benchmark-readiness.json") -Encoding UTF8
  $release | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory "benchmark-release.json") -Encoding UTF8
  $Result | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Directory "benchmark-result.json") -Encoding UTF8
  $candidate | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory "candidate-ready.json") -Encoding UTF8
  Set-Content (Join-Path $Directory "service-restored-state.txt") $restored -Encoding UTF8
  Set-Content (Join-Path $Directory "study-result.txt") "status_class=$StudyStatus" -Encoding UTF8
  Set-Content (Join-Path $Directory "sensor-series.txt") "raspberry_system_sample phase=startup thermal_max_millicelsius=42000 mem_available_kb=100000 throttled=0x0 current_throttled_mask=0 undervoltage=0`nraspberry_system_sample phase=runtime thermal_max_millicelsius=43000 mem_available_kb=99000 throttled=0x0 current_throttled_mask=0 undervoltage=0" -Encoding UTF8
}

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-raspberry-validation-" + [guid]::NewGuid().ToString("N"))
try {
  $failedResult = New-TestResult -Status fail -OverrunCount 1
  if (Test-RaspberryLiveBenchmarkClean $failedResult $inline) { throw "Non-clean Raspberry evidence was accepted as clean." }
  Assert-RaspberryLiveBenchmarkResult $failedResult $inline $testHash 1234 "invocation"
  $measuredRoot = Join-Path $testRoot "measured"
  Write-TestEvidence $measuredRoot $failedResult "measured_failure"
  $measured = Get-RaspberryLiveHostEvidence $measuredRoot $inline $testHash
  if ($measured.StatusClass -cne "measured_failure") { throw "Structurally valid non-clean evidence was not measured_failure." }
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
  if ($missing.StatusClass -cne "infrastructure_failure" -or $missing.Reason -cne "Raspberry live benchmark system evidence is missing.") { throw "Missing sensor evidence was not classified deterministically as infrastructure failure." }
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
if ($printOnly -notmatch "no transport is invoked" -or $printOnly -notmatch "U16 scenario=capacity_analogue_16 executor=Inline output=256 period=64 internal=128 lookahead=0 measure=30 label=30-second screen") { throw "Inline PrintOnly output changed." }
$multicorePrintOnly = Invoke-PrintOnly @{ Units = 32; ExecutorMode = "Multicore"; MeasureSeconds = 120; PrintOnly = $true }
if ($multicorePrintOnly -notmatch "U32 scenario=capacity_analogue_32 executor=Multicore output=256 period=64 internal=128 lookahead=128 measure=120 label=120-second repeat") { throw "Multicore PrintOnly output changed." }
Assert-Throws { & $runner -Units 16 } "missing explicit interruption consent"

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
if ($runnerSource -match "with-orange-ssh|orange_audio_benchmark|fallback") { throw "Raspberry runner contains Orange or fallback behavior." }

$bash = Get-Command bash -ErrorAction SilentlyContinue
$wsl = Get-Command wsl.exe -ErrorAction SilentlyContinue
if (($null -ne $bash -and [string]$bash.Source -notmatch "WindowsApps") -or $null -ne $wsl) {
  $payloadMatch = [regex]::Match($runnerSource, '(?s)\$body = @''(.*?)''@')
  if (-not $payloadMatch.Success) { throw "Raspberry runner payload was not found." }
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
