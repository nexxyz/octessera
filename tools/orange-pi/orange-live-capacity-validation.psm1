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

$script:OrangeFrameSearchProfiles = @{
  OI32 = [pscustomobject]@{ ExecutorMode = "inline"; OutputFrames = 128; AlsaPeriodFrames = 32; InternalFrames = 32; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 128; WorkerTimingMode = "disabled"; ContinueOnRecoveredMiss = $false }
  OI64 = [pscustomobject]@{ ExecutorMode = "inline"; OutputFrames = 128; AlsaPeriodFrames = 32; InternalFrames = 64; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 128; WorkerTimingMode = "disabled"; ContinueOnRecoveredMiss = $false }
  OI256 = [pscustomobject]@{ ExecutorMode = "inline"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 256; WorkerTimingMode = "disabled"; ContinueOnRecoveredMiss = $false }
  OM32 = [pscustomobject]@{ ExecutorMode = "routing_tree_persistent"; OutputFrames = 128; AlsaPeriodFrames = 32; InternalFrames = 32; LookaheadFrames = 32; EffectiveOutputLatencyFrames = 160; WorkerTimingMode = "disabled"; ContinueOnRecoveredMiss = $true }
  OM64 = [pscustomobject]@{ ExecutorMode = "routing_tree_persistent"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 64; EffectiveOutputLatencyFrames = 320; WorkerTimingMode = "disabled"; ContinueOnRecoveredMiss = $true }
  OM128 = [pscustomobject]@{ ExecutorMode = "routing_tree_persistent"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 128; EffectiveOutputLatencyFrames = 384; WorkerTimingMode = "disabled"; ContinueOnRecoveredMiss = $true }
}

function ConvertTo-OrangeFrameSearchU {
  param([AllowNull()][object]$Value)

  if ($Value -is [string]) {
    if ($Value -cnotmatch '^(?:[1-9]|[1-3][0-9]|4[0-2])$') { throw "Frame-search U must be a canonical integer from 1 through 42." }
    return [int]$Value
  }
  if ($Value -is [byte] -or $Value -is [sbyte] -or $Value -is [int16] -or $Value -is [uint16] -or $Value -is [int32] -or $Value -is [uint32] -or $Value -is [int64] -or $Value -is [uint64]) {
    if ([decimal]$Value -lt 1 -or [decimal]$Value -gt 42) { throw "Frame-search U must be a canonical integer from 1 through 42." }
    return [int]$Value
  }
  throw "Frame-search U must be a canonical integer from 1 through 42."
}

