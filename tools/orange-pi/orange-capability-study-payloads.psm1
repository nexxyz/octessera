Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "orange-profile-baseline-payloads.psm1") -Force

function Quote-ShValue {
  param([Parameter(Mandatory)][string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

$readinessHelpers = @'
marker_field() {
  local key="$1"
  local marker="$2"
  sed -n "s/^[[:space:]]*\"$key\"[[:space:]]*:[[:space:]]*//p" "$marker" | sed 's/[",]//g; s/[[:space:]]//g' | head -n 1
}

unit_main_pid() {
  local unit="$1"
  sudo -n systemctl show "$unit" --property=MainPID --value
}

unit_invocation_id() {
  local unit="$1"
  sudo -n systemctl show "$unit" --property=InvocationID --value
}

positive_pid() {
  local value="$1"
  case "$value" in
    ''|0|*[!0-9]*) return 1 ;;
    *) return 0 ;;
  esac
}

validate_readiness_marker() {
  local marker="$1"
  local expected_pid="$2"
  local expected_invocation="$3"
  local marker_pid
  local marker_invocation
  if [ ! -r "$marker" ]; then
    return 1
  fi
  [ "$(marker_field schema_version "$marker")" = 1 ]
  marker_pid="$(marker_field pid "$marker")"
  marker_invocation="$(marker_field systemd_invocation_id "$marker")"
  [ "$(marker_field board_profile "$marker")" = orange-pi-zero-2w ]
  positive_pid "$marker_pid"
  [ "$marker_pid" = "$expected_pid" ]
  [ -n "$expected_invocation" ]
  [ "$marker_invocation" = "$expected_invocation" ]
}

capture_readiness() {
  local unit="$1"
  local marker="$2"
  local marker_evidence="$3"
  local properties_evidence="$4"
  local check_evidence="$5"
  local current_pid="$(unit_main_pid "$unit")"
  local current_invocation="$(unit_invocation_id "$unit")"
  printf 'unit=%s\nmain_pid=%s\ninvocation_id=%s\n' "$unit" "$current_pid" "$current_invocation" > "$properties_evidence"
  if ! sudo -n systemctl is-active --quiet "$unit" || ! positive_pid "$current_pid" || [ -z "$current_invocation" ]; then
    printf 'status=invalid-unit-state\nmain_pid=%s\ninvocation_id=%s\n' "$current_pid" "$current_invocation" > "$check_evidence"
    return 1
  fi
  if [ ! -r "$marker" ]; then
    printf 'status=missing-marker\n' > "$check_evidence"
    return 1
  fi
  if ! cp -- "$marker" "$marker_evidence"; then
    printf 'status=missing-marker\n' > "$check_evidence"
    return 1
  fi
  if ! validate_readiness_marker "$marker_evidence" "$current_pid" "$current_invocation"; then
    printf 'status=invalid-marker\nmain_pid=%s\ninvocation_id=%s\n' "$current_pid" "$current_invocation" > "$check_evidence"
    return 1
  fi
  printf 'status=valid\nmain_pid=%s\ninvocation_id=%s\n' "$current_pid" "$current_invocation" > "$check_evidence"
}

readiness_matches_unit() {
  local unit="$1"
  local marker="$2"
  local expected_pid="$3"
  local expected_invocation="$4"
  local current_pid="$(unit_main_pid "$unit")"
  local current_invocation="$(unit_invocation_id "$unit")"
  sudo -n systemctl is-active --quiet "$unit"
  positive_pid "$current_pid"
  [ "$current_pid" = "$expected_pid" ]
  [ "$current_invocation" = "$expected_invocation" ]
  validate_readiness_marker "$marker" "$expected_pid" "$expected_invocation"
}

wait_for_stable_readiness() {
  local unit="$1"
  local marker="$2"
  local marker_evidence="$3"
  local properties_evidence="$4"
  local check_evidence="$5"
  local deadline=$(( $(date +%s) + 20 ))
  local candidate_pid
  local candidate_invocation
  local stable_pid
  local stable_invocation
  local stable_deadline
  local stable
  local current_pid
  local current_invocation
  while [ "$(date +%s)" -lt "$deadline" ]; do
    candidate_pid="$(unit_main_pid "$unit")"
    candidate_invocation="$(unit_invocation_id "$unit")"
    if readiness_matches_unit "$unit" "$marker" "$candidate_pid" "$candidate_invocation"; then
      stable_pid="$candidate_pid"
      stable_invocation="$candidate_invocation"
      stable_deadline=$(( $(date +%s) + 5 ))
      stable=1
      while [ "$(date +%s)" -lt "$stable_deadline" ]; do
        sleep 1
        current_pid="$(unit_main_pid "$unit")"
        current_invocation="$(unit_invocation_id "$unit")"
        if ! sudo -n systemctl is-active --quiet "$unit" || [ "$current_pid" != "$stable_pid" ] || [ "$current_invocation" != "$stable_invocation" ] || ! validate_readiness_marker "$marker" "$stable_pid" "$stable_invocation"; then
          stable=0
          break
        fi
      done
      if [ "$stable" -eq 1 ] && capture_readiness "$unit" "$marker" "$marker_evidence" "$properties_evidence" "$check_evidence"; then
        printf 'stable_seconds=5\nmain_pid=%s\ninvocation_id=%s\n' "$stable_pid" "$stable_invocation" >> "$check_evidence"
        return 0
      fi
    fi
    sleep 1
  done
  printf 'status=readiness-timeout\n' > "$check_evidence"
  return 1
}
'@

function New-DspBody {
  param(
    [Parameter(Mandatory)][int]$BlockFrames,
    [Parameter(Mandatory)][int]$TimeoutSeconds,
    [Parameter(Mandatory)][string]$ProfileMode
  )
  $sampleCount = [Math]::Min(60, [Math]::Max(2, [Math]::Ceiling($TimeoutSeconds / 5)))
  $body = @'
unset OCTESSERA_PI_PROFILE_DSP || true
profile_status=0
(
  i=0
  while [ "$i" -lt __SAMPLE_COUNT__ ]; do
    sample_system
    i=$((i + 1))
    [ "$i" -lt __SAMPLE_COUNT__ ] && sleep 5
  done
) > "$root/system-evidence.txt" 2>&1 &
sampler_pid=$!
sudo -n systemctl stop "$service"
timeout --signal=TERM --kill-after=5 __TIMEOUT_SECONDS__s sudo -n systemd-run --quiet --unit="$unit" --service-type=exec --wait --pipe --collect --property=RuntimeMaxSec=__TIMEOUT_SECONDS__s --property=TimeoutStopSec=5s --property=User=octessera-runtime --property=Group=octessera-runtime --property=Nice=-10 --property=LimitRTPRIO=70 --property=LimitMEMLOCK=infinity --property=NoNewPrivileges=yes --property=ProtectSystem=strict --property=ProtectHome=yes --property=ProtectKernelTunables=yes --property=ProtectKernelModules=yes --property=ProtectControlGroups=yes --property=RestrictNamespaces=yes --property=LockPersonality=yes --property=PrivateTmp=no --property=RuntimeDirectory=octessera --property=RuntimeDirectoryMode=0755 --property="ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot" --setenv=OCTESSERA_AUDIO_BLOCK_FRAMES=__BLOCK_FRAMES__ --setenv=OCTESSERA_SYNTH_SLOT_WORKERS=2 --setenv=OCTESSERA_PI_PROFILE_MODE=__PROFILE_MODE__ "$binary" --profile-dsp > "$root/profile.csv" 2> "$root/profile.stderr" || profile_status=$?
stop_sampler
printf 'mode=dsp\ninternal_block_frames=__BLOCK_FRAMES__\nstatus=%s\n' "$profile_status" > "$root/study-result.txt"
exit "$profile_status"
'@
  return $body.Replace("__SAMPLE_COUNT__", [string]$sampleCount).Replace("__TIMEOUT_SECONDS__", [string]$TimeoutSeconds).Replace("__BLOCK_FRAMES__", [string]$BlockFrames).Replace("__PROFILE_MODE__", (Quote-ShValue $ProfileMode))
}

function New-LiveCandidateBody {
  param(
    [Parameter(Mandatory)][int]$LiveSeconds,
    [Parameter(Mandatory)][int]$StartupTimeoutSeconds
  )
  $sampleCount = [Math]::Min(60, [Math]::Max(2, [Math]::Ceiling(($LiveSeconds + $StartupTimeoutSeconds) / 5)))
  $runtimeMaxSeconds = $LiveSeconds + $StartupTimeoutSeconds + 10
  $body = @'
candidate_status=0
(
  i=0
  while [ "$i" -lt __SAMPLE_COUNT__ ]; do
    sample_system
    i=$((i + 1))
    [ "$i" -lt __SAMPLE_COUNT__ ] && sleep 5
  done
) > "$root/system-evidence.txt" 2>&1 &
sampler_pid=$!
sudo -n systemctl stop "$service"
printf '%s\n' 'expected_dac=hw:CARD=octesseradac,DEV=0' 'store_dir=/var/lib/octessera/presets' 'samples_dir=/var/lib/octessera/samples' > "$root/candidate-contract.txt"
launch_status=0
sudo -n systemd-run --unit="$unit" --service-type=exec --no-block --property=RuntimeMaxSec=__RUNTIME_MAX_SECONDS__s --property=TimeoutStopSec=5s --property=User=octessera-runtime --property=Group=octessera-runtime --property=Nice=-10 --property=LimitRTPRIO=70 --property=LimitMEMLOCK=infinity --property=NoNewPrivileges=yes --property=ProtectSystem=strict --property=ProtectHome=yes --property=ProtectKernelTunables=yes --property=ProtectKernelModules=yes --property=ProtectControlGroups=yes --property=RestrictNamespaces=yes --property=LockPersonality=yes --property=PrivateTmp=no --property=RuntimeDirectory=octessera --property=RuntimeDirectoryMode=0755 --property="ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot" --setenv=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w --setenv=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets --setenv=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples --setenv=OCTESSERA_OLED_BOOT_HANDOFF=v1 --setenv=OCTESSERA_CANDIDATE_HEALTH_PATH=__HEALTH__ "$binary" || launch_status=$?
deadline=$(( $(date +%s) + __STARTUP_TIMEOUT_SECONDS__ ))
ready=0
pinned_main_pid=
pinned_invocation_id=
if [ "$launch_status" -ne 0 ]; then
  candidate_status=4
else
  while [ "$ready" -eq 0 ] && [ "$(date +%s)" -lt "$deadline" ]; do
    if capture_readiness "$unit" __HEALTH__ "$root/candidate-health.json" "$root/candidate-readiness-properties.txt" "$root/candidate-readiness-check.txt"; then
      pinned_main_pid="$(sed -n 's/^main_pid=//p' "$root/candidate-readiness-properties.txt")"
      pinned_invocation_id="$(sed -n 's/^invocation_id=//p' "$root/candidate-readiness-properties.txt")"
      printf 'pinned_main_pid=%s\npinned_invocation_id=%s\n' "$pinned_main_pid" "$pinned_invocation_id" >> "$root/candidate-readiness-properties.txt"
      ready=1
      break
    fi
    sleep 1
  done
  if [ "$ready" -eq 1 ]; then
    measured_seconds=0
    while [ "$measured_seconds" -lt __LIVE_SECONDS__ ]; do
      readiness_matches_unit "$unit" __HEALTH__ "$pinned_main_pid" "$pinned_invocation_id" || {
        candidate_status=3
        break
      }
      sleep 1
      measured_seconds=$((measured_seconds + 1))
    done
    if [ "$candidate_status" -eq 0 ] && ! readiness_matches_unit "$unit" __HEALTH__ "$pinned_main_pid" "$pinned_invocation_id"; then
      candidate_status=3
    fi
  else
    candidate_status=2
  fi
fi
stop_sampler
sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=SubState --property=Result --property=ExecMainCode --property=ExecMainStatus > "$root/candidate-unit-before-stop.txt" 2>&1 || true
sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=SubState --property=Result --property=ExecMainCode --property=ExecMainStatus > "$root/candidate-unit-final.txt" 2>&1 || true
sudo -n journalctl -u "$unit" -n 80 --no-pager > "$root/candidate-journal.txt" 2>&1 || true
printf 'mode=live-candidate\nready=%s\nstatus=%s\n' "$ready" "$candidate_status" > "$root/study-result.txt"
exit "$candidate_status"
'@
  return $body.Replace("__SAMPLE_COUNT__", [string]$sampleCount).Replace("__LIVE_SECONDS__", [string]$LiveSeconds).Replace("__RUNTIME_MAX_SECONDS__", [string]$runtimeMaxSeconds).Replace("__STARTUP_TIMEOUT_SECONDS__", [string]$StartupTimeoutSeconds)
}

function New-RemoteStudyPayload {
  param(
    [Parameter(Mandatory)][ValidateSet("PassiveBaseline", "ProfileBaseline", "Dsp64", "Dsp256", "LiveCandidate")][string]$Mode,
    [Parameter(Mandatory)][string]$ProfileMode,
    [Parameter(Mandatory)][int]$TimeoutSeconds,
    [Parameter(Mandatory)][int]$LiveSeconds,
    [Parameter(Mandatory)][int]$StartupTimeoutSeconds,
    [Parameter(Mandatory)][string]$RemoteRoot,
    [Parameter(Mandatory)][string]$HealthPath,
    [Parameter(Mandatory)][string]$ArtifactHash,
    [Parameter(Mandatory)][string]$Unit,
    [Parameter(Mandatory)][string]$Service,
    [Parameter(Mandatory)][bool]$ActiveMode,
    [Parameter(Mandatory)][bool]$ArtifactRequired,
    [string]$Scenario = "",
    [int]$InternalFrames = 0,
    [int]$MeasureFrames = 0,
    [int]$Workers = 2
  )
  $body = switch ($Mode) {
    "PassiveBaseline" {
      @'
sample_system > "$root/system-evidence.txt"
printf 'mode=PassiveBaseline\n' > "$root/study-result.txt"
'@
    }
    "ProfileBaseline" { New-OrangeProfileBaselineBody $Scenario $InternalFrames $MeasureFrames $Workers $TimeoutSeconds }
    "Dsp64" { New-DspBody 64 $TimeoutSeconds $ProfileMode }
    "Dsp256" { New-DspBody 256 $TimeoutSeconds $ProfileMode }
    "LiveCandidate" { New-LiveCandidateBody $LiveSeconds $StartupTimeoutSeconds }
  }
  $payload = @'
set -eu
umask 077
root=__ROOT__
health=__HEALTH__
production_health=/run/octessera/candidate-ready.json
binary="$root/octessera-pi"
metadata="$binary.metadata.json"
service=__SERVICE__
expected_sha=__HASH__
unit=__UNIT__
initial_active=unknown
initial_enabled=unknown

__READINESS_HELPERS__

capture_service_state() {
  initial_active="$(systemctl is-active "$service" 2>/dev/null || true)"
  initial_enabled="$(systemctl is-enabled "$service" 2>/dev/null || true)"
  printf 'active=%s\nenabled=%s\n' "$initial_active" "$initial_enabled" > "$root/service-initial-state.txt"
}

sample_system() {
  local thermal
  local frequency
  printf 'unix_time=%s\n' "$(date +%s)"
  printf 'loadavg='; cut -d' ' -f1-3 /proc/loadavg
  awk '/^MemAvailable:/ { print "mem_available_kb=" $2 }' /proc/meminfo
  for thermal in /sys/class/thermal/thermal_zone*/temp; do
    test -r "$thermal" || continue
    printf 'thermal=%s\n' "$(cat "$thermal")"
  done
  for frequency in /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq; do
    test -r "$frequency" || continue
    printf 'frequency=%s:%s\n' "$frequency" "$(cat "$frequency")"
  done
  printf 'service_active=%s\n' "$(systemctl is-active "$service" 2>/dev/null || true)"
  printf 'service_enabled=%s\n' "$(systemctl is-enabled "$service" 2>/dev/null || true)"
}

stop_sampler() {
  if [ -n "${sampler_pid:-}" ]; then
    kill -TERM "$sampler_pid" 2>/dev/null || true
    wait "$sampler_pid" 2>/dev/null || true
    sampler_pid=
  fi
}

restore_service() {
  local unit_state
  local final_active
  local final_enabled
  set +e
  unit_state="$(sudo -n systemctl is-active "$unit" 2>/dev/null || true)"
  case "$unit_state" in
    active|activating|deactivating|failed)
      if ! timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "$unit" >/dev/null 2>&1; then
        restore_status=1
      fi
      ;;
  esac
  if ! timeout --signal=TERM --kill-after=2 10s sudo -n rm -f -- "$health"; then
    cleanup_status=1
  fi
  if ! timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service" >/dev/null 2>&1; then
    restore_status=1
  fi
  if ! wait_for_stable_readiness "$service" "$production_health" "$root/service-restored-candidate-ready.json" "$root/service-restored-readiness-properties.txt" "$root/service-restored-readiness-check.txt"; then
    restore_status=1
  fi
  final_active="$(systemctl is-active "$service" 2>/dev/null || true)"
  final_enabled="$(systemctl is-enabled "$service" 2>/dev/null || true)"
  if [ "$final_active" != active ] || [ "$final_enabled" != enabled ]; then
    restore_status=1
  fi
  if ! printf 'initial_active=%s\ninitial_enabled=%s\nfinal_active=%s\nfinal_enabled=%s\nrestore_status=%s\ncleanup_status=%s\n' "$initial_active" "$initial_enabled" "$final_active" "$final_enabled" "$restore_status" "$cleanup_status" > "$root/service-restored-state.txt"; then
    restore_status=1
  fi
}

on_exit() {
  local exit_status=$?
  trap - EXIT HUP INT TERM
  stop_sampler
  restore_service
  if [ "$restore_status" -ne 0 ]; then
    exit_status=70
  elif [ "$cleanup_status" -ne 0 ]; then
    exit_status=71
  fi
  exit "$exit_status"
}

test -d "$root"
capture_service_state
if [ "__ACTIVE_MODE__" = yes ]; then
  if [ "$initial_active" != active ] || [ "$initial_enabled" != enabled ]; then
    printf 'mode=refused\ninitial_active=%s\ninitial_enabled=%s\n' "$initial_active" "$initial_enabled" > "$root/study-result.txt"
    exit 64
  fi
  if ! wait_for_stable_readiness "$service" "$production_health" "$root/service-initial-candidate-ready.json" "$root/service-initial-readiness-properties.txt" "$root/service-initial-readiness-check.txt"; then
    printf 'mode=refused\nreason=initial-readiness-invalid\n' > "$root/study-result.txt"
    exit 65
  fi
  restore_status=0
  cleanup_status=0
  trap on_exit EXIT
  trap 'exit 143' INT TERM
  trap 'exit 129' HUP
fi
if [ "__VERIFY_ARTIFACT__" = yes ]; then
  chmod 0755 "$binary"
  sudo -n chgrp octessera-runtime "$root"
  chmod 0710 "$root"
  sudo -n chgrp octessera-runtime "$binary" "$metadata"
  chmod 0750 "$binary"
  chmod 0640 "$metadata"
  test -x "$binary"
  test -r "$metadata"
  remote_sha="$(sha256sum -- "$binary" | awk 'NR == 1 { print $1 }')"
  printf '%s\n' "$remote_sha" > "$root/runtime-candidate-sha256.txt"
  test "$remote_sha" = "$expected_sha"
  "$binary" --print-build-metadata > "$root/runtime-candidate-metadata.json"
  grep -q '"artifact_kind":"runtime-candidate"' "$root/runtime-candidate-metadata.json"
  grep -q '"profile":"release"' "$root/runtime-candidate-metadata.json"
fi
__BODY__
'@
  return $payload.Replace("__READINESS_HELPERS__", [string]$readinessHelpers).Replace("__BODY__", [string]$body).Replace("__ROOT__", (Quote-ShValue $RemoteRoot)).Replace("__HEALTH__", (Quote-ShValue $HealthPath)).Replace("__SERVICE__", (Quote-ShValue $Service)).Replace("__HASH__", (Quote-ShValue $ArtifactHash)).Replace("__UNIT__", (Quote-ShValue $Unit)).Replace("__ACTIVE_MODE__", $(if ($ActiveMode) { "yes" } else { "no" })).Replace("__VERIFY_ARTIFACT__", $(if ($ArtifactRequired) { "yes" } else { "no" }))
}

