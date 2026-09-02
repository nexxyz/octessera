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
    [bool]$RequireShutdown = $false
  )
  $executor = $Evidence.PSObject.Properties["executor_mode"]
  $health = $Evidence.PSObject.Properties["worker_health"]
  $name0 = $Evidence.PSObject.Properties["worker_thread_name_0"]
  $name1 = $Evidence.PSObject.Properties["worker_thread_name_1"]
  if ($null -eq $executor -or [string]$executor.Value -cne "persistent_two_workers" -or $null -eq $health -or [string]$health.Value -cne "healthy" -or $null -eq $name0 -or [string]$name0.Value -cne "oct-dsp-src-0" -or $null -eq $name1 -or [string]$name1.Value -cne "oct-dsp-src-1") {
    throw "Live benchmark worker executor evidence is invalid."
  }
  if ($RequireShutdown) {
    $joined = $Evidence.PSObject.Properties["joined_workers"]
    $retirement = $Evidence.PSObject.Properties["retirement_error"]
    if ($null -eq $joined -or [int]$joined.Value -ne 2 -or $null -eq $retirement -or $null -ne $retirement.Value) {
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
