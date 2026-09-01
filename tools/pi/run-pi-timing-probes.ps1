param(
  [string]$Target = "pi@192.168.0.218",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$Binary = "/usr/local/bin/octessera-pi",
  [string]$Service = "octessera.service",
  [ValidateSet("RuntimeOnly", "Live", "AudioDrain", "DspFxLimits", "DspSoak", "ProfileBaseline")]
  [string]$Mode = "RuntimeOnly",
  [string]$Durations = "5s",
  [string]$Scenarios = "idle,pulses-stress",
  [int]$AudioRenderQuantumFrames = 0,
  [int]$ProfileMeasureFrames = 0,
  [int]$AudioOutputBufferFrames = 0,
  [int]$AudioDrainIntervalMs = 10,
  [string]$Scenario = "",
  [string]$Metadata = "",
  [switch]$AllowServiceInterruption,
  [switch]$Snapshots,
  [switch]$KeepServiceRunning,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"
$boardProfilePath = Join-Path $PSScriptRoot "board-profile.ps1"
. $boardProfilePath
$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "with-pi-ssh.ps1")).Path
$interruptsService = -not $KeepServiceRunning -and $Mode -ne "RuntimeOnly"
if (-not $PrintOnly -and $interruptsService -and -not $AllowServiceInterruption) { throw "$Mode requires -AllowServiceInterruption." }

