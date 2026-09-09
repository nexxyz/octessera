Set-StrictMode -Version Latest

$script:RaspberryFrameSearchProfiles = [ordered]@{
  RI64 = [pscustomobject][ordered]@{ Profile = "RI64"; ExecutorMode = "Inline"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 256 }
  RI128 = [pscustomobject][ordered]@{ Profile = "RI128"; ExecutorMode = "Inline"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 256 }
  RI512 = [pscustomobject][ordered]@{ Profile = "RI512"; ExecutorMode = "Inline"; OutputFrames = 512; AlsaPeriodFrames = 128; InternalFrames = 128; LookaheadFrames = 0; EffectiveOutputLatencyFrames = 512 }
  RM64 = [pscustomobject][ordered]@{ Profile = "RM64"; ExecutorMode = "Multicore"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 64; LookaheadFrames = 64; EffectiveOutputLatencyFrames = 320 }
  RM128 = [pscustomobject][ordered]@{ Profile = "RM128"; ExecutorMode = "Multicore"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 128; LookaheadFrames = 128; EffectiveOutputLatencyFrames = 384 }
  RM256 = [pscustomobject][ordered]@{ Profile = "RM256"; ExecutorMode = "Multicore"; OutputFrames = 256; AlsaPeriodFrames = 64; InternalFrames = 256; LookaheadFrames = 256; EffectiveOutputLatencyFrames = 512 }
}

function Test-RaspberryFrameSearchCanonicalInteger {
  param([object]$Value)
  if ($null -eq $Value -or $Value -is [bool]) { return $false }
  $typeName = $Value.GetType().FullName
  if (@("System.Byte", "System.UInt16", "System.UInt32", "System.UInt64") -contains $typeName) { return $true }
  if (@("System.SByte", "System.Int16", "System.Int32", "System.Int64") -contains $typeName) { return [int64]$Value -ge 0 }
  if ($typeName -eq "System.Decimal") {
    $bits = [decimal]::GetBits([decimal]$Value)
    return ($bits[3] -band 0x00FF0000) -eq 0 -and [decimal]$Value -ge 0 -and [decimal]$Value -le [decimal]::MaxValue
  }
  return $false
}

function Get-RaspberryFrameSearchProfile {
  param([Parameter(Mandatory)][ValidateSet("RI64", "RI128", "RI512", "RM64", "RM128", "RM256")][string]$Profile)
  $definition = $script:RaspberryFrameSearchProfiles[$Profile]
  if ($null -eq $definition) { throw "Unknown Raspberry frame-search profile: $Profile" }
  return $definition
}

function Get-RaspberryFrameSearchGeometryIdentity {
  param([Parameter(Mandatory)][pscustomobject]$Profile)
  return "output=$($Profile.OutputFrames),period=$($Profile.AlsaPeriodFrames),internal=$($Profile.InternalFrames),lookahead=$($Profile.LookaheadFrames),effective=$($Profile.EffectiveOutputLatencyFrames)"
}

