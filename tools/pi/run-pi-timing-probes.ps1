param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$Binary = "/usr/local/bin/octessera-pi",
  [string]$Service = "octessera.service",
  [ValidateSet("RuntimeOnly", "Live", "AudioDrain", "DspFxLimits")]
  [string]$Mode = "RuntimeOnly",
  [string]$Durations = "5s",
  [string]$Scenarios = "idle,pulses-stress",
  [int]$SynthSlotWorkers = -1,
  [int]$AudioBlockFrames = 0,
  [int]$AudioOutputBufferFrames = 0,
  [int]$AudioDrainIntervalMs = 10,
  [switch]$Snapshots,
  [switch]$KeepServiceRunning,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"

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
  $Command | ssh -i $Key -o IdentitiesOnly=yes $Target "tr -d '\r' | bash -s"
  if ($LASTEXITCODE -ne 0) {
    throw "ssh command failed with exit code $LASTEXITCODE"
  }
}

$envParts = @(
  (Env-Assignment "OCTESSERA_PI_STORE_DIR" "/home/pi/presets"),
  (Env-Assignment "OCTESSERA_PI_SAMPLES_DIR" "/home/pi/samples")
)

if ($AudioOutputBufferFrames -gt 0) {
  $envParts += Env-Assignment "OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES" ([string]$AudioOutputBufferFrames)
}

if ($AudioBlockFrames -gt 0) {
  $envParts += Env-Assignment "OCTESSERA_AUDIO_BLOCK_FRAMES" ([string]$AudioBlockFrames)
}

if ($SynthSlotWorkers -ge 0) {
  $envParts += Env-Assignment "OCTESSERA_SYNTH_SLOT_WORKERS" ([string]$SynthSlotWorkers)
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
}

$sudo = if ($Mode -eq "RuntimeOnly") { "" } else { "sudo " }
$commandLine = "$sudo" + "env " + ($envParts -join " ") + " " + (Quote-ShValue $Binary) + " " + ($args -join " ")
$shouldStopService = -not $KeepServiceRunning -and $Mode -ne "RuntimeOnly"

if ($shouldStopService) {
  $remote = @"
set -e
sudo systemctl stop $(Quote-ShValue $Service)
set +e
$commandLine
status=`$?
set -e
sudo systemctl start $(Quote-ShValue $Service)
exit `$status
"@
} else {
  $remote = @"
set -e
$commandLine
"@
}

Invoke-PiSsh $remote
