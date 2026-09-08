[CmdletBinding()]
param(
  [ValidateSet(8, 12, 16, 24, 32)][int]$Units = 16,
  [ValidateSet("Inline", "Multicore")][string]$ExecutorMode = "Inline",
  [ValidateSet(30, 120, 180, 300)][int]$MeasureSeconds = 30,
  [string]$Target = "pi@192.168.0.218",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$Artifact = "",
  [string]$Metadata = "",
  [string]$OutputDirectory = "",
  [switch]$AllowServiceInterruption,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
. (Join-Path $PSScriptRoot "board-profile.ps1")
Import-Module (Join-Path $PSScriptRoot "raspberry-live-benchmark-metadata.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "raspberry-live-benchmark-validation.psm1") -Force
$selection = Assert-RaspberryLiveBenchmarkSelection -Units $Units -ExecutorMode $ExecutorMode -MeasureSeconds $MeasureSeconds
$transport = Join-Path $PSScriptRoot "with-pi-ssh.ps1"
if ([string]::IsNullOrWhiteSpace($Artifact)) { $Artifact = Join-Path $repoRoot "target\pi-cross-diagnostics\routing-tree-benchmark\benchmark-voice-pools-128\octessera-pi" }
if ([string]::IsNullOrWhiteSpace($Metadata)) { $Metadata = "$Artifact.metadata.json" }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $repoRoot "target\raspberry-live-audio-benchmark" }
if (-not $PrintOnly -and -not $AllowServiceInterruption) { throw "Raspberry live audio benchmark requires -AllowServiceInterruption." }

function Quote-ShValue {
  param([Parameter(Mandatory)][string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Write-PayloadFile {
  param([Parameter(Mandatory)][string]$Contents, [Parameter(Mandatory)][string]$RunId, [Parameter(Mandatory)][string]$Name)
  $path = Join-Path ([IO.Path]::GetTempPath()) "octessera-raspberry-live-$RunId-$Name.sh"
  [IO.File]::WriteAllText($path, "$Contents`n", (New-Object System.Text.UTF8Encoding($false)))
  return $path
}

function New-RaspberryLivePayload {
  param([Parameter(Mandatory)][string]$RemoteRoot, [Parameter(Mandatory)][string]$RunId, [Parameter(Mandatory)][string]$ArtifactHash, [Parameter(Mandatory)][pscustomobject]$Selection)
  $body = @'
set -eu
umask 077
root=__ROOT__
binary="$root/octessera-pi"
service=octessera.service
unit=octessera-raspberry-live-__RUN_ID__.service
benchmark_root=/run/octessera/raspberry-live-__RUN_ID__
readiness="$benchmark_root/readiness.json"
progress="$benchmark_root/progress.json"
result="$benchmark_root/result.json"
release="$benchmark_root/release.json"
candidate_readiness=/run/octessera/candidate-ready.json
expected_sha=__HASH__
sensor_abort="$root/sensor-abort.txt"
sensor_series="$root/sensor-series.txt"
sampler_pid=
interruption_started=false
restore_status=0
benchmark_pid=0
benchmark_invocation=
restored_pid=0
restored_invocation=
restoration_ready=false
json_field() { sed -n "s/^[[:space:]]*\"$1\"[[:space:]]*:[[:space:]]*//p" "$2" | sed 's/[",]//g; s/^[[:space:]]*//; s/[[:space:]]*$//' | head -n 1; }
positive_number() { case "$1" in ''|0|*[!0-9]*) return 1;; *) return 0;; esac; }
nonnegative_number() { case "$1" in ''|*[!0-9]*) return 1;; *) return 0;; esac; }
unit_pid() { sudo -n systemctl show "$unit" --property=MainPID --value 2>/dev/null || printf 0; }
unit_invocation() { sudo -n systemctl show "$unit" --property=InvocationID --value 2>/dev/null || true; }
stop_unit() { local state; state="$(sudo -n systemctl is-active "$unit" 2>/dev/null || true)"; case "$state" in active|activating|deactivating|failed) timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true;; esac; }
capture_system_sample() {
  local phase="$1" mem thermal value count=0 maximum=0 throttled hex numeric mask
  mem="$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo || true)"
  case "$mem" in ''|*[!0-9]*) printf 'raspberry_system_error phase=%s reason=memory_malformed\n' "$phase"; return 1;; esac
  for thermal in /sys/class/thermal/thermal_zone*/temp; do
    [ -e "$thermal" ] || continue
    count=$((count + 1)); value="$(cat "$thermal" 2>/dev/null || true)"
    case "$value" in ''|*[!0-9]*) printf 'raspberry_system_error phase=%s reason=thermal_malformed\n' "$phase"; return 1;; esac
    [ "$value" -le "$maximum" ] || maximum="$value"
  done
  [ "$count" -gt 0 ] || { printf 'raspberry_system_error phase=%s reason=thermal_missing\n' "$phase"; return 1; }
  throttled="$(vcgencmd get_throttled 2>/dev/null || true)"
  case "$throttled" in throttled=0x*) ;; *) printf 'raspberry_system_error phase=%s reason=throttling_malformed\n' "$phase"; return 1;; esac
  hex="${throttled#throttled=0x}"
  case "$hex" in ''|*[!0-9a-fA-F]*) printf 'raspberry_system_error phase=%s reason=throttling_malformed\n' "$phase"; return 1;; esac
  numeric="$(printf '%d' "0x$hex" 2>/dev/null || true)"; nonnegative_number "$numeric" || return 1
  mask=$((numeric & 15))
  printf 'raspberry_system_sample phase=%s thermal_max_millicelsius=%s mem_available_kb=%s throttled=%s current_throttled_mask=%s undervoltage=%s\n' "$phase" "$maximum" "$mem" "0x$hex" "$mask" "$((mask & 1))"
  if [ "$((mask & 1))" -ne 0 ]; then printf 'raspberry_system_abort phase=%s reason=undervoltage\n' "$phase"; return 1; fi
}
sensor_loop() { while [ ! -e "$sensor_abort" ]; do capture_system_sample runtime >> "$sensor_series" 2>&1 || { printf 'reason=runtime-sensor-gate\n' > "$sensor_abort"; break; }; sleep 1; done; }
wait_for_ready() {
  local deadline=$(( $(date +%s) + 20 )) pid invocation
  while [ "$(date +%s)" -lt "$deadline" ]; do
    pid="$(unit_pid)"; invocation="$(unit_invocation)"
    if sudo -n systemctl is-active --quiet "$unit" && positive_number "$pid" && [ -n "$invocation" ] && [ -r "$readiness" ] && [ "$(json_field kind "$readiness")" = raspberry_audio_benchmark_readiness ] && [ "$(json_field status "$readiness")" = ready ] && [ "$(json_field pid "$readiness")" = "$pid" ] && [ "$(json_field systemd_invocation_id "$readiness")" = "$invocation" ] && [ "$(json_field artifact_sha256 "$readiness")" = "$expected_sha" ]; then
      benchmark_pid="$pid"; benchmark_invocation="$invocation"; printf 'unit=%s\nmain_pid=%s\ninvocation_id=%s\n' "$unit" "$benchmark_pid" "$benchmark_invocation" > "$root/benchmark-identity.txt"; cp -- "$readiness" "$root/benchmark-readiness.json"; return 0
    fi
    sleep 1
  done
  return 1
}
capture_alsa_release() {
  local path=/proc/asound/sndrpihifiberry/pcm0p/sub0/hw_params buffer period pid invocation
  [ -r "$path" ] || return 1
  pid="$benchmark_pid"; invocation="$benchmark_invocation"
  [ "$(unit_pid)" = "$pid" ] && [ "$(unit_invocation)" = "$invocation" ] || return 1
  buffer="$(sudo -n cat -- "$path" | sed -n 's/^[[:space:]]*buffer_size[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -n 1)"
  period="$(sudo -n cat -- "$path" | sed -n 's/^[[:space:]]*period_size[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -n 1)"
  [ "$buffer" = 256 ] && [ "$period" = 64 ] || return 1
  sudo -n install -o pi -g pi -m 0640 -- "$path" "$root/alsa-hw-params.txt"
  printf 'path=%s\nbuffer_size=%s\nperiod_size=%s\n' "$path" "$buffer" "$period" > "$root/alsa-geometry.txt"
  printf '{"schema_version":2,"kind":"raspberry_audio_benchmark_release","status":"released","board_profile":"raspberry-pi-zero-2w","pid":%s,"systemd_invocation_id":"%s","artifact_sha256":"%s","scenario":"__SCENARIO__","expected_alsa_buffer_frames":256,"observed_alsa_buffer_frames":%s,"expected_alsa_period_frames":64,"observed_alsa_period_frames":%s}\n' "$pid" "$invocation" "$expected_sha" "$buffer" "$period" > "$release"
}
wait_for_terminal() {
  local deadline=$(( $(date +%s) + __RUNTIME_MAX__ )) pid now
  while [ "$(date +%s)" -lt "$deadline" ]; do
    [ -e "$sensor_abort" ] && return 75
    if [ -r "$result" ] && ! sudo -n systemctl is-active --quiet "$unit"; then
      cp -- "$result" "$root/benchmark-result.json"; [ -r "$progress" ] && cp -- "$progress" "$root/benchmark-progress.json"
      [ "$(json_field status "$result")" = pass ] && return 0 || return 20
    fi
    pid="$(unit_pid)"
    [ "$pid" = 0 ] || [ "$pid" = "$benchmark_pid" ] || return 66
    [ "$(unit_invocation)" = "$benchmark_invocation" ] || return 66
    [ -r "$progress" ] || return 66
    now="$(date +%s)"; [ $((now - $(stat -c %Y "$progress" 2>/dev/null || printf 0))) -le 10 ] || return 66
    sleep 1
  done
  return 66
}
reset_transient_unit() {
  local load_state active_state
  load_state="$(sudo -n systemctl show "$unit" --no-pager --property=LoadState --value 2>/dev/null || true)"
  active_state="$(sudo -n systemctl show "$unit" --no-pager --property=ActiveState --value 2>/dev/null || true)"
  if [ "$load_state" = loaded ] && [ "$active_state" = failed ]; then
    sudo -n systemctl reset-failed "$unit" >/dev/null 2>&1 || true
  fi
}
restore_service() {
  local active= enabled= deadline candidate_copy="$root/candidate-ready.json"
  stop_unit; sudo -n rm -f -- "$candidate_readiness" "$readiness" "$progress" "$result" "$release" || restore_status=1
  timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service" >/dev/null 2>&1 || restore_status=1
  deadline=$(( $(date +%s) + 20 )); while [ "$(date +%s)" -lt "$deadline" ]; do active="$(sudo -n systemctl is-active "$service" 2>/dev/null || true)"; enabled="$(sudo -n systemctl is-enabled "$service" 2>/dev/null || true)"; restored_pid="$(sudo -n systemctl show "$service" --property=MainPID --value 2>/dev/null || printf 0)"; restored_invocation="$(sudo -n systemctl show "$service" --property=InvocationID --value 2>/dev/null || true)"; if [ "$active" = active ] && [ "$enabled" = enabled ] && positive_number "$restored_pid" && [ -n "$restored_invocation" ] && sudo -n test -r "$candidate_readiness" && sudo -n install -o pi -g pi -m 0640 -- "$candidate_readiness" "$candidate_copy" && [ "$(json_field kind "$candidate_copy")" = octessera_candidate_readiness ] && [ "$(json_field status "$candidate_copy")" = ready ] && [ "$(json_field pid "$candidate_copy")" = "$restored_pid" ] && [ "$(json_field systemd_invocation_id "$candidate_copy")" = "$restored_invocation" ] && positive_number "$(json_field ready_at_unix_ms "$candidate_copy")"; then restoration_ready=true; break; fi; sleep 1; done
  [ "$restoration_ready" = true ] || restore_status=1
  printf 'initial_active=%s\ninitial_enabled=%s\nfinal_active=%s\nfinal_enabled=%s\nfinal_pid=%s\nfinal_invocation_id=%s\nrestore_status=%s\n' "$initial_active" "$initial_enabled" "$active" "$enabled" "$restored_pid" "$restored_invocation" "$restore_status" > "$root/service-restored-state.txt"
}
on_exit() {
  local status=$?
  set +e; trap - EXIT HUP INT TERM
  [ -n "${sampler_pid:-}" ] && kill -TERM "$sampler_pid" 2>/dev/null || true; [ -n "${sampler_pid:-}" ] && wait "$sampler_pid" 2>/dev/null || true
  stop_unit; sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=SubState --property=Result --property=ExecMainCode --property=ExecMainStatus --property=MainPID --property=InvocationID > "$root/unit-final.txt" 2>&1 || true; sudo -n journalctl -u "$unit" -n 200 --no-pager > "$root/unit-journal.txt" 2>&1 || true
  [ -r "$readiness" ] && cp -- "$readiness" "$root/benchmark-readiness-final.json"; [ -r "$progress" ] && cp -- "$progress" "$root/benchmark-progress-final.json"; [ -r "$result" ] && cp -- "$result" "$root/benchmark-result-final.json"; [ -r "$release" ] && cp -- "$release" "$root/benchmark-release.json"
  local class=infrastructure_failure retained_result="$root/benchmark-result.json"; [ "$status" -eq 20 ] && [ -r "$retained_result" ] && [ "$(json_field status "$retained_result")" = fail ] && class=measured_failure; [ "$status" -eq 0 ] && [ -r "$retained_result" ] && [ "$(json_field status "$retained_result")" = pass ] && class=pass; [ -e "$sensor_abort" ] && class=safety_failure
  reset_transient_unit
  if [ "$interruption_started" = true ]; then restore_service; else printf 'initial_active=%s\ninitial_enabled=%s\nfinal_active=%s\nfinal_enabled=%s\nrestore_status=0\n' "$initial_active" "$initial_enabled" "$initial_active" "$initial_enabled" > "$root/service-restored-state.txt"; fi
  [ "$restore_status" -ne 0 ] && class=restoration_failure
  printf 'mode=LiveAudioBenchmark\nstatus_class=%s\nstatus=%s\ninterruption_started=%s\nrestore_status=%s\nsensor_abort=%s\n' "$class" "$status" "$interruption_started" "$restore_status" "$([ -e "$sensor_abort" ] && printf true || printf false)" > "$root/study-result.txt"
  [ "$restore_status" -eq 0 ] || status=70; exit "$status"
}
if ! sudo -n -v >/dev/null 2>&1; then
  printf 'mode=LiveAudioBenchmark\nstatus_class=infrastructure_failure\ninterruption_started=false\nreason=operator-sudo-authorization-unavailable\n' > "$root/study-result.txt"
  exit 64
