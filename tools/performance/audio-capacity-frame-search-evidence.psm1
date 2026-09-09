Set-StrictMode -Version Latest

Import-Module (Join-Path $PSScriptRoot "..\pi\raspberry-live-benchmark-validation.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "..\orange-pi\orange-live-benchmark-validation.psm1") -Force

$script:FrameSearchEvidenceGrades = [ordered]@{ Stable = 0; Stretched = 1; Compromised = 2 }

function Get-FrameSearchEvidenceProperty {
  param([Parameter(Mandatory)][object]$Object, [Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][string]$Context)
  $properties = @($Object.PSObject.Properties | Where-Object { $_.Name -ceq $Name })
  if ($properties.Count -ne 1) { throw "$Context is missing '$Name'." }
  return $properties[0].Value
}

function Get-FrameSearchEvidenceString {
  param([AllowNull()][object]$Value, [Parameter(Mandatory)][string]$Context)
  if ($Value -isnot [string]) { throw "$Context must be a JSON string." }
  return [string]$Value
}

function Get-FrameSearchEvidenceBoolean {
  param([AllowNull()][object]$Value, [Parameter(Mandatory)][string]$Context)
  if ($Value -isnot [bool]) { throw "$Context must be a JSON boolean." }
  return [bool]$Value
}

function Get-FrameSearchEvidenceInteger {
  param([AllowNull()][object]$Value, [Parameter(Mandatory)][string]$Context)
  if ($null -eq $Value -or $Value -is [bool] -or ($Value -isnot [byte] -and $Value -isnot [sbyte] -and $Value -isnot [int16] -and $Value -isnot [uint16] -and $Value -isnot [int32] -and $Value -isnot [uint32] -and $Value -isnot [int64] -and $Value -isnot [uint64])) { throw "$Context must be a non-negative JSON integer." }
  if ([decimal]$Value -lt 0) { throw "$Context must be a non-negative JSON integer." }
  return [uint64]$Value
}

function Get-FrameSearchEvidenceNumber {
  param([AllowNull()][object]$Value, [Parameter(Mandatory)][string]$Context)
  if ($null -eq $Value -or $Value -is [bool] -or ($Value -isnot [byte] -and $Value -isnot [sbyte] -and $Value -isnot [int16] -and $Value -isnot [uint16] -and $Value -isnot [int32] -and $Value -isnot [uint32] -and $Value -isnot [int64] -and $Value -isnot [uint64] -and $Value -isnot [single] -and $Value -isnot [double] -and $Value -isnot [decimal])) { throw "$Context must be a finite JSON number." }
  $number = [double]$Value
  if ([double]::IsNaN($number) -or [double]::IsInfinity($number) -or $number -lt 0) { throw "$Context must be a finite non-negative JSON number." }
  return $number
}

function Get-FrameSearchEvidenceWorstIncident {
  param([Parameter(Mandatory)][uint64]$RepeatIncidents, [Parameter(Mandatory)][uint64]$SilentIncidents, [Parameter(Mandatory)][uint64]$AlsaRecoveryLogIncidents, [Parameter(Mandatory)][uint64]$CpalStreamErrors, [Parameter(Mandatory)][uint64]$CpalDeviceErrors)
  return [uint64](@($RepeatIncidents, $SilentIncidents, $AlsaRecoveryLogIncidents, $CpalStreamErrors, $CpalDeviceErrors) | Measure-Object -Maximum).Maximum
}

function Get-FrameSearchEvidenceGrade {
  param([Parameter(Mandatory)][uint64]$Worst, [Parameter(Mandatory)][int]$MeasureSeconds)
  if ($MeasureSeconds -eq 180) { if ($Worst -le 2) { return "Stable" }; if ($Worst -le 7) { return "Stretched" }; return "Compromised" }
  if ($MeasureSeconds -eq 600) { if ($Worst -le 9) { return "Stable" }; if ($Worst -le 24) { return "Stretched" }; return "Compromised" }
  throw "Unsupported frame-search duration: $MeasureSeconds seconds."
}

function Get-FrameSearchEvidencePath {
  param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Kind)
  if (-not (Test-Path -LiteralPath $Path -PathType $Kind)) { throw "Frame-search evidence $Kind was not found: $Path" }
  return ([IO.Path]::GetFullPath((Resolve-Path -LiteralPath $Path).Path))
}

function Read-FrameSearchEvidenceJson {
  param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Context)
  try { return [IO.File]::ReadAllText($Path) | ConvertFrom-Json } catch { throw "$Context is not valid JSON: $Path" }
}

