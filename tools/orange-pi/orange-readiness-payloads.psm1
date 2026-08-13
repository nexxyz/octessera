Set-StrictMode -Version Latest

function Get-OrangeReadinessHelpers {
  return @'
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
  if [ ! -r "$marker" ]; then return 1; fi
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
  if [ ! -r "$marker" ]; then printf 'status=missing-marker\n' > "$check_evidence"; return 1; fi
  if ! cp -- "$marker" "$marker_evidence"; then printf 'status=missing-marker\n' > "$check_evidence"; return 1; fi
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
  local candidate_pid candidate_invocation stable_pid stable_invocation stable_deadline stable current_pid current_invocation
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
}

Export-ModuleMember -Function Get-OrangeReadinessHelpers
