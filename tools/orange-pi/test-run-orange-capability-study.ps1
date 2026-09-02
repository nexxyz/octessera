$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
Import-Module (Join-Path $PSScriptRoot "orange-live-benchmark-validation.psm1") -Force

function Invoke-StudyPrintOnly {
  param([hashtable]$Parameters)
  try {
    $output = @(& $scriptPath @Parameters 2>&1)
    if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) {
      throw "study runner exited with code $LASTEXITCODE"
    }
    return ($output | ForEach-Object { [string]$_ }) -join "`n"
  } catch {
    throw "Study runner PrintOnly failed: $($_.Exception.Message)"
  }
}

function Assert-Contains {
  param(
    [Parameter(Mandatory)][string]$Text,
    [Parameter(Mandatory)][string]$Value
  )
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) {
    throw "Study runner output is missing: $Value"
  }
}

function Assert-NotContains {
  param(
    [Parameter(Mandatory)][string]$Text,
    [Parameter(Mandatory)][string]$Value
  )
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -ge 0) {
    throw "Study runner output unexpectedly contains: $Value"
  }
}

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) {
    throw "Expected validation failure did not occur."
  }
}

function Assert-NoPayloadPlaceholders {
  param([Parameter(Mandatory)][string]$Text)
  if ($Text -match '__[A-Z_]+__') {
    throw "Study runner output contains an unresolved payload placeholder."
  }
}

function Assert-Ordered {
  param(
    [Parameter(Mandatory)][string]$Text,
    [Parameter(Mandatory)][string[]]$Values
  )
  $previous = -1
  foreach ($value in $Values) {
    $current = $Text.IndexOf($value, [StringComparison]::Ordinal)
    if ($current -lt 0 -or $current -le $previous) {
      throw "Study runner ordering is missing or incorrect: $value"
    }
    $previous = $current
  }
}

$passive = Invoke-StudyPrintOnly -Parameters @{ Mode = "PassiveBaseline"; PrintOnly = $true }
Assert-NoPayloadPlaceholders $passive
Assert-Contains $passive "PrintOnly: no Orange transport is invoked."
Assert-Contains $passive "octessera@192.168.0.217"
Assert-Contains $passive "with-orange-ssh.ps1"
Assert-Contains $passive "systemctl is-active"
Assert-Contains $passive "thermal_zone"
Assert-Contains $passive "scp"
Assert-NotContains $passive 'systemctl stop "$service"'
Assert-NotContains $passive "--wait --pipe"
$passiveCleanup = $passive.Substring($passive.LastIndexOf("Cleanup payload:", [StringComparison]::Ordinal))
Assert-NotContains $passiveCleanup "systemctl"
Assert-NotContains $passiveCleanup "sudo"
if ($passive -match "run-pi-timing-probes|192\.168\.0\.211|ssh -i") {
  throw "Passive PrintOnly output routed through Raspberry or direct SSH tooling."
}

$missingArtifact = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-study-test-release-missing"
foreach ($dspMode in @("Dsp64", "Dsp256")) {
  $dspRefused = $false
  try {
    Invoke-StudyPrintOnly -Parameters @{ Mode = $dspMode; Artifact = $missingArtifact; PrintOnly = $true } | Out-Null
  } catch {
    $dspRefused = $true
  }
  if (-not $dspRefused) {
    throw "$dspMode PrintOnly did not require -AllowServiceInterruption."
  }
}