function Assert-FrameSearchRecoveredDeadlineMiss {
  param([Parameter(Mandatory)][pscustomobject]$Result, [Parameter(Mandatory)][pscustomobject]$Run)
  if ($Run.Mode -cne "Multicore" -or [string]$Result.worker_health -cne "deadline_miss" -or [string]$Result.status -cne "fail" -or $null -ne $Result.terminal_error) { throw "Recovered deadline-miss evidence is not a completed failed observation." }
  if ($Run.Board -ceq "Raspberry") {
    $selection = Assert-RaspberryFrameSearchSelection -FrameSearchProfile $Run.Profile -FrameSearchPhase $Run.Phase -FrameSearchU $Run.U
    Assert-RaspberryLiveBenchmarkResult -Result $Result -Selection $selection -ArtifactHash ([string]$Result.artifact_sha256) -ExpectedPid ([int]$Result.pid) -ExpectedInvocation ([string]$Result.systemd_invocation_id)
    $validator = Get-Module -Name "raspberry-live-benchmark-validation"
    if ($null -eq $validator -or -not (& $validator { param($Value) Test-RaspberryLiveBenchmarkMeasurementComplete $Value } $Result)) { throw "Raspberry deadline-miss evidence does not satisfy the completed-observation contract." }
    return
  }
  if ($Run.Board -ceq "Orange") {
    $selection = Assert-OrangeLiveBenchmarkSelection -FrameSearchProfile $Run.Profile -FrameSearchPhase $Run.Phase -FrameSearchU $Run.U
    Assert-OrangeLiveResult -Result $Result -Selection $selection
    return
  }
  throw "Recovered deadline-miss evidence has an unsupported board: $($Run.Board)"
}

