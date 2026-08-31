Set-StrictMode -Version Latest

$script:OfflineRowFields = @(
  "kind", "scenario", "metric", "value", "block_frames", "sample_rate", "blocks", "avg", "p95", "p99", "max", "notes", "internal_block_frames", "schema_version", "p99_9", "over_audio_duration_budget_count", "requested_measure_frames", "requested_internal_block_frames", "workers_requested_count", "workers_effective_count", "peak_synth_voices", "peak_sample_voices", "peak_preview_sample_voices", "peak_momentary_fx", "peak_bus_fx_slots", "peak_global_fx_slots", "peak_voice_steals", "voice_steal_delta", "synth_parallel_dispatch_delta", "synth_parallel_light_skip_delta", "synth_parallel_backoff_skip_delta", "synth_parallel_timing_backoff_delta", "synth_parallel_failure_delta", "synth_parallel_unhealthy"
)

function Get-OfflineInteger {
  param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Context)
  $parsed = 0L
  if (-not [long]::TryParse($Value, [Globalization.NumberStyles]::Integer, [Globalization.CultureInfo]::InvariantCulture, [ref]$parsed) -or $parsed -lt 0) { throw "$Context must be a non-negative integer." }
  return $parsed
}

function Get-OfflineNumber {
  param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Context)
  $parsed = 0.0
  if (-not [double]::TryParse($Value, [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$parsed) -or [double]::IsNaN($parsed) -or [double]::IsInfinity($parsed)) { throw "$Context must be a finite number." }
  return $parsed
}

function Assert-OfflineRowShape {
  param([Parameter(Mandatory)][pscustomobject]$Row)
  $actual = @($Row.PSObject.Properties.Name)
  if (($actual -join ",") -cne ($script:OfflineRowFields -join ",")) { throw "Offline profile row must use schema 2 field order." }
  if ($Row.kind -cne "engine_source" -or $Row.metric -cne "raw_ratio") { throw "Offline profile row is not an engine-source raw-ratio row." }
  if ((Get-OfflineInteger $Row.schema_version "offline schema_version") -ne 2) { throw "Offline profile row schema_version must be 2." }
}

function Test-OfflineMultiSlotSynthScenario {
  param([Parameter(Mandatory)][string]$Scenario)
  return @("synth_cross_slot_16", "synth_cross_slot_32_no_steal", "synth_cross_slot_64_no_steal", "mixed_16_synth_32_sample") -contains $Scenario -or $Scenario.StartsWith("fixed_8_synth_8_sample_", [StringComparison]::Ordinal)
}

function ConvertTo-PerformanceBaselineOfflineResult {
  param(
    [Parameter(Mandatory)][pscustomobject]$Row,
    [Parameter(Mandatory)][pscustomobject]$Cell,
    [Parameter(Mandatory)][int]$SampleRate,
    [Parameter(Mandatory)][int]$Observations
  )
  Assert-OfflineRowShape $Row
  if ($Row.scenario -cne $Cell.scenario) { throw "Offline profile row scenario does not match the requested cell." }
  $block = Get-OfflineInteger $Row.block_frames "offline block_frames"
  $sampleRateValue = Get-OfflineInteger $Row.sample_rate "offline sample_rate"
  $blocks = Get-OfflineInteger $Row.blocks "offline observations"
  $internal = Get-OfflineInteger $Row.internal_block_frames "offline internal_block_frames"
  $measure = Get-OfflineInteger $Row.requested_measure_frames "offline requested_measure_frames"
  $requestedInternal = Get-OfflineInteger $Row.requested_internal_block_frames "offline requested_internal_block_frames"
  $requestedWorkers = Get-OfflineInteger $Row.workers_requested_count "offline workers_requested_count"
  $effectiveWorkers = Get-OfflineInteger $Row.workers_effective_count "offline workers_effective_count"
  if ($block -ne $Cell.measure_frames -or $sampleRateValue -ne $SampleRate -or $blocks -ne $Observations -or $internal -ne $Cell.internal_frames -or $measure -ne $Cell.measure_frames -or $requestedInternal -ne $Cell.internal_frames -or $requestedWorkers -ne $Cell.workers) { throw "Offline profile row requested geometry does not match the requested cell." }
  $expectedWorkers = if ($Cell.internal_frames -lt 256 -or $Cell.workers -eq 0) { 0 } else { [Math]::Min($Cell.workers, 3) }
  if ($effectiveWorkers -ne $expectedWorkers) { throw "Offline profile row effective worker geometry does not match the requested cell." }
  $overBudget = Get-OfflineInteger $Row.over_audio_duration_budget_count "offline over-budget count"
  $p999 = Get-OfflineNumber $Row.p99_9 "offline p99.9"
  $max = Get-OfflineNumber $Row.max "offline max"
  $dispatch = Get-OfflineInteger $Row.synth_parallel_dispatch_delta "offline dispatch delta"
  $lightSkips = Get-OfflineInteger $Row.synth_parallel_light_skip_delta "offline light-skip delta"
  $backoffSkips = Get-OfflineInteger $Row.synth_parallel_backoff_skip_delta "offline backoff delta"
  $timingBackoffs = Get-OfflineInteger $Row.synth_parallel_timing_backoff_delta "offline timing-backoff delta"
  $failures = Get-OfflineInteger $Row.synth_parallel_failure_delta "offline failure delta"
  if (@("true", "false") -notcontains $Row.synth_parallel_unhealthy) { throw "Offline worker unhealthy field is invalid." }
  $unhealthy = $Row.synth_parallel_unhealthy -eq "true"
  $eligibleDispatch = Test-OfflineMultiSlotSynthScenario $Cell.scenario
  $workerReasons = @()
  if ($backoffSkips -gt 0) { $workerReasons += "backoff skips" }
  if ($timingBackoffs -gt 0) { $workerReasons += "timing backoffs" }
  if ($failures -gt 0) { $workerReasons += "worker failures" }
  if ($unhealthy) { $workerReasons += "worker unhealthy" }
  if ($eligibleDispatch -and $expectedWorkers -gt 0 -and $dispatch -eq 0) { $workerReasons += "required dispatch missing" }
  if ($eligibleDispatch -and $lightSkips -gt 0) { $workerReasons += "light skips" }
  $status = if ($workerReasons.Count -gt 0) { "measured_failure" } elseif ($overBudget -gt 0) { "over_budget" } else { "pass" }
  return [pscustomobject]@{
    StatusClass = $status
    Scenario = $Cell.scenario
    OverBudget = $overBudget
    P99_9 = $p999
    Max = $max
    WorkersRequested = $requestedWorkers
    WorkersEffective = $effectiveWorkers
    WorkersExpected = $expectedWorkers
    WorkerFailure = $workerReasons.Count -gt 0
    WorkerFailureReason = $workerReasons -join ", "
    Row = $Row
  }
}

Export-ModuleMember -Function @("ConvertTo-PerformanceBaselineOfflineResult", "Test-OfflineMultiSlotSynthScenario")