$dsp64 = Invoke-StudyPrintOnly -Parameters @{ Mode = "Dsp64"; Artifact = $missingArtifact; AllowServiceInterruption = $true; PrintOnly = $true }
Assert-NoPayloadPlaceholders $dsp64
Assert-Contains $dsp64 "OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES=64"
Assert-Contains $dsp64 "--profile-dsp"
Assert-Contains $dsp64 "internal_block_frames=64"
Assert-Contains $dsp64 "--wait --pipe --collect"
Assert-Contains $dsp64 "systemd-run --quiet --unit"
Assert-Contains $dsp64 "--property=User=octessera-runtime"
Assert-Contains $dsp64 "--property=Nice=-10"
Assert-Contains $dsp64 "--property=LimitRTPRIO=70"
Assert-Contains $dsp64 "--property=ProtectKernelTunables=yes"
Assert-Contains $dsp64 "--property=ProtectKernelModules=yes"
Assert-Contains $dsp64 "--property=ProtectControlGroups=yes"
Assert-Contains $dsp64 "--property=RestrictNamespaces=yes"
Assert-Contains $dsp64 "--property=LockPersonality=yes"
Assert-Contains $dsp64 "--property=RuntimeDirectory=octessera"
Assert-Contains $dsp64 "--property=RuntimeDirectoryMode=0755"
Assert-Contains $dsp64 'systemctl stop "$service"'
Assert-Contains $dsp64 "mkdir -m 0700"
Assert-Contains $dsp64 "chmod 0710"
Assert-Contains $dsp64 "chgrp octessera-runtime"
Assert-NotContains $dsp64 "chmod 0777"
Assert-NotContains $dsp64 'chmod 0755 "$root"'
Assert-Contains $dsp64 "mode=dsp"
Assert-Contains $dsp64 "stop_sampler"
Assert-Contains $dsp64 'kill -TERM "$sampler_pid"'
Assert-Contains $dsp64 "timeout --signal=TERM --kill-after=2 10s sudo -n rm -f --"
Assert-NotContains $dsp64 'systemctl enable "'
Assert-NotContains $dsp64 'systemctl disable "'
if ($dsp64 -match "OCTESSERA_PI_PROFILE_DSP=") {
  throw "Orange DSP plan used an environment-only profile trigger."
}

$hashArtifact = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-study-test-" + [guid]::NewGuid().ToString("N"))
try {
  [IO.File]::WriteAllBytes($hashArtifact, [byte[]](1, 2, 3, 4))
  $expectedHash = (Get-FileHash -LiteralPath $hashArtifact -Algorithm SHA256).Hash.ToLowerInvariant()
  $hashedPlan = Invoke-StudyPrintOnly -Parameters @{ Mode = "Dsp64"; Artifact = $hashArtifact; AllowServiceInterruption = $true; PrintOnly = $true }
  Assert-Contains $hashedPlan "/tmp/octessera-orange-study-$expectedHash-"
} finally {
  Remove-Item -LiteralPath $hashArtifact -Force -ErrorAction SilentlyContinue
}

$dsp256 = Invoke-StudyPrintOnly -Parameters @{ Mode = "Dsp256"; Artifact = $missingArtifact; AllowServiceInterruption = $true; PrintOnly = $true }
Assert-NoPayloadPlaceholders $dsp256
Assert-Contains $dsp256 "OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES=256"
Assert-Contains $dsp256 "internal_block_frames=256"

$refused = $false
try {
  Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveCandidate"; PrintOnly = $true } | Out-Null
} catch {
  $refused = $true
}
if (-not $refused) {
  throw "LiveCandidate PrintOnly did not require -AllowServiceInterruption."
}

