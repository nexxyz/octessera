Set-StrictMode -Version Latest

$script:OrangeProfileBaselineScenarioIds = @("baseline_idle", "synth_shipped_policy_8", "synth_cross_slot_16", "sample_8", "sample_cross_slot_64", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_0_bus_2_global_0_momentary", "fixed_8_synth_8_sample_6_bus_2_global_2_momentary", "fixed_8_synth_8_sample_12_bus_2_global_0_momentary", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "synth_cross_slot_32_no_steal", "synth_cross_slot_64_no_steal")
$script:OrangeBaselineLiveScenarioIds = @("synth_cross_slot_16", "sample_cross_slot_64", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "synth_cross_slot_32_no_steal")

function Get-OrangeBaselineLiveScenarioIds {
  return @($script:OrangeBaselineLiveScenarioIds)
}

function Assert-OrangeProfileBaselineSelection {
  param(
    [Parameter(Mandatory)][string]$Scenario,
    [Parameter(Mandatory)][int]$InternalFrames,
    [Parameter(Mandatory)][int]$MeasureFrames
  )
  if ($script:OrangeProfileBaselineScenarioIds -notcontains $Scenario) {
    throw "ProfileBaseline scenario is not an approved Phase-1 baseline ID: $Scenario"
  }
  if (@(64, 128, 256) -notcontains $InternalFrames -or @($InternalFrames) -notcontains $MeasureFrames) {
    throw "ProfileBaseline internal and measure frames must be the same approved value: 64, 128, or 256."
  }
  return [pscustomobject]@{
    Scenario = $Scenario
    InternalFrames = $InternalFrames
    MeasureFrames = $MeasureFrames
    WarmupSeconds = 2
    Observations = 4096
  }
}

Export-ModuleMember -Function @("Assert-OrangeProfileBaselineSelection", "Get-OrangeBaselineLiveScenarioIds")