function Assert-OrangeFrameSearchSelection {
  param(
    [Parameter(Mandatory)][string]$FrameSearchProfile,
    [Parameter(Mandatory)][string]$FrameSearchPhase,
    [AllowNull()][object]$FrameSearchU,
    [AllowNull()][object]$Scenario,
    [AllowNull()][object]$OutputFrames,
    [AllowNull()][object]$EngineBlockFrames,
    [AllowNull()][object]$MeasureSeconds,
    [AllowNull()][object]$ExecutorMode,
    [AllowNull()][object]$WorkerTimingMode,
    [AllowNull()][object]$ContinueOnRecoveredMiss,
    [AllowNull()][object]$AllowLongRepeat,
    [AllowNull()][hashtable]$Constraints
  )
  $bound = if ($null -ne $Constraints) { $Constraints } else { $PSBoundParameters }
  if ($script:OrangeFrameSearchProfiles.Keys -cnotcontains $FrameSearchProfile) { throw "Frame-search profile is not approved: $FrameSearchProfile" }
  if (@("Preliminary", "Soak") -cnotcontains $FrameSearchPhase) { throw "Frame-search phase must be Preliminary or Soak." }
  $u = ConvertTo-OrangeFrameSearchU $FrameSearchU
  $profile = $script:OrangeFrameSearchProfiles[$FrameSearchProfile]
  $expectedScenario = "capacity_analogue_$u"
  $measure = if ($FrameSearchPhase -ceq "Preliminary") { 180 } else { 600 }
  if ($bound.ContainsKey("Scenario") -and -not [string]::IsNullOrWhiteSpace([string]$Scenario) -and ([string]$Scenario -cne $expectedScenario)) { throw "Frame-search scenario contradicts the selected U." }
  if ($bound.ContainsKey("OutputFrames") -and $OutputFrames -cne $profile.OutputFrames) { throw "Frame-search profile geometry contradicts output frames." }
  if ($bound.ContainsKey("EngineBlockFrames") -and $EngineBlockFrames -ne $profile.InternalFrames) { throw "Frame-search profile geometry contradicts internal frames." }
  if ($bound.ContainsKey("MeasureSeconds") -and $MeasureSeconds -ne $measure) { throw "Frame-search phase requires exactly $measure measure seconds." }
  if ($bound.ContainsKey("ExecutorMode") -and ([string]$ExecutorMode -cne $profile.ExecutorMode)) { throw "Frame-search profile contradicts the executor mode." }
  if ($bound.ContainsKey("WorkerTimingMode") -and -not [string]::IsNullOrWhiteSpace([string]$WorkerTimingMode) -and ([string]$WorkerTimingMode -cne $profile.WorkerTimingMode)) { throw "Frame-search profile contradicts worker timing mode." }
  if ($bound.ContainsKey("ContinueOnRecoveredMiss") -and $ContinueOnRecoveredMiss -isnot [bool]) { throw "Frame-search continuation must be a Boolean." }
  if ($bound.ContainsKey("ContinueOnRecoveredMiss") -and [bool]$ContinueOnRecoveredMiss -ne $profile.ContinueOnRecoveredMiss) { throw "Frame-search profile continuation does not match the executor mode." }
  if ($bound.ContainsKey("AllowLongRepeat") -and [bool]$AllowLongRepeat) { throw "-AllowLongRepeat is not valid for frame-search runs." }

  $capacityScenario = ConvertFrom-OrangeCapacityScenario $expectedScenario
  $selection = [ordered]@{
    FrameSearchProfile = $FrameSearchProfile
    FrameSearchPhase = $FrameSearchPhase
    FrameSearchU = $u
    FrameSearchGeometry = [pscustomobject]@{
      OutputFrames = $profile.OutputFrames
      AlsaPeriodFrames = $profile.AlsaPeriodFrames
      InternalFrames = $profile.InternalFrames
      LookaheadFrames = $profile.LookaheadFrames
      EffectiveOutputLatencyFrames = $profile.EffectiveOutputLatencyFrames
    }
    Scenario = $expectedScenario
    OutputFrames = $profile.OutputFrames
    AlsaPeriodFrames = $profile.AlsaPeriodFrames
    EngineBlockFrames = $profile.InternalFrames
    InternalFrames = $profile.InternalFrames
    MeasureSeconds = $measure
    WarmupSeconds = 5
    MatrixClass = "diagnostic"
    LongRepeat = $false
    ExecutorMode = $profile.ExecutorMode
    WorkerTimingMode = $profile.WorkerTimingMode
    ContinueOnRecoveredMiss = $profile.ContinueOnRecoveredMiss
    LookaheadFrames = $profile.LookaheadFrames
    EffectiveOutputLatencyFrames = $profile.EffectiveOutputLatencyFrames
    IsCapacityDiagnostic = $true
    CapacityKind = $capacityScenario.Kind
    SynthCount = $capacityScenario.SynthCount
    SampleCount = $capacityScenario.SampleCount
    RequiredPoolCapacity = $capacityScenario.RequiredPoolCapacity
    RequiredPoolStage = 128
    ExpectedActiveSynthVoices = $capacityScenario.SynthCount
    ExpectedActiveSampleVoices = $capacityScenario.SampleCount
    ExpectedActivePreviewSampleVoices = 0
    ExpectedActiveMomentaryFx = [int][math]::Min([math]::Ceiling([double]$u / 4), 2)
    ExpectedActiveBusFxSlots = [int][math]::Min([math]::Ceiling([double]$u / 2), 12)
    ExpectedActiveGlobalFxSlots = [int][math]::Min([math]::Ceiling([double]$u / 8), 2)
    ExpectedVoiceSteals = 0
    ExpectedVoiceAdmissionDropsStart = 0
    ExpectedVoiceAdmissionDropsEnd = 0
    DiagnosticPoolIdentity = "benchmark-voice-pools-128"
    DiagnosticArtifactFeature = if ($profile.ExecutorMode -ceq "routing_tree_persistent") { "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128" } else { "hardware-orange-pi-zero-2w benchmark-voice-pools-128" }
  }
  return [pscustomobject]$selection
}

