Set-StrictMode -Version Latest

function Assert-OrangeLiveStudyOutcome {
  param(
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$HostStatusClass,
    [string]$HostReason = "",
    [AllowNull()][object]$StudyFailure,
    [AllowNull()][object]$RecoveryFailure,
    [bool]$ContinueOnRecoveredMiss = $false
  )
  if ($null -ne $RecoveryFailure) { throw $RecoveryFailure }
  $continueProperty = $Selection.PSObject.Properties["ContinueOnRecoveredMiss"]
  $capacityKindProperty = $Selection.PSObject.Properties["CapacityKind"]
  $capacityKind = if ($null -ne $capacityKindProperty) { $capacityKindProperty.Value } else { $null }
  $numericIdentity = @($Selection.MeasureSeconds, $Selection.OutputFrames, $Selection.AlsaPeriodFrames, $Selection.EngineBlockFrames, $Selection.InternalFrames, $Selection.LookaheadFrames, $Selection.EffectiveOutputLatencyFrames)
  $canonicalNumerics = @($numericIdentity | Where-Object { $_ -is [byte] -or $_ -is [sbyte] -or $_ -is [int16] -or $_ -is [uint16] -or $_ -is [int32] -or $_ -is [uint32] -or $_ -is [int64] -or $_ -is [uint64] }).Count -eq $numericIdentity.Count
  $continuationMatches = $null -ne $continueProperty -and $continueProperty.Value -is [bool] -and $continueProperty.Value -eq $ContinueOnRecoveredMiss
  $commonProfile = $canonicalNumerics -and $null -ne $capacityKindProperty -and $capacityKind -is [string] -and $capacityKind -ceq "analogue" -and $Selection.WorkerTimingMode -is [string] -and $Selection.WorkerTimingMode -ceq "disabled" -and $Selection.MeasureSeconds -eq 120
  $routingProfile = $commonProfile -and $continuationMatches -and $ContinueOnRecoveredMiss -and $Selection.ExecutorMode -is [string] -and $Selection.ExecutorMode -ceq "routing_tree_persistent" -and $Selection.OutputFrames -eq 256 -and $Selection.AlsaPeriodFrames -eq 64 -and $Selection.EngineBlockFrames -eq 64 -and $Selection.InternalFrames -eq 64 -and $Selection.LookaheadFrames -eq 64 -and $Selection.EffectiveOutputLatencyFrames -eq 320
  $inlineProfile = $commonProfile -and $continuationMatches -and -not $ContinueOnRecoveredMiss -and $Selection.Scenario -is [string] -and @("capacity_analogue_12", "capacity_analogue_16", "capacity_analogue_24") -ccontains $Selection.Scenario -and $Selection.ExecutorMode -is [string] -and $Selection.ExecutorMode -ceq "inline" -and $Selection.OutputFrames -eq 128 -and $Selection.AlsaPeriodFrames -eq 32 -and $Selection.EngineBlockFrames -eq 32 -and $Selection.InternalFrames -eq 32 -and $Selection.LookaheadFrames -eq 0 -and $Selection.EffectiveOutputLatencyFrames -eq 128
  $legacyEligible = $commonProfile -and ($routingProfile -or $inlineProfile)
  $frameEligible = $false
  $frameProfile = $Selection.PSObject.Properties["FrameSearchProfile"]
  $framePhase = $Selection.PSObject.Properties["FrameSearchPhase"]
  $frameU = $Selection.PSObject.Properties["FrameSearchU"]
  if ($null -ne $frameProfile -and $null -ne $framePhase -and $null -ne $frameU -and $frameU.Value -is [int]) {
    try {
      $expected = Assert-OrangeFrameSearchSelection `
        -FrameSearchProfile $frameProfile.Value `
        -FrameSearchPhase $framePhase.Value `
        -FrameSearchU $frameU.Value `
        -Scenario $Selection.Scenario `
        -OutputFrames $Selection.OutputFrames `
        -EngineBlockFrames $Selection.EngineBlockFrames `
        -MeasureSeconds $Selection.MeasureSeconds `
        -ExecutorMode $Selection.ExecutorMode `
        -WorkerTimingMode $Selection.WorkerTimingMode `
        -ContinueOnRecoveredMiss $continueProperty.Value
      $frameGeometry = $Selection.PSObject.Properties["FrameSearchGeometry"]
      $frameGeometryMatches = $null -ne $frameGeometry -and $frameGeometry.Value.OutputFrames -eq $expected.OutputFrames -and $frameGeometry.Value.AlsaPeriodFrames -eq $expected.AlsaPeriodFrames -and $frameGeometry.Value.InternalFrames -eq $expected.InternalFrames -and $frameGeometry.Value.LookaheadFrames -eq $expected.LookaheadFrames -and $frameGeometry.Value.EffectiveOutputLatencyFrames -eq $expected.EffectiveOutputLatencyFrames
      $frameEligible = $continuationMatches -and $frameGeometryMatches -and $Selection.MeasureSeconds -in @(180, 600) -and $Selection.CapacityKind -is [string] -and $Selection.CapacityKind -ceq "analogue" -and $Selection.IsCapacityDiagnostic -is [bool] -and $Selection.IsCapacityDiagnostic -and $Selection.RequiredPoolStage -eq 128 -and $Selection.DiagnosticPoolIdentity -ceq "benchmark-voice-pools-128"
    } catch {
      $frameEligible = $false
    }
  }
  $eligible = $legacyEligible -or $frameEligible
  $failureMessage = if ($null -eq $StudyFailure) { "" } elseif ($StudyFailure.PSObject.Properties["Exception"] -and $null -ne $StudyFailure.Exception) { [string]$StudyFailure.Exception.Message } else { [string]$StudyFailure }
  $expectedCompletedFailure = $failureMessage -ceq "Orange transport failed with exit code 20: ssh-payload"
  if ($eligible -and @("measured_failure", "over_budget") -ccontains $HostStatusClass -and $expectedCompletedFailure) { return }
  if ($null -ne $StudyFailure) { throw $StudyFailure }
  if ($HostStatusClass -cne "pass") { throw $HostReason }
}

Export-ModuleMember -Function @("Assert-OrangeLiveStudyOutcome")
