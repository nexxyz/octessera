$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-state.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-domain.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-plan.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "..\pi\raspberry-live-benchmark-metadata.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "..\orange-pi\orange-cross-metadata.psm1") -Force

function Assert-True {
  param([Parameter(Mandatory)][bool]$Condition, [Parameter(Mandatory)][string]$Message)
  if (-not $Condition) { throw $Message }
}

function Assert-Equal {
  param([Parameter(Mandatory)][object]$Actual, [Parameter(Mandatory)][object]$Expected, [Parameter(Mandatory)][string]$Message)
  if ($Actual -cne $Expected) { throw "$Message (actual=$Actual expected=$Expected)" }
}

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action, [Parameter(Mandatory)][string]$Message)
  try { & $Action } catch { return }
  throw "Expected failure did not occur: $Message"
}

function Assert-DirtyLaunchRejected {
  param([Parameter(Mandatory)][hashtable]$Parameters, [Parameter(Mandatory)][string]$Message)
  $result = Invoke-FrameSearchRunner $Parameters
  $output = $result.Output -join "`n"
  if ([int]$result.ExitCode -eq 0) { throw "Expected dirty active launch to fail: $Message" }
  if ($output -notlike "*active frame-search launch requires a clean exact-source tree*") { throw "Dirty launch failed for the wrong reason: $Message" }
}

function Invoke-FrameSearchRunner {
  param([Parameter(Mandatory)][hashtable]$Parameters)
  $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $runnerPath)
  foreach ($parameter in $Parameters.GetEnumerator()) {
    if ($parameter.Value -is [bool]) {
      if ($parameter.Value) { $arguments += "-$($parameter.Key)" }
    } else {
      $arguments += @("-$($parameter.Key)", [string]$parameter.Value)
    }
  }
  $previousErrorActionPreference = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try { $output = @(& powershell.exe @arguments 2>&1) } finally { $ErrorActionPreference = $previousErrorActionPreference }
  [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = $output }
}

function New-TestObservation {
  param([Parameter(Mandatory)][string]$Profile, [Parameter(Mandatory)][string]$Board, [Parameter(Mandatory)][string]$Mode, [Parameter(Mandatory)][int]$U, [Parameter(Mandatory)][string]$Grade, [uint64]$Worst = 0, [bool]$Adaptive = $false, [ValidateSet("pass", "fail")][string]$NativeStatus = "pass")
  [pscustomobject][ordered]@{ Cell = "$Profile-U$U"; StructurallyComplete = $true; Grade = $Grade; GradeOrder = Get-FrameSearchGradeOrder $Grade; Worst = $Worst; RepeatIncidents = $Worst; SilentIncidents = 0; AlsaRecoveryLogIncidents = 0; CpalStreamErrors = 0; CpalDeviceErrors = 0; CallbackOverBudget = 0; P999 = 0.1; NativeStatus = $NativeStatus; NativeStatusRank = if ($NativeStatus -ceq "pass") { 0 } else { 2 }; Profile = $Profile; Board = $Board; Mode = $Mode; U = $U; Effective = (Get-FrameSearchProfileDefinition $Profile).Effective; Adaptive = $Adaptive }
}

function New-TestRun {
  param([Parameter(Mandatory)][string]$Cell, [Parameter(Mandatory)][string]$Profile, [Parameter(Mandatory)][string]$Board, [Parameter(Mandatory)][string]$Mode, [Parameter(Mandatory)][int]$U, [Parameter(Mandatory)][string]$Grade, [uint64]$Worst = 0, [int]$Rep = 1)
  [pscustomobject][ordered]@{ Run = "$Cell-rep$Rep"; Cell = $Cell; Rep = $Rep; Wave = "W1"; Phase = "Preliminary"; Profile = $Profile; Board = $Board; Mode = $Mode; Output = (Get-FrameSearchProfileDefinition $Profile).Output; Period = (Get-FrameSearchProfileDefinition $Profile).Period; Internal = (Get-FrameSearchProfileDefinition $Profile).Internal; Lookahead = (Get-FrameSearchProfileDefinition $Profile).Lookahead; Effective = (Get-FrameSearchProfileDefinition $Profile).Effective; U = $U; Sec = 180; Status = "completed"; Grade = $Grade; Worst = $Worst; RepeatIncidents = $Worst; SilentIncidents = 0; AlsaRecoveryLogIncidents = 0; CpalStreamErrors = 0; CpalDeviceErrors = 0; CallbackOverBudget = 0; P999 = 0.1; NativeStatus = "pass" }
}

function New-TestBoardProfileSnapshot {
  param([Parameter(Mandatory)][int]$U)
  [pscustomobject][ordered]@{ active_synth_voices = 3 * $U; active_sample_voices = $U; active_preview_sample_voices = 0; active_momentary_fx = [math]::Min([math]::Ceiling($U / 4), 2); active_bus_fx_slots = [math]::Min([math]::Ceiling($U / 2), 12); active_global_fx_slots = [math]::Min([math]::Ceiling($U / 8), 2); cumulative_voice_steals = 0; cumulative_voice_admission_drops = 0 }
}

function New-TestBoardCallback {
  param([Parameter(Mandatory)][int]$Frames)
  [pscustomobject][ordered]@{ lifetime_callback_count = 10; callback_count = 10; first_measured_callback_ns = 1; last_measured_callback_ns = 10; measured_elapsed_ns = 9; callback_frames_min = $Frames; callback_frames_max = $Frames; callback_frame_sample_count = 10; callback_frame_size_change_count = 0; invalid_callback_frame_count = 0; lifetime_callback_frames_min = $Frames; lifetime_callback_frames_max = $Frames; lifetime_callback_frame_sample_count = 10; lifetime_callback_frame_size_change_count = 0; lifetime_invalid_callback_frame_count = 0; rendered_frames = 10 * $Frames; render_audio_duration_ns = 100; render_audio_duration_ratio_p50 = 0.1; render_audio_duration_ratio_p95 = 0.1; render_audio_duration_ratio_p99 = 0.1; render_audio_duration_ratio_p99_9 = 0.1; render_audio_duration_ratio_max = 0.2; over_audio_duration_budget_count = 0; callback_spacing_min_ns = 1; callback_spacing_max_ns = 1; callback_lateness_max_ns = 0; callback_timestamp_observed = $true; pre_mute_nonzero_samples = 10; pre_mute_peak = 1; post_mute_nonzero_samples = 0; cpal_device_error_count = 0; cpal_stream_error_count = 0; worker_terminal = $false; terminal_error = $false }
}

function New-TestBoardPersistentCounters {
  $zero = [pscustomobject][ordered]@{ rendered_quantums = 0; repeated_quantums = 0; dropped_quantums = 0; deadline_misses = 0; deadline_recoveries = 0 }
  [pscustomobject][ordered]@{ observable = $true; warmup = $zero; start = $zero; end = $zero; delta = $zero }
}

