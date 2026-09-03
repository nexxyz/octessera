$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "orange-worker-timing-validation.psm1") -Force

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected worker timing validation failure did not occur." }
}

function New-TimingResult {
  $worker0 = [pscustomobject]@{ sequence = 7; render_ns = 10; dispatch_to_finish_ns = 20; cpu_start = 2; cpu_end = 2; finished = $true }
  $worker1 = [pscustomobject]@{ sequence = 7; render_ns = 11; dispatch_to_finish_ns = 25; cpu_start = 3; cpu_end = 3; finished = $true }
  $coordinator = [pscustomobject]@{ sequence = 7; deadline_ns = 100; dispatch_to_deadline_start_ns = 10; dispatch_to_deadline_elapsed_ns = $null; in_flight_mask = 0; completed_mask = 3; first_parity = 0; dispatch_to_first_ns = 20; dispatch_to_both_ns = 25; reduction_ns = 4; coordinator_remainder_ns = 5; engine_block_total_ns = 40; callback_total_ns = 50; failed = $false; frozen = $true }
  [pscustomobject]@{
    worker_timing_mode = "enabled"
    executor_mode = "persistent_two_workers"
    joined_workers = 2
    worker_timing = [pscustomobject]@{ workers = @($worker0, $worker1); coordinator = $coordinator; late_after_deadline_ns = $null; cpu_endpoint_changed = $false }
  }
}

function Copy-TimingResult {
  param([Parameter(Mandatory)][pscustomobject]$Result)
  return (ConvertFrom-Json -InputObject ($Result | ConvertTo-Json -Depth 8))
}

function New-DeadlineTimingResult {
  $result = New-TimingResult
  $result.worker_timing.workers[1].dispatch_to_finish_ns = 125
  $result.worker_timing.coordinator.dispatch_to_deadline_elapsed_ns = 110
  $result.worker_timing.coordinator.in_flight_mask = 2
  $result.worker_timing.coordinator.completed_mask = 1
  $result.worker_timing.coordinator.dispatch_to_both_ns = $null
  $result.worker_timing.coordinator.reduction_ns = $null
  $result.worker_timing.coordinator.coordinator_remainder_ns = $null
  $result.worker_timing.coordinator.failed = $true
  $result.worker_timing.late_after_deadline_ns = 15
  return $result
}

function Assert-Rejects {
  param([Parameter(Mandatory)][scriptblock]$Mutate)
  $candidate = Copy-TimingResult (New-TimingResult)
  & $Mutate $candidate
  Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $candidate }
}