fi
initial_active="$(sudo -n systemctl is-active "$service" 2>/dev/null || true)"; initial_enabled="$(sudo -n systemctl is-enabled "$service" 2>/dev/null || true)"
mkdir -p -- "$root"; printf 'active=%s\nenabled=%s\n' "$initial_active" "$initial_enabled" > "$root/service-initial-state.txt"
[ "$initial_active" = active ] && [ "$initial_enabled" = enabled ] || { printf 'mode=LiveAudioBenchmark\nstatus_class=infrastructure_failure\ninterruption_started=false\nreason=production-service-not-active-enabled\n' > "$root/study-result.txt"; exit 64; }
trap on_exit EXIT; trap 'exit 143' INT TERM
capture_system_sample startup > "$sensor_series" 2>&1 || { printf 'reason=startup-sensor-gate\n' > "$sensor_abort"; exit 75; }
chmod 0750 -- "$binary"; test -x "$binary"; test -r "$binary.metadata.json"; remote_sha="$(sha256sum -- "$binary" | awk 'NR == 1 {print $1}')"; printf '%s\n' "$remote_sha" > "$root/runtime-binary-sha256.txt"; [ "$remote_sha" = "$expected_sha" ]
"$binary" --print-build-metadata > "$root/runtime-metadata.json"; grep -q '"artifact_kind":"diagnostic-only"' "$root/runtime-metadata.json"; grep -q '"cargo_feature":"hardware-raspberry-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128"' "$root/runtime-metadata.json"
interruption_started=true
sudo -n systemctl stop "$service"
sudo -n install -d -o pi -g pi -m 0755 /run/octessera
sudo -n install -d -o pi -g pi -m 0750 "$benchmark_root"
sudo -n systemd-run --unit="$unit" --service-type=exec --no-block --property=RuntimeMaxSec=__RUNTIME_MAX__s --property=TimeoutStopSec=5s --property=User=pi --property=Group=pi --property=Nice=-10 --property=LimitRTPRIO=70 --property=LimitMEMLOCK=infinity --property=NoNewPrivileges=yes --property=ProtectSystem=strict --property=ProtectHome=read-only --property=PrivateTmp=no --property="ReadWritePaths=/run/octessera /tmp" --setenv=OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w --setenv=OCTESSERA_PI_STORE_DIR=/home/pi/presets --setenv=OCTESSERA_OLED_BOOT_HANDOFF=v1 "$binary" --benchmark-raspberry-audio --executor __EXECUTOR__ --scenario __SCENARIO__ --output-frames 256 --engine-block-frames 128 --worker-timing __WORKER_TIMING__ --warmup-seconds 5 --measure-seconds __MEASURE_SECONDS__ --readiness "$readiness" --progress "$progress" --result "$result" --release-gate "$release" --release-timeout-seconds 120 --artifact-sha256 "$expected_sha"
sensor_loop >> "$sensor_series" 2>&1 & sampler_pid=$!; wait_for_ready; capture_alsa_release; wait_for_terminal; exit $?
'@
  return $body.Replace("__ROOT__", (Quote-ShValue $RemoteRoot)).Replace("__RUN_ID__", $RunId).Replace("__HASH__", (Quote-ShValue $ArtifactHash)).Replace("__RUNTIME_MAX__", [string]($Selection.MeasureSeconds + 180)).Replace("__SCENARIO__", $Selection.Scenario).Replace("__EXECUTOR__", $Selection.NativeExecutorMode).Replace("__WORKER_TIMING__", $Selection.WorkerTimingMode).Replace("__MEASURE_SECONDS__", [string]$Selection.MeasureSeconds)
}

