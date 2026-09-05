Set-StrictMode -Version Latest

function ConvertFrom-OrangeCapacityScenario {
  param([Parameter(Mandatory)][string]$Scenario)

  if ($Scenario -notlike "capacity_*") { return $null }
  $kind = $null
  $synthCount = 0
  $sampleCount = 0
  $units = 0
  if ($Scenario -cmatch '^capacity_synth_(?<count>[1-9][0-9]{0,2})$') {
    $kind = "synth"
    $synthCount = [int]$Matches.count
  } elseif ($Scenario -cmatch '^capacity_sample_(?<count>[1-9][0-9]{0,2})$') {
    $kind = "sample"
    $sampleCount = [int]$Matches.count
  } elseif ($Scenario -cmatch '^capacity_mixed_(?<synth>[1-9][0-9]{0,2})_(?<sample>[1-9][0-9]{0,2})$') {
    $kind = "mixed"
    $synthCount = [int]$Matches.synth
    $sampleCount = [int]$Matches.sample
  } elseif ($Scenario -cmatch '^capacity_analogue_(?<units>[1-9][0-9]*)$') {
    [uint64]$units = 0
    if (-not [uint64]::TryParse($Matches.units, [ref]$units) -or $units -gt 85) {
      throw "LiveAudioBenchmark capacity_analogue scenario units must be no greater than 85."
    }
    $kind = "analogue"
    $synthCount = [int](3 * $units)
    $sampleCount = [int]$units
  } else {
    throw "LiveAudioBenchmark capacity scenario must use capacity_synth_<N>, capacity_sample_<N>, capacity_mixed_<S>_<P>, or capacity_analogue_<u> with positive decimal counts and no leading zeros."
  }
  if ($synthCount -gt 256 -or $sampleCount -gt 256) {
    throw "LiveAudioBenchmark capacity scenario counts must be no greater than 256."
  }
  return [pscustomobject]@{
    Kind = $kind
    SynthCount = $synthCount
    SampleCount = $sampleCount
    RequiredPoolCapacity = [math]::Max($synthCount, $sampleCount)
  }
}

