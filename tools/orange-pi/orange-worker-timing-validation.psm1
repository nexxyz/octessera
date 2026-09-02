Set-StrictMode -Version Latest

function Assert-OrangeWorkerTimingProperties {
  param(
    [Parameter(Mandatory)][psobject]$Object,
    [Parameter(Mandatory)][string[]]$Allowed,
    [Parameter(Mandatory)][string]$Path
  )
  if ($Object -isnot [pscustomobject]) { throw "Live benchmark worker timing object is invalid: $Path" }
  foreach ($property in $Object.PSObject.Properties) {
    if ($Allowed -notcontains $property.Name) { throw "Live benchmark worker timing field is unknown: $Path.$($property.Name)" }
  }
}

function Get-OrangeWorkerTimingInteger {
  param(
    [AllowNull()][object]$Value,
    [Parameter(Mandatory)][string]$Path,
    [uint64]$Maximum = [uint64]::MaxValue,
    [switch]$Nullable
  )
  if ($null -eq $Value) {
    if ($Nullable) { return $null }
    throw "Live benchmark worker timing integer is missing: $Path"
  }
  if ($Value -isnot [byte] -and $Value -isnot [sbyte] -and $Value -isnot [int16] -and $Value -isnot [uint16] -and $Value -isnot [int32] -and $Value -isnot [uint32] -and $Value -isnot [int64] -and $Value -isnot [uint64]) {
    throw "Live benchmark worker timing integer type is invalid: $Path"
  }
  $numeric = [decimal]$Value
  if ($numeric -lt 0 -or $numeric -gt [decimal]$Maximum) { throw "Live benchmark worker timing integer is out of range: $Path" }
  return [uint64]$Value
}

function Get-OrangeWorkerTimingBoolean {
  param([AllowNull()][object]$Value, [Parameter(Mandatory)][string]$Path)
  if ($Value -isnot [bool]) { throw "Live benchmark worker timing boolean type is invalid: $Path" }
  return [bool]$Value
}