if ($PrintOnly) {
  $runLabel = if ($MeasureSeconds -eq 30) { "30-second screen" } elseif ($MeasureSeconds -eq 120) { "120-second repeat" } else { "$MeasureSeconds-second extended diagnostic" }
  Write-Output "Raspberry live audio benchmark PrintOnly: no transport is invoked."
  Write-Output "Selection: U$Units scenario=$($selection.Scenario) executor=$($selection.ExecutorMode) output=256 period=64 internal=128 lookahead=$($selection.LookaheadFrames) measure=$MeasureSeconds label=$runLabel"
  Write-Output "Artifact: $Artifact"
  Write-Output "Metadata: $Metadata"
  exit 0
}

$sourceCommit = (& git -C $repoRoot rev-parse HEAD 2>$null | Out-String).Trim().ToLowerInvariant()
$artifactHash = Get-RaspberryLiveBenchmarkBinarySha256 $Artifact
Assert-RaspberryLiveBenchmarkMetadata -Metadata (Read-RaspberryLiveBenchmarkMetadata $Metadata) -SourceCommit $sourceCommit -BinaryPath $Artifact | Out-Null
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$runId = [guid]::NewGuid().ToString("N")
$remoteRoot = "/tmp/octessera-raspberry-live-$artifactHash-$runId"
$localRunDirectory = Join-Path $OutputDirectory "raspberry-live-$runId"
New-Item -ItemType Directory -Force -Path $localRunDirectory | Out-Null
$preparePath = Write-PayloadFile -Contents ("set -eu`ntest ! -e " + (Quote-ShValue $remoteRoot) + "`nmkdir -m 0700 -- " + (Quote-ShValue $remoteRoot)) -RunId $runId -Name "prepare"
$payloadPath = Write-PayloadFile -Contents (New-RaspberryLivePayload -RemoteRoot $remoteRoot -RunId $runId -ArtifactHash $artifactHash -Selection $selection) -RunId $runId -Name "study"
$cleanupContents = @"
set -eu
set +e
unit=octessera-raspberry-live-$runId.service
unit_status=0
unit_load_state=`$(sudo -n systemctl show "`$unit" --no-pager --property=LoadState --value 2>/dev/null || true)
unit_active_state=`$(sudo -n systemctl show "`$unit" --no-pager --property=ActiveState --value 2>/dev/null || true)
if [ "`$unit_active_state" = active ] || [ "`$unit_active_state" = activating ] || [ "`$unit_active_state" = deactivating ] || [ "`$unit_active_state" = failed ]; then
  timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "`$unit" >/dev/null 2>&1 || unit_status=1