function New-CleanupPayload {
  param(
    [Parameter(Mandatory)][bool]$ActiveMode,
    [Parameter(Mandatory)][string]$RemoteRoot,
    [Parameter(Mandatory)][string]$HealthPath,
    [Parameter(Mandatory)][string]$Unit
  )
  if (-not $ActiveMode) {
    return "set -eu`nrm -f -- $(Quote-ShValue $HealthPath)`nrm -rf -- $(Quote-ShValue $RemoteRoot)"
  }
  $payload = @'
set -eu
unit=__UNIT__
service='octessera.service'
production_health=/run/octessera/candidate-ready.json
health=__HEALTH__
root=__ROOT__
__READINESS_HELPERS__
state_file="$root/service-initial-state.txt"
initial_state_valid=0
if [ -r "$state_file" ] && grep -Fxq 'active=active' "$state_file" && grep -Fxq 'enabled=enabled' "$state_file"; then
  initial_state_valid=1
fi
unit_state="$(sudo -n systemctl is-active "$unit" 2>/dev/null || true)"
case "$unit_state" in
  active|activating|deactivating|failed)
    timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "$unit"
    ;;
  inactive|unknown|'')
    ;;
  *)
    exit 72
    ;;
esac
if [ "$initial_state_valid" -eq 1 ]; then
  timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"
  wait_for_stable_readiness "$service" "$production_health" "$root/host-recovered-candidate-ready.json" "$root/host-recovered-readiness-properties.txt" "$root/host-recovered-readiness-check.txt"
  final_active="$(systemctl is-active "$service" 2>/dev/null || true)"
  final_enabled="$(systemctl is-enabled "$service" 2>/dev/null || true)"
  if [ "$final_active" != active ] || [ "$final_enabled" != enabled ]; then
    exit 72
  fi
