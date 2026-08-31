Set-StrictMode -Version Latest

function Get-OrangeLiveWorkerValidation {
  param(
    [Parameter(Mandatory)][pscustomobject]$Result,
    [Parameter(Mandatory)][pscustomobject]$Selection
  )

  $profileStart = $Result.profile_start
  $profileEnd = $Result.profile_end
  $workerDelta = $Result.worker_delta
  if ($null -eq $profileStart -or $null -eq $profileEnd -or $null -eq $workerDelta) {
    throw "Live benchmark worker profiles or delta are incomplete."
  }
  $fields = @(
    "synth_parallel_dispatches",
    "synth_parallel_light_skips",
    "synth_parallel_backoff_skips",
    "synth_parallel_timing_backoffs",
    "synth_parallel_failures"
  )
  $delta = [ordered]@{}
  foreach ($field in $fields) {
    $start = $profileStart.PSObject.Properties[$field]
    $end = $profileEnd.PSObject.Properties[$field]
    $actual = $workerDelta.PSObject.Properties[$field]
    if ($null -eq $start -or $null -eq $end -or $null -eq $actual -or $null -eq $start.Value -or $null -eq $end.Value -or $null -eq $actual.Value) {
      throw "Live benchmark worker profile or delta field is missing: $field"
    }
    $startValue = [uint64]$start.Value
    $endValue = [uint64]$end.Value
    if ($endValue -lt $startValue) { throw "Live benchmark worker profile counter regressed: $field" }
    $delta[$field] = $endValue - $startValue
    if ([uint64]$actual.Value -cne $delta[$field]) { throw "Live benchmark worker delta does not match profile counters: $field" }
  }
  $startUnhealthy = $profileStart.PSObject.Properties["synth_parallel_unhealthy"]
  $endUnhealthy = $profileEnd.PSObject.Properties["synth_parallel_unhealthy"]
  $actualUnhealthy = $workerDelta.PSObject.Properties["synth_parallel_unhealthy"]
  if ($null -eq $startUnhealthy -or $null -eq $endUnhealthy -or $null -eq $actualUnhealthy -or $null -eq $startUnhealthy.Value -or $null -eq $endUnhealthy.Value -or $null -eq $actualUnhealthy.Value) {
    throw "Live benchmark worker unhealthy state is missing."
  }
  $delta.synth_parallel_unhealthy = [bool]$startUnhealthy.Value -or [bool]$endUnhealthy.Value
  if ([bool]$actualUnhealthy.Value -ne $delta.synth_parallel_unhealthy) { throw "Live benchmark worker unhealthy delta does not match profile state." }

  $expectedWorkers = $Selection.InternalFrames -ge 256 -and $Selection.Workers -gt 0
  if ([bool]$Result.workers_effective -ne $expectedWorkers) { throw "Live benchmark result worker effectiveness mismatch." }
  $requiredDispatch = $expectedWorkers -and $Selection.InternalFrames -eq 256 -and @("synth_cross_slot_96_steal", "mixed_cross_slot_48_48_steal", "synth_cross_slot_32_no_steal", "mixed_16_synth_32_sample") -contains $Selection.Scenario
  $reasons = @()
  if (-not $expectedWorkers -and ($delta.synth_parallel_dispatches -gt 0 -or $delta.synth_parallel_light_skips -gt 0 -or $delta.synth_parallel_backoff_skips -gt 0 -or $delta.synth_parallel_timing_backoffs -gt 0 -or $delta.synth_parallel_failures -gt 0 -or $delta.synth_parallel_unhealthy)) { $reasons += "ineffective worker telemetry" }
  if ($delta.synth_parallel_light_skips -gt 0) { $reasons += "light skips" }
  if ($delta.synth_parallel_backoff_skips -gt 0) { $reasons += "backoff skips" }
  if ($delta.synth_parallel_timing_backoffs -gt 0) { $reasons += "timing backoffs" }
  if ($delta.synth_parallel_failures -gt 0) { $reasons += "worker failures" }
  if ($delta.synth_parallel_unhealthy) { $reasons += "worker unhealthy" }
  if ($requiredDispatch -and $delta.synth_parallel_dispatches -eq 0) { $reasons += "required dispatch missing" }
  $policyViolation = $reasons.Count -gt 0
  $policyProperty = $Result.PSObject.Properties["worker_policy_error"]
  if ($null -eq $policyProperty) { throw "Live benchmark worker policy error field is missing." }
  $policyError = if ($null -eq $policyProperty.Value) { "" } else { [string]$policyProperty.Value }
  $hasPolicyError = -not [string]::IsNullOrWhiteSpace($policyError)
  if ([string]$Result.status -eq "pass" -and ($policyViolation -or $hasPolicyError)) { throw "Live benchmark pass worker policy is inconsistent with worker evidence." }
  if ([string]$Result.status -eq "fail" -and $policyViolation -ne $hasPolicyError) { throw "Live benchmark failed worker policy is inconsistent with worker evidence." }
  return [pscustomobject]@{
    Delta = [pscustomobject]$delta
    PolicyViolation = $policyViolation
    PolicyViolationReason = $reasons -join ", "
    PolicyError = if ($hasPolicyError) { $policyError } else { $null }
    HasPolicyError = $hasPolicyError
    RequiredDispatch = $requiredDispatch
  }
}

Export-ModuleMember -Function "Get-OrangeLiveWorkerValidation"
