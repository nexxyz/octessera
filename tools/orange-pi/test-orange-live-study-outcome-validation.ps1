$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Import-Module (Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "orange-live-study-outcome-validation.psm1") -Force

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected validation failure did not occur." }
}

$expectedFailure = "Orange transport failed with exit code 20: ssh-payload"
foreach ($scenario in @("capacity_analogue_12", "capacity_analogue_16", "capacity_analogue_24")) {
  $approvedSelection = Assert-OrangeLiveBenchmarkSelection -Scenario $scenario -OutputFrames 128 -EngineBlockFrames 32 -MeasureSeconds 120 -ExecutorMode inline -WorkerTimingMode disabled
  foreach ($status in @("measured_failure", "over_budget")) {
    Assert-OrangeLiveStudyOutcome -Selection $approvedSelection -HostStatusClass $status -HostReason "completed inline observation" -StudyFailure $expectedFailure
  }
}
$selection = Assert-OrangeLiveBenchmarkSelection -Scenario "capacity_analogue_16" -OutputFrames 128 -EngineBlockFrames 32 -MeasureSeconds 120 -ExecutorMode inline -WorkerTimingMode disabled
$routingSelection = Assert-OrangeLiveBenchmarkSelection -Scenario "capacity_analogue_16" -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 120 -ExecutorMode routing_tree_persistent -WorkerTimingMode disabled
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $routingSelection -HostStatusClass measured_failure -StudyFailure $expectedFailure }
$routingContinuationSelection = Assert-OrangeLiveBenchmarkSelection -Scenario "capacity_analogue_16" -OutputFrames 256 -EngineBlockFrames 64 -MeasureSeconds 120 -ExecutorMode routing_tree_persistent -WorkerTimingMode disabled -ContinueOnRecoveredMiss:$true
Assert-OrangeLiveStudyOutcome -Selection $routingContinuationSelection -HostStatusClass measured_failure -StudyFailure $expectedFailure -ContinueOnRecoveredMiss:$true

$malformed = ConvertFrom-Json -InputObject ($selection | ConvertTo-Json -Depth 4)
$malformed.ExecutorMode = "Inline"
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $malformed -HostStatusClass measured_failure -StudyFailure $expectedFailure }
$malformed = ConvertFrom-Json -InputObject ($selection | ConvertTo-Json -Depth 4)
$malformed.ContinueOnRecoveredMiss = "false"
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $malformed -HostStatusClass measured_failure -StudyFailure $expectedFailure }
$malformed = ConvertFrom-Json -InputObject ($selection | ConvertTo-Json -Depth 4)
$malformed.MeasureSeconds = "120"
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $malformed -HostStatusClass measured_failure -StudyFailure $expectedFailure }
$malformed = ConvertFrom-Json -InputObject ($selection | ConvertTo-Json -Depth 4)
$malformed.MeasureSeconds = 119.6
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $malformed -HostStatusClass measured_failure -StudyFailure $expectedFailure }
$u85 = Assert-OrangeLiveBenchmarkSelection -Scenario "capacity_analogue_85" -OutputFrames 128 -EngineBlockFrames 32 -MeasureSeconds 120 -ExecutorMode inline -WorkerTimingMode disabled
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $u85 -HostStatusClass measured_failure -StudyFailure $expectedFailure }
$otherGeometry = ConvertFrom-Json -InputObject ($selection | ConvertTo-Json -Depth 4)
$otherGeometry.OutputFrames = 256
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $otherGeometry -HostStatusClass measured_failure -StudyFailure $expectedFailure }
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass measured_failure -StudyFailure "Orange transport failed with exit code 19: ssh-payload" }
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass "MEASURED_FAILURE" -StudyFailure $expectedFailure }
foreach ($status in @("infrastructure_failure", "safety_failure", "thermal_failure", "restoration_failure")) {
  Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass $status -StudyFailure $expectedFailure }
}
Assert-Throws { Assert-OrangeLiveStudyOutcome -Selection $selection -HostStatusClass measured_failure -StudyFailure $expectedFailure -RecoveryFailure ([Exception]::new("cleanup failed")) }

Write-Output "Orange live study outcome validation tests passed"
