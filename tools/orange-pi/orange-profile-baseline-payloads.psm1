Set-StrictMode -Version Latest

function Quote-ProfileBaselineShValue {
  param([Parameter(Mandatory)][string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

function New-OrangeProfileBaselineBody {
  param(
    [Parameter(Mandatory)][string]$Scenario,
    [Parameter(Mandatory)][int]$InternalFrames,
    [Parameter(Mandatory)][int]$MeasureFrames,
    [Parameter(Mandatory)][int]$TimeoutSeconds
  )
  $body = @'
unset OCTESSERA_PI_PROFILE_DSP || true
profile_status=0
safety_abort="$root/safety-abort.txt"
low_memory_samples=0
check_profile_safety() {
  local phase="$1" mem thermal thermal_value max_thermal=0 thermal_count=0
  mem="$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo || true)"
  for thermal in /sys/class/thermal/thermal_zone*/temp; do
    [ -e "$thermal" ] || continue
    thermal_count=$((thermal_count + 1))
    thermal_value="$(cat "$thermal" 2>/dev/null || true)"
    case "$thermal_value" in ''|0|*[!0-9]*) printf 'reason=thermal-unreadable\nphase=%s\n' "$phase" > "$safety_abort"; return 1;; esac
    [ "$thermal_value" -le "$max_thermal" ] || max_thermal="$thermal_value"
  done
  case "$mem" in ''|0|*[!0-9]*) printf 'reason=memory-unreadable\nphase=%s\n' "$phase" > "$safety_abort"; return 1;; esac
  if [ "$thermal_count" -eq 0 ]; then printf 'reason=thermal-missing\nphase=%s\n' "$phase" > "$safety_abort"; return 1; fi
  if [ "$phase" = startup ]; then
    if [ "$mem" -lt 524288 ]; then
      printf 'reason=startup-safety-limit\nphase=%s\nmax_millicelsius=%s\nmem_available_kb=%s\n' "$phase" "$max_thermal" "$mem" > "$safety_abort"
      return 1
    fi
  else
    if [ "$mem" -lt 262144 ]; then low_memory_samples=$((low_memory_samples + 1)); else low_memory_samples=0; fi
    if [ "$low_memory_samples" -ge 2 ]; then
      printf 'reason=runtime-memory-abort\nphase=%s\nmem_available_kb=%s\nconsecutive_samples=%s\n' "$phase" "$mem" "$low_memory_samples" > "$safety_abort"
      sudo -n systemctl stop "$unit" >/dev/null 2>&1 || true
      return 1
    fi
  fi
}
capture_governors() {
  local governor
  for governor in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    [ -r "$governor" ] && printf '%s:%s\n' "$governor" "$(cat "$governor")"
  done
}
sample_system > "$root/system-evidence.txt" 2>&1
check_profile_safety startup || profile_status=75
capture_governors > "$root/governor-before.txt"
if [ "$profile_status" -eq 0 ]; then
  sudo -n systemctl stop "$service"
  (
    while [ ! -e "$safety_abort" ]; do
      sample_system >> "$root/system-evidence.txt" 2>&1
      check_profile_safety runtime || break
      sleep 1
    done
  ) &
  sampler_pid=$!
  timeout --signal=TERM --kill-after=5 __TIMEOUT_SECONDS__s sudo -n systemd-run --quiet --unit="$unit" --service-type=exec --wait --pipe --collect --property=RuntimeMaxSec=__TIMEOUT_SECONDS__s --property=TimeoutStopSec=5s --property=User=octessera-runtime --property=Group=octessera-runtime --property=Nice=-10 --property=LimitRTPRIO=70 --property=LimitMEMLOCK=infinity --property=NoNewPrivileges=yes --property=ProtectSystem=strict --property=ProtectHome=yes --property=ProtectKernelTunables=yes --property=ProtectKernelModules=yes --property=ProtectControlGroups=yes --property=RestrictNamespaces=yes --property=LockPersonality=yes --property=PrivateTmp=no --property=RuntimeDirectory=octessera --property=RuntimeDirectoryMode=0755 --property="ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot" --setenv=OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES=__INTERNAL_FRAMES__ --setenv=OCTESSERA_PI_PROFILE_MEASURE_FRAMES=__MEASURE_FRAMES__ --setenv=OCTESSERA_PI_PROFILE_SAMPLE_RATE=44100 --setenv=OCTESSERA_PI_PROFILE_MODE=baseline --setenv=OCTESSERA_PI_PROFILE_SCENARIO=__SCENARIO__ "$binary" --profile-dsp > "$root/profile.csv" 2> "$root/profile.stderr" || profile_status=$?
  kill -TERM "$sampler_pid" 2>/dev/null || true
  wait "$sampler_pid" 2>/dev/null || true
  if [ -e "$safety_abort" ]; then profile_status=75; fi
fi
capture_governors > "$root/governor-after.txt"
if ! cmp -s "$root/governor-before.txt" "$root/governor-after.txt"; then
  printf 'reason=governor-changed\n' > "$root/governor-abort.txt"
  profile_status=75
fi
printf 'mode=profile-baseline\nscenario=__SCENARIO__\ninternal_block_frames=__INTERNAL_FRAMES__\nmeasure_frames=__MEASURE_FRAMES__\nstatus=%s\nstatus_class=%s\nsafety_abort=%s\ngovernor_before=%s\ngovernor_after=%s\n' "$profile_status" "$([ "$profile_status" -eq 75 ] && printf safety_failure || printf measured)" "$([ -e "$safety_abort" ] && printf true || printf false)" "$root/governor-before.txt" "$root/governor-after.txt" > "$root/study-result.txt"
exit "$profile_status"
'@
  return $body.Replace("__SCENARIO__", (Quote-ProfileBaselineShValue $Scenario)).Replace("__INTERNAL_FRAMES__", [string]$InternalFrames).Replace("__MEASURE_FRAMES__", [string]$MeasureFrames).Replace("__TIMEOUT_SECONDS__", [string]$TimeoutSeconds)
}

Export-ModuleMember -Function "New-OrangeProfileBaselineBody"