$valid = New-TimingResult
Assert-OrangeWorkerTimingEvidence -Result $valid
Assert-OrangeWorkerTimingEvidence -Result (New-DeadlineTimingResult)
$disabled = Copy-TimingResult $valid
$disabled.worker_timing_mode = "disabled"
$disabled.worker_timing = $null
Assert-OrangeWorkerTimingEvidence -Result $disabled
$invalidMode = Copy-TimingResult $valid
$invalidMode.worker_timing_mode = "invalid"
Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $invalidMode }
$coercedMode = Copy-TimingResult $valid
$coercedMode.worker_timing_mode = 1
Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $coercedMode }
$missingMode = Copy-TimingResult $valid
$missingMode.PSObject.Properties.Remove("worker_timing_mode")
Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $missingMode }
$enabledNull = Copy-TimingResult $valid
$enabledNull.worker_timing = $null
Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $enabledNull }
$disabledTiming = Copy-TimingResult $disabled
$disabledTiming.worker_timing = $valid.worker_timing
Assert-Throws { Assert-OrangeWorkerTimingEvidence -Result $disabledTiming }
Assert-Rejects { param($result) $result.worker_timing.coordinator.engine_block_total_ns = "40" }
Assert-Rejects { param($result) $result.worker_timing.coordinator.callback_total_ns = $null }
Assert-Rejects { param($result) $result.worker_timing.coordinator.deadline_ns = -1 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_deadline_start_ns = $null }
Assert-Rejects { param($result) $result.worker_timing.coordinator.in_flight_mask = 4 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.in_flight_mask = 0; $result.worker_timing.coordinator.completed_mask = 1 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.in_flight_mask = 1 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.frozen = "true" }
Assert-Rejects { param($result) $result.worker_timing.cpu_endpoint_changed = 1 }
Assert-Rejects { param($result) $result.worker_timing.cpu_endpoint_changed = $true }
Assert-Rejects { param($result) $result.worker_timing.workers[0].cpu_start = "2" }
Assert-Rejects { param($result) $result.worker_timing.workers[0].cpu_start = 3 }
Assert-Rejects { param($result) $result.worker_timing.workers[1].cpu_end = 2 }
Assert-Rejects { param($result) $result.worker_timing.workers[0].finished = 1 }
Assert-Rejects { param($result) $result.worker_timing.workers[0].render_ns = $null }
Assert-Rejects { param($result) $result.worker_timing.coordinator.first_parity = 1 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.completed_mask = 0; $result.worker_timing.coordinator.in_flight_mask = 3 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_both_ns = $null }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_both_ns = 24 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.reduction_ns = $null }
Assert-Rejects { param($result) $result.worker_timing.coordinator.coordinator_remainder_ns = $null }
Assert-Rejects { param($result) $result.worker_timing.coordinator.sequence = $null }
Assert-Rejects {
  param($result)
  $result.worker_timing.coordinator.sequence = $null
  foreach ($name in @("deadline_ns", "dispatch_to_deadline_start_ns", "dispatch_to_deadline_elapsed_ns", "in_flight_mask", "completed_mask", "first_parity", "dispatch_to_first_ns", "dispatch_to_both_ns", "reduction_ns", "coordinator_remainder_ns", "engine_block_total_ns", "callback_total_ns")) { $result.worker_timing.coordinator.$name = $null }
}
Assert-Rejects { param($result) $result.worker_timing = $null }
Assert-Rejects { param($result) $result.PSObject.Properties.Remove("worker_timing") }
Assert-Rejects { param($result) $result.worker_timing.PSObject.Properties.Remove("coordinator") }
Assert-Rejects { param($result) $result.worker_timing.PSObject.Properties.Add([psnoteproperty]::new("unknown", $true)) }
Assert-Rejects { param($result) $result.worker_timing.workers[0].PSObject.Properties.Add([psnoteproperty]::new("unknown", $true)) }
Assert-Rejects { param($result) $result.worker_timing.coordinator.PSObject.Properties.Add([psnoteproperty]::new("unknown", $true)) }
Assert-Rejects { param($result) $result.worker_timing.workers[1].sequence = 8 }
Assert-Rejects { param($result) $result.worker_timing.workers[0].dispatch_to_finish_ns = 9 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_first_ns = 30 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_both_ns = 19 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.engine_block_total_ns = 51 }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_deadline_elapsed_ns = 99; $result.worker_timing.coordinator.failed = $true }
Assert-Rejects { param($result) $result.worker_timing.coordinator.dispatch_to_deadline_start_ns = 50; $result.worker_timing.coordinator.dispatch_to_deadline_elapsed_ns = 149; $result.worker_timing.coordinator.failed = $true }
Assert-Rejects { param($result) $result.worker_timing.late_after_deadline_ns = 1 }
Assert-Rejects { param($result) $result.worker_timing.workers[0].finished = $false; $result.worker_timing.workers[0].sequence = $null; $result.worker_timing.workers[0].render_ns = $null; $result.worker_timing.workers[0].dispatch_to_finish_ns = $null; $result.worker_timing.workers[0].cpu_start = $null; $result.worker_timing.workers[0].cpu_end = $null }

Write-Output "Orange worker timing validator malformed-fixture tests passed"
