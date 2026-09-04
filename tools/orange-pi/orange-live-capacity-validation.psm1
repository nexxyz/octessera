Set-StrictMode -Version Latest

function ConvertFrom-OrangeCapacityScenario {
  param([Parameter(Mandatory)][string]$Scenario)

  if ($Scenario -notlike "capacity_*") { return $null }
  $kind = $null
  $synthCount = 0
  $sampleCount = 0
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
  } else {
    throw "LiveAudioBenchmark capacity scenario must use capacity_synth_<N>, capacity_sample_<N>, or capacity_mixed_<S>_<P> with positive decimal counts and no leading zeros."
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
    [bool]$AllowLongRepeat = $false
  )
  if ($OutputFrames -ne 256 -or $EngineBlockFrames -ne 64) { throw "LiveAudioBenchmark capacity scenarios require output=256 and engine=64." }
  if (@(30, 180) -notcontains $MeasureSeconds) { throw "LiveAudioBenchmark capacity scenarios require a 30- or 180-second measurement." }
  if ($AllowLongRepeat) { throw "-AllowLongRepeat is only valid for a 120-second A repeat." }
  return [pscustomobject]@{
    Scenario = $Scenario
    OutputFrames = 256
    AlsaPeriodFrames = 64
    EngineBlockFrames = 64
    InternalFrames = 64
    MeasureSeconds = $MeasureSeconds
    WarmupSeconds = 5
    MatrixClass = "diagnostic"
    LongRepeat = $false
    IsCapacityDiagnostic = $true
    CapacityKind = $CapacityScenario.Kind
    SynthCount = $CapacityScenario.SynthCount
    SampleCount = $CapacityScenario.SampleCount
    RequiredPoolCapacity = $CapacityScenario.RequiredPoolCapacity
  }
}

Export-ModuleMember -Function @("Assert-OrangeCapacityBenchmarkSelection", "ConvertFrom-OrangeCapacityScenario")