function Assert-RaspberryLiveBenchmarkSelection {
  param(
    [object]$Units = 16,
    [ValidateSet("Inline", "Multicore", "")][string]$ExecutorMode = "",
    [object]$MeasureSeconds = 30,
    [switch]$ObserveCompromises,
    [ValidateSet("RI64", "RI128", "RI512", "RM64", "RM128", "RM256", "")][string]$FrameSearchProfile = "",
    [ValidateSet("Preliminary", "Soak", "")][string]$FrameSearchPhase = "",
    [AllowNull()][object]$OutputFrames = $null,
    [AllowNull()][object]$AlsaPeriodFrames = $null,
    [AllowNull()][Alias("EngineBlockFrames")][object]$InternalFrames = $null,
    [AllowNull()][object]$LookaheadFrames = $null,
    [AllowNull()][object]$EffectiveOutputLatencyFrames = $null
  )
  $hasProfile = -not [string]::IsNullOrWhiteSpace($FrameSearchProfile)
  $hasPhase = -not [string]::IsNullOrWhiteSpace($FrameSearchPhase)
  $measureSpecified = $PSBoundParameters.ContainsKey("MeasureSeconds")
  if ($hasProfile -ne $hasPhase) { throw "-FrameSearchProfile and -FrameSearchPhase must be provided together." }
  if ($hasProfile) {
    if (-not (Test-RaspberryFrameSearchCanonicalInteger $Units)) { throw "Raspberry frame-search Units must be a canonical integer." }
    if (-not (Test-RaspberryFrameSearchCanonicalInteger $MeasureSeconds)) { throw "Raspberry frame-search measure seconds must be a canonical integer." }
    $unitsValue = [uint64]$Units
    $measureValue = [uint64]$MeasureSeconds
    $profile = Get-RaspberryFrameSearchProfile $FrameSearchProfile
    if ($ObserveCompromises) { throw "-ObserveCompromises is not valid in Raspberry frame-search mode." }
    if (-not [string]::IsNullOrWhiteSpace($ExecutorMode) -and $ExecutorMode -cne $profile.ExecutorMode -and $ExecutorMode -ine $profile.ExecutorMode) { throw "Raspberry frame-search profile $($profile.Profile) requires executor mode $($profile.ExecutorMode)." }
    if ($unitsValue -lt 1 -or $unitsValue -gt 42) { throw "Raspberry frame-search Units must be an integer from 1 through 42." }
    $phase = if ($FrameSearchPhase -ieq "Preliminary") { "Preliminary" } elseif ($FrameSearchPhase -ieq "Soak") { "Soak" } else { throw "Raspberry frame-search phase must be Preliminary or Soak." }
    $expectedSeconds = if ($phase -ceq "Preliminary") { 180 } else { 600 }
    if (-not $measureSpecified) { $measureValue = $expectedSeconds } elseif ($measureValue -ne $expectedSeconds) { throw "Raspberry frame-search phase $phase requires exactly $expectedSeconds seconds." }
    $nativeExecutor = if ($profile.ExecutorMode -ceq "Inline") { "inline" } else { "routing_tree_persistent" }
    $expectedGeometry = @{ OutputFrames = $profile.OutputFrames; AlsaPeriodFrames = $profile.AlsaPeriodFrames; InternalFrames = $profile.InternalFrames; LookaheadFrames = $profile.LookaheadFrames; EffectiveOutputLatencyFrames = $profile.EffectiveOutputLatencyFrames }
    foreach ($field in $expectedGeometry.Keys) { if ($PSBoundParameters.ContainsKey($field) -and $expectedGeometry[$field] -ne $PSBoundParameters[$field]) { throw "Raspberry frame-search profile geometry contradicts $field." } }
    return [pscustomobject][ordered]@{
      Units = [int]$unitsValue
      Scenario = "capacity_analogue_$unitsValue"
      ExecutorMode = $profile.ExecutorMode
      NativeExecutorMode = $nativeExecutor
      WorkerTimingMode = "disabled"
      OutputFrames = $profile.OutputFrames
      AlsaPeriodFrames = $profile.AlsaPeriodFrames
      InternalFrames = $profile.InternalFrames
      LookaheadFrames = $profile.LookaheadFrames
      EffectiveOutputLatencyFrames = $profile.EffectiveOutputLatencyFrames
      MeasureSeconds = [int]$measureValue
      ObserveCompromises = $false
      ContinueOnRecoveredMiss = $profile.ExecutorMode -ceq "Multicore"
      ContinueOnRecoveredMissArgument = if ($profile.ExecutorMode -ceq "Multicore") { "--continue-on-recovered-miss" } else { "" }
      FrameSearchProfile = $profile.Profile
      FrameSearchPhase = $phase
      FrameSearchU = [int]$unitsValue
      FrameSearchGeometry = [pscustomobject][ordered]@{ OutputFrames = $profile.OutputFrames; AlsaPeriodFrames = $profile.AlsaPeriodFrames; InternalFrames = $profile.InternalFrames; LookaheadFrames = $profile.LookaheadFrames; EffectiveOutputLatencyFrames = $profile.EffectiveOutputLatencyFrames }
      IsFrameSearch = $true
      PhysicalRuns = 1
      GeometryIdentity = Get-RaspberryFrameSearchGeometryIdentity $profile
    }
  }
  try { $unitsValue = [int]$Units; $measureValue = [int]$MeasureSeconds } catch { throw "Raspberry benchmark Units and measure seconds must be integers." }
  if ($unitsValue -notin @(8, 12, 16, 24, 32)) { throw "Raspberry benchmark Units must be one of 8, 12, 16, 24, or 32 outside frame-search mode." }
  if ([string]::IsNullOrWhiteSpace($ExecutorMode)) { $ExecutorMode = "Inline" }
  if ($ExecutorMode -ieq "Inline") { $ExecutorMode = "Inline" } elseif ($ExecutorMode -ieq "Multicore") { $ExecutorMode = "Multicore" } else { throw "Raspberry benchmark executor mode must be Inline or Multicore." }
  if ($ObserveCompromises -and $measureValue -ne 120) { throw "-ObserveCompromises is only valid for 120-second capacity cells." }
  if ($measureValue -notin @(30, 120, 180, 300)) { throw "Raspberry benchmark measure seconds must be 30, 120, 180, or 300 outside frame-search mode." }
  $nativeExecutor = if ($ExecutorMode -ceq "Inline") { "inline" } else { "routing_tree_persistent" }
  $lookahead = if ($ExecutorMode -ceq "Inline") { 0 } else { 128 }
  $outputFrames = 256
  $alsaPeriodFrames = 64
  $internalFrames = 128
  $geometry = [pscustomobject][ordered]@{ OutputFrames = $outputFrames; AlsaPeriodFrames = $alsaPeriodFrames; InternalFrames = $internalFrames; LookaheadFrames = $lookahead; EffectiveOutputLatencyFrames = $outputFrames + $lookahead }
  return [pscustomobject][ordered]@{
    Units = [int]$unitsValue
    Scenario = "capacity_analogue_$unitsValue"
    ExecutorMode = $ExecutorMode
    NativeExecutorMode = $nativeExecutor
    WorkerTimingMode = "disabled"
    OutputFrames = $geometry.OutputFrames
    AlsaPeriodFrames = $geometry.AlsaPeriodFrames
    InternalFrames = $geometry.InternalFrames
    LookaheadFrames = $geometry.LookaheadFrames
    EffectiveOutputLatencyFrames = $geometry.EffectiveOutputLatencyFrames
    MeasureSeconds = [int]$measureValue
    ObserveCompromises = [bool]$ObserveCompromises
    ContinueOnRecoveredMiss = [bool]($ObserveCompromises -and $ExecutorMode -ceq "Multicore")
    ContinueOnRecoveredMissArgument = if ($ObserveCompromises -and $ExecutorMode -ceq "Multicore") { "--continue-on-recovered-miss" } else { "" }
    FrameSearchProfile = ""
    FrameSearchPhase = ""
    FrameSearchU = $null
    FrameSearchGeometry = $geometry
    IsFrameSearch = $false
    PhysicalRuns = $null
    GeometryIdentity = Get-RaspberryFrameSearchGeometryIdentity $geometry
  }
}

