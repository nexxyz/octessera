Set-StrictMode -Version Latest

$readinessPayloadModule = Join-Path $PSScriptRoot "orange-readiness-payloads.psm1"
Import-Module $readinessPayloadModule -Force

function Quote-LiveShValue {
  param([Parameter(Mandatory)][string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

function New-OrangeLiveBenchmarkPayloadBundle {
  param(
    [Parameter(Mandatory)][pscustomobject]$Selection,
    [Parameter(Mandatory)][string]$RemoteRoot,
    [Parameter(Mandatory)][string]$BenchmarkRoot,
    [Parameter(Mandatory)][string]$HealthPath,
    [Parameter(Mandatory)][string]$ArtifactHash,
    [Parameter(Mandatory)][string]$Unit,
    [Parameter(Mandatory)][string]$Service,
    [Parameter(Mandatory)][int]$StartupTimeoutSeconds,
    [Parameter(Mandatory)][int]$ReleaseTimeoutSeconds,
    [Parameter(Mandatory)][int]$RuntimeMaxSeconds,
    [Parameter(Mandatory)][ValidateSet("enabled", "disabled")][string]$WorkerTimingMode,
    [Parameter(Mandatory)][ValidateSet("inline", "persistent_two_workers", "routing_tree_persistent")][string]$ExecutorMode,
    [Parameter(Mandatory)][ValidateSet("runtime-candidate", "diagnostic-only")][string]$ExpectedArtifactKind,
    [Parameter(Mandatory)][ValidateSet("hardware-orange-pi-zero-2w", "hardware-orange-pi-zero-2w benchmark-voice-pools-128", "hardware-orange-pi-zero-2w benchmark-voice-pools-256", "hardware-orange-pi-zero-2w routing-tree-benchmark", "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128", "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-256")][string]$ExpectedCargoFeature
  )
  if (@("enabled", "disabled") -cnotcontains $WorkerTimingMode) { throw "WorkerTimingMode must be exactly enabled or disabled." }
  if (@("inline", "persistent_two_workers", "routing_tree_persistent") -cnotcontains $ExecutorMode) { throw "ExecutorMode must be exactly inline, persistent_two_workers, or routing_tree_persistent." }
  if ($Selection.ExecutorMode -cne $ExecutorMode -or $Selection.WorkerTimingMode -cne $WorkerTimingMode) { throw "Live payload executor and worker timing do not match the selected contract." }
  $expectedLookahead = if ($ExecutorMode -eq "routing_tree_persistent") { $Selection.EngineBlockFrames } else { 0 }
  if ($Selection.LookaheadFrames -ne $expectedLookahead -or $Selection.EffectiveOutputLatencyFrames -ne ($Selection.OutputFrames + $Selection.LookaheadFrames)) { throw "Live payload selection geometry is inconsistent with the selected executor." }
  $isCapacityDiagnostic = $null -ne $Selection.PSObject.Properties["IsCapacityDiagnostic"] -and [bool]$Selection.IsCapacityDiagnostic
  $isRoutingExecutor = $ExecutorMode -ceq "routing_tree_persistent"
  if ($isCapacityDiagnostic -and ($ExpectedArtifactKind -cne "diagnostic-only" -or ($isRoutingExecutor -and $ExpectedCargoFeature -cnotmatch '^hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-(128|256)$') -or (-not $isRoutingExecutor -and $ExpectedCargoFeature -cnotmatch '^hardware-orange-pi-zero-2w benchmark-voice-pools-(128|256)$'))) { throw "Dynamic capacity payload identity must be diagnostic-only with an exact benchmark pool feature." }
  if (-not $isCapacityDiagnostic -and (($isRoutingExecutor -and ($ExpectedArtifactKind -cne "diagnostic-only" -or $ExpectedCargoFeature -cne "hardware-orange-pi-zero-2w routing-tree-benchmark")) -or (-not $isRoutingExecutor -and ($ExpectedArtifactKind -cne "runtime-candidate" -or $ExpectedCargoFeature -cne "hardware-orange-pi-zero-2w")))) { throw "Fixed live payload identity does not match the selected executor." }

  $readinessHelpers = Get-OrangeReadinessHelpers
  $body = @'
set -eu
umask 077
root=__ROOT__
binary="$root/octessera-pi"
metadata="$binary.metadata.json"
service=__SERVICE__
unit=__UNIT__
expected_sha=__HASH__
benchmark_root=__BENCHMARK_ROOT__
health=__HEALTH__
production_health=/run/octessera/candidate-ready.json
readiness="$benchmark_root/readiness.json"
progress="$benchmark_root/progress.json"
result="$benchmark_root/result.json"
release="$benchmark_root/release.json"
sensor_abort="$root/sensor-abort.txt"
sensor_series="$root/sensor-series.txt"
sampler_pid=
benchmark_pid=
benchmark_invocation=
low_memory_samples=0
study_status=0
study_class=infrastructure_failure
interruption_started=false
restore_status=0
cleanup_status=0

json_field() {
  local key="$1" marker="$2"
  sed -n "s/^[[:space:]]*\"$key\"[[:space:]]*:[[:space:]]*//p" "$marker" | sed 's/[",]//g; s/^[[:space:]]*//; s/[[:space:]]*$//' | head -n 1
}
positive_number() { case "$1" in ''|0|*[!0-9]*) return 1;; *) return 0;; esac; }
nonnegative_number() { case "$1" in ''|*[!0-9]*) return 1;; *) return 0;; esac; }
copy_evidence() {
  local source="$1" destination="$2"
  if [ -r "$source" ]; then
    sudo -n cp -- "$source" "$destination"
    sudo -n chown octessera:octessera-runtime -- "$destination"
    sudo -n chmod 0640 -- "$destination"
  fi
}
capture_service_state() {
  initial_active="$(systemctl is-active "$service" 2>/dev/null || true)"
  initial_enabled="$(systemctl is-enabled "$service" 2>/dev/null || true)"
  printf 'active=%s\nenabled=%s\n' "$initial_active" "$initial_enabled" > "$root/service-initial-state.txt"
}
capture_sensor_sample() {
  local phase="$1" now mem thermal_zone thermal thermal_value thermal_type frequency cooling_device cooling_type cur_state max_state
  local thermal_count=0 max_thermal=0
  local cooling_count=0
  now="$(date +%s)"
  mem="$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo || true)"
  if ! positive_number "$mem"; then
    printf 'reason=memory-unreadable\ntime=%s\n' "$now" > "$sensor_abort"
    [ "$phase" = startup ] || sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
    return 1
  fi
  printf 'sample=memory phase=%s time=%s mem_available_kb=%s loadavg=%s\n' "$phase" "$now" "$mem" "$(cut -d' ' -f1-3 /proc/loadavg)" >> "$sensor_series"
  for thermal_zone in /sys/class/thermal/thermal_zone*; do
    [ -e "$thermal_zone" ] || continue
    thermal="$thermal_zone/temp"
    thermal_count=$((thermal_count + 1))
    if [ ! -r "$thermal" ] || ! thermal_value="$(cat "$thermal" 2>/dev/null)" || ! positive_number "$thermal_value"; then
      printf 'reason=thermal-unreadable\nphase=%s\ntime=%s\npath=%s\n' "$phase" "$now" "$thermal" > "$sensor_abort"
      [ "$phase" = startup ] || sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
      return 1
    fi
    thermal_type=unknown
    [ -r "$thermal_zone/type" ] && thermal_type="$(cat "$thermal_zone/type" 2>/dev/null || printf unknown)"
    printf 'sample=thermal phase=%s time=%s path=%s type=%s millicelsius=%s\n' "$phase" "$now" "$thermal" "$thermal_type" "$thermal_value" >> "$sensor_series"
    [ "$thermal_value" -le "$max_thermal" ] || max_thermal="$thermal_value"
  done
  if [ "$thermal_count" -eq 0 ]; then
    printf 'reason=thermal-missing\ntime=%s\n' "$now" > "$sensor_abort"
    [ "$phase" = startup ] || sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
    return 1
  fi
  for frequency in /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq; do
    [ -e "$frequency" ] || continue
    printf 'sample=frequency phase=%s time=%s path=%s khz=%s\n' "$phase" "$now" "$frequency" "$(cat "$frequency" 2>/dev/null || printf unreadable)" >> "$sensor_series"
  done
  for cooling_device in /sys/class/thermal/cooling_device*; do
    [ -e "$cooling_device" ] || continue
    cooling_count=$((cooling_count + 1))
    cooling_type="$(cat "$cooling_device/type" 2>/dev/null || true)"
    cooling_type="$(printf '%s' "$cooling_type" | tr -c 'A-Za-z0-9_.:-' '_')"
    cur_state="$(cat "$cooling_device/cur_state" 2>/dev/null || true)"
    max_state="$(cat "$cooling_device/max_state" 2>/dev/null || true)"
    if [ -z "$cooling_type" ] || ! nonnegative_number "$cur_state" || ! nonnegative_number "$max_state" || [ "$cur_state" -gt "$max_state" ]; then
      printf 'sample=cooling phase=%s time=%s observed=false reason=cooling-device-unreadable path=%s type=%s cur_state=%s max_state=%s\n' "$phase" "$now" "$cooling_device" "$cooling_type" "$cur_state" "$max_state" >> "$sensor_series"
      printf 'reason=cooling-device-unreadable\nphase=%s\ntime=%s\npath=%s\n' "$phase" "$now" "$cooling_device" > "$sensor_abort"
      [ "$phase" = startup ] || sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
      return 1
    fi
    printf 'sample=cooling phase=%s time=%s path=%s type=%s cur_state=%s max_state=%s observed=true\n' "$phase" "$now" "$cooling_device" "$cooling_type" "$cur_state" "$max_state" >> "$sensor_series"
  done
  if [ "$cooling_count" -eq 0 ]; then
    printf 'sample=cooling phase=%s time=%s observed=false reason=cooling-devices-unobserved\n' "$phase" "$now" >> "$sensor_series"
  fi
  if [ "$phase" = startup ]; then
    if [ "$mem" -lt 524288 ]; then
      printf 'reason=startup-safety-limit\nphase=%s\ntime=%s\nmax_millicelsius=%s\nmem_available_kb=%s\n' "$phase" "$now" "$max_thermal" "$mem" > "$sensor_abort"
      return 1
    fi
  else
    if [ "$mem" -lt 262144 ]; then low_memory_samples=$((low_memory_samples + 1)); else low_memory_samples=0; fi
    if [ "$low_memory_samples" -ge 2 ]; then
      printf 'reason=runtime-memory-abort\ntime=%s\nmem_available_kb=%s\nconsecutive_samples=%s\n' "$now" "$mem" "$low_memory_samples" > "$sensor_abort"
      sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
      return 1
    fi
  fi
}
sensor_loop() { while [ ! -e "$sensor_abort" ]; do capture_sensor_sample runtime || true; [ -e "$sensor_abort" ] && break; sleep 1; done; }
stop_sampler() { if [ -n "${sampler_pid:-}" ]; then kill -TERM "$sampler_pid" 2>/dev/null || true; wait "$sampler_pid" 2>/dev/null || true; sampler_pid=; fi; }
stop_benchmark_unit() {
  local state
  state="$(sudo -n systemctl is-active "$unit" 2>/dev/null || true)"
  case "$state" in
    active|activating|deactivating|failed)
      if [ -e "$result" ] && [ ! -e "$root/unit-stop-evidence.txt" ]; then
        sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=Result --property=ExecMainCode --property=ExecMainStatus > "$root/unit-stop-before.txt" 2>&1 || true
        timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "$unit" >/dev/null 2>&1 || study_status=66
        sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=Result --property=ExecMainCode --property=ExecMainStatus > "$root/unit-stop-after.txt" 2>&1 || true
        printf 'unit=%s\nexplicit_stop_after_result=true\nresult_present=true\n' "$unit" > "$root/unit-stop-evidence.txt"
      else
        timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "$unit" >/dev/null 2>&1 || study_status=66
      fi
      ;;
  esac
}
reset_failed_unit() {
  local load_state active_state
  load_state="$(sudo -n systemctl show "$unit" --no-pager --property=LoadState --value 2>/dev/null || true)"
  active_state="$(sudo -n systemctl show "$unit" --no-pager --property=ActiveState --value 2>/dev/null || true)"
  if [ "$load_state" = loaded ] && [ "$active_state" = failed ]; then
    if ! timeout --signal=TERM --kill-after=2 10s sudo -n systemctl reset-failed "$unit" >/dev/null 2>&1; then
      cleanup_status=1
      restore_status=1
    fi
  fi
}
validate_benchmark_readiness() {
  local marker="$1" expected_pid="$2" expected_invocation="$3"
  [ -r "$marker" ] || return 1
  [ "$(json_field schema_version "$marker")" = 5 ]
  [ "$(json_field kind "$marker")" = orange_audio_benchmark_readiness ]
  [ "$(json_field status "$marker")" = ready ]
  [ "$(json_field board_profile "$marker")" = orange-pi-zero-2w ]
  [ "$(json_field pid "$marker")" = "$expected_pid" ]
  [ "$(json_field systemd_invocation_id "$marker")" = "$expected_invocation" ]
  [ "$(json_field artifact_sha256 "$marker")" = "$expected_sha" ]
  [ "$(json_field scenario "$marker")" = __SCENARIO__ ]
  [ "$(json_field executor_mode "$marker")" = __EXECUTOR_MODE__ ]
  [ "$(json_field lookahead_frames "$marker")" = __LOOKAHEAD_FRAMES__ ]
  [ "$(json_field requested_output_buffer_frames "$marker")" = __OUTPUT_FRAMES__ ]
  [ "$(json_field expected_alsa_buffer_frames "$marker")" = __OUTPUT_FRAMES__ ]
  [ "$(json_field expected_alsa_period_frames "$marker")" = __ALSA_PERIOD_FRAMES__ ]
  [ "$(json_field sample_rate "$marker")" = 44100 ]
  [ "$(json_field channels "$marker")" = 2 ]
  [ "$(json_field internal_block_frames "$marker")" = __INTERNAL_FRAMES__ ]
  callback_min="$(json_field callback_frames_min "$marker")"
  callback_max="$(json_field callback_frames_max "$marker")"
  callback_samples="$(json_field callback_frame_sample_count "$marker")"
  callback_invalid="$(json_field invalid_callback_frame_count "$marker")"
  positive_number "$callback_min" || return 1
  positive_number "$callback_max" || return 1
  positive_number "$callback_samples" || return 1
  [ "$callback_min" -le "$callback_max" ]
  [ "$callback_max" -le __OUTPUT_FRAMES__ ]
  [ "$callback_samples" -ge 3 ]
  [ "$callback_invalid" = 0 ]
  case "$(json_field sample_format "$marker")" in F32|I16|U16) ;; *) return 1;; esac
  [ "$(json_field scheduler_qualified "$marker")" = true ]
  [ "$(json_field post_dsp_zero "$marker")" = true ]
  validate_benchmark_worker_evidence "$marker"
}
validate_benchmark_worker_evidence() {
  local marker="$1" require_shutdown="${2:-false}"
  [ "$(json_field executor_mode "$marker")" = __EXECUTOR_MODE__ ]
  case "$(json_field executor_mode "$marker")" in
    persistent_two_workers)
      [ "$(json_field worker_health "$marker")" = healthy ]
      [ "$(json_field worker_thread_name_0 "$marker")" = oct-dsp-src-0 ]
      [ "$(json_field worker_thread_name_1 "$marker")" = oct-dsp-src-1 ]
      if [ "$require_shutdown" = true ]; then
        [ "$(json_field joined_workers "$marker")" = 2 ]
        [ "$(json_field retirement_error "$marker")" = null ]
      fi
      ;;
    inline)
      [ "$(json_field worker_health "$marker")" = disabled ]
      [ -z "$(json_field worker_thread_name_0 "$marker")" ]
      [ -z "$(json_field worker_thread_name_1 "$marker")" ]
      if [ "$require_shutdown" = true ]; then
        [ "$(json_field joined_workers "$marker")" = 0 ]
        [ "$(json_field retirement_error "$marker")" = null ]
      fi
      ;;
    routing_tree_persistent)
      [ "$(json_field worker_health "$marker")" = healthy ]
      [ "$(json_field worker_thread_name_0 "$marker")" = oct-dsp-tree-0 ]
      [ "$(json_field worker_thread_name_1 "$marker")" = oct-dsp-tree-1 ]
      if [ "$require_shutdown" = true ]; then
        [ "$(json_field joined_workers "$marker")" = 2 ]
        [ "$(json_field retirement_error "$marker")" = null ]
      fi
      ;;
    *) return 1;;
  esac
}
validate_benchmark_worker_threads() {
  local benchmark_pid="$1" proc_root="${2:-/proc}" task comm worker_zero=0 worker_one=0 reaper=0
  [ -d "$proc_root/$benchmark_pid/task" ] || return 1
  for task in "$proc_root/$benchmark_pid/task"/*; do
    [ -r "$task/comm" ] || return 1
    comm="$(cat "$task/comm")" || return 1
    case "$comm" in
      oct-dsp-src-0|oct-dsp-tree-0)
        if { [ "__EXECUTOR_MODE__" = persistent_two_workers ] && [ "$comm" != oct-dsp-src-0 ]; } || { [ "__EXECUTOR_MODE__" = routing_tree_persistent ] && [ "$comm" != oct-dsp-tree-0 ]; } || [ "__EXECUTOR_MODE__" = inline ]; then return 1; fi
        worker_zero=$((worker_zero + 1));;
      oct-dsp-src-1|oct-dsp-tree-1)
        if { [ "__EXECUTOR_MODE__" = persistent_two_workers ] && [ "$comm" != oct-dsp-src-1 ]; } || { [ "__EXECUTOR_MODE__" = routing_tree_persistent ] && [ "$comm" != oct-dsp-tree-1 ]; } || [ "__EXECUTOR_MODE__" = inline ]; then return 1; fi
        worker_one=$((worker_one + 1));;
      oct-dsp-src-*|oct-dsp-tree-*) return 1;;
      oct-src-reaper) reaper=$((reaper + 1));;
    esac
  done
  if [ "__EXECUTOR_MODE__" != inline ]; then
    [ "$worker_zero" = 1 ] && [ "$worker_one" = 1 ] && [ "$reaper" = 1 ]
  else
    [ "$worker_zero" = 0 ] && [ "$worker_one" = 0 ] && [ "$reaper" = 1 ]
  fi
}
wait_for_benchmark_readiness() {
  local deadline=$(( $(date +%s) + __STARTUP_TIMEOUT_SECONDS__ )) pid invocation
  while [ "$(date +%s)" -lt "$deadline" ]; do
    pid="$(unit_main_pid "$unit")"; invocation="$(unit_invocation_id "$unit")"
    if sudo -n systemctl is-active --quiet "$unit" && positive_number "$pid" && [ -n "$invocation" ] && validate_benchmark_readiness "$readiness" "$pid" "$invocation" && validate_benchmark_worker_threads "$pid"; then
      benchmark_pid="$pid"; benchmark_invocation="$invocation"
      copy_evidence "$readiness" "$root/benchmark-readiness.json"
      printf 'unit=%s\nmain_pid=%s\ninvocation_id=%s\n' "$unit" "$pid" "$invocation" > "$root/benchmark-identity.txt"
      return 0
    fi
    sleep 1
  done
  return 1
}
find_dac_hw_params() {
  local preferred=/proc/asound/octesseradac/pcm0p/sub0/hw_params id_path card_dir candidate=
  if [ -r "$preferred" ]; then printf '%s\n' "$preferred"; return 0; fi
  for id_path in /proc/asound/card*/id; do
    [ -r "$id_path" ] || continue
    if [ "$(cat "$id_path" 2>/dev/null || true)" = octesseradac ]; then
      card_dir="${id_path%/id}"
      if [ -r "$card_dir/pcm0p/sub0/hw_params" ]; then [ -z "$candidate" ] || return 1; candidate="$card_dir/pcm0p/sub0/hw_params"; fi
    fi
  done
  [ -n "$candidate" ] || return 1
  printf '%s\n' "$candidate"
}
capture_alsa_release() {
  local hw_params buffer period release_source release_tmp
  if ! sudo -n rm -f -- "$release" "$release.tmp-$$"; then return 1; fi
  release_source="$root/benchmark-release.json"
  release_tmp="$release.tmp-$$"
  if ! rm -f -- "$release_source"; then return 1; fi
  if ! hw_params="$(find_dac_hw_params)" || [ ! -r "$hw_params" ]; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n cp -- "$hw_params" "$root/alsa-hw-params.txt"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n chown octessera:octessera-runtime -- "$root/alsa-hw-params.txt"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n chmod 0640 -- "$root/alsa-hw-params.txt"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! buffer="$(sed -n 's/^[[:space:]]*buffer_size[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$hw_params" | head -n 1)" || ! positive_number "$buffer"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! period="$(sed -n 's/^[[:space:]]*period_size[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$hw_params" | head -n 1)" || ! positive_number "$period"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if [ "$buffer" != __OUTPUT_FRAMES__ ] || [ "$period" != __ALSA_PERIOD_FRAMES__ ]; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! printf 'path=%s\nbuffer_size=%s\nperiod_size=%s\n' "$hw_params" "$buffer" "$period" > "$root/alsa-geometry.txt"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! printf '{"schema_version":2,"kind":"orange_audio_benchmark_release","status":"released","board_profile":"orange-pi-zero-2w","pid":%s,"systemd_invocation_id":"%s","artifact_sha256":"%s","scenario":"%s","expected_alsa_buffer_frames":%s,"observed_alsa_buffer_frames":%s,"expected_alsa_period_frames":%s,"observed_alsa_period_frames":%s}\n' "$benchmark_pid" "$benchmark_invocation" "$expected_sha" "__SCENARIO__" __OUTPUT_FRAMES__ "$buffer" __ALSA_PERIOD_FRAMES__ "$period" > "$release_source"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n cp -- "$release_source" "$release_tmp"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n chown octessera-runtime:octessera-runtime -- "$release_tmp"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n chmod 0640 -- "$release_tmp"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
  if ! sudo -n mv -f -- "$release_tmp" "$release"; then sudo -n rm -f -- "$release" "$release_tmp" "$release_source"; return 1; fi
}
validate_benchmark_progress() {
  [ -r "$progress" ] && [ "$(json_field schema_version "$progress")" = 5 ] && [ "$(json_field kind "$progress")" = orange_audio_benchmark_progress ]
  [ "$(json_field board_profile "$progress")" = orange-pi-zero-2w ] && [ "$(json_field pid "$progress")" = "$benchmark_pid" ]
  [ "$(json_field systemd_invocation_id "$progress")" = "$benchmark_invocation" ] && [ "$(json_field artifact_sha256 "$progress")" = "$expected_sha" ]
  [ "$(json_field scenario "$progress")" = __SCENARIO__ ] && [ "$(json_field executor_mode "$progress")" = __EXECUTOR_MODE__ ]
  [ "$(json_field lookahead_frames "$progress")" = __LOOKAHEAD_FRAMES__ ]
  [ "$(json_field requested_output_buffer_frames "$progress")" = __OUTPUT_FRAMES__ ]
  [ "$(json_field expected_alsa_buffer_frames "$progress")" = __OUTPUT_FRAMES__ ] && [ "$(json_field expected_alsa_period_frames "$progress")" = __ALSA_PERIOD_FRAMES__ ]
  [ "$(json_field internal_block_frames "$progress")" = __INTERNAL_FRAMES__ ]
  validate_benchmark_worker_evidence "$progress"
}
validate_benchmark_result() {
  [ -r "$result" ] && [ "$(json_field schema_version "$result")" = 12 ] && [ "$(json_field kind "$result")" = orange_audio_benchmark_result ]
  [ "$(json_field board_profile "$result")" = orange-pi-zero-2w ] && [ "$(json_field pid "$result")" = "$benchmark_pid" ]
  [ "$(json_field systemd_invocation_id "$result")" = "$benchmark_invocation" ] && [ "$(json_field artifact_sha256 "$result")" = "$expected_sha" ]
  [ "$(json_field scenario "$result")" = __SCENARIO__ ] && [ "$(json_field executor_mode "$result")" = __EXECUTOR_MODE__ ]
  [ "$(json_field lookahead_frames "$result")" = __LOOKAHEAD_FRAMES__ ] && [ "$(json_field effective_output_latency_frames "$result")" = __EFFECTIVE_OUTPUT_LATENCY_FRAMES__ ]
  [ "$(json_field worker_timing_mode "$result")" = __WORKER_TIMING_MODE__ ]
  [ "$(json_field requested_output_buffer_frames "$result")" = __OUTPUT_FRAMES__ ] && [ "$(json_field expected_alsa_buffer_frames "$result")" = __OUTPUT_FRAMES__ ]
  [ "$(json_field expected_alsa_period_frames "$result")" = __ALSA_PERIOD_FRAMES__ ] && [ "$(json_field internal_block_frames "$result")" = __INTERNAL_FRAMES__ ]
  validate_benchmark_worker_evidence "$result" true
}
wait_for_benchmark_terminal() {
  local deadline=$(( $(date +%s) + __RUNTIME_MAX_SECONDS__ + 15 )) pid invocation phase mtime now result_status
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ -e "$sensor_abort" ]; then study_status=75; study_class=safety_failure; stop_benchmark_unit; return 1; fi
    pid="$(unit_main_pid "$unit")"; invocation="$(unit_invocation_id "$unit")"
    if [ "$pid" != 0 ] && [ "$pid" != "$benchmark_pid" ]; then study_status=66; stop_benchmark_unit; return 1; fi
    if [ -n "$invocation" ] && [ "$invocation" != "$benchmark_invocation" ]; then study_status=66; stop_benchmark_unit; return 1; fi
    if [ -e "$result" ] && ! sudo -n systemctl is-active --quiet "$unit"; then
      result_status="$(json_field status "$result" || true)"
      if ! validate_benchmark_result; then
        study_status=66
        study_class=infrastructure_failure
        return 1
      fi
      copy_evidence "$result" "$root/benchmark-result.json"
      if [ "$result_status" = pass ]; then study_status=0; study_class=pass; else study_status=20; study_class=measured_failure; fi
      return 0
    fi
    if ! validate_benchmark_progress; then study_status=66; study_class=infrastructure_failure; stop_benchmark_unit; return 1; fi
    if [ "$(json_field terminal_error "$progress")" = true ] || [ "$(json_field cpal_device_error_count "$progress")" != 0 ] || [ "$(json_field cpal_stream_error_count "$progress")" != 0 ]; then
      study_status=66
      study_class=infrastructure_failure
      stop_benchmark_unit
      return 1
    fi
    phase="$(json_field phase "$progress")"; mtime="$(stat -c %Y "$progress" 2>/dev/null || printf 0)"; now="$(date +%s)"
    if [ "$phase" = waiting_release ]; then
      [ $((now - mtime)) -le $((__RELEASE_TIMEOUT_SECONDS__ + 15)) ] || { study_status=66; stop_benchmark_unit; return 1; }
    else
      [ $((now - mtime)) -le 5 ] || { study_status=66; stop_benchmark_unit; return 1; }
    fi
    sleep 1
  done
  study_status=66; study_class=infrastructure_failure; stop_benchmark_unit; return 1
}
capture_transient_evidence() {
  sudo -n systemctl show "$unit" --no-pager --property=ActiveState --property=SubState --property=Result --property=ExecMainCode --property=ExecMainStatus --property=MainPID --property=InvocationID > "$root/unit-final.txt" 2>&1 || true
  sudo -n journalctl -u "$unit" -n 200 --no-pager > "$root/unit-journal.txt" 2>&1 || true
  copy_evidence "$readiness" "$root/benchmark-readiness-final.json"
  copy_evidence "$progress" "$root/benchmark-progress-final.json"
  copy_evidence "$result" "$root/benchmark-result-final.json"
  copy_evidence "$release" "$root/benchmark-release.json"
  copy_evidence "$root/unit-stop-before.txt" "$root/unit-stop-before.json"
  copy_evidence "$root/unit-stop-after.txt" "$root/unit-stop-after.json"
  copy_evidence "$root/unit-stop-evidence.txt" "$root/unit-stop-evidence.json"
}
restore_service() {
  local active enabled
  set +e
  stop_benchmark_unit
  reset_failed_unit
  sudo -n rm -f -- "$health" "$readiness" "$progress" "$result" "$release"
  sudo -n rm -rf -- "$benchmark_root"
  timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service" >/dev/null 2>&1 || restore_status=1
  wait_for_stable_readiness "$service" "$production_health" "$root/service-restored-candidate-ready.json" "$root/service-restored-readiness-properties.txt" "$root/service-restored-readiness-check.txt" || restore_status=1
  active="$(systemctl is-active "$service" 2>/dev/null || true)"; enabled="$(systemctl is-enabled "$service" 2>/dev/null || true)"
  [ "$active" = active ] && [ "$enabled" = enabled ] || restore_status=1
  printf 'initial_active=%s\ninitial_enabled=%s\nfinal_active=%s\nfinal_enabled=%s\nrestore_status=%s\ncleanup_status=%s\n' "$initial_active" "$initial_enabled" "$active" "$enabled" "$restore_status" "$cleanup_status" > "$root/service-restored-state.txt"
  copy_evidence "$production_health" "$root/service-restored-ready.json"
}
on_exit() {
  local exit_status=$?
  set +e
  trap - EXIT HUP INT TERM
  stop_sampler
  capture_transient_evidence
  restore_service
  [ -e "$sensor_abort" ] && study_class=safety_failure
  printf 'mode=LiveAudioBenchmark\nstatus_class=%s\nstatus=%s\ninterruption_started=%s\nrestore_status=%s\nsensor_abort=%s\n' "$study_class" "$exit_status" "$interruption_started" "$restore_status" "$([ -e "$sensor_abort" ] && printf true || printf false)" > "$root/study-result.txt"
  [ "$restore_status" -eq 0 ] || exit_status=70
  [ "$cleanup_status" -eq 0 ] || exit_status=71
  exit "$exit_status"
}
test -d "$root"
capture_service_state
[ "$initial_active" = active ] && [ "$initial_enabled" = enabled ] || { printf 'mode=LiveAudioBenchmark\nstatus_class=infrastructure_failure\ninterruption_started=false\nreason=production-service-not-active-enabled\n' > "$root/study-result.txt"; exit 64; }
wait_for_stable_readiness "$service" "$production_health" "$root/service-initial-candidate-ready.json" "$root/service-initial-readiness-properties.txt" "$root/service-initial-readiness-check.txt" || { printf 'mode=LiveAudioBenchmark\nstatus_class=infrastructure_failure\ninterruption_started=false\nreason=initial-readiness-invalid\n' > "$root/study-result.txt"; exit 65; }
capture_sensor_sample startup || { printf 'mode=LiveAudioBenchmark\nstatus_class=safety_failure\ninterruption_started=false\nreason=startup-sensor-gate\n' > "$root/study-result.txt"; exit 75; }
printf 'expected_dac=hw:CARD=octesseradac,DEV=0\n' > "$root/candidate-contract.txt"
trap on_exit EXIT
trap 'exit 143' INT TERM
trap 'exit 129' HUP
sudo -n install -d -o octessera-runtime -g octessera-runtime -m 0750 "$benchmark_root"
chmod 0755 "$binary"; sudo -n chgrp octessera-runtime "$root"; chmod 0710 "$root"; sudo -n chgrp octessera-runtime "$binary" "$metadata"; chmod 0750 "$binary"; chmod 0640 "$metadata"
test -x "$binary"; test -r "$metadata"; remote_sha="$(sha256sum -- "$binary" | awk 'NR == 1 {print $1}')"; printf '%s\n' "$remote_sha" > "$root/runtime-candidate-sha256.txt"; test "$remote_sha" = "$expected_sha"; "$binary" --print-build-metadata > "$root/runtime-candidate-metadata.json"; grep -q '"artifact_kind":"__ARTIFACT_KIND__"' "$root/runtime-candidate-metadata.json"; grep -q '"cargo_feature":"__CARGO_FEATURE__"' "$root/runtime-candidate-metadata.json"; grep -q '"profile":"release"' "$root/runtime-candidate-metadata.json"
sudo -n systemctl stop "$service"
interruption_started=true
launch_status=0
sudo -n systemd-run --unit="$unit" --service-type=exec --no-block --property=RuntimeMaxSec=__RUNTIME_MAX_SECONDS__s --property=TimeoutStopSec=5s --property=User=octessera-runtime --property=Group=octessera-runtime --property=Nice=-10 --property=LimitRTPRIO=70 --property=LimitMEMLOCK=infinity --property=NoNewPrivileges=yes --property=ProtectSystem=strict --property=ProtectHome=yes --property=ProtectKernelTunables=yes --property=ProtectKernelModules=yes --property=ProtectControlGroups=yes --property=RestrictNamespaces=yes --property=LockPersonality=yes --property=PrivateTmp=no --property=RuntimeDirectory=octessera --property=RuntimeDirectoryMode=0755 --property=RuntimeDirectoryPreserve=yes --property="ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot" --setenv=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w --setenv=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets --setenv=OCTESSERA_OLED_BOOT_HANDOFF=v1 --setenv=OCTESSERA_CANDIDATE_HEALTH_PATH=__HEALTH__ "$binary" --benchmark-orange-audio --executor __EXECUTOR_MODE__ --scenario __SCENARIO__ --output-frames __OUTPUT_FRAMES__ --engine-block-frames __INTERNAL_FRAMES__ --worker-timing __WORKER_TIMING_MODE__ --warmup-seconds 5 --measure-seconds __MEASURE_SECONDS__ --readiness "$readiness" --progress "$progress" --result "$result" --release-gate "$release" --release-timeout-seconds __RELEASE_TIMEOUT_SECONDS__ --artifact-sha256 "$expected_sha" || launch_status=$?
[ "$launch_status" -eq 0 ] || { study_status=66; exit "$study_status"; }
sensor_loop > "$root/sensor-sampler.stderr" 2>&1 & sampler_pid=$!
wait_for_benchmark_readiness || { study_status=66; stop_benchmark_unit; exit "$study_status"; }
capture_alsa_release || { study_status=66; stop_benchmark_unit; exit "$study_status"; }
wait_for_benchmark_terminal || true
exit "$study_status"
'@
  $body = $body.Replace("__ROOT__", (Quote-LiveShValue $RemoteRoot)).Replace("__BENCHMARK_ROOT__", (Quote-LiveShValue $BenchmarkRoot)).Replace("__HEALTH__", (Quote-LiveShValue $HealthPath)).Replace("__HASH__", (Quote-LiveShValue $ArtifactHash)).Replace("__UNIT__", (Quote-LiveShValue $Unit)).Replace("__SERVICE__", (Quote-LiveShValue $Service)).Replace("__SCENARIO__", $Selection.Scenario).Replace("__OUTPUT_FRAMES__", [string]$Selection.OutputFrames).Replace("__ALSA_PERIOD_FRAMES__", [string]$Selection.AlsaPeriodFrames).Replace("__INTERNAL_FRAMES__", [string]$Selection.InternalFrames).Replace("__MEASURE_SECONDS__", [string]$Selection.MeasureSeconds).Replace("__STARTUP_TIMEOUT_SECONDS__", [string]$StartupTimeoutSeconds).Replace("__RELEASE_TIMEOUT_SECONDS__", [string]$ReleaseTimeoutSeconds).Replace("__RUNTIME_MAX_SECONDS__", [string]$RuntimeMaxSeconds).Replace("__EXECUTOR_MODE__", $ExecutorMode).Replace("__LOOKAHEAD_FRAMES__", [string]$Selection.LookaheadFrames).Replace("__EFFECTIVE_OUTPUT_LATENCY_FRAMES__", [string]$Selection.EffectiveOutputLatencyFrames).Replace("__ARTIFACT_KIND__", $ExpectedArtifactKind).Replace("__CARGO_FEATURE__", $ExpectedCargoFeature)
  $body = $body.Replace("__WORKER_TIMING_MODE__", $WorkerTimingMode)
  $study = "set -eu`numask 077`nroot=$(Quote-LiveShValue $RemoteRoot)`nhealth=$(Quote-LiveShValue $HealthPath)`nunit=$(Quote-LiveShValue $Unit)`n$readinessHelpers`n$body"
  $prepare = "set -eu`numask 077`ntest ! -e $(Quote-LiveShValue $RemoteRoot)`nmkdir -m 0700 -- $(Quote-LiveShValue $RemoteRoot)`nsudo -n chgrp octessera-runtime $(Quote-LiveShValue $RemoteRoot)`nchmod 0710 $(Quote-LiveShValue $RemoteRoot)"
  $cleanup = @'