function New-TestRaspberryRecoveredDeadlineResult {
  $callback = New-TestBoardCallback 256
  $profile = New-TestBoardProfileSnapshot 12
  [pscustomobject][ordered]@{ schema_version = 13; kind = "raspberry_audio_benchmark_result"; status = "fail"; board_profile = "raspberry-pi-zero-2w"; artifact_sha256 = "a" * 64; scenario = "capacity_analogue_12"; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 128; lookahead_frames = 128; effective_output_latency_frames = 384; sample_format = "F32"; pid = 1234; systemd_invocation_id = "invocation"; sample_rate = 44100; channels = 2; executor_mode = "routing_tree_persistent"; worker_timing_mode = "disabled"; warmup_seconds = 5; measure_seconds = 180; scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; post_dsp_zero = $true; recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; continue_on_recovered_miss = $true; persistent_output_provenance = [pscustomobject][ordered]@{ observable = $true; repeated_quantum_incidents = 0; repeated_pcm_frames = 0; silent_quantum_incidents = 0; silent_pcm_frames = 0 }; worker_health = "deadline_miss"; worker_thread_name_0 = "oct-dsp-tree-0"; worker_thread_name_1 = "oct-dsp-tree-1"; joined_workers = 2; retirement_error = $null; worker_timing = $null; detected_continuity_events = 0; profile_start = $profile; profile_end = $profile; callback = $callback; persistent_output_counters = New-TestBoardPersistentCounters }
}

