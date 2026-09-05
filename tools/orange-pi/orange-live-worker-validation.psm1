Set-StrictMode -Version Latest

function Get-OrangeRequiredNonNegativeInteger {
  param(
    [Parameter(Mandatory)][object]$Parent,
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Context
  )
  $property = $Parent.PSObject.Properties[$Name]
  if ($null -eq $property -or $null -eq $property.Value) { throw "$Context.$Name is required." }
  $parsed = 0L
  if (-not [long]::TryParse([string]$property.Value, [Globalization.NumberStyles]::Integer, [Globalization.CultureInfo]::InvariantCulture, [ref]$parsed) -or $parsed -lt 0) { throw "$Context.$Name must be a non-negative integer." }
  return $parsed
}

function Assert-OrangeWorkerEvidence {
  param(
    [Parameter(Mandatory)][pscustomobject]$Evidence,
    [bool]$RequireShutdown = $false,
    [switch]$AllowTerminalHealth
  )
  $executor = $Evidence.PSObject.Properties["executor_mode"]
  $health = $Evidence.PSObject.Properties["worker_health"]
  $name0 = $Evidence.PSObject.Properties["worker_thread_name_0"]
  $name1 = $Evidence.PSObject.Properties["worker_thread_name_1"]
  if ($null -eq $executor -or $executor.Value -isnot [string] -or @("inline", "persistent_two_workers", "routing_tree_persistent") -cnotcontains $executor.Value -or $null -eq $health -or $health.Value -isnot [string] -or $null -eq $name0 -or $name0.Value -isnot [string] -or $null -eq $name1 -or $name1.Value -isnot [string]) {
    throw "Live benchmark worker executor evidence is invalid."
  }
  if ($executor.Value -ceq "inline") {
    if ($health.Value -cne "disabled" -or $name0.Value -cne "" -or $name1.Value -cne "") {
      throw "Inline benchmark worker executor evidence is invalid."
    }
  } else {
    $expectedName0 = if ($executor.Value -ceq "routing_tree_persistent") { "oct-dsp-tree-0" } else { "oct-dsp-src-0" }
    $expectedName1 = if ($executor.Value -ceq "routing_tree_persistent") { "oct-dsp-tree-1" } else { "oct-dsp-src-1" }
    if (($AllowTerminalHealth -and @("healthy", "deadline_miss", "dispatch_failed", "completion_failed", "worker_exited", "invalid_block") -cnotcontains $health.Value) -or (-not $AllowTerminalHealth -and $health.Value -cne "healthy") -or $name0.Value -cne $expectedName0 -or $name1.Value -cne $expectedName1) {
      throw "Persistent benchmark worker executor evidence is invalid."
    }
  }
  if ($RequireShutdown) {
    $joined = $Evidence.PSObject.Properties["joined_workers"]
    $retirement = $Evidence.PSObject.Properties["retirement_error"]
    $expectedJoined = if ($executor.Value -ceq "inline") { 0 } else { 2 }
    if ($null -eq $joined -or $joined.Value -isnot [byte] -and $joined.Value -isnot [int16] -and $joined.Value -isnot [uint16] -and $joined.Value -isnot [int32] -and $joined.Value -isnot [uint32] -and $joined.Value -isnot [int64] -and $joined.Value -isnot [uint64] -or [int]$joined.Value -ne $expectedJoined -or $null -eq $retirement -or $null -ne $retirement.Value) {
      throw "Live benchmark worker shutdown evidence is invalid."
    }
  }
}

function Get-OrangeExpectedAdmissionDrops {
  param(
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$Name
  )
  $property = $Selection.PSObject.Properties[$Name]
  if ($null -eq $property) { return 0L }
  return Get-OrangeRequiredNonNegativeInteger $Selection $Name "selection"
}

Export-ModuleMember -Function Assert-OrangeWorkerEvidence, Get-OrangeExpectedAdmissionDrops, Get-OrangeRequiredNonNegativeInteger