function Assert-OrangeCapacityBenchmarkSelection {
  param(
    [Parameter(Mandatory)][string]$Scenario,
    [Parameter(Mandatory)][pscustomobject]$CapacityScenario,
    [Parameter(Mandatory)][int]$OutputFrames,
    [Parameter(Mandatory)][int]$EngineBlockFrames,
    [Parameter(Mandatory)][int]$MeasureSeconds,
    [Parameter(Mandatory)][ValidateSet("inline", "routing_tree_persistent")][string]$ExecutorMode,
    [string]$WorkerTimingMode = "",
    [bool]$AllowLongRepeat = $false,
    [bool]$ContinueOnRecoveredMiss = $false
  )
  if (-not [string]::IsNullOrWhiteSpace($WorkerTimingMode) -and @("enabled", "disabled") -cnotcontains $WorkerTimingMode) { throw "WorkerTimingMode must be exactly enabled or disabled when provided." }
  $isAnalogue = $CapacityScenario.Kind -ceq "analogue"
  $expectedWorkerTimingMode = if ($ExecutorMode -eq "inline" -or $isAnalogue) { "disabled" } else { "enabled" }
  if ([string]::IsNullOrWhiteSpace($WorkerTimingMode)) { $WorkerTimingMode = $expectedWorkerTimingMode }
  if ($ExecutorMode -eq "inline" -and $WorkerTimingMode -cne "disabled") { throw "Inline executor requires disabled worker timing." }
  if ($ExecutorMode -eq "routing_tree_persistent" -and (($isAnalogue -and $WorkerTimingMode -cne "disabled") -or (-not $isAnalogue -and $WorkerTimingMode -cne "enabled"))) { throw "Capacity worker timing mode does not match the selected executor." }
  $isAnalogueInline = $isAnalogue -and $ExecutorMode -ceq "inline" -and $WorkerTimingMode -ceq "disabled"
  $isAnalogueRouting = $isAnalogue -and $ExecutorMode -ceq "routing_tree_persistent" -and $WorkerTimingMode -ceq "disabled"
  if ($OutputFrames -eq 128) {
    if (-not $isAnalogueInline -or $EngineBlockFrames -ne 32) { throw "Inline analogue capacity scenarios require output=128, period=32, internal=32, and disabled worker timing." }
  } elseif ($OutputFrames -ne 256 -or $EngineBlockFrames -ne 64) { throw "LiveAudioBenchmark capacity scenarios require output=256 and engine=64." } elseif ($isAnalogue -and -not $isAnalogueRouting) { throw "Routing analogue capacity scenarios require output=256, period=64, internal=64, and disabled worker timing." }
  if (@(30, 120, 180) -notcontains $MeasureSeconds) { throw "LiveAudioBenchmark capacity scenarios require a 30-, 120-, or 180-second measurement." }
  if ($AllowLongRepeat) { throw "-AllowLongRepeat is only valid for a 120-second A repeat." }
  if ($ContinueOnRecoveredMiss -and (-not $isAnalogueRouting -or $MeasureSeconds -ne 120)) { throw "-ContinueOnRecoveredMiss requires a routing analogue capacity observation at output=256, period=64, internal=64, measure=120, with worker timing disabled." }
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
    ContinueOnRecoveredMiss = $ContinueOnRecoveredMiss
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

Export-ModuleMember -Function @("Assert-OrangeAnalogueProfileEvidence", "Assert-OrangeCapacityBenchmarkSelection", "Assert-OrangeFrameSearchSelection", "ConvertFrom-OrangeCapacityScenario")
