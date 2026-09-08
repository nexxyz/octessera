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
  $capacityKind = if ($null -ne $capacityKindProperty) { [string]$capacityKindProperty.Value } else { "" }
  $eligible = $ContinueOnRecoveredMiss -and $null -ne $continueProperty -and [bool]$continueProperty.Value -and $capacityKind -ceq "analogue" -and [int]$Selection.MeasureSeconds -eq 120 -and [string]$Selection.ExecutorMode -ceq "routing_tree_persistent" -and [string]$Selection.WorkerTimingMode -ceq "disabled" -and [int]$Selection.OutputFrames -eq 256 -and [int]$Selection.AlsaPeriodFrames -eq 64 -and [int]$Selection.EngineBlockFrames -eq 64 -and [int]$Selection.InternalFrames -eq 64 -and [int]$Selection.LookaheadFrames -eq 64 -and [int]$Selection.EffectiveOutputLatencyFrames -eq 320
  $failureMessage = if ($null -eq $StudyFailure) { "" } elseif ($StudyFailure.PSObject.Properties["Exception"] -and $null -ne $StudyFailure.Exception) { [string]$StudyFailure.Exception.Message } else { [string]$StudyFailure }
  $expectedCompletedFailure = $failureMessage -ceq "Orange transport failed with exit code 20: ssh-payload"
  if ($eligible -and @("measured_failure", "over_budget") -contains $HostStatusClass -and $expectedCompletedFailure) { return }
  if ($null -ne $StudyFailure) { throw $StudyFailure }
  if ($HostStatusClass -cne "pass") { throw $HostReason }
}

Export-ModuleMember -Function @("Assert-OrangeLiveStudyOutcome")