function Assert-OrangeCapacityBenchmarkSelection {
  param(
    [Parameter(Mandatory)][string]$Scenario,
    [Parameter(Mandatory)][pscustomobject]$CapacityScenario,
    [Parameter(Mandatory)][int]$OutputFrames,
    [Parameter(Mandatory)][int]$EngineBlockFrames,
    [Parameter(Mandatory)][int]$MeasureSeconds,
    [Parameter(Mandatory)][ValidateSet("inline", "persistent_two_workers", "routing_tree_persistent")][string]$ExecutorMode,
    [string]$WorkerTimingMode = "",
    [bool]$AllowLongRepeat = $false
  )
  if (-not [string]::IsNullOrWhiteSpace($WorkerTimingMode) -and @("enabled", "disabled") -cnotcontains $WorkerTimingMode) { throw "WorkerTimingMode must be exactly enabled or disabled when provided." }
  $expectedWorkerTimingMode = if ($ExecutorMode -eq "inline") { "disabled" } else { "enabled" }
  if ([string]::IsNullOrWhiteSpace($WorkerTimingMode)) { $WorkerTimingMode = $expectedWorkerTimingMode }
  if ($ExecutorMode -eq "inline" -and $WorkerTimingMode -cne "disabled") { throw "Inline executor requires disabled worker timing." }
  if ($ExecutorMode -eq "routing_tree_persistent" -and $WorkerTimingMode -cne "enabled") { throw "Routing-tree persistent executor requires enabled worker timing." }
  $isAnalogueU16 = $CapacityScenario.Kind -ceq "analogue" -and $ExecutorMode -ceq "inline" -and $WorkerTimingMode -ceq "disabled"
  if ($OutputFrames -eq 128) {
    if (-not $isAnalogueU16 -or @(32, 64) -notcontains $EngineBlockFrames) { throw "Analogue U16 capacity scenarios require output=128 and engine=32 or 64." }
  } elseif ($OutputFrames -ne 256 -or $EngineBlockFrames -ne 64) { throw "LiveAudioBenchmark capacity scenarios require output=256 and engine=64." }
  if (@(30, 120, 180) -notcontains $MeasureSeconds) { throw "LiveAudioBenchmark capacity scenarios require a 30-, 120-, or 180-second measurement." }
  if ($AllowLongRepeat) { throw "-AllowLongRepeat is only valid for a 120-second A repeat." }
  $selection = [ordered]@{
    Scenario = $Scenario
    OutputFrames = $OutputFrames
    AlsaPeriodFrames = if ($OutputFrames -eq 128) { 32 } else { 64 }
    EngineBlockFrames = $EngineBlockFrames
    InternalFrames = $EngineBlockFrames
    MeasureSeconds = $MeasureSeconds
    WarmupSeconds = 5
    MatrixClass = "diagnostic"
    LongRepeat = $false
    ExecutorMode = $ExecutorMode
    WorkerTimingMode = $WorkerTimingMode
    LookaheadFrames = if ($ExecutorMode -eq "routing_tree_persistent") { $EngineBlockFrames } else { 0 }
    EffectiveOutputLatencyFrames = if ($ExecutorMode -eq "routing_tree_persistent") { $OutputFrames + $EngineBlockFrames } else { $OutputFrames }
    IsCapacityDiagnostic = $true
    CapacityKind = $CapacityScenario.Kind
    SynthCount = $CapacityScenario.SynthCount
    SampleCount = $CapacityScenario.SampleCount
    RequiredPoolCapacity = $CapacityScenario.RequiredPoolCapacity
  }
  if ($CapacityScenario.Kind -ceq "analogue") {
    $units = $CapacityScenario.SampleCount
    $selection.RequiredPoolStage = if ($units -le 42) { 128 } else { 256 }
    $selection.ExpectedActiveSynthVoices = $CapacityScenario.SynthCount
    $selection.ExpectedActiveSampleVoices = $CapacityScenario.SampleCount
    $selection.ExpectedActivePreviewSampleVoices = 0
    $selection.ExpectedActiveMomentaryFx = [int][math]::Min([math]::Ceiling([double]$units / 4), 2)
    $selection.ExpectedActiveBusFxSlots = [int][math]::Min([math]::Ceiling([double]$units / 2), 12)
    $selection.ExpectedActiveGlobalFxSlots = [int][math]::Min([math]::Ceiling([double]$units / 8), 2)
    $selection.ExpectedVoiceSteals = 0
    $selection.ExpectedVoiceAdmissionDropsStart = 0
    $selection.ExpectedVoiceAdmissionDropsEnd = 0
  }
  return [pscustomobject]$selection
}

function Assert-OrangeAnalogueProfileEvidence {
  param(
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][pscustomobject]$ProfileStart,
    [Parameter(Mandatory)][pscustomobject]$ProfileEnd
  )
  if ($Selection.CapacityKind -cne "analogue") { return }
  $expected = [ordered]@{
    active_synth_voices = $Selection.ExpectedActiveSynthVoices
    active_sample_voices = $Selection.ExpectedActiveSampleVoices
    active_preview_sample_voices = $Selection.ExpectedActivePreviewSampleVoices
    active_momentary_fx = $Selection.ExpectedActiveMomentaryFx
    active_bus_fx_slots = $Selection.ExpectedActiveBusFxSlots
    active_global_fx_slots = $Selection.ExpectedActiveGlobalFxSlots
    cumulative_voice_steals = $Selection.ExpectedVoiceSteals
  }
  foreach ($profile in @(@{ Name = "profile_start"; Value = $ProfileStart }, @{ Name = "profile_end"; Value = $ProfileEnd })) {
    foreach ($field in $expected.Keys) {
      if ([uint64]$profile.Value.$field -ne [uint64]$expected[$field]) { throw "Analogue $($profile.Name) profile field mismatch: $field" }
    }
    $expectedDrops = if ($profile.Name -ceq "profile_start") { $Selection.ExpectedVoiceAdmissionDropsStart } else { $Selection.ExpectedVoiceAdmissionDropsEnd }
    if ([uint64]$profile.Value.cumulative_voice_admission_drops -ne [uint64]$expectedDrops) { throw "Analogue $($profile.Name) profile field mismatch: cumulative_voice_admission_drops" }
  }
}

Export-ModuleMember -Function @("Assert-OrangeAnalogueProfileEvidence", "Assert-OrangeCapacityBenchmarkSelection", "ConvertFrom-OrangeCapacityScenario")
