$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$runner = Join-Path $PSScriptRoot "run-pi-timing-probes.ps1"
$wakeIntervals = "2,4,6,8,10,12"
$testTarget = "pi@pi.test.invalid"

function Invoke-PrintOnly {
  param([hashtable]$Parameters)
  $arguments = @(
    "-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $runner
  )
  foreach ($key in $Parameters.Keys) {
    if ($Parameters[$key] -is [bool]) {
      if ($Parameters[$key]) { $arguments += "-$key" }
    } else {
      $arguments += @("-$key", [string]$Parameters[$key])
    }
  }
  $output = @(& (Join-Path $PSHOME "powershell.exe") @arguments 2>&1)
  if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "PrintOnly failed." }
  ($output | ForEach-Object { [string]$_ }) -join "`n"
}

function Assert-Contains {
  param([string]$Text, [string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) { throw "Missing expected text: $Value" }
}

function Assert-NotContains {
  param([string]$Text, [string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -ge 0) { throw "Unexpected text: $Value" }
}

function Assert-Fails {
  param([hashtable]$Parameters, [string]$Message)
  $failed = $false
  try {
    Invoke-PrintOnly $Parameters | Out-Null
  } catch {
    $failed = $true
    if ($_.Exception.Message.IndexOf($Message, [StringComparison]::Ordinal) -lt 0) {
      throw
    }
  }
  if (-not $failed) { throw "Expected command generation to fail: $Message" }
}

$runtime = Invoke-PrintOnly @{ Target = $testTarget; Mode = "RuntimeOnly"; WakeIntervalsMs = $wakeIntervals; Snapshots = $true; PrintOnly = $true }
Assert-Contains $runtime "--timing-probe-wake-intervals-ms '2,4,6,8,10,12'"
Assert-Contains $runtime "--timing-probe-snapshots"

$live = Invoke-PrintOnly @{ Target = $testTarget; Mode = "Live"; WakeIntervalsMs = $wakeIntervals; PrintOnly = $true }
Assert-Contains $live "--timing-probe-wake-intervals-ms '2,4,6,8,10,12'"

$audioDrain = Invoke-PrintOnly @{ Target = $testTarget; Mode = "AudioDrain"; WakeIntervalsMs = $wakeIntervals; PrintOnly = $true }
Assert-NotContains $audioDrain "--timing-probe-wake-intervals-ms"

$dspFx = Invoke-PrintOnly @{ Target = $testTarget; Mode = "DspFxLimits"; WakeIntervalsMs = $wakeIntervals; PrintOnly = $true }
Assert-NotContains $dspFx "--timing-probe-wake-intervals-ms"

$dspSoak = Invoke-PrintOnly @{ Target = $testTarget; Mode = "DspSoak"; WakeIntervalsMs = $wakeIntervals; PrintOnly = $true }
Assert-NotContains $dspSoak "--timing-probe-wake-intervals-ms"

$profile = Invoke-PrintOnly @{
  Target = $testTarget
  Mode = "ProfileBaseline"
  Scenario = "synth_cross_slot_16"
  AudioRenderQuantumFrames = 256
  ProfileMeasureFrames = 256
  WakeIntervalsMs = $wakeIntervals
  PrintOnly = $true
}
Assert-NotContains $profile "--timing-probe-wake-intervals-ms"
Assert-Fails @{ Mode = "RuntimeOnly"; PrintOnly = $true } "missing mandatory"

Write-Output "Raspberry timing-probe wake interval wrapper checks passed"
