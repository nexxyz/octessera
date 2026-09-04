$ErrorActionPreference = "Stop"

Import-Module (Join-Path $PSScriptRoot "orange-live-sensor-validation.psm1") -Force

function Get-SensorSummaryForLines {
  param([Parameter(Mandatory)][string[]]$Lines)
  $path = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-sensor-validation-" + [guid]::NewGuid().ToString("N"))
  try {
    Set-Content -LiteralPath $path -Value $Lines
    return Get-OrangeLiveSensorEvidence $path
  } finally {
    Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
  }
}

function Assert-InvalidCoolingEvidence {
  param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string[]]$Lines
  )
  if ((Get-SensorSummaryForLines $Lines).CoolingEvidenceValid) { throw "Invalid cooling evidence was accepted: $Name" }
}

$startup = "sample=cooling phase=startup time=1 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=0 max_state=10 observed=true"
$runtime = "sample=cooling phase=runtime time=2 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=3 max_state=10 observed=true"
$valid = Get-SensorSummaryForLines @($startup, $runtime)
if (-not $valid.CoolingEvidenceValid -or -not $valid.CoolingObserved -or $valid.MaxCoolingState -ne 3) { throw "Canonical startup/runtime cooling evidence was not accepted." }

$unobserved = Get-SensorSummaryForLines @(
  "sample=cooling phase=startup time=1 observed=false reason=cooling-devices-unobserved",
  "sample=cooling phase=runtime time=2 observed=false reason=cooling-devices-unobserved"
)
if (-not $unobserved.CoolingEvidenceValid -or $unobserved.CoolingObserved -or $null -ne $unobserved.MaxCoolingState) { throw "Explicit unobserved cooling evidence was not accepted." }

Assert-InvalidCoolingEvidence "missing runtime" @($startup)
Assert-InvalidCoolingEvidence "missing startup" @($runtime)
Assert-InvalidCoolingEvidence "missing both" @("sample=memory phase=startup mem_available_kb=600000")
foreach ($case in @(
    @{ Name = "phase"; Line = "sample=cooling phase=boot time=2 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=3 max_state=10 observed=true" },
    @{ Name = "path"; Line = "sample=cooling phase=runtime time=2 path=/sys/class/thermal/cooling_deviceX type=thermal-cpufreq-0 cur_state=3 max_state=10 observed=true" },
    @{ Name = "time"; Line = "sample=cooling phase=runtime time=0 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=3 max_state=10 observed=true" },
    @{ Name = "type"; Line = "sample=cooling phase=runtime time=2 path=/sys/class/thermal/cooling_device0 type= cur_state=3 max_state=10 observed=true" },
    @{ Name = "state"; Line = "sample=cooling phase=runtime time=2 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=-1 max_state=10 observed=true" },
    @{ Name = "max state"; Line = "sample=cooling phase=runtime time=2 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=3 max_state=-1 observed=true" },
    @{ Name = "state above max"; Line = "sample=cooling phase=runtime time=2 path=/sys/class/thermal/cooling_device0 type=thermal-cpufreq-0 cur_state=11 max_state=10 observed=true" },
    @{ Name = "unobserved extras"; Line = "sample=cooling phase=runtime time=2 observed=false reason=cooling-devices-unobserved extra=true" }
  )) {
  Assert-InvalidCoolingEvidence $case.Name @($startup, $runtime, $case.Line)
}

$malformedFrequency = Get-SensorSummaryForLines @($startup, $runtime, "sample=frequency phase=runtime time=2 path=/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq khz=bad")
if ($malformedFrequency.FrequencyEvidenceValid) { throw "Malformed frequency evidence was accepted." }
foreach ($case in @(
    "sample=frequency phase=boot time=2 path=/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq khz=800000",
    "sample=frequency phase=runtime time=2 path=/sys/devices/system/cpu/cpuX/cpufreq/scaling_cur_freq khz=800000",
    "sample=frequency phase=runtime time=0 path=/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq khz=800000",
    "sample=frequency phase=runtime time=2 path=/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq khz=0"
  )) {
  if ((Get-SensorSummaryForLines @($startup, $runtime, $case)).FrequencyEvidenceValid) { throw "Malformed frequency evidence was accepted: $case" }
}

Write-Output "Orange live sensor evidence validation tests passed"
