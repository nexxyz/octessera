Set-StrictMode -Version Latest

$script:OfflineRowFields = @(
  "kind", "scenario", "metric", "value", "block_frames", "sample_rate", "blocks", "avg", "p95", "p99", "max", "notes", "internal_block_frames", "schema_version", "p99_9", "over_audio_duration_budget_count", "requested_measure_frames", "requested_internal_block_frames", "peak_synth_voices", "peak_sample_voices", "peak_preview_sample_voices", "peak_momentary_fx", "peak_bus_fx_slots", "peak_global_fx_slots", "peak_voice_steals", "voice_steal_delta", "peak_voice_admission_drops", "voice_admission_drop_delta"
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

function Get-RequiredOfflineInteger {
  param(
    [Parameter(Mandatory)][pscustomobject]$Row,
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Context
  )
  $property = $Row.PSObject.Properties[$Name]
  if ($null -eq $property) { throw "$Context.$Name is required." }
  return Get-OfflineInteger ([string]$property.Value) "$Context.$Name"
}

function Get-ExpectedOfflineAdmissionDrops {
  param(
    [Parameter(Mandatory)][pscustomobject]$Cell,
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Context
  )
  $property = $Cell.PSObject.Properties[$Name]
  if ($null -eq $property) { return 0L }
  return Get-OfflineInteger ([string]$property.Value) "$Context.$Name"
}

function Assert-OfflineAdmissionDropEvidence {
  param(
    [Parameter(Mandatory)][long]$Peak,
    [Parameter(Mandatory)][long]$Delta,
    [Parameter(Mandatory)][long]$ExpectedStart,
    [Parameter(Mandatory)][long]$ExpectedEnd,
    [Parameter(Mandatory)][string]$Context
  )
  if ($ExpectedEnd -lt $ExpectedStart) { throw "$Context expected admission-drop end is below start." }
  $expectedDelta = $ExpectedEnd - $ExpectedStart
  if ($Peak -ne $ExpectedEnd -or $Delta -ne $expectedDelta) {
    throw "$Context admission-drop evidence does not reconcile with expected start/end values."
  }
}

function Assert-OfflineRowShape {
  param([Parameter(Mandatory)][pscustomobject]$Row)
  $actual = @($Row.PSObject.Properties.Name)
  if (($actual -join ",") -cne ($script:OfflineRowFields -join ",")) { throw "Offline profile row must use schema 4 field order." }
  if ($Row.kind -cne "engine_source" -or $Row.metric -cne "raw_ratio") { throw "Offline profile row is not an engine-source raw-ratio row." }
  if ((Get-OfflineInteger $Row.schema_version "offline schema_version") -ne 4) { throw "Offline profile row schema_version must be 4." }
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
  if ($block -ne $Cell.measure_frames -or $sampleRateValue -ne $SampleRate -or $blocks -ne $Observations -or $internal -ne $Cell.internal_frames -or $measure -ne $Cell.measure_frames -or $requestedInternal -ne $Cell.internal_frames) { throw "Offline profile row requested geometry does not match the requested cell." }
  $overBudget = Get-OfflineInteger $Row.over_audio_duration_budget_count "offline over-budget count"
  $p999 = Get-OfflineNumber $Row.p99_9 "offline p99.9"
  $max = Get-OfflineNumber $Row.max "offline max"
  $expectedAdmissionDropsStart = Get-ExpectedOfflineAdmissionDrops $Cell "expected_admission_drops_start" "offline cell"
  $expectedAdmissionDropsEnd = Get-ExpectedOfflineAdmissionDrops $Cell "expected_admission_drops_end" "offline cell"
  $admissionDropsPeak = Get-RequiredOfflineInteger $Row "peak_voice_admission_drops" "offline profile row"
  $admissionDropsDelta = Get-RequiredOfflineInteger $Row "voice_admission_drop_delta" "offline profile row"
  Assert-OfflineAdmissionDropEvidence $admissionDropsPeak $admissionDropsDelta $expectedAdmissionDropsStart $expectedAdmissionDropsEnd "offline profile row"
  $status = if ($overBudget -gt 0) { "over_budget" } else { "pass" }
  return [pscustomobject]@{
    StatusClass = $status
    Scenario = $Cell.scenario
    OverBudget = $overBudget
    P99_9 = $p999
    Max = $max
    Row = $Row
  }
}

Export-ModuleMember -Function "ConvertTo-PerformanceBaselineOfflineResult"