fi
unit_load_state=`$(sudo -n systemctl show "`$unit" --no-pager --property=LoadState --value 2>/dev/null || true)
unit_active_state=`$(sudo -n systemctl show "`$unit" --no-pager --property=ActiveState --value 2>/dev/null || true)
if [ "`$unit_load_state" = loaded ] && [ "`$unit_active_state" = failed ]; then
  timeout --signal=TERM --kill-after=2 10s sudo -n systemctl reset-failed "`$unit" >/dev/null 2>&1 || unit_status=1
fi
sudo -n rm -rf -- /run/octessera/raspberry-live-$runId
privileged_status=`$?
rm -rf -- $(Quote-ShValue $remoteRoot)
unprivileged_status=`$?
[ "`$unit_status" -eq 0 ] && [ "`$privileged_status" -eq 0 ] && [ "`$unprivileged_status" -eq 0 ]
"@
$cleanupPath = Write-PayloadFile -Contents $cleanupContents -RunId $runId -Name "cleanup"
$studyFailure = $null; $retrievalFailure = $null; $cleanupFailure = $null
try {
  & $transport "ssh-payload" -Target $Target -Key $Key $preparePath; if ($LASTEXITCODE -ne 0) { throw "Raspberry live benchmark prepare failed with exit code $LASTEXITCODE." }
  & $transport "scp" -Target $Target -Key $Key $Artifact "$Target`:$remoteRoot/octessera-pi"; if ($LASTEXITCODE -ne 0) { throw "Raspberry live benchmark artifact transfer failed with exit code $LASTEXITCODE." }
  & $transport "scp" -Target $Target -Key $Key $Metadata "$Target`:$remoteRoot/octessera-pi.metadata.json"; if ($LASTEXITCODE -ne 0) { throw "Raspberry live benchmark metadata transfer failed with exit code $LASTEXITCODE." }
  & $transport "ssh-payload" -Target $Target -Key $Key $payloadPath; if ($LASTEXITCODE -ne 0) { $studyFailure = "Raspberry live benchmark remote study exited with code $LASTEXITCODE." }
} catch { $studyFailure = $_ } finally {
  try { & $transport "scp" -Target $Target -Key $Key "-r" "$Target`:$remoteRoot/." $localRunDirectory; if ($LASTEXITCODE -ne 0) { throw "Raspberry live benchmark evidence retrieval failed with exit code $LASTEXITCODE." } } catch { $retrievalFailure = $_ }
  try { & $transport "ssh-payload" -Target $Target -Key $Key $cleanupPath; if ($LASTEXITCODE -ne 0) { throw "Raspberry live benchmark cleanup failed with exit code $LASTEXITCODE." } } catch { $cleanupFailure = $_ }
  foreach ($path in @($preparePath, $payloadPath, $cleanupPath)) { Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue }
}
$hostEvidence = Get-RaspberryLiveHostEvidence -EvidenceDirectory $localRunDirectory -Selection $selection -ArtifactHash $artifactHash
if ($null -ne $retrievalFailure -and $hostEvidence.StatusClass -cne "restoration_failure") { $hostEvidence.StatusClass = "infrastructure_failure"; $hostEvidence.Reason = $retrievalFailure.Exception.Message }
if ($null -ne $cleanupFailure -and $hostEvidence.StatusClass -ceq "pass") { $hostEvidence.StatusClass = "infrastructure_failure"; $hostEvidence.Reason = "Raspberry live benchmark cleanup failed: $cleanupFailure" }
$hostEvidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $localRunDirectory "host-evidence.json") -Encoding UTF8
Write-Output "Evidence directory: $localRunDirectory"
if ($hostEvidence.StatusClass -ne "pass") { throw "Raspberry live benchmark status class was $($hostEvidence.StatusClass): $($hostEvidence.Reason)" }
$runLabel = if ($MeasureSeconds -eq 30) { "30-second screen" } elseif ($MeasureSeconds -eq 120) { "120-second repeat" } else { "$MeasureSeconds-second extended diagnostic" }
Write-Output "Raspberry live audio benchmark passed: U$Units $ExecutorMode ($runLabel)"