function New-TestOrangeRecoveredDeadlineResult {
  $callback = New-TestBoardCallback 256
  $profile = New-TestBoardProfileSnapshot 12
  [pscustomobject][ordered]@{ schema_version = 13; kind = "orange_audio_benchmark_result"; status = "fail"; board_profile = "orange-pi-zero-2w"; scenario = "capacity_analogue_12"; requested_output_buffer_frames = 256; expected_alsa_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 64; sample_format = "F32"; channels = 2; sample_rate = 44100; warmup_seconds = 5; measure_seconds = 180; scheduler_qualified = $true; callback_scheduling_policy = "SCHED_FIFO"; callback_scheduling_priority = 70; callback_scheduling_cpu = 1; post_dsp_zero = $true; measurement_stop_acknowledged = $true; stream_stopped = $true; final_progress_write_succeeded = $true; pid = 1234; systemd_invocation_id = "invocation"; artifact_sha256 = "a" * 64; callback = $callback; persistent_output_counters = New-TestBoardPersistentCounters; persistent_output_provenance = [pscustomobject][ordered]@{ observable = $true; repeated_quantum_incidents = 0; repeated_pcm_frames = 0; silent_quantum_incidents = 0; silent_pcm_frames = 0 }; detected_continuity_events = 0; profile_start = $profile; profile_end = $profile; recovered_alsa_epipe_count = $null; recovered_alsa_epipe_observable = $false; terminal_error = $null; executor_mode = "routing_tree_persistent"; continue_on_recovered_miss = $true; lookahead_frames = 64; effective_output_latency_frames = 320; worker_health = "deadline_miss"; worker_thread_name_0 = "oct-dsp-tree-0"; worker_thread_name_1 = "oct-dsp-tree-1"; joined_workers = 2; retirement_error = $null; worker_timing_mode = "disabled"; worker_timing = $null }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$planPath = Join-Path $repoRoot "docs\internal\pi-audio-capacity-frame-search.md"
$runnerPath = Join-Path $repoRoot "tools\performance\run-audio-capacity-frame-search.ps1"
$runnerSource = [IO.File]::ReadAllText($runnerPath, (New-Object Text.UTF8Encoding($false, $true)))
$plan = Read-FrameSearchPlan $planPath
Assert-Equal $plan.CandidateProfiles.Count 12 "candidate profile count"
Assert-Equal $plan.SeedQueue.Count 42 "approved seed cell count"
Assert-Equal $plan.SoakPlaceholders.Count 4 "soak placeholder count"
Assert-Equal @($plan.SeedQueue | Where-Object { $_.Cell -ceq "RI128-U8" }).Count 1 "RI128 U8 W2 placement"
Assert-Equal @($plan.SeedQueue | Where-Object { $_.Wave -ceq "W1" }).Count 12 "W1 count"
Assert-Equal @($plan.SeedQueue | Where-Object { $_.Wave -ceq "W2" }).Count 6 "W2 count"
Assert-Equal @($plan.SeedQueue | Where-Object { $_.Wave -ceq "W3" }).Count 12 "W3 count"
Assert-Equal @($plan.SeedQueue | Where-Object { $_.Wave -ceq "W4" }).Count 12 "W4 count"
Assert-True ($runnerSource.Contains('$pairs = @(Get-FrameSearchPairObservations @($State.runs))')) "empty first-wave observations remain an array"
$reserveFinalizationIndex = $runnerSource.LastIndexOf('if ($State.adaptive_reserve_reached) {', [StringComparison]::Ordinal)
$finalAnalysisIndex = $runnerSource.IndexOf('$finalAnalyses = @(', [StringComparison]::Ordinal)
Assert-True ($reserveFinalizationIndex -ge 0 -and $reserveFinalizationIndex -lt $finalAnalysisIndex) "reserve finalization precedes empty retained-profile analysis"

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-frame-search-test-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null
try {
  $planText = [IO.File]::ReadAllText($planPath, (New-Object Text.UTF8Encoding($false, $true)))
  $dirtyMarker = Join-Path $repoRoot ".frame-search-clean-tree-test-$([guid]::NewGuid().ToString('N')).tmp"
  [IO.File]::WriteAllText($dirtyMarker, "dirty`n", (New-Object Text.UTF8Encoding($false)))
  try {
    $printOnlyResult = Invoke-FrameSearchRunner @{ PlanPath = $planPath; PrintOnly = $true }
    if ([int]$printOnlyResult.ExitCode -ne 0 -or ($printOnlyResult.Output -join "`n") -notmatch "Frame-search PrintOnly: no board transport is invoked\.") { throw "Dirty PrintOnly was rejected." }
    Assert-DirtyLaunchRejected @{ PlanPath = $planPath; StudyId = "dirty-direct-$([guid]::NewGuid().ToString('N'))"; AllowServiceInterruption = $true } "direct active launch"
    Assert-DirtyLaunchRejected @{ PlanPath = $planPath; StudyId = "dirty-resume-$([guid]::NewGuid().ToString('N'))"; Resume = $true; AllowServiceInterruption = $true } "direct resume launch"
    Assert-DirtyLaunchRejected @{ PlanPath = $planPath; StudyId = "dirty-detached-new-$([guid]::NewGuid().ToString('N'))"; Detach = $true; AllowServiceInterruption = $true } "detached new launch"
    Assert-DirtyLaunchRejected @{ PlanPath = $planPath; StudyId = "dirty-detached-$([guid]::NewGuid().ToString('N'))"; Detach = $true; Resume = $true; AllowServiceInterruption = $true } "detached resume launch"
  } finally {
    Remove-Item -LiteralPath $dirtyMarker -Force -ErrorAction SilentlyContinue
  }
  $malformedHeader = Join-Path $tempRoot "malformed-header.md"
  [IO.File]::WriteAllText($malformedHeader, $planText.Replace("| Profile | Board | Mode | Output |", "| Profile | Board | Mode | Changed |"), (New-Object Text.UTF8Encoding($false)))
  Assert-Throws { Read-FrameSearchPlan $malformedHeader } "malformed candidate header"
  $alteredTuple = Join-Path $tempRoot "altered-tuple.md"
  [IO.File]::WriteAllText($alteredTuple, $planText.Replace("| RI64-U12 | W1 | RI64 | Raspberry | Inline | 256 | 64 | 64 | 0 | 256 |", "| RI64-U12 | W1 | RI64 | Raspberry | Inline | 512 | 64 | 64 | 0 | 512 |"), (New-Object Text.UTF8Encoding($false)))
  Assert-Throws { Read-FrameSearchPlan $alteredTuple } "altered queue tuple"
  $duplicateCell = Join-Path $tempRoot "duplicate-cell.md"
  [IO.File]::WriteAllText($duplicateCell, $planText.Replace("| RI64-U12 | W1 |", "| RI128-U12 | W1 |"), (New-Object Text.UTF8Encoding($false)))
  Assert-Throws { Read-FrameSearchPlan $duplicateCell } "duplicate queue cell"
  $mismatchedProfile = Join-Path $tempRoot "mismatched-profile.md"
  [IO.File]::WriteAllText($mismatchedProfile, $planText.Replace("| RI64-U12 | W1 | RI64 |", "| RI64-U12 | W1 | RI128 |"), (New-Object Text.UTF8Encoding($false)))
  Assert-Throws { Read-FrameSearchPlan $mismatchedProfile } "queue profile does not match cell identity"
  $caseMutatedIdentity = Join-Path $tempRoot "case-mutated-identity.md"
  [IO.File]::WriteAllText($caseMutatedIdentity, $planText.Replace("| RI64-U12 | W1 | RI64 |", "| ri64-U12 | W1 | ri64 |"), (New-Object Text.UTF8Encoding($false)))
  Assert-Throws { Read-FrameSearchPlan $caseMutatedIdentity } "case-mutated seed identity"

  $schedule = @(New-FrameSearchPhysicalSchedule $plan.SeedQueue "SEED")
  Assert-Equal $schedule.Count 84 "seed physical run count"
  $rep1 = @($schedule | Where-Object { $_.Rep -eq 1 }); $rep2 = @($schedule | Where-Object { $_.Rep -eq 2 })
  Assert-True (Test-FrameSearchEpochOrder $rep1 $rep2) "seed epoch interlacing"
  $repeatDistances = @(); foreach ($cell in @($plan.SeedQueue.Cell)) { $first = [array]::IndexOf(@($schedule | ForEach-Object { $_.Cell }), $cell); $last = [array]::LastIndexOf(@($schedule | ForEach-Object { $_.Cell }), $cell); $repeatDistances += $last - $first }
  Assert-True (@($repeatDistances | Where-Object { $_ -lt 5 }).Count -eq 0) "same-cell repetitions have four intervening runs"
  $scheduleAgain = @(New-FrameSearchPhysicalSchedule $plan.SeedQueue "SEED")
  Assert-Equal (($schedule | ForEach-Object Run) -join ",") (($scheduleAgain | ForEach-Object Run) -join ",") "deterministic schedule resume order"
  foreach ($count in @(2, 3, 5, 7)) {
    $smallRows = @($plan.SeedQueue | Select-Object -First $count)
    $smallSchedule = @(New-FrameSearchPhysicalSchedule $smallRows "SMALL$count")
    Assert-Equal $smallSchedule.Count ($count * 2) "small batch physical run count $count"
    Assert-True (Test-FrameSearchEpochOrder @($smallSchedule | Where-Object { $_.Rep -eq 1 }) @($smallSchedule | Where-Object { $_.Rep -eq 2 })) "small batch epoch order $count"
    $smallRep1 = @($smallSchedule | Where-Object { $_.Rep -eq 1 }); $smallRep2 = @($smallSchedule | Where-Object { $_.Rep -eq 2 }); $positions = @{}
    for ($index = 0; $index -lt $count; $index++) { $positions[$smallRep1[$index].Cell] = $index }
    $smallDistances = @(); for ($index = 0; $index -lt $count; $index++) { $smallDistances += $count + $index - $positions[$smallRep2[$index].Cell] }
    Assert-Equal ([int]($smallDistances | Measure-Object -Minimum).Minimum) ($count - 1) "small batch maximum minimum repetition distance $count"
    if ($count -ge 3) { for ($index = 1; $index -lt $smallSchedule.Count; $index++) { Assert-True ($smallSchedule[$index - 1].Cell -cne $smallSchedule[$index].Cell) "small batch avoids adjacent repetitions $count" } }
  }
  Assert-Equal (@(New-FrameSearchPhysicalSchedule @($plan.SeedQueue[0]) "SINGLE").Count) 0 "singleton batch is deferred"

  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 2 2 0 0 0) 180) "Stable" "180 Stable boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 3 0 0 0 0) 180) "Stretched" "180 Stretched lower boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 7 0 0 0 0) 180) "Stretched" "180 Stretched upper boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 8 0 0 0 0) 180) "Compromised" "180 Compromised boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 9 0 0 0 0) 600) "Stable" "600 Stable boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 10 0 0 0 0) 600) "Stretched" "600 Stretched lower boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 24 0 0 0 0) 600) "Stretched" "600 Stretched upper boundary"
  Assert-Equal (Get-FrameSearchGrade (Get-FrameSearchWorstIncident 25 0 0 0 0) 600) "Compromised" "600 Compromised boundary"
  $pairRuns = @((New-TestRun "RI128-U12" "RI128" "Raspberry" "Inline" 12 "Stable" 2 1), (New-TestRun "RI128-U12" "RI128" "Raspberry" "Inline" 12 "Stable" 2 2))
  $pair = Get-FrameSearchPairObservation "RI128-U12" $pairRuns
  Assert-Equal $pair.Worst 2 "paired cell uses worse repetition, not sum"
  Assert-Equal $pair.Grade "Stable" "paired cell grade"

  $fatalRun = New-TestRun "RI64-U12" "RI64" "Raspberry" "Inline" 12 "Stable" 0 1; $fatalRun.Status = "failed"
  $fatalRow = @($plan.SeedQueue | Where-Object { $_.Cell -ceq "RI64-U24" })[0]
  Assert-True ([string](Get-FrameSearchSeedSkipReason $fatalRow @($fatalRun) @()) -like "skip if profile dominated/unsafe*") "fatal profile skip"
  $u24Row = @($plan.SeedQueue | Where-Object { $_.Cell -ceq "RI128-U24" })[0]
  Assert-Equal (Get-FrameSearchSeedSkipReason $u24Row @() @()) "" "U24 is not skipped solely from U12"
  $skipObservations = @(
    (New-TestObservation "RI64" "Raspberry" "Inline" 24 "Stable" 2),
    (New-TestObservation "RI512" "Raspberry" "Inline" 24 "Stable" 2)
  )
  $highRow = @($plan.SeedQueue | Where-Object { $_.Cell -ceq "RI512-U32" })[0]
  Assert-True ([string](Get-FrameSearchSeedSkipReason $highRow @() $skipObservations) -like "skip if profile dominated/unsafe*") "high-latency U32 skip"
  $retainedObservations = @(
    (New-TestObservation "RI64" "Raspberry" "Inline" 12 "Stable" 4), (New-TestObservation "RI64" "Raspberry" "Inline" 24 "Stable" 2),
    (New-TestObservation "RI512" "Raspberry" "Inline" 12 "Stable" 2), (New-TestObservation "RI512" "Raspberry" "Inline" 24 "Stable" 2)
  )
  Assert-Equal (Get-FrameSearchSeedSkipReason $highRow @() $retainedObservations) "" "higher-latency profile with an earlier shared-U advantage is retained"
  $sharedDominance = Get-FrameSearchNondominatedProfiles (Get-FrameSearchProfileDefinitions) @(
    (New-TestObservation "RI64" "Raspberry" "Inline" 12 "Stable" 1), (New-TestObservation "RI64" "Raspberry" "Inline" 24 "Stable" 1),
    (New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable" 2), (New-TestObservation "RI128" "Raspberry" "Inline" 24 "Stable" 2)
  ) "Raspberry" "Inline"
  Assert-Equal (@($sharedDominance.Dominated | Where-Object { $_.Profile -ceq "RI128" }).Count) 1 "dominance evaluates multiple shared U observations"
  $dominance = Get-FrameSearchNondominatedProfiles (Get-FrameSearchProfileDefinitions) @(
    (New-TestObservation "RI64" "Raspberry" "Inline" 24 "Stable" 3),
    (New-TestObservation "RI128" "Raspberry" "Inline" 32 "Stable" 2),
    (New-TestObservation "RI512" "Raspberry" "Inline" 12 "Stable" 4)
  ) "Raspberry" "Inline"
  Assert-True ($dominance.Selected.Count -le 2) "maximum two nondominated profiles"

  $crossing = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(
    (New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable" 1),
    (New-TestObservation "RI128" "Raspberry" "Inline" 32 "Compromised" 8)
  )
  Assert-True (@($crossing.Rows | Where-Object { $_.Profile -ceq "RI128" -and $_.U -eq 22 }).Count -eq 1) "crossing bisection"
  $upward = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(New-TestObservation "RI128" "Raspberry" "Inline" 32 "Stable" 1)
  Assert-True (@($upward.Rows | Where-Object { $_.U -in @(36, 42) }).Count -eq 2) "U36 and U42 upward probes"
  $downward = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stretched" 3)
  Assert-True (@($downward.Rows | Where-Object { $_.Profile -ceq "RI128" -and $_.U -eq 6 }).Count -eq 1) "low-end downward probe"
  $reversal = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(
    (New-TestObservation "RI128" "Raspberry" "Inline" 12 "Compromised" 8),
    (New-TestObservation "RI128" "Raspberry" "Inline" 24 "Stable" 1)
  )
  Assert-True (@($reversal.Decisions | Where-Object { $_.Kind -ceq "reversal" }).Count -gt 0) "reversal decision"
  Assert-True (@($reversal.Rows | Where-Object { $_.Profile -ceq "RI128" -and $_.U -eq 18 }).Count -eq 1) "local reversal probe"

  $stableToStretched = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @((New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable"), (New-TestObservation "RI128" "Raspberry" "Inline" 13 "Stretched"))
  Assert-Equal (@($stableToStretched.Borders | Where-Object { $_.Kind -ceq "StableToStretched" }).Count) 1 "StableToStretched border"
  Assert-Equal $stableToStretched.Resolution "inconclusive" "StableToStretched requires upper capacity work"
  $stretchedToCompromised = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @((New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stretched"), (New-TestObservation "RI128" "Raspberry" "Inline" 13 "Compromised"))
  Assert-Equal (@($stretchedToCompromised.Borders | Where-Object { $_.Kind -ceq "StretchedToCompromised" }).Count) 1 "StretchedToCompromised border"
  Assert-True (@($stretchedToCompromised.RequiredProbes | Where-Object { $_.U -eq 6 }).Count -eq 1) "non-Stable downward probe"
  $direct = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @((New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable"), (New-TestObservation "RI128" "Raspberry" "Inline" 13 "Compromised"))
  Assert-Equal (@($direct.Borders | Where-Object { $_.Kind -ceq "DirectStableToCompromised" -and $_.Resolved }).Count) 1 "adjacent direct StableToCompromised resolution"
  Assert-Equal $direct.Resolution "resolved" "direct StableToCompromised profile resolution"
  foreach ($case in @(
    @{ Low = "Stretched"; High = "Stable"; Name = "StretchedToStable" },
    @{ Low = "Compromised"; High = "Stretched"; Name = "CompromisedToStretched" }
  )) {
    $improvement = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @((New-TestObservation "RI128" "Raspberry" "Inline" 12 $case.Low), (New-TestObservation "RI128" "Raspberry" "Inline" 13 $case.High))
    Assert-Equal (@($improvement.Borders | Where-Object { $_.Kind -ceq "Reversal" -and $_.Resolved }).Count) 1 "$($case.Name) adjacent reversal"
  }
  $gap = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @((New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable"), (New-TestObservation "RI128" "Raspberry" "Inline" 16 "Stretched"))
  Assert-True (@($gap.Borders | Where-Object { $_.Kind -ceq "StableToStretched" -and -not $_.Resolved -and $_.ProbeU -eq 14 }).Count -eq 1) "grade-change gap midpoint"
  $bound = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @((New-TestObservation "RI128" "Raspberry" "Inline" 1 "Stable"), (New-TestObservation "RI128" "Raspberry" "Inline" 42 "Stable"))
  Assert-Equal $bound.Resolution "bounded_at_U42" "U42 bounded profile"
  $single = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @(New-TestObservation "RI128" "Raspberry" "Inline" 24 "Stable")
  Assert-Equal $single.Resolution "inconclusive" "single observation cannot win"
  Assert-True (@($single.RequiredProbes | Where-Object { $_.U -eq 32 }).Count -eq 1) "single observation upward probe"
  Assert-Equal (@(Get-FrameSearchWinnerSet (Get-FrameSearchProfileDefinitions) @(New-TestObservation "RI128" "Raspberry" "Inline" 24 "Stable") @($single)).Count) 0 "single observation cannot produce a winner"
  $blockedProbe = Get-FrameSearchProfileAnalysis (Get-FrameSearchProfileDefinition "RI128") @(New-TestObservation "RI128" "Raspberry" "Inline" 24 "Stable") -BlockedCells @("RI128-U32")
  Assert-Equal $blockedProbe.BlockedProbeCount 1 "skipped required probe makes profile inconclusive"
  $deferredReadiness = Get-FrameSearchCampaignReadiness @($direct) @([pscustomobject]@{ Cell = "RI128-U1" })
  Assert-True (-not $deferredReadiness.Ready -and $deferredReadiness.DeferredCount -eq 1) "deferred singleton suppresses campaign winners and soaks"
  $budget = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable") -MaxAdaptiveTotal 0
  Assert-True (@($budget.Analyses | Where-Object { $_.Profile -ceq "RI128" -and $_.BudgetBlocked -and $_.Resolution -ceq "inconclusive" }).Count -eq 1) "adaptive budget suppresses unresolved profile"
  $budgetReadiness = Get-FrameSearchCampaignReadiness @($budget.Analyses | Where-Object { $_.Profile -ceq "RI128" }) @()
  Assert-True (-not $budgetReadiness.Ready) "budget exhaustion suppresses campaign winners and soaks"
  $fair = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable") -InitialAdaptivePerProfile 4 -MaxAdaptivePerProfile 8 -MaxAdaptiveTotal 32
  Assert-True (@($fair.Rows | Group-Object Profile | Where-Object { $_.Count -gt 4 }).Count -eq 0) "initial adaptive target four per profile"
  $adaptiveRows = @($fair.Rows)
  Assert-True ($adaptiveRows.Count -le 32) "global adaptive paired budget"
  Assert-True (@($adaptiveRows | Group-Object Profile | Where-Object { $_.Count -gt 8 }).Count -eq 0) "adaptive profile maximum eight"
  $expansionObservations = @(
    (New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable" 0 $true),
    (New-TestObservation "RI128" "Raspberry" "Inline" 16 "Compromised" 8 $true),
    (New-TestObservation "RI128" "Raspberry" "Inline" 24 "Stable" 0 $true),
    (New-TestObservation "RI128" "Raspberry" "Inline" 32 "Stable" 0 $true)
  )
  $expansion = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) $expansionObservations -InitialAdaptivePerProfile 4 -MaxAdaptivePerProfile 8 -MaxAdaptiveTotal 32
  Assert-Equal (@($expansion.Rows | Where-Object { $_.Profile -ceq "RI128" }).Count) 4 "retained unresolved finalist expands from four to eight"
  Assert-True ($expansion.GlobalAdaptiveCount -le 32) "expanded adaptive work remains globally bounded"
  $consumed = @(1..30 | ForEach-Object { New-TestObservation "OI32" "Orange" "Inline" $_ "Stable" 0 $true })
  $starvation = Get-FrameSearchAdaptivePlan (Get-FrameSearchProfileDefinitions) @(
    $consumed + @(
      (New-TestObservation "RI64" "Raspberry" "Inline" 1 "Stable"), (New-TestObservation "RI64" "Raspberry" "Inline" 10 "Stretched"),
      (New-TestObservation "RI64" "Raspberry" "Inline" 20 "Compromised"), (New-TestObservation "RI64" "Raspberry" "Inline" 30 "Stable"),
      (New-TestObservation "RI128" "Raspberry" "Inline" 1 "Stable"), (New-TestObservation "RI128" "Raspberry" "Inline" 10 "Stretched"),
      (New-TestObservation "RI128" "Raspberry" "Inline" 20 "Compromised"), (New-TestObservation "RI128" "Raspberry" "Inline" 30 "Stable")
    )
  ) -InitialAdaptivePerProfile 4 -MaxAdaptivePerProfile 8 -MaxAdaptiveTotal 32
  Assert-True (@($starvation.Rows | Where-Object { $_.Profile -ceq "RI64" }).Count -gt 0) "30 consumed adaptive cells leave fair initial work for RI64"
  Assert-True (@($starvation.Rows | Where-Object { $_.Profile -ceq "RI128" }).Count -gt 0) "30 consumed adaptive cells do not starve RI128"
  Assert-True ($starvation.GlobalAdaptiveCount -le 32) "starvation case remains globally bounded"
  $winnerObservations = @(
    (New-TestObservation "RI64" "Raspberry" "Inline" 12 "Stable" 2 $false "pass"), (New-TestObservation "RI64" "Raspberry" "Inline" 13 "Compromised" 8),
    (New-TestObservation "RI128" "Raspberry" "Inline" 12 "Stable" 2 $false "fail"), (New-TestObservation "RI128" "Raspberry" "Inline" 13 "Compromised" 8),
    (New-TestObservation "RI512" "Raspberry" "Inline" 12 "Stable" 1), (New-TestObservation "RI512" "Raspberry" "Inline" 13 "Compromised" 8),
    (New-TestObservation "RM64" "Raspberry" "Multicore" 12 "Stable" 2), (New-TestObservation "RM64" "Raspberry" "Multicore" 13 "Compromised" 8),
    (New-TestObservation "OI32" "Orange" "Inline" 12 "Stable" 2), (New-TestObservation "OI32" "Orange" "Inline" 13 "Compromised" 8),
    (New-TestObservation "OM64" "Orange" "Multicore" 12 "Stable" 2), (New-TestObservation "OM64" "Orange" "Multicore" 13 "Compromised" 8)
  )
  $winners = @(Get-FrameSearchWinnerSet (Get-FrameSearchProfileDefinitions) $winnerObservations)
  Assert-Equal $winners.Count 4 "four board/mode winners"
  Assert-Equal (@($winners | Where-Object { $_.Board -ceq "Raspberry" -and $_.Mode -ceq "Inline" })[0].Profile) "RI64" "winner tie ranking prefers native pass"
  $gains = @(Get-FrameSearchGainResults $winners); Assert-Equal $gains.Count 2 "gain result count"

  $evidenceDirectory = Join-Path $tempRoot "evidence"
  New-Item -ItemType Directory -Force -Path $evidenceDirectory | Out-Null
  $run = New-TestRun "RI128-U12" "RI128" "Raspberry" "Inline" 12 "Stable" 0 1
  $run.Phase = "Preliminary"
  $result = [pscustomobject][ordered]@{ board_profile = "raspberry-pi-zero-2w"; artifact_sha256 = "a" * 64; requested_output_buffer_frames = 256; expected_alsa_period_frames = 64; internal_block_frames = 128; lookahead_frames = 0; effective_output_latency_frames = 256; measure_seconds = 180; persistent_output_provenance = [pscustomobject][ordered]@{ observable = $false; repeated_quantum_incidents = 0; repeated_pcm_frames = 0; silent_quantum_incidents = 0; silent_pcm_frames = 0 }; callback = [pscustomobject][ordered]@{ cpal_device_error_count = 0; cpal_stream_error_count = 0; over_audio_duration_budget_count = 0; render_audio_duration_ratio_p99_9 = 0.1; render_audio_duration_ratio_max = 0.2 }; status = "pass"; worker_health = "disabled" }
  $hostEvidence = [pscustomobject][ordered]@{ StatusClass = "pass"; ArtifactSha256 = "a" * 64; FrameSearchProfile = "RI128"; FrameSearchPhase = "Preliminary"; FrameSearchU = 12; FrameSearchGeometry = [pscustomobject][ordered]@{ OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 256 }; RepeatIncidents = 0; SilentIncidents = 0; AlsaRecoveryLogIncidents = 0; PracticalGrade = "Stable" }
  $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-result.json") -Encoding UTF8
  $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  Assert-Equal (Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w").Grade "Stable" "valid host evidence"
  $hostEvidence.ArtifactSha256 = "b" * 64; $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w" } "artifact evidence mismatch"
  $hostEvidence.ArtifactSha256 = "a" * 64; $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  $hostEvidence.StatusClass = "PASS"; $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w" } "uppercase host status class"
  $hostEvidence.StatusClass = "pass"; $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  $uppercaseResult = ConvertFrom-Json -InputObject ($result | ConvertTo-Json -Depth 8); $uppercaseResult.status = "PASS"; $uppercaseResult | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w" } "uppercase result status"
  $uppercaseResult.status = "pass"; $uppercaseResult.worker_health = "HEALTHY"; $uppercaseResult | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w" } "uppercase worker health"
  $uppercaseResult.worker_health = "disabled"; $hostEvidence.PracticalGrade = "stable"; $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w" } "lowercase practical grade"
  $hostEvidence.PracticalGrade = "Stable"; $hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "host-evidence.json") -Encoding UTF8
  $uppercaseResult | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-result.json") -Encoding UTF8
  $invalidInteger = ConvertFrom-Json -InputObject ($result | ConvertTo-Json -Depth 8); $invalidInteger.measure_seconds = "180"; $invalidInteger | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidenceDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $evidenceDirectory $run ("a" * 64) "raspberry-pi-zero-2w" } "string evidence integer"

  $raspberryDeadlineRun = New-TestRun "RM128-U12" "RM128" "Raspberry" "Multicore" 12 "Compromised" 8 1
  $raspberryDeadlineResult = New-TestRaspberryRecoveredDeadlineResult
  $raspberryDeadlineHost = [pscustomobject][ordered]@{ StatusClass = "measured_failure"; ArtifactSha256 = "a" * 64; FrameSearchProfile = "RM128"; FrameSearchPhase = "Preliminary"; FrameSearchU = 12; FrameSearchGeometry = [pscustomobject][ordered]@{ OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 128; EffectiveOutputLatencyFrames = 384 }; RepeatIncidents = 0; SilentIncidents = 0; AlsaRecoveryLogIncidents = 0; PracticalGrade = "Stable" }
  $raspberryDeadlineDirectory = Join-Path $tempRoot "raspberry-deadline-miss"
  New-Item -ItemType Directory -Force -Path $raspberryDeadlineDirectory | Out-Null
  $raspberryDeadlineResult | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $raspberryDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  $raspberryDeadlineHost | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $raspberryDeadlineDirectory "host-evidence.json") -Encoding UTF8
  Assert-Equal (Assert-FrameSearchHostEvidence $raspberryDeadlineDirectory $raspberryDeadlineRun ("a" * 64) "raspberry-pi-zero-2w").NativeWorker "deadline_miss" "Raspberry recovered deadline miss"
  $raspberryUnrecovered = ConvertFrom-Json -InputObject ($raspberryDeadlineResult | ConvertTo-Json -Depth 12); $raspberryUnrecovered.continue_on_recovered_miss = $false; $raspberryUnrecovered | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $raspberryDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $raspberryDeadlineDirectory $raspberryDeadlineRun ("a" * 64) "raspberry-pi-zero-2w" } "Raspberry unrecovered deadline miss"
  $raspberryTerminal = ConvertFrom-Json -InputObject ($raspberryDeadlineResult | ConvertTo-Json -Depth 12); $raspberryTerminal.terminal_error = "worker terminated"; $raspberryTerminal | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $raspberryDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $raspberryDeadlineDirectory $raspberryDeadlineRun ("a" * 64) "raspberry-pi-zero-2w" } "Raspberry terminal deadline miss"
  $raspberryTerminalCallback = ConvertFrom-Json -InputObject ($raspberryDeadlineResult | ConvertTo-Json -Depth 12); $raspberryTerminalCallback.callback.worker_terminal = $true; $raspberryTerminalCallback | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $raspberryDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $raspberryDeadlineDirectory $raspberryDeadlineRun ("a" * 64) "raspberry-pi-zero-2w" } "Raspberry terminal callback deadline miss"
  $raspberryPartial = ConvertFrom-Json -InputObject ($raspberryDeadlineResult | ConvertTo-Json -Depth 12); $raspberryPartial.callback.callback_count = 0; $raspberryPartial | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $raspberryDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $raspberryDeadlineDirectory $raspberryDeadlineRun ("a" * 64) "raspberry-pi-zero-2w" } "Raspberry partial deadline miss"

  $orangeDeadlineRun = New-TestRun "OM64-U12" "OM64" "Orange" "Multicore" 12 "Compromised" 8 1
  $orangeDeadlineResult = New-TestOrangeRecoveredDeadlineResult
  $orangeDeadlineHost = [pscustomobject][ordered]@{ StatusClass = "measured_failure"; ArtifactSha256 = "a" * 64; FrameSearchProfile = "OM64"; FrameSearchPhase = "Preliminary"; FrameSearchU = 12; FrameSearchGeometry = [pscustomobject][ordered]@{ OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 64; EffectiveOutputLatencyFrames = 320 }; RepeatIncidents = 0; SilentIncidents = 0; AlsaRecoveryLogIncidents = 0; PracticalGrade = "Stable" }
  $orangeDeadlineDirectory = Join-Path $tempRoot "orange-deadline-miss"
  New-Item -ItemType Directory -Force -Path $orangeDeadlineDirectory | Out-Null
  $orangeDeadlineResult | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $orangeDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  $orangeDeadlineHost | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $orangeDeadlineDirectory "host-evidence.json") -Encoding UTF8
  Assert-Equal (Assert-FrameSearchHostEvidence $orangeDeadlineDirectory $orangeDeadlineRun ("a" * 64) "orange-pi-zero-2w").NativeWorker "deadline_miss" "Orange recovered deadline miss"
  $orangeUnrecovered = ConvertFrom-Json -InputObject ($orangeDeadlineResult | ConvertTo-Json -Depth 12); $orangeUnrecovered.continue_on_recovered_miss = $false; $orangeUnrecovered | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $orangeDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $orangeDeadlineDirectory $orangeDeadlineRun ("a" * 64) "orange-pi-zero-2w" } "Orange unrecovered deadline miss"
  $orangeTerminal = ConvertFrom-Json -InputObject ($orangeDeadlineResult | ConvertTo-Json -Depth 12); $orangeTerminal.terminal_error = "worker terminated"; $orangeTerminal | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $orangeDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $orangeDeadlineDirectory $orangeDeadlineRun ("a" * 64) "orange-pi-zero-2w" } "Orange terminal deadline miss"
  $orangeTerminalCallback = ConvertFrom-Json -InputObject ($orangeDeadlineResult | ConvertTo-Json -Depth 12); $orangeTerminalCallback.callback.worker_terminal = $true; $orangeTerminalCallback | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $orangeDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $orangeDeadlineDirectory $orangeDeadlineRun ("a" * 64) "orange-pi-zero-2w" } "Orange terminal callback deadline miss"
  $orangePartial = ConvertFrom-Json -InputObject ($orangeDeadlineResult | ConvertTo-Json -Depth 12); $orangePartial.callback.callback_count = 0; $orangePartial | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $orangeDeadlineDirectory "benchmark-result.json") -Encoding UTF8
  Assert-Throws { Assert-FrameSearchHostEvidence $orangeDeadlineDirectory $orangeDeadlineRun ("a" * 64) "orange-pi-zero-2w" } "Orange partial deadline miss"

  Assert-Throws { Assert-FrameSearchChildExit 7 } "fatal child exit"
  $reserveNow = [datetime]::new(2030, 1, 1, 0, 0, 0, [datetimekind]::Utc)
  Assert-True (Test-FrameSearchReserveWindow $reserveNow.AddMinutes(75) $reserveNow) "75-minute reserve boundary stops work"
  Assert-True (-not (Test-FrameSearchReserveWindow $reserveNow.AddMinutes(76) $reserveNow)) "reserve does not stop work at 76 minutes"
  $definitions = Get-FrameSearchArtifactDefinitions "r.bin" "r.bin.metadata.json" "oi.bin" "oi.bin.metadata.json" "om.bin" "om.bin.metadata.json"
  $command = New-FrameSearchDetachedCommand (Join-Path $repoRoot "tools\performance\run-audio-capacity-frame-search.ps1") "test-study" $plan.Path $definitions -AllowServiceInterruption
  Assert-True ($command.Arguments -contains "-Worker") "detached worker argument"
  Assert-True ($command.ArgumentString -notmatch "PASSPHRASE| -Key ") "detached command omits secrets"
  $artifactDirectory = Join-Path $tempRoot "artifacts"; New-Item -ItemType Directory -Force -Path $artifactDirectory | Out-Null
  $rArtifact = Join-Path $artifactDirectory "raspberry"; $rMetadata = "$rArtifact.metadata.json"; $oiArtifact = Join-Path $artifactDirectory "orange-inline"; $oiMetadata = "$oiArtifact.metadata.json"; $omArtifact = Join-Path $artifactDirectory "orange-multicore"; $omMetadata = "$omArtifact.metadata.json"
  foreach ($artifactPath in @($rArtifact, $oiArtifact, $omArtifact)) { [IO.File]::WriteAllBytes($artifactPath, [byte[]](1, 2, 3, 4)) }
  $artifactSource = Get-FrameSearchRepoCommit $repoRoot
  Write-RaspberryLiveBenchmarkMetadata $rMetadata $artifactSource $rArtifact
  Publish-OrangeBuildMetadata -MetadataPath $oiMetadata -BinaryPath $oiArtifact -SelectedBinary "octessera-pi" -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec ([pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w benchmark-voice-pools-128"; ArtifactKind = "diagnostic-only" }) -SourceCommit $artifactSource
  Publish-OrangeBuildMetadata -MetadataPath $omMetadata -BinaryPath $omArtifact -SelectedBinary "octessera-pi" -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec ([pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128"; ArtifactKind = "diagnostic-only" }) -SourceCommit $artifactSource
  $resolvedArtifacts = Get-FrameSearchArtifactSet (Get-FrameSearchArtifactDefinitions $rArtifact $rMetadata $oiArtifact $oiMetadata $omArtifact $omMetadata) $artifactSource -RequireFiles
  Assert-True (@($resolvedArtifacts | Where-Object { $_.Resolved -and $_.Sha256 -match '^[0-9a-f]{64}$' -and $_.MetadataSourceCommit -ceq $artifactSource }).Count -eq 3) "exact three artifact hashes and metadata commits"
  $wrongOrangeMetadata = Join-Path $artifactDirectory "orange-wrong-source.metadata.json"
  Publish-OrangeBuildMetadata -MetadataPath $wrongOrangeMetadata -BinaryPath $oiArtifact -SelectedBinary "octessera-pi" -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec ([pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w benchmark-voice-pools-128"; ArtifactKind = "diagnostic-only" }) -SourceCommit ("b" * 40)
  Assert-Throws { Get-FrameSearchArtifactSet (Get-FrameSearchArtifactDefinitions $rArtifact $rMetadata $oiArtifact $wrongOrangeMetadata $omArtifact $omMetadata) $artifactSource -RequireFiles } "Orange source commit mismatch"
  $artifactExpectations = Get-FrameSearchArtifactSet $definitions ("a" * 40)
  $state = New-FrameSearchState "test-study" $plan ("a" * 40) $artifactExpectations ([datetime]::UtcNow) 20
  $statePath = Join-Path $tempRoot "state\study-state.json"
  Write-FrameSearchAtomicJson $statePath $state
  $state.status = "completed"
  Write-FrameSearchAtomicJson $statePath $state
  $loaded = Read-FrameSearchState $statePath
  Assert-Equal ([string]$loaded.status) "completed" "atomic state overwrite"
  Assert-Equal @(Get-ChildItem -LiteralPath (Split-Path -Parent $statePath) -Filter "study-state.json.*-*" -File).Count 0 "atomic state overwrite removes temporary files"
  $resultsPath = Join-Path $tempRoot "state\study-results.md"
  Write-FrameSearchResultsMarkdown $state $resultsPath
  $state.status = "inconclusive"
  Write-FrameSearchResultsMarkdown $state $resultsPath
  Assert-True ([IO.File]::ReadAllText($resultsPath).Contains("- Status: inconclusive")) "atomic Markdown overwrite"
  Assert-Equal @(Get-ChildItem -LiteralPath (Split-Path -Parent $resultsPath) -Filter "study-results.md.*-*" -File).Count 0 "atomic Markdown overwrite removes temporary files"
  $childDirectory = Join-Path $tempRoot "child process"
  New-Item -ItemType Directory -Force -Path $childDirectory | Out-Null
  $childScript = Join-Path $childDirectory "echo-argument.ps1"
  [IO.File]::WriteAllText($childScript, 'param([string]$Value) Write-Output "child=$Value"', (New-Object Text.UTF8Encoding($false)))
  $child = Invoke-FrameSearchChildProcess $childScript @("-Value", "runner proof") (Join-Path $childDirectory "stdout.txt") (Join-Path $childDirectory "stderr.txt")
  Assert-Equal ([int]$child.ExitCode) 0 "child runner exit"
  Assert-True ($child.Stdout.Trim() -ceq "child=runner proof") "child runner invokes the script path and preserves arguments"
  Assert-FrameSearchResumeIdentity $loaded "test-study" $plan ("a" * 40) $artifactExpectations
  $changedPlan = [pscustomobject]@{ Path = $plan.Path; Sha256 = "b" * 64 }
  Assert-Throws { Assert-FrameSearchResumeIdentity $loaded "test-study" $changedPlan ("a" * 40) $artifactExpectations } "resume plan identity mismatch"
  $lockDirectory = Join-Path $tempRoot "study-lock"
  $lock = Enter-FrameSearchStudyLock $lockDirectory
  Assert-Throws { Enter-FrameSearchStudyLock $lockDirectory } "study lock contention"
  Exit-FrameSearchStudyLock $lock
  [IO.File]::WriteAllText((Join-Path $lockDirectory "worker.lock"), "stale", (New-Object Text.UTF8Encoding($false)))
  $lock = Enter-FrameSearchStudyLock $lockDirectory
  Exit-FrameSearchStudyLock $lock
  $lock = Enter-FrameSearchStudyLock $lockDirectory
  try { throw "simulated worker failure" } catch { } finally { Exit-FrameSearchStudyLock $lock }
  $lock = Enter-FrameSearchStudyLock $lockDirectory
  Exit-FrameSearchStudyLock $lock
  $failureStudyId = "lock-failure-$([guid]::NewGuid().ToString('N'))"
  $failureStudyDirectory = Join-Path $repoRoot (Join-Path "target\audio-capacity-frame-search" $failureStudyId)
  $failureCommand = New-FrameSearchDetachedCommand (Join-Path $repoRoot "tools\performance\run-audio-capacity-frame-search.ps1") $failureStudyId $plan.Path $definitions -AllowServiceInterruption
  $failureProcess = Start-Process -FilePath $failureCommand.FilePath -ArgumentList $failureCommand.ArgumentString -RedirectStandardOutput (Join-Path $tempRoot "lock-failure.stdout.txt") -RedirectStandardError (Join-Path $tempRoot "lock-failure.stderr.txt") -PassThru -Wait -WindowStyle Hidden
  Assert-True ($failureProcess.ExitCode -ne 0) "failed worker exits unsuccessfully"
  $lock = Enter-FrameSearchStudyLock $failureStudyDirectory
  $resumeCommand = New-FrameSearchDetachedCommand (Join-Path $repoRoot "tools\performance\run-audio-capacity-frame-search.ps1") $failureStudyId $plan.Path $definitions -Resume -AllowServiceInterruption
  $resumeProcess = Start-Process -FilePath $resumeCommand.FilePath -ArgumentList $resumeCommand.ArgumentString -RedirectStandardOutput (Join-Path $tempRoot "lock-resume.stdout.txt") -RedirectStandardError (Join-Path $tempRoot "lock-resume.stderr.txt") -PassThru -Wait -WindowStyle Hidden
  Assert-True ($resumeProcess.ExitCode -ne 0) "concurrent resume refuses the study lock"
  Exit-FrameSearchStudyLock $lock
  Remove-Item -LiteralPath $failureStudyDirectory -Recurse -Force -ErrorAction SilentlyContinue
  $adaptiveState = New-FrameSearchState "adaptive-study" $plan ("a" * 40) $artifactExpectations ([datetime]::UtcNow) 20
  $adaptiveState.runs = @(
    [pscustomobject]@{ Run = "A1-second"; Cell = "RI128-U18"; Wave = "A1"; Status = "pending"; ScheduleIndex = 2 },
    [pscustomobject]@{ Run = "A1-first"; Cell = "RI128-U18"; Wave = "A1"; Status = "pending"; ScheduleIndex = 1 },
    [pscustomobject]@{ Run = "A1-done"; Cell = "RI128-U22"; Wave = "A1"; Status = "completed"; ScheduleIndex = 3 }
  )
  $pendingAdaptive = @(Get-FrameSearchPendingAdaptiveRuns $adaptiveState)
  Assert-Equal (($pendingAdaptive | ForEach-Object Run) -join ",") "A1-first,A1-second" "persisted adaptive run order"
  $pendingAdaptive[0].Status = "completed"
  Assert-Equal ((Get-FrameSearchPendingAdaptiveRuns $adaptiveState | ForEach-Object Run) -join ",") "A1-second" "interruption resume leaves later adaptive run pending"
  $publicationState = New-FrameSearchState "publication-study" $plan ("a" * 40) $artifactExpectations ([datetime]::UtcNow) 20
  $publicationState.provisional_winners = @([pscustomobject]@{ Profile = "RI128" })
  $publicationState.winners = @([pscustomobject]@{ Profile = "RI128" })
  $publicationState.gains = @([pscustomobject]@{ Board = "Raspberry" })
  Clear-FrameSearchPublicationState $publicationState | Out-Null
  Assert-True ($publicationState.provisional_winners.Count -eq 0 -and $publicationState.winners.Count -eq 0 -and $publicationState.gains.Count -eq 0) "fatal finalization clears publication state"
  $publicationState.status = "failed"
  $publicationStatePath = Join-Path $tempRoot "publication-failure\study-state.json"
  Write-FrameSearchAtomicJson $publicationStatePath $publicationState
  $persistedPublicationState = Read-FrameSearchState $publicationStatePath
  Assert-True ([string]$persistedPublicationState.status -ceq "failed" -and $persistedPublicationState.provisional_winners.Count -eq 0 -and $persistedPublicationState.winners.Count -eq 0 -and $persistedPublicationState.gains.Count -eq 0) "failed publication state was persisted without winners"
  Write-Output "Audio capacity frame-search plan, scheduler, grading, adaptive, evidence, resume, reserve, and detached-command tests passed"
} finally {
  Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