fi
timeout --signal=TERM --kill-after=2 10s sudo -n rm -f -- "$health"
timeout --signal=TERM --kill-after=2 10s sudo -n rm -rf -- "$root"
'@
  return $payload.Replace("__READINESS_HELPERS__", [string]$readinessHelpers).Replace("__UNIT__", (Quote-ShValue $Unit)).Replace("__HEALTH__", (Quote-ShValue $HealthPath)).Replace("__ROOT__", (Quote-ShValue $RemoteRoot))
}

function New-PreparePayload {
  param([Parameter(Mandatory)][string]$RemoteRoot)
  return "set -eu`numask 077`ntest ! -e $(Quote-ShValue $RemoteRoot)`nmkdir -m 0700 -- $(Quote-ShValue $RemoteRoot)`nsudo -n chgrp octessera-runtime $(Quote-ShValue $RemoteRoot)`nchmod 0710 $(Quote-ShValue $RemoteRoot)"
}

function New-OrangeCapabilityStudyPayloadBundle {
  param(
    [Parameter(Mandatory)][ValidateSet("PassiveBaseline", "ProfileBaseline", "Dsp64", "Dsp256", "LiveCandidate")][string]$Mode,
    [Parameter(Mandatory)][string]$ProfileMode,
    [Parameter(Mandatory)][int]$TimeoutSeconds,
    [Parameter(Mandatory)][int]$LiveSeconds,
    [Parameter(Mandatory)][int]$StartupTimeoutSeconds,
    [Parameter(Mandatory)][string]$RemoteRoot,
    [Parameter(Mandatory)][string]$HealthPath,
    [Parameter(Mandatory)][string]$ArtifactHash,
    [Parameter(Mandatory)][string]$Unit,
    [Parameter(Mandatory)][string]$Service,
    [Parameter(Mandatory)][bool]$ActiveMode,
    [Parameter(Mandatory)][bool]$ArtifactRequired,
    [string]$Scenario = "",
    [int]$InternalFrames = 0,
    [int]$MeasureFrames = 0,
    [int]$Workers = 2
  )
  [pscustomobject]@{
    Study = New-RemoteStudyPayload $Mode $ProfileMode $TimeoutSeconds $LiveSeconds $StartupTimeoutSeconds $RemoteRoot $HealthPath $ArtifactHash $Unit $Service $ActiveMode $ArtifactRequired $Scenario $InternalFrames $MeasureFrames $Workers
    Prepare = New-PreparePayload $RemoteRoot
    Cleanup = New-CleanupPayload $ActiveMode $RemoteRoot $HealthPath $Unit
  }
}

function Get-OrangeReadinessHelpers {
  return $readinessHelpers
}

Export-ModuleMember -Function @(
  "Get-OrangeReadinessHelpers",
  "New-OrangeCapabilityStudyPayloadBundle"
)