$live = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveCandidate"; Artifact = $missingArtifact; AllowServiceInterruption = $true; PrintOnly = $true; LiveSeconds = 30 }
$live120 = Invoke-StudyPrintOnly -Parameters @{ Mode = "LiveCandidate"; Artifact = $missingArtifact; AllowServiceInterruption = $true; PrintOnly = $true; LiveSeconds = 120 }
Assert-NoPayloadPlaceholders $live
Assert-NoPayloadPlaceholders $live120
foreach ($required in @(
    "restore_service()",
    "trap on_exit EXIT",
    "trap - EXIT HUP INT TERM",
    "trap 'exit 129' HUP",
    "service-initial-state.txt",
    "service-restored-state.txt",
    "production_health=/run/octessera/candidate-ready.json",
    "schema_version",
    "board_profile",
    "systemd_invocation_id",
    "MainPID",
    "InvocationID",
    "capture_readiness",
    "readiness_matches_unit",
    'local unit="$1"',
    'local key="$1"',
    'local marker="$2"',
    'local expected_pid="$2"',
    'local expected_invocation="$3"',
    'local marker_pid',
    'local marker_invocation',
    'local marker_evidence="$3"',
    'local properties_evidence="$4"',
    'local check_evidence="$5"',
    'local current_pid="$(unit_main_pid "$unit")"',
    'local current_invocation="$(unit_invocation_id "$unit")"',
    'local candidate_pid',
    'local candidate_invocation',
    'local stable_pid',
    'local stable_invocation',
    'local stable_deadline',
    'local stable',
    'cp -- "$marker" "$marker_evidence"',
    'wait_for_stable_readiness "$service" "$production_health"',
    'current_pid" = "$expected_pid"',
    'current_invocation" = "$expected_invocation"',
    'pinned_main_pid="$(sed -n ''s/^main_pid=//p'' "$root/candidate-readiness-properties.txt")"',
    'pinned_invocation_id="$(sed -n ''s/^invocation_id=//p'' "$root/candidate-readiness-properties.txt")"',
    "service-initial-candidate-ready.json",
    "service-initial-readiness-properties.txt",
    "service-restored-candidate-ready.json",
    "service-restored-readiness-properties.txt",
    "stable_seconds=5",
    'initial_active="$(systemctl is-active',
    'initial_enabled="$(systemctl is-enabled',
    'initial_active" != active',
    'initial_enabled" != enabled',
    "RuntimeMaxSec=60s",
    "User=octessera-runtime",
    "Nice=-10",
    "LimitRTPRIO=70",
    "ProtectKernelTunables=yes",
    "ProtectKernelModules=yes",
    "ProtectControlGroups=yes",
    "RestrictNamespaces=yes",
    "LockPersonality=yes",
    "RuntimeDirectory=octessera",
    "RuntimeDirectoryMode=0755",
    "stop_sampler()",
    'kill -TERM "$sampler_pid"',
    'wait "$sampler_pid" 2>/dev/null || true',
    "timeout --signal=TERM --kill-after=2 10s sudo -n rm -f --",
    "timeout --signal=TERM --kill-after=2 15s",
    "wait_for_stable_readiness",
    "exit_status=70",
    'systemctl show "$unit"',
    "candidate-unit-before-stop.txt",
    "candidate-unit-final.txt",
    "candidate-journal.txt",
    "ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot",
    "OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets",
    "OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples",
    "OCTESSERA_OLED_BOOT_HANDOFF=v1",
    "expected_dac=hw:CARD=octesseradac,DEV=0",
    "candidate-health.json",
    'state_file="$root/service-initial-state.txt"',
    "initial_state_valid=0",
    "grep -Fxq 'active=active'",
    "grep -Fxq 'enabled=enabled'",
    'if [ "$initial_state_valid" -eq 1 ]; then',
    "/run/octessera/candidate-health-",
    "rm -f --",
    "rm -rf --"
  )) {
  Assert-Contains $live $required
}
Assert-NotContains $live 'if ! capture_readiness "$service" "$production_health"'
Assert-NotContains $live "sudo -n cp"
Assert-NotContains $live "/tmp/octessera-orange-candidate-health-"
Assert-NotContains $live "measured_seconds"
Assert-NotContains $live120 "measured_seconds"
Assert-Contains $live 'if [ ! -r "$marker" ]; then'
Assert-Contains $live 'measurement_started_at="$(date +%s)"'
Assert-Contains $live 'measurement_deadline=$((measurement_started_at + 30))'
Assert-Contains $live 'remaining_seconds=$((measurement_deadline - now))'
Assert-Contains $live 'sleep "$sleep_seconds"'
Assert-Contains $live120 'RuntimeMaxSec=150s'
Assert-Contains $live120 'measurement_deadline=$((measurement_started_at + 120))'
foreach ($candidate in @($live, $live120)) {
  if ([regex]::Matches($candidate, 'readiness_matches_unit "\$unit" .*\$pinned_main_pid.*\$pinned_invocation_id').Count -ne 2) {
    throw "LiveCandidate did not pin both transient identities for liveness and final checks."
  }
}
$startupTimeoutSeconds = 20
$finalReadinessOverheadSeconds = 2
$stopOverheadSeconds = 5
foreach ($seconds in @(30, 120)) {
  $runtimeMaxSeconds = $startupTimeoutSeconds + $seconds + 10
  $modeledStopAt = $startupTimeoutSeconds + $seconds + $finalReadinessOverheadSeconds + $stopOverheadSeconds
  if ($modeledStopAt -ge $runtimeMaxSeconds) {
    throw "LiveCandidate modeled final readiness and stop reached RuntimeMaxSec for $seconds seconds."
  }
}
Assert-NotContains $live 'systemctl enable "'
Assert-NotContains $live 'systemctl disable "'
Assert-Contains $live "mode=live-candidate"
$liveCleanup = $live.Substring($live.LastIndexOf("Cleanup payload:", [StringComparison]::Ordinal))
Assert-Ordered $liveCleanup @(
  'systemctl stop "$unit"',
  'if [ "$initial_state_valid" -eq 1 ]; then',
  'systemctl start "$service"',
  'wait_for_stable_readiness "$service"',
  'systemctl is-enabled "$service"',
  'sudo -n rm -f -- "$health"',
  'sudo -n rm -rf -- "$root"'
)
Assert-Contains $liveCleanup "initial_state_valid=0"
Assert-Contains $liveCleanup 'if [ "$initial_state_valid" -eq 1 ]; then'
Assert-NotContains $liveCleanup 'systemctl enable "'
Assert-NotContains $liveCleanup 'systemctl disable "'
if ($live -match "flash|reboot|gpio|suspend|poweroff") {
  throw "LiveCandidate plan contains an unapproved hardware reconfiguration path."
}

