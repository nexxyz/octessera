Set-StrictMode -Version Latest

$script:OrangePersistentOutputCounterFields = @("rendered_quantums", "repeated_quantums", "dropped_quantums", "deadline_misses", "deadline_recoveries")
$script:OrangeLiveResultFields = @("schema_version", "kind", "status", "board_profile", "scenario", "requested_output_buffer_frames", "expected_alsa_buffer_frames", "expected_alsa_period_frames", "internal_block_frames", "sample_format", "channels", "sample_rate", "warmup_seconds", "measure_seconds", "scheduler_qualified", "callback_scheduling_policy", "callback_scheduling_priority", "callback_scheduling_cpu", "post_dsp_zero", "measurement_stop_acknowledged", "stream_stopped", "final_progress_write_succeeded", "pid", "systemd_invocation_id", "artifact_sha256", "callback", "persistent_output_counters", "detected_continuity_events", "profile_start", "profile_end", "recovered_alsa_epipe_count", "recovered_alsa_epipe_observable", "terminal_error", "executor_mode", "worker_health", "worker_thread_name_0", "worker_thread_name_1", "joined_workers", "retirement_error", "worker_timing_mode", "worker_timing")
function Get-OrangeLiveStrictInteger {
  param(
    [AllowNull()][object]$Value,
    [Parameter(Mandatory)][string]$Path
  )
  if ($null -eq $Value -or $Value -isnot [byte] -and $Value -isnot [sbyte] -and $Value -isnot [int16] -and $Value -isnot [uint16] -and $Value -isnot [int32] -and $Value -isnot [uint32] -and $Value -isnot [int64] -and $Value -isnot [uint64]) {
    throw "Live benchmark integer field is missing or invalid: $Path"
  }
  if ([decimal]$Value -lt 0) { throw "Live benchmark integer field is negative: $Path" }
  return [uint64]$Value
}
function Get-OrangeLiveStrictBoolean {
  param([AllowNull()][object]$Value, [Parameter(Mandatory)][string]$Path)
  if ($Value -isnot [bool]) { throw "Live benchmark boolean field is missing or invalid: $Path" }
  return [bool]$Value
}
function Assert-OrangeLivePersistentOutputCounterSnapshot {
  param(
    [AllowNull()][object]$Snapshot,
    [Parameter(Mandatory)][string]$Path
  )
  if ($Snapshot -isnot [pscustomobject]) { throw "Live benchmark persistent output counter snapshot is invalid: $Path" }
  foreach ($property in $Snapshot.PSObject.Properties) {
    if ($script:OrangePersistentOutputCounterFields -cnotcontains $property.Name) { throw "Live benchmark persistent output counter field is unknown: $Path.$($property.Name)" }
  }
  $values = [ordered]@{}
  foreach ($name in $script:OrangePersistentOutputCounterFields) {
    $property = $Snapshot.PSObject.Properties[$name]
    if ($null -eq $property) { throw "Live benchmark persistent output counter field is missing: $Path.$name" }
    $values[$name] = Get-OrangeLiveStrictInteger -Value $property.Value -Path "$Path.$name"
  }
  return [pscustomobject]$values
}
function Assert-OrangeLiveResultFieldNames {
  param([AllowNull()][object]$Result)
  if ($Result -isnot [pscustomobject]) { throw "Live benchmark result is missing or invalid." }
  foreach ($property in $Result.PSObject.Properties) {
    if ($script:OrangeLiveResultFields -cnotcontains $property.Name) { throw "Live benchmark result field is unknown: $($property.Name)" }
  }
  foreach ($name in $script:OrangeLiveResultFields) {
    if ($null -eq $Result.PSObject.Properties[$name]) { throw "Live benchmark result field is missing: $name" }
  }
}
function Assert-OrangeLivePersistentOutputEvidence {
  param(
    [AllowNull()][object]$Evidence,
    [Parameter(Mandatory)][string]$ExecutorMode
  )
  if ($Evidence -isnot [pscustomobject]) { throw "Live benchmark persistent output counter evidence is missing or invalid." }
  $allowed = @("observable", "warmup", "start", "end", "delta")
  foreach ($property in $Evidence.PSObject.Properties) {
    if ($allowed -cnotcontains $property.Name) { throw "Live benchmark persistent output evidence field is unknown: $($property.Name)" }
  }
  foreach ($name in $allowed) {
    if ($null -eq $Evidence.PSObject.Properties[$name]) { throw "Live benchmark persistent output evidence field is missing: $name" }
  }
  $observable = Get-OrangeLiveStrictBoolean -Value $Evidence.observable -Path "persistent_output_counters.observable"
  $expectedObservable = $ExecutorMode -ceq "persistent_two_workers"
  if ($observable -ne $expectedObservable) { throw "Live benchmark persistent output observability does not match executor." }
  $snapshots = [ordered]@{}
  foreach ($name in @("warmup", "start", "end", "delta")) {
    $snapshots[$name] = Assert-OrangeLivePersistentOutputCounterSnapshot -Snapshot $Evidence.$name -Path "persistent_output_counters.$name"
  }
  foreach ($name in @("warmup", "start", "end")) {
    $snapshot = $snapshots[$name]
    if ($snapshot.deadline_recoveries -gt $snapshot.deadline_misses) { throw "Live benchmark persistent output recoveries exceed misses: persistent_output_counters.$name" }
    $dispositionTotal = [decimal]$snapshot.repeated_quantums + [decimal]$snapshot.dropped_quantums
    if ($dispositionTotal -gt [decimal][uint64]::MaxValue -or $snapshot.repeated_quantums -gt $snapshot.deadline_misses -or [decimal]$snapshot.deadline_misses -gt $dispositionTotal) { throw "Live benchmark persistent output dispositions are inconsistent: persistent_output_counters.$name" }
  }
  $delta = $snapshots.delta
  $startOutstandingMiss = $snapshots.start.deadline_misses -gt $snapshots.start.deadline_recoveries
  $deltaRecoveryLimit = [decimal]$delta.deadline_misses + [decimal]([uint64]$startOutstandingMiss)
  if ([decimal]$delta.deadline_recoveries -gt $deltaRecoveryLimit) { throw "Live benchmark persistent output delta recoveries exceed the allowed carry-in recovery: persistent_output_counters.delta" }
  $deltaDispositionTotal = [decimal]$delta.repeated_quantums + [decimal]$delta.dropped_quantums
  if ($deltaDispositionTotal -gt [decimal][uint64]::MaxValue -or $delta.repeated_quantums -gt $delta.deadline_misses -or [decimal]$delta.deadline_misses -gt $deltaDispositionTotal) { throw "Live benchmark persistent output delta dispositions are inconsistent: persistent_output_counters.delta" }
  foreach ($name in $script:OrangePersistentOutputCounterFields) {
    if ([decimal]$snapshots.warmup.$name -gt [decimal]$snapshots.start.$name -or [decimal]$snapshots.start.$name -gt [decimal]$snapshots.end.$name) {
      throw "Live benchmark persistent output counter snapshots are not monotonic: $name"
    }
    $expectedDelta = [uint64]([decimal]$snapshots.end.$name - [decimal]$snapshots.start.$name)
    if ($snapshots.delta.$name -ne $expectedDelta) { throw "Live benchmark persistent output counter delta is not end minus start: $name" }
  }
  if (-not $observable) {
    foreach ($name in @("warmup", "start", "end", "delta")) {
      foreach ($field in $script:OrangePersistentOutputCounterFields) {
        if ($snapshots[$name].$field -ne 0) { throw "Inline executor must report zero persistent output counters." }
      }
    }
  }
  return [pscustomobject]@{
    observable = $observable
    warmup = $snapshots.warmup
    start = $snapshots.start
    end = $snapshots.end
    delta = $snapshots.delta
  }
}
function Get-OrangeLiveAggregateRenderAudioDurationRatio {
  param([Parameter(Mandatory)][pscustomobject]$Result)
  $callback = $Result.PSObject.Properties["callback"]
  $duration = if ($null -ne $callback) { $callback.Value.PSObject.Properties["render_audio_duration_ns"] } else { $null }
  $frames = if ($null -ne $callback) { $callback.Value.PSObject.Properties["rendered_frames"] } else { $null }
  $count = if ($null -ne $callback) { $callback.Value.PSObject.Properties["callback_count"] } else { $null }; $minimum = if ($null -ne $callback) { $callback.Value.PSObject.Properties["callback_frames_min"] } else { $null }; $maximum = if ($null -ne $callback) { $callback.Value.PSObject.Properties["callback_frames_max"] } else { $null }
  $rate = $Result.PSObject.Properties["sample_rate"]
  if ($null -eq $duration -or $null -eq $frames -or $null -eq $count -or $null -eq $minimum -or $null -eq $maximum -or $null -eq $rate -or $null -eq $duration.Value -or $null -eq $frames.Value -or $null -eq $count.Value -or $null -eq $minimum.Value -or $null -eq $maximum.Value -or $null -eq $rate.Value) { throw "Live benchmark aggregate render-duration evidence is missing." }
  try { $durationValue = [double]$duration.Value; $frameValue = [double]$frames.Value; $countValue = [double]$count.Value; $minimumValue = [double]$minimum.Value; $maximumValue = [double]$maximum.Value; $rateValue = [double]$rate.Value } catch { throw "Live benchmark aggregate render-duration evidence is invalid." }
  if ([double]::IsNaN($durationValue) -or [double]::IsInfinity($durationValue) -or [double]::IsNaN($frameValue) -or [double]::IsInfinity($frameValue) -or [double]::IsNaN($countValue) -or [double]::IsInfinity($countValue) -or [double]::IsNaN($minimumValue) -or [double]::IsInfinity($minimumValue) -or [double]::IsNaN($maximumValue) -or [double]::IsInfinity($maximumValue) -or [double]::IsNaN($rateValue) -or [double]::IsInfinity($rateValue) -or $durationValue -ne [math]::Truncate($durationValue) -or $frameValue -ne [math]::Truncate($frameValue) -or $countValue -ne [math]::Truncate($countValue) -or $minimumValue -ne [math]::Truncate($minimumValue) -or $maximumValue -ne [math]::Truncate($maximumValue) -or $rateValue -ne [math]::Truncate($rateValue) -or $durationValue -le 0 -or $frameValue -le 0 -or $countValue -le 0 -or $minimumValue -le 0 -or $maximumValue -le 0 -or $rateValue -le 0 -or $minimumValue -gt $maximumValue) {
    throw "Live benchmark aggregate render-duration evidence is zero or invalid."
  }
  $lowerFrameBound = $countValue * $minimumValue; $upperFrameBound = $countValue * $maximumValue
  if ([double]::IsInfinity($lowerFrameBound) -or [double]::IsInfinity($upperFrameBound) -or $frameValue -lt $lowerFrameBound -or $frameValue -gt $upperFrameBound) { throw "Live benchmark callback frame aggregate is outside its measured bounds." }
  $ratio = $durationValue / ($frameValue * 1e9 / $rateValue)
  if ([double]::IsNaN($ratio) -or [double]::IsInfinity($ratio) -or $ratio -le 0) { throw "Live benchmark aggregate render-duration ratio is invalid." }
  return $ratio
}
Export-ModuleMember -Function @("Assert-OrangeLivePersistentOutputEvidence", "Assert-OrangeLiveResultFieldNames", "Get-OrangeLiveAggregateRenderAudioDurationRatio", "Get-OrangeLiveStrictInteger")