if ($Mode -eq "ProfileBaseline") {
  $profileBaselineScenarioIds = @("baseline_idle", "synth_shipped_policy_8", "synth_cross_slot_16", "sample_8", "sample_cross_slot_64", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_0_bus_2_global_0_momentary", "fixed_8_synth_8_sample_6_bus_2_global_2_momentary", "fixed_8_synth_8_sample_12_bus_2_global_0_momentary", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "synth_cross_slot_32_no_steal", "synth_cross_slot_64_no_steal")
  if ([string]::IsNullOrWhiteSpace($Scenario)) { throw "ProfileBaseline requires -Scenario." }
  if ($profileBaselineScenarioIds -notcontains $Scenario) { throw "ProfileBaseline scenario is not an approved Phase-1 baseline ID: $Scenario" }
  if (@(64, 128, 256) -notcontains $AudioRenderQuantumFrames -or @($AudioRenderQuantumFrames) -notcontains $ProfileMeasureFrames) {
    throw "ProfileBaseline requires equal -AudioRenderQuantumFrames and -ProfileMeasureFrames of 64, 128, or 256."
  }
  if (-not $PrintOnly -and -not $AllowServiceInterruption) { throw "ProfileBaseline requires -AllowServiceInterruption." }
  if (-not $PrintOnly) {
    if ([string]::IsNullOrWhiteSpace($Metadata)) { throw "ProfileBaseline requires exact Raspberry board metadata with -Metadata." }
    Read-RaspberryBoardMetadata $Metadata | Out-Null
  }
}

function Quote-ShValue {
  param([string]$Value)
  "'" + $Value.Replace("'", "'\''") + "'"
}

function Env-Assignment {
  param([string]$Name, [string]$Value)
  "$Name=$(Quote-ShValue $Value)"
}

function Invoke-PiSsh {
  param([string]$Command)
  if ($PrintOnly) {
    $Command
    return
  }
  $payloadPath = Join-Path $env:TEMP ("octessera-pi-timing-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($payloadPath, $Command, $encoding)
    & $transport "ssh-payload" -Target $Target -Key $Key $payloadPath
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
  } finally {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
  if ($exitCode -ne 0) {
    exit $exitCode
  }
}

$envParts = @(
  (Env-Assignment "OCTESSERA_PI_STORE_DIR" "/home/pi/presets"),
  (Env-Assignment "OCTESSERA_PI_SAMPLES_DIR" "/home/pi/samples")
)

if ($AudioOutputBufferFrames -gt 0) {
  $envParts += Env-Assignment "OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES" ([string]$AudioOutputBufferFrames)
}

if ($AudioRenderQuantumFrames -gt 0) {
  $envParts += Env-Assignment "OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES" ([string]$AudioRenderQuantumFrames)
}

if ($ProfileMeasureFrames -gt 0) {
  $envParts += Env-Assignment "OCTESSERA_PI_PROFILE_MEASURE_FRAMES" ([string]$ProfileMeasureFrames)
}

$args = @()
switch ($Mode) {
  "RuntimeOnly" {
    $args = @(
      "--timing-probe",
      "--timing-probe-runtime-only",
      "--timing-probe-durations", (Quote-ShValue $Durations),
      "--timing-probe-scenarios", (Quote-ShValue $Scenarios)
    )
  }
  "Live" {
    $args = @(
      "--timing-probe",
      "--timing-probe-durations", (Quote-ShValue $Durations),
      "--timing-probe-scenarios", (Quote-ShValue $Scenarios)
    )
    if ($Snapshots) {
      $args += "--timing-probe-snapshots"
    }
  }
  "AudioDrain" {
    $envParts += Env-Assignment "OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN_INTERVAL_MS" ([string]$AudioDrainIntervalMs)
    $args = @(
      "--timing-probe",
      "--timing-probe-audio-drain",
      "--timing-probe-durations", (Quote-ShValue $Durations)
    )
  }
  "DspFxLimits" {
    $envParts += Env-Assignment "OCTESSERA_PI_PROFILE_MODE" "fx-limits"
    $args = @("--profile-dsp")
  }
  "DspSoak" {
    $envParts += Env-Assignment "OCTESSERA_PI_PROFILE_MODE" "soak"
    $args = @("--profile-dsp")
  }
  "ProfileBaseline" {
    $envParts += Env-Assignment "OCTESSERA_PI_PROFILE_MODE" "baseline"
    $envParts += Env-Assignment "OCTESSERA_PI_PROFILE_SAMPLE_RATE" "44100"
    $envParts += Env-Assignment "OCTESSERA_PI_PROFILE_SCENARIO" $Scenario
    $args = @("--profile-dsp")
  }
}

$sudo = if ($Mode -eq "RuntimeOnly") { "" } else { "sudo " }
$commandLine = "$sudo" + "env " + ($envParts -join " ") + " " + (Quote-ShValue $Binary) + " " + ($args -join " ")
$shouldStopService = -not $KeepServiceRunning -and $Mode -ne "RuntimeOnly"
$measurementSupport = @'
system_evidence="$(mktemp)"
probe_output="$(mktemp)"
probe_pid=
sampler_pid=
capture_system_sample() {
  local phase="$1" mem thermal thermal_value thermal_count=0 thermal_max=0 throttled throttled_hex throttled_value current_mask
  mem="$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo || true)"
  for thermal in /sys/class/thermal/thermal_zone*/temp; do
    [ -e "$thermal" ] || continue
    thermal_count=$((thermal_count + 1))
    if [ ! -r "$thermal" ] || ! thermal_value="$(cat "$thermal" 2>/dev/null)"; then
      printf 'raspberry_system_error phase=%s reason=thermal_unreadable\n' "$phase"
      return 1
    fi
    case "$thermal_value" in ''|*[!0-9]*) printf 'raspberry_system_error phase=%s reason=thermal_malformed\n' "$phase"; return 1;; esac
    [ "$thermal_value" -le "$thermal_max" ] || thermal_max="$thermal_value"
  done
  if [ "$thermal_count" -eq 0 ]; then
    printf 'raspberry_system_error phase=%s reason=thermal_missing\n' "$phase"
    return 1
  fi
  case "$mem" in ''|*[!0-9]*) printf 'raspberry_system_error phase=%s reason=memory_malformed\n' "$phase"; return 1;; esac
  throttled="$(vcgencmd get_throttled 2>/dev/null || true)"
  case "$throttled" in throttled=0x*) ;; *) printf 'raspberry_system_error phase=%s reason=throttling_malformed\n' "$phase"; return 1;; esac
  throttled_hex="${throttled#throttled=0x}"
  case "$throttled_hex" in ''|*[!0-9a-fA-F]*) printf 'raspberry_system_error phase=%s reason=throttling_malformed\n' "$phase"; return 1;; esac
  if ! throttled_value="$(printf '%d' "0x$throttled_hex" 2>/dev/null)"; then
    printf 'raspberry_system_error phase=%s reason=throttling_malformed\n' "$phase"
    return 1
  fi
  case "$throttled_value" in ''|*[!0-9]*) printf 'raspberry_system_error phase=%s reason=throttling_malformed\n' "$phase"; return 1;; esac
  current_mask=$((throttled_value & 15))
  case "$phase" in startup|runtime) ;; *) printf 'raspberry_system_error phase=%s reason=phase_malformed\n' "$phase"; return 1;; esac
  printf 'raspberry_system_sample phase=%s thermal_max_millicelsius=%s mem_available_kb=%s throttled=%s current_throttled_mask=%s undervoltage=%s\n' "$phase" "$thermal_max" "$mem" "$throttled" "$current_mask" "$((current_mask & 1))"
  if [ "$((current_mask & 1))" -ne 0 ]; then
    printf 'raspberry_system_abort phase=%s reason=undervoltage\n' "$phase"
    return 1
  fi
}
run_measurement() {
  if ! capture_system_sample startup >> "$system_evidence" 2>&1; then
    cat "$system_evidence"
    rm -f -- "$system_evidence" "$probe_output"
    return 75
  fi
  __COMMAND_LINE__ > "$probe_output" 2>&1 &
  probe_pid=$!
  (
    while kill -0 "$probe_pid" 2>/dev/null; do
      if ! capture_system_sample runtime >> "$system_evidence" 2>&1; then
        kill -TERM "$probe_pid" 2>/dev/null || true
        break
      fi
      sleep 1
    done
  ) &
  sampler_pid=$!
  set +e
  wait "$probe_pid"
  status=$?
  kill -TERM "$sampler_pid" 2>/dev/null || true
  wait "$sampler_pid" 2>/dev/null || true
  if grep -Eq '^raspberry_system_(error|abort)' "$system_evidence"; then status=75; fi
  cat "$system_evidence"
  cat "$probe_output"
  rm -f -- "$system_evidence" "$probe_output"
  return "$status"
}
run_measurement
'@
$measurementSupport = $measurementSupport.Replace("__COMMAND_LINE__", $commandLine)