function Assert-FrameSearchHostEvidence {
  param([Parameter(Mandatory)][string]$EvidenceDirectory, [Parameter(Mandatory)][object]$Run, [Parameter(Mandatory)][string]$ArtifactHash, [Parameter(Mandatory)][string]$ExpectedBoardProfile)
  $directory = Get-FrameSearchEvidencePath $EvidenceDirectory "Container"
  $hostPath = Get-FrameSearchEvidencePath (Join-Path $directory "host-evidence.json") "Leaf"
  $resultPath = Get-FrameSearchEvidencePath (Join-Path $directory "benchmark-result.json") "Leaf"
  $host = Read-FrameSearchEvidenceJson $hostPath "Host evidence"
  $result = Read-FrameSearchEvidenceJson $resultPath "Benchmark result"
  $status = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $host "StatusClass" "host evidence") "host evidence.StatusClass"
  if (@("pass", "measured_failure", "over_budget") -cnotcontains $status) { throw "Host evidence has a fatal or unsupported status: $status" }
  $hostHash = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $host "ArtifactSha256" "host evidence") "host evidence.ArtifactSha256"
  if ($hostHash -cnotmatch '^[0-9a-f]{64}$' -or $hostHash -cne $ArtifactHash) { throw "Host evidence artifact identity does not match." }
  if ((Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $host "FrameSearchProfile" "host evidence") "host evidence.FrameSearchProfile") -cne $Run.Profile -or (Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $host "FrameSearchPhase" "host evidence") "host evidence.FrameSearchPhase") -cne $Run.Phase -or (Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $host "FrameSearchU" "host evidence") "host evidence.FrameSearchU") -ne [uint64]$Run.U) { throw "Host evidence profile, phase, or U does not match." }
  $hostGeometry = Get-FrameSearchEvidenceProperty $host "FrameSearchGeometry" "host evidence"
  foreach ($field in @(@("OutputFrames", "Output"), @("AlsaPeriodFrames", "Period"), @("InternalFrames", "Internal"), @("LookaheadFrames", "Lookahead"), @("EffectiveOutputLatencyFrames", "Effective"))) {
    if ((Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $hostGeometry $field[0] "host geometry") "host geometry.$($field[0])") -ne [uint64]$Run.($field[1])) { throw "Host evidence geometry does not match $($field[1])." }
  }
  $resultBoard = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $result "board_profile" "benchmark result") "benchmark result.board_profile"
  $resultHash = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $result "artifact_sha256" "benchmark result") "benchmark result.artifact_sha256"
  if ($resultBoard -cne $ExpectedBoardProfile -or $resultHash -cne $ArtifactHash -or $resultHash -cnotmatch '^[0-9a-f]{64}$') { throw "Benchmark result board or artifact identity does not match." }
  foreach ($field in @(@("requested_output_buffer_frames", "Output"), @("expected_alsa_period_frames", "Period"), @("internal_block_frames", "Internal"), @("lookahead_frames", "Lookahead"), @("effective_output_latency_frames", "Effective"), @("measure_seconds", "Sec"))) {
    if ((Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $result $field[0] "benchmark result") "benchmark result.$($field[0])") -ne [uint64]$Run.($field[1])) { throw "Benchmark result identity does not match $($field[0])." }
  }
  $resultStatus = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $result "status" "benchmark result") "benchmark result.status"
  if (@("pass", "fail") -cnotcontains $resultStatus) { throw "Benchmark result status is unsupported: $resultStatus" }
  if (($status -ceq "pass") -ne ($resultStatus -ceq "pass")) { throw "Host and benchmark result status classes disagree." }
  $worker = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $result "worker_health" "benchmark result") "benchmark result.worker_health"
  if ($worker -ceq "deadline_miss") { Assert-FrameSearchRecoveredDeadlineMiss $result $Run } elseif (@("disabled", "healthy") -cnotcontains $worker) { throw "Benchmark result worker_health is unsupported: $worker" }
  $provenance = Get-FrameSearchEvidenceProperty $result "persistent_output_provenance" "benchmark result"
  $observable = Get-FrameSearchEvidenceBoolean (Get-FrameSearchEvidenceProperty $provenance "observable" "persistent output provenance") "persistent output provenance.observable"
  if ($observable -ne ($Run.Mode -ceq "Multicore")) { throw "Persistent output observability does not match board mode." }
  foreach ($name in @("repeated_quantum_incidents", "repeated_pcm_frames", "silent_quantum_incidents", "silent_pcm_frames")) { Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $provenance $name "persistent output provenance") "persistent output provenance.$name" | Out-Null }
  if (-not $observable -and @(@("repeated_quantum_incidents", "repeated_pcm_frames", "silent_quantum_incidents", "silent_pcm_frames") | Where-Object { (Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $provenance $_ "persistent output provenance") "persistent output provenance.$_") -ne 0 }).Count -gt 0) { throw "Inline host evidence did not record its required unobservable/zero provenance contract." }
  $callback = Get-FrameSearchEvidenceProperty $result "callback" "benchmark result"
  $cpalDevice = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $callback "cpal_device_error_count" "callback") "callback.cpal_device_error_count"
  $cpalStream = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $callback "cpal_stream_error_count" "callback") "callback.cpal_stream_error_count"
  $callbackOver = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $callback "over_audio_duration_budget_count" "callback") "callback.over_audio_duration_budget_count"
  $p999 = Get-FrameSearchEvidenceNumber (Get-FrameSearchEvidenceProperty $callback "render_audio_duration_ratio_p99_9" "callback") "callback.render_audio_duration_ratio_p99_9"
  $maxRatio = Get-FrameSearchEvidenceNumber (Get-FrameSearchEvidenceProperty $callback "render_audio_duration_ratio_max" "callback") "callback.render_audio_duration_ratio_max"
  if ($maxRatio -lt $p999) { throw "Callback maximum ratio is below P99.9." }
  $repeat = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $host "RepeatIncidents" "host evidence") "host evidence.RepeatIncidents"
  $silent = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $host "SilentIncidents" "host evidence") "host evidence.SilentIncidents"
  $alsa = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $host "AlsaRecoveryLogIncidents" "host evidence") "host evidence.AlsaRecoveryLogIncidents"
  $repeatExpected = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $provenance "repeated_quantum_incidents" "persistent output provenance") "persistent output provenance.repeated_quantum_incidents"
  $silentExpected = Get-FrameSearchEvidenceInteger (Get-FrameSearchEvidenceProperty $provenance "silent_quantum_incidents" "persistent output provenance") "persistent output provenance.silent_quantum_incidents"
  if ($repeat -ne $repeatExpected -or $silent -ne $silentExpected) { throw "Host evidence incident fields do not match benchmark provenance." }
  $worst = Get-FrameSearchEvidenceWorstIncident $repeat $silent $alsa $cpalStream $cpalDevice
  $grade = Get-FrameSearchEvidenceGrade $worst ([int]$Run.Sec)
  $practicalGrade = Get-FrameSearchEvidenceString (Get-FrameSearchEvidenceProperty $host "PracticalGrade" "host evidence") "host evidence.PracticalGrade"
  if (@("Stable", "Stretched", "Compromised") -cnotcontains $practicalGrade) { throw "Host evidence practical grade is unsupported: $practicalGrade" }
  if ($practicalGrade -cne $grade) { throw "Host evidence PracticalGrade does not independently match Worst." }
  return [pscustomobject][ordered]@{ StatusClass = $status; Grade = $grade; Worst = $worst; RepeatIncidents = $repeat; SilentIncidents = $silent; AlsaRecoveryLogIncidents = $alsa; CpalStreamErrors = $cpalStream; CpalDeviceErrors = $cpalDevice; CallbackOverBudget = $callbackOver; P999 = $p999; MaxRatio = $maxRatio; NativeStatus = $resultStatus; NativeWorker = $worker; EvidenceDirectory = $directory; HostEvidencePath = $hostPath; ResultPath = $resultPath; EvidencePath = $directory; BoardProfile = $resultBoard; ObservableProvenance = $observable }
}

Export-ModuleMember -Function Get-FrameSearchEvidenceProperty, Get-FrameSearchEvidenceInteger, Get-FrameSearchEvidenceNumber, Get-FrameSearchEvidenceWorstIncident, Get-FrameSearchEvidenceGrade, Assert-FrameSearchHostEvidence