if ((Get-OrangeBaselineLiveScenarioIds) -notcontains "mixed_ramp_16_48" -or (Get-OrangeLiveScenarioIds) -notcontains "mixed_ramp_32_32" -or (Get-OrangeLiveScenarioIds) -contains "mixed_ramp_16_48" -or @(Get-OrangeLiveMatrixPlan | Where-Object { $_.Scenario -eq "mixed_ramp_16_48" }).Count -ne 0) {
  throw "The current-contract scenario was not kept separate from the canonical matrix."
}
foreach ($seconds in @(30, 120, 300)) {
  $allowLongRepeat = $seconds -eq 120
  $selection = Assert-OrangeLiveBenchmarkSelection -Scenario "mixed_ramp_16_48" -OutputFrames 256 -EngineBlockFrames 128 -MeasureSeconds $seconds -AllowLongRepeat:$allowLongRepeat
  if ($selection.MeasureSeconds -ne $seconds -or $selection.InternalFrames -ne 128) { throw "Current-contract scenario selection changed for $seconds seconds." }
}
Assert-Throws { Assert-OrangeLiveBenchmarkSelection -Scenario "mixed_ramp_16_48" -OutputFrames 256 -EngineBlockFrames 32 -MeasureSeconds 300 }

$live300Parameters = @{ Mode = "LiveAudioBenchmark"; Scenario = "mixed_ramp_16_48"; OutputFrames = 256; EngineBlockFrames = 128; MeasureSeconds = 300; Artifact = $missingArtifact; AllowServiceInterruption = $true; PrintOnly = $true }
$live300 = Invoke-StudyPrintOnly -Parameters $live300Parameters
Assert-NoPayloadPlaceholders $live300
Assert-Contains $live300 "Live selection: A output=256 period=64 engine=128 internal=128 scenario=mixed_ramp_16_48 measure=300 warmup=5"
Assert-Contains $live300 "RuntimeMaxSec=455s"
Assert-Contains $live300 "with-orange-ssh.ps1"
Assert-Contains $live300 "sensor_loop"
Assert-Contains $live300 "validate_benchmark_progress"
Assert-Contains $live300 "wait_for_benchmark_terminal"
Assert-Contains $live300 'waiting_release'
Assert-Contains $live300 'sleep 1'
Assert-Contains $live300 '-le $((120 + 15))'
Assert-Contains $live300 '-le 5'
Assert-Contains $live300 '--worker-timing enabled'

$disabledTimingParameters = $live300Parameters.Clone()
$disabledTimingParameters.WorkerTimingMode = "disabled"
$disabledTiming = Invoke-StudyPrintOnly -Parameters $disabledTimingParameters
Assert-NoPayloadPlaceholders $disabledTiming
Assert-Contains $disabledTiming "worker-timing=disabled"
Assert-Contains $disabledTiming "--worker-timing disabled"
Assert-Throws {
  $invalidTimingParameters = $live300Parameters.Clone()
  $invalidTimingParameters.WorkerTimingMode = "invalid"
  Invoke-StudyPrintOnly -Parameters $invalidTimingParameters | Out-Null
}
foreach ($seconds in @(299, 3000)) {
  $rejectedParameters = $live300Parameters.Clone()
  $rejectedParameters.MeasureSeconds = $seconds
  Assert-Throws { Invoke-StudyPrintOnly -Parameters $rejectedParameters | Out-Null }
}

Write-Output "Orange capability study PrintOnly, safety, DSP-mode, and transient-unit tests passed"