set -eu
unit=__UNIT__
root=__ROOT__
health=__HEALTH__
benchmark_root=__BENCHMARK_ROOT__
service='octessera.service'
production_health=/run/octessera/candidate-ready.json
__READINESS_HELPERS__
reset_status=0
reset_failed_unit() {
  local load_state active_state
  load_state="$(sudo -n systemctl show "$unit" --no-pager --property=LoadState --value 2>/dev/null || true)"
  active_state="$(sudo -n systemctl show "$unit" --no-pager --property=ActiveState --value 2>/dev/null || true)"
  if [ "$load_state" = loaded ] && [ "$active_state" = failed ]; then
    if ! timeout --signal=TERM --kill-after=2 10s sudo -n systemctl reset-failed "$unit" >/dev/null 2>&1; then
      reset_status=1
    fi
  fi
}
state="$(sudo -n systemctl is-active "$unit" 2>/dev/null || true)"
case "$state" in
  active|activating|deactivating|failed) timeout --signal=TERM --kill-after=2 10s sudo -n systemctl stop "$unit" || exit 72;;
  inactive|unknown|'') ;;
  *) exit 72;;
esac
reset_failed_unit
sudo -n rm -f -- "$health"
sudo -n rm -rf -- "$benchmark_root"
initial_state_valid=0
[ -r "$root/service-initial-state.txt" ] && grep -Fxq 'active=active' "$root/service-initial-state.txt" && grep -Fxq 'enabled=enabled' "$root/service-initial-state.txt" && initial_state_valid=1
if [ "$initial_state_valid" -eq 1 ]; then
  timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"
  wait_for_stable_readiness "$service" "$production_health" "$root/service-cleanup-ready.json" "$root/service-cleanup-properties.txt" "$root/service-cleanup-check.txt"
  test "$(systemctl is-active "$service" 2>/dev/null || true)" = active
  test "$(systemctl is-enabled "$service" 2>/dev/null || true)" = enabled
fi
if [ "$reset_status" -ne 0 ]; then exit 72; fi
sudo -n rm -rf -- "$root"
'@
  $cleanup = $cleanup.Replace("__READINESS_HELPERS__", [string]$readinessHelpers).Replace("__UNIT__", (Quote-LiveShValue $Unit)).Replace("__ROOT__", (Quote-LiveShValue $RemoteRoot)).Replace("__HEALTH__", (Quote-LiveShValue $HealthPath)).Replace("__BENCHMARK_ROOT__", (Quote-LiveShValue $BenchmarkRoot))
  return [pscustomobject]@{ Study = $study; Prepare = $prepare; Cleanup = $cleanup }
}

Export-ModuleMember -Function New-OrangeLiveBenchmarkPayloadBundle