if ($shouldStopService) {
  $restoreEvidence = if ($Mode -eq "ProfileBaseline") {
    @"
initial_active=`$(sudo systemctl is-active $(Quote-ShValue $Service) 2>/dev/null || true)
initial_enabled=`$(sudo systemctl is-enabled $(Quote-ShValue $Service) 2>/dev/null || true)
sudo systemctl stop $(Quote-ShValue $Service)
set +e
$measurementSupport
status=`$?
set -e
restore_status=0
sudo systemctl start $(Quote-ShValue $Service) || restore_status=`$?
final_active=`$(sudo systemctl is-active $(Quote-ShValue $Service) 2>/dev/null || true)
final_enabled=`$(sudo systemctl is-enabled $(Quote-ShValue $Service) 2>/dev/null || true)
printf 'restore_status=%s\ninitial_active=%s\ninitial_enabled=%s\nfinal_active=%s\nfinal_enabled=%s\n' "`$restore_status" "`$initial_active" "`$initial_enabled" "`$final_active" "`$final_enabled"
if [ "`$restore_status" -ne 0 ] || [ "`$final_active" != active ] || [ "`$final_enabled" != enabled ]; then exit 70; fi
exit `$status
"@
  } else {
    @"
set -e
initial_active=`$(sudo systemctl is-active $(Quote-ShValue $Service) 2>/dev/null || true)
initial_enabled=`$(sudo systemctl is-enabled $(Quote-ShValue $Service) 2>/dev/null || true)
sudo systemctl stop $(Quote-ShValue $Service)
set +e
$measurementSupport
status=`$?
set -e
restore_status=0
sudo systemctl start $(Quote-ShValue $Service) || restore_status=`$?
final_active=`$(sudo systemctl is-active $(Quote-ShValue $Service) 2>/dev/null || true)
final_enabled=`$(sudo systemctl is-enabled $(Quote-ShValue $Service) 2>/dev/null || true)
printf 'restore_status=%s\ninitial_active=%s\ninitial_enabled=%s\nfinal_active=%s\nfinal_enabled=%s\n' "`$restore_status" "`$initial_active" "`$initial_enabled" "`$final_active" "`$final_enabled"
if [ "`$restore_status" -ne 0 ] || [ "`$final_active" != active ] || [ "`$final_enabled" != enabled ]; then exit 70; fi
exit `$status
"@
  }
  $remote = @"
$restoreEvidence
"@
} else {
  $remote = @"
set -e
$commandLine
"@
}

Invoke-PiSsh $remote