function Assert-RaspberryFrameSearchSelection {
  param(
    [Parameter(Mandatory)][ValidateSet("RI64", "RI128", "RI512", "RM64", "RM128", "RM256")][string]$FrameSearchProfile,
    [Parameter(Mandatory)][ValidateSet("Preliminary", "Soak")][string]$FrameSearchPhase,
    [AllowNull()][object]$Units = $null,
    [AllowNull()][object]$FrameSearchU = $null,
    [AllowNull()][object]$Scenario = $null,
    [AllowNull()][object]$OutputFrames = $null,
    [AllowNull()][object]$AlsaPeriodFrames = $null,
    [AllowNull()][Alias("EngineBlockFrames")][object]$InternalFrames = $null,
    [AllowNull()][object]$LookaheadFrames = $null,
    [AllowNull()][object]$EffectiveOutputLatencyFrames = $null,
    [AllowNull()][object]$MeasureSeconds = $null,
    [AllowNull()][object]$ExecutorMode = $null,
    [AllowNull()][object]$WorkerTimingMode = $null,
    [AllowNull()][object]$ContinueOnRecoveredMiss = $null,
    [AllowNull()][hashtable]$Constraints = $null
  )
  $bound = if ($null -ne $Constraints) { $Constraints } else { $PSBoundParameters }
  if (-not $bound.ContainsKey("Units") -and -not $bound.ContainsKey("FrameSearchU")) { throw "Raspberry frame-search Units must be supplied." }
  if ($bound.ContainsKey("Units") -and $bound.ContainsKey("FrameSearchU")) { throw "Raspberry frame-search Units must be supplied only once." }
  $unitsValue = if ($bound.ContainsKey("FrameSearchU")) { $bound.FrameSearchU } elseif ($bound.ContainsKey("Units")) { $bound.Units } else { $Units }
  $expectedSeconds = if ($FrameSearchPhase -ieq "Preliminary") { 180 } else { 600 }
  $executorValue = if ($bound.ContainsKey("ExecutorMode")) { [string]$bound.ExecutorMode } else { [string]$ExecutorMode }
  $measureValue = if ($bound.ContainsKey("MeasureSeconds")) { $bound.MeasureSeconds } else { $expectedSeconds }
  $selection = Assert-RaspberryLiveBenchmarkSelection -Units $unitsValue -ExecutorMode $executorValue -MeasureSeconds $measureValue -FrameSearchProfile $FrameSearchProfile -FrameSearchPhase $FrameSearchPhase
  $expectedValues = @{ Scenario = $selection.Scenario; OutputFrames = $selection.OutputFrames; AlsaPeriodFrames = $selection.AlsaPeriodFrames; InternalFrames = $selection.InternalFrames; LookaheadFrames = $selection.LookaheadFrames; EffectiveOutputLatencyFrames = $selection.EffectiveOutputLatencyFrames; MeasureSeconds = $selection.MeasureSeconds; ExecutorMode = $selection.ExecutorMode; WorkerTimingMode = $selection.WorkerTimingMode; ContinueOnRecoveredMiss = $selection.ContinueOnRecoveredMiss }
  foreach ($field in $expectedValues.Keys) {
    if ($bound.ContainsKey($field) -and $expectedValues[$field] -ne $bound[$field]) { throw "Raspberry frame-search profile or phase contradicts $field." }
  }
  return $selection
}

Export-ModuleMember -Function Assert-RaspberryLiveBenchmarkSelection, Assert-RaspberryFrameSearchSelection, Get-RaspberryFrameSearchProfile, Get-RaspberryFrameSearchGeometryIdentity