function Assert-OrangeWorkerTimingEvidence {
  param([Parameter(Mandatory)][pscustomobject]$Result)
  $modeProperty = $Result.PSObject.Properties["worker_timing_mode"]
  if ($null -eq $modeProperty -or $modeProperty.Value -isnot [string] -or @("enabled", "disabled") -cnotcontains [string]$modeProperty.Value) {
    throw "Live benchmark worker timing mode is missing or invalid."
  }
  $timingProperty = $Result.PSObject.Properties["worker_timing"]
  if ($modeProperty.Value -ceq "disabled") {
    if ($null -eq $timingProperty -or $null -ne $timingProperty.Value) { throw "Disabled worker timing mode must have null worker timing evidence." }
    return
  }
  if ($null -eq $timingProperty -or $timingProperty.Value -isnot [pscustomobject]) { throw "Live benchmark worker timing evidence is missing or invalid." }
  $timing = $timingProperty.Value
  Assert-OrangeWorkerTimingProperties -Object $timing -Allowed @("workers", "coordinator", "late_after_deadline_ns", "cpu_endpoint_changed") -Path "worker_timing"
  if ($timing.workers -isnot [array] -or $timing.workers.Count -ne 2 -or $timing.coordinator -isnot [pscustomobject]) { throw "Live benchmark worker timing shape is invalid." }

  $coordinatorRequired = @("sequence", "deadline_ns", "dispatch_to_deadline_start_ns", "dispatch_to_deadline_elapsed_ns", "in_flight_mask", "completed_mask", "first_parity", "dispatch_to_first_ns", "dispatch_to_both_ns", "reduction_ns", "coordinator_remainder_ns", "engine_block_total_ns", "callback_total_ns", "failed", "frozen")
  Assert-OrangeWorkerTimingProperties -Object $timing.coordinator -Allowed $coordinatorRequired -Path "worker_timing.coordinator"
  foreach ($name in $coordinatorRequired) {
    if ($null -eq $timing.coordinator.PSObject.Properties[$name]) { throw "Live benchmark coordinator timing field is missing: $name" }
  }
  $coordinatorSequence = Get-OrangeWorkerTimingInteger $timing.coordinator.sequence "worker_timing.coordinator.sequence" -Nullable
  $deadline = Get-OrangeWorkerTimingInteger $timing.coordinator.deadline_ns "worker_timing.coordinator.deadline_ns" -Nullable
  $dispatchToDeadlineStart = Get-OrangeWorkerTimingInteger $timing.coordinator.dispatch_to_deadline_start_ns "worker_timing.coordinator.dispatch_to_deadline_start_ns" -Nullable
  $deadlineElapsed = Get-OrangeWorkerTimingInteger $timing.coordinator.dispatch_to_deadline_elapsed_ns "worker_timing.coordinator.dispatch_to_deadline_elapsed_ns" -Nullable
  $inFlight = Get-OrangeWorkerTimingInteger $timing.coordinator.in_flight_mask "worker_timing.coordinator.in_flight_mask" -Maximum 3 -Nullable
  $completed = Get-OrangeWorkerTimingInteger $timing.coordinator.completed_mask "worker_timing.coordinator.completed_mask" -Maximum 3 -Nullable
  $firstParity = Get-OrangeWorkerTimingInteger $timing.coordinator.first_parity "worker_timing.coordinator.first_parity" -Maximum 1 -Nullable
  $dispatchToFirst = Get-OrangeWorkerTimingInteger $timing.coordinator.dispatch_to_first_ns "worker_timing.coordinator.dispatch_to_first_ns" -Nullable
  $dispatchToBoth = Get-OrangeWorkerTimingInteger $timing.coordinator.dispatch_to_both_ns "worker_timing.coordinator.dispatch_to_both_ns" -Nullable
  $reduction = Get-OrangeWorkerTimingInteger $timing.coordinator.reduction_ns "worker_timing.coordinator.reduction_ns" -Nullable
  $remainder = Get-OrangeWorkerTimingInteger $timing.coordinator.coordinator_remainder_ns "worker_timing.coordinator.coordinator_remainder_ns" -Nullable
  $engineTotal = Get-OrangeWorkerTimingInteger $timing.coordinator.engine_block_total_ns "worker_timing.coordinator.engine_block_total_ns" -Nullable
  $callbackTotal = Get-OrangeWorkerTimingInteger $timing.coordinator.callback_total_ns "worker_timing.coordinator.callback_total_ns" -Nullable
  $failed = Get-OrangeWorkerTimingBoolean $timing.coordinator.failed "worker_timing.coordinator.failed"
  $frozen = Get-OrangeWorkerTimingBoolean $timing.coordinator.frozen "worker_timing.coordinator.frozen"
  if (-not $frozen) { throw "Live benchmark coordinator timing was not frozen." }

  $workers = @()
  $cpuAvailability = $null
  $expectedEndpointChange = $false
  foreach ($worker in $timing.workers) {
    $required = @("sequence", "render_ns", "dispatch_to_finish_ns", "cpu_start", "cpu_end", "finished")
    Assert-OrangeWorkerTimingProperties -Object $worker -Allowed $required -Path "worker_timing.workers"
    foreach ($name in $required) {
      if ($null -eq $worker.PSObject.Properties[$name]) { throw "Live benchmark worker timing field is missing: $name" }
    }
    $workerSequence = Get-OrangeWorkerTimingInteger $worker.sequence "worker_timing.workers.sequence" -Nullable
    $render = Get-OrangeWorkerTimingInteger $worker.render_ns "worker_timing.workers.render_ns" -Nullable
    $dispatchToFinish = Get-OrangeWorkerTimingInteger $worker.dispatch_to_finish_ns "worker_timing.workers.dispatch_to_finish_ns" -Nullable
    $cpuStart = Get-OrangeWorkerTimingInteger $worker.cpu_start "worker_timing.workers.cpu_start" -Maximum ([uint64]([uint32]::MaxValue - 1)) -Nullable
    $cpuEnd = Get-OrangeWorkerTimingInteger $worker.cpu_end "worker_timing.workers.cpu_end" -Maximum ([uint64]([uint32]::MaxValue - 1)) -Nullable
    $finished = Get-OrangeWorkerTimingBoolean $worker.finished "worker_timing.workers.finished"
    if ($finished) {
      if ($null -eq $workerSequence -or $null -eq $render -or $null -eq $dispatchToFinish) { throw "Finished worker timing has nullable required fields." }
      if ($null -ne $coordinatorSequence -and $workerSequence -ne $coordinatorSequence) { throw "Worker timing sequence does not match the coordinator sequence." }
      if ($dispatchToFinish -lt $render) { throw "Worker dispatch timing precedes render timing." }
      $hasCpu = $null -ne $cpuStart
      if ($hasCpu -ne ($null -ne $cpuEnd)) { throw "Finished worker timing has partial CPU evidence." }
      if ($null -ne $cpuAvailability -and $cpuAvailability -ne $hasCpu) { throw "Worker CPU evidence disagrees about sampler availability." }
      $cpuAvailability = $hasCpu
      if ($hasCpu -and $cpuStart -ne $cpuEnd) { $expectedEndpointChange = $true }
    } elseif ($null -ne $workerSequence -or $null -ne $render -or $null -ne $dispatchToFinish -or $null -ne $cpuStart -or $null -ne $cpuEnd) {
      throw "Unexecuted worker timing must retain null measurements."
    }
    $workers += [pscustomobject]@{ sequence = $workerSequence; render_ns = $render; dispatch_to_finish_ns = $dispatchToFinish; cpu_start = $cpuStart; cpu_end = $cpuEnd; finished = $finished }
  }

  $late = Get-OrangeWorkerTimingInteger $timing.late_after_deadline_ns "worker_timing.late_after_deadline_ns" -Nullable
  $cpuEndpointChanged = Get-OrangeWorkerTimingBoolean $timing.cpu_endpoint_changed "worker_timing.cpu_endpoint_changed"
  if ($cpuEndpointChanged -ne $expectedEndpointChange) { throw "Worker CPU endpoint-change summary does not match sampled endpoints." }

  if ($null -eq $coordinatorSequence) {
    foreach ($name in @("deadline_ns", "dispatch_to_deadline_start_ns", "dispatch_to_deadline_elapsed_ns", "in_flight_mask", "completed_mask", "first_parity", "dispatch_to_first_ns", "dispatch_to_both_ns", "reduction_ns", "coordinator_remainder_ns", "engine_block_total_ns", "callback_total_ns")) {
      if ($null -ne $timing.coordinator.$name) { throw "Unexecuted coordinator timing must retain null measurements." }
    }
    if (@($workers | Where-Object { $_.finished }).Count -ne 0) { throw "Unexecuted coordinator timing has finished worker evidence." }
    if ($null -ne $late -or $cpuEndpointChanged) { throw "Unexecuted coordinator timing has a non-null summary." }
  } else {
    if ($null -eq $deadline -or $null -eq $dispatchToDeadlineStart -or $null -eq $inFlight -or $null -eq $completed) { throw "Executed coordinator timing is missing identity, dispatch origin, or masks." }
    if ($dispatchToDeadlineStart -gt ([uint64]::MaxValue - $deadline)) { throw "Executed coordinator timing has an overflowing deadline boundary." }
    $deadlineBoundary = [uint64]$dispatchToDeadlineStart + [uint64]$deadline
    if ($null -eq $engineTotal -or $null -eq $callbackTotal) { throw "Executed coordinator timing is missing terminal totals." }
    if (($inFlight -band $completed) -ne 0 -or ($inFlight -bor $completed) -ne 3) { throw "Coordinator masks do not partition both dispatched workers." }
    if ($engineTotal -gt $callbackTotal) { throw "Engine timing exceeds callback timing." }
    if ($null -ne $deadlineElapsed -and $deadlineElapsed -lt $deadlineBoundary) { throw "Deadline elapsed timing precedes the dispatch deadline boundary." }
    if (-not $failed -and $null -ne $deadlineElapsed) { throw "Healthy timing has a deadline elapsed value." }
    if (-not $failed) {
      if ($inFlight -ne 0 -or $completed -ne 3 -or $null -eq $reduction -or $null -eq $remainder) { throw "Healthy timing is missing complete execution evidence." }
    } elseif ($completed -ne 3 -and ($null -ne $reduction -or $null -ne $remainder)) {
      throw "Failed timing contains reduction evidence without both completions."
    } elseif ($null -ne $remainder -and $null -eq $reduction) {
      throw "Coordinator remainder evidence has no reduction evidence."
    }

    if ($completed -eq 0) {
      if ($null -ne $firstParity -or $null -ne $dispatchToFirst -or $null -ne $dispatchToBoth) { throw "Zero completed workers have completion evidence." }
    } else {
      if ($null -eq $firstParity -or $null -eq $dispatchToFirst) { throw "Completed mask has incomplete first-completion evidence." }
      if (($completed -band ([uint64]1 -shl [int]$firstParity)) -eq 0) { throw "First-completion parity is absent from the completed mask." }
      $firstWorker = $workers[[int]$firstParity]
      if (-not $firstWorker.finished -or $null -eq $firstWorker.dispatch_to_finish_ns) { throw "First completion has no finished worker evidence." }
      if ($dispatchToFirst -lt $firstWorker.dispatch_to_finish_ns -or $dispatchToFirst -gt $deadlineBoundary -or $firstWorker.dispatch_to_finish_ns -gt $deadlineBoundary) { throw "Completed-before-deadline first evidence is temporally impossible." }
      if ($completed -eq 3) {
        if ($null -eq $dispatchToBoth) { throw "Both-completion mask has no timing." }
        if ($dispatchToFirst -gt $dispatchToBoth -or $dispatchToBoth -gt $deadlineBoundary) { throw "Completion timing order is impossible." }
        foreach ($parity in 0..1) {
          $completedWorker = $workers[$parity]
          if (-not $completedWorker.finished -or $null -eq $completedWorker.dispatch_to_finish_ns -or $completedWorker.dispatch_to_finish_ns -gt $deadlineBoundary -or $dispatchToBoth -lt $completedWorker.dispatch_to_finish_ns) { throw "Both-completion evidence is temporally impossible." }
        }
      } elseif ($null -ne $dispatchToBoth) {
        throw "Both-completion timing exists without both completions."
      }
    }
  }

  if ($null -ne $coordinatorSequence) {
    $expectedLate = $null
    foreach ($worker in $workers) {
      if ($worker.finished -and $worker.sequence -eq $coordinatorSequence -and $worker.dispatch_to_finish_ns -gt $deadlineBoundary) {
        $candidate = $worker.dispatch_to_finish_ns - $deadlineBoundary
        if ($null -eq $expectedLate -or $candidate -gt $expectedLate) { $expectedLate = $candidate }
      }
    }
    if ($null -eq $expectedLate -and $null -ne $late) { throw "Late-after-deadline timing is non-null without a late worker." }
    if ($null -ne $expectedLate -and $late -ne $expectedLate) { throw "Late-after-deadline timing does not match worker timing." }
  }
  $joinedProperty = $Result.PSObject.Properties["joined_workers"]
  if ($null -eq $joinedProperty) { throw "Live benchmark joined-worker evidence is missing." }
  $joined = Get-OrangeWorkerTimingInteger $joinedProperty.Value "joined_workers" -Maximum 2
  if ($joined -ne 2 -or @($workers | Where-Object { $_.finished }).Count -ne 2) { throw "Persistent benchmark timing is missing a finished worker after both joins." }
}

Export-ModuleMember -Function @("Assert-OrangeWorkerTimingEvidence")
