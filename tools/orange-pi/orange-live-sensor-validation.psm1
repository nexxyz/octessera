Set-StrictMode -Version Latest

function Get-OrangeLiveSensorEvidence {
  param([Parameter(Mandatory)][string]$Path)
  $thermal = @()
  $memory = @()
  $startupThermal = @()
  $runtimeThermal = @()
  $startupMemory = @()
  $runtimeMemory = @()
  $coolingStates = @()
  $frequencies = @()
  $coolingObserved = $false
  $coolingEvidenceValid = $true
  $frequencyEvidenceValid = $true
  $coolingObservedByPhase = @{ startup = $false; runtime = $false }
  $coolingUnobservedByPhase = @{ startup = $false; runtime = $false }
  if (Test-Path -LiteralPath $Path -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $Path) {
      if ($line -match "sample=thermal phase=(?<phase>[^ ]+).*millicelsius=(?<value>[0-9]+)") {
        $value = [int]$Matches.value
        $thermal += $value
        if ($Matches.phase -eq "startup") { $startupThermal += $value } else { $runtimeThermal += $value }
      }
      if ($line -match "sample=memory phase=(?<phase>[^ ]+).*mem_available_kb=(?<value>[0-9]+)") {
        $value = [int64]$Matches.value
        $memory += $value
        if ($Matches.phase -eq "startup") { $startupMemory += $value } else { $runtimeMemory += $value }
      }
      if ($line -cmatch '^sample=cooling phase=(?<phase>startup|runtime) time=(?<time>[1-9][0-9]*) path=(?<path>/sys/class/thermal/cooling_device[0-9]+) type=(?<type>[A-Za-z0-9_.:-]+) cur_state=(?<cur>0|[1-9][0-9]*) max_state=(?<max>0|[1-9][0-9]*) observed=true$') {
        $curState = 0L
        $maxState = 0L
        if (-not [int64]::TryParse($Matches.cur, [Globalization.NumberStyles]::None, [Globalization.CultureInfo]::InvariantCulture, [ref]$curState) -or -not [int64]::TryParse($Matches.max, [Globalization.NumberStyles]::None, [Globalization.CultureInfo]::InvariantCulture, [ref]$maxState) -or $curState -gt $maxState) {
          $coolingEvidenceValid = $false
        } elseif ($coolingUnobservedByPhase[$Matches.phase]) {
          $coolingEvidenceValid = $false
        } else {
          $coolingObservedByPhase[$Matches.phase] = $true
          $coolingObserved = $true
          $coolingStates += $curState
        }
      } elseif ($line -cmatch '^sample=cooling phase=(?<phase>startup|runtime) time=[1-9][0-9]* observed=false reason=cooling-devices-unobserved$') {
        if ($coolingObservedByPhase[$Matches.phase] -or $coolingUnobservedByPhase[$Matches.phase]) { $coolingEvidenceValid = $false } else { $coolingUnobservedByPhase[$Matches.phase] = $true }
      } elseif ($line -cmatch '^sample=cooling(?: |$)') {
        $coolingEvidenceValid = $false
      }
      if ($line -cmatch '^sample=frequency phase=(startup|runtime) time=[1-9][0-9]* path=/sys/devices/system/cpu/cpu[0-9]+/cpufreq/scaling_cur_freq khz=(?<value>[1-9][0-9]*)$') {
        $frequencyValue = 0L
        if (-not [int64]::TryParse($Matches.value, [Globalization.NumberStyles]::None, [Globalization.CultureInfo]::InvariantCulture, [ref]$frequencyValue)) { $frequencyEvidenceValid = $false } else { $frequencies += $frequencyValue }
      } elseif ($line -cmatch '^sample=frequency(?: |$)') {
        $frequencyEvidenceValid = $false
      }
    }
  }
  foreach ($phase in @("startup", "runtime")) {
    if (-not $coolingObservedByPhase[$phase] -and -not $coolingUnobservedByPhase[$phase]) { $coolingEvidenceValid = $false }
  }
  return [pscustomobject]@{
    MaxThermalMillicelsius = if ($thermal.Count -gt 0) { ($thermal | Measure-Object -Maximum).Maximum } else { $null }
    MinMemAvailableKb = if ($memory.Count -gt 0) { ($memory | Measure-Object -Minimum).Minimum } else { $null }
    StartupMaxThermalMillicelsius = if ($startupThermal.Count -gt 0) { ($startupThermal | Measure-Object -Maximum).Maximum } else { $null }
    StartupMinMemAvailableKb = if ($startupMemory.Count -gt 0) { ($startupMemory | Measure-Object -Minimum).Minimum } else { $null }
    RuntimeMaxThermalMillicelsius = if ($runtimeThermal.Count -gt 0) { ($runtimeThermal | Measure-Object -Maximum).Maximum } else { $null }
    RuntimeMinMemAvailableKb = if ($runtimeMemory.Count -gt 0) { ($runtimeMemory | Measure-Object -Minimum).Minimum } else { $null }
    StartupSampleCount = $startupMemory.Count
    RuntimeSampleCount = $runtimeMemory.Count
    CoolingObserved = $coolingObserved
    MaxCoolingState = if ($coolingStates.Count -gt 0) { ($coolingStates | Measure-Object -Maximum).Maximum } else { $null }
    MinFrequencyKhz = if ($frequencies.Count -gt 0) { ($frequencies | Measure-Object -Minimum).Minimum } else { $null }
    CoolingEvidenceValid = $coolingEvidenceValid
    FrequencyEvidenceValid = $frequencyEvidenceValid
  }
}

Export-ModuleMember -Function @("Get-OrangeLiveSensorEvidence")
