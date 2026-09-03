Set-StrictMode -Version Latest

function Assert-OrangeGeneratedLivePayloadSyntax {
  param([Parameter(Mandatory)][string]$Payload)
  $bashCommand = Get-Command bash -ErrorAction SilentlyContinue
  $wslCommand = Get-Command wsl.exe -ErrorAction SilentlyContinue
  if ($null -ne $bashCommand -and [string]$bashCommand.Source -notmatch "WindowsApps") {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
    try {
      [IO.File]::WriteAllText($temporary, $Payload, (New-Object System.Text.UTF8Encoding($false)))
      & bash -n $temporary
      if ($LASTEXITCODE -ne 0) { throw "Generated live payload failed bash -n." }
    } finally {
      Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
    }
  } elseif ($null -ne $wslCommand) {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
    try {
      [IO.File]::WriteAllText($temporary, $Payload, (New-Object System.Text.UTF8Encoding($false)))
      $drive = $temporary.Substring(0, 1).ToLowerInvariant()
      $wslPath = "/mnt/$drive" + ($temporary.Substring(2) -replace "\\", "/")
      & wsl.exe bash -n $wslPath
      if ($LASTEXITCODE -ne 0) { throw "Generated live payload failed WSL bash -n." }
    } finally {
      Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
    }
  }
}

function Get-OrangeGeneratedLiveFunction {
  param(
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$EndName
  )
  $start = $Payload.IndexOf("$Name() {", [StringComparison]::Ordinal)
  $end = $Payload.IndexOf("$EndName() {", $start, [StringComparison]::Ordinal)
  if ($start -lt 0 -or $end -le $start) { throw "Generated live payload did not contain $Name." }
  return $Payload.Substring($start, $end - $start)
}

function Assert-OrangeGeneratedWorkerTaskAudit {
  param(
    [Parameter(Mandatory)][string]$PersistentPayload,
    [Parameter(Mandatory)][string]$InlinePayload
  )
  foreach ($payload in @($PersistentPayload, $InlinePayload)) {
    if ([regex]::Matches($payload, 'validate_benchmark_worker_threads "\$pid"').Count -lt 1) { throw "Generated live payload did not audit worker tasks during readiness." }
  }
  $persistentAudit = Get-OrangeGeneratedLiveFunction $PersistentPayload "validate_benchmark_worker_threads" "wait_for_benchmark_readiness"
  $inlineAudit = Get-OrangeGeneratedLiveFunction $InlinePayload "validate_benchmark_worker_threads" "wait_for_benchmark_readiness"
  foreach ($case in @(
      @{ Name = "persistent"; Function = $persistentAudit; Valid = @("oct-dsp-src-0", "oct-dsp-src-1", "oct-src-reaper", "unrelated-runtime-task"); Invalid = @(@("oct-dsp-src-0", "oct-src-reaper"), @("oct-dsp-src-0", "oct-dsp-src-0", "oct-dsp-src-1", "oct-src-reaper"), @("oct-dsp-src-0", "oct-dsp-src-1", "oct-dsp-src-2", "oct-src-reaper"), @("oct-dsp-src-0", "oct-dsp-src-1"), @("oct-dsp-src-0", "oct-dsp-src-1", "oct-src-reaper", "oct-src-reaper")) };
      @{ Name = "inline"; Function = $inlineAudit; Valid = @("oct-src-reaper", "unrelated-runtime-task"); Invalid = @(@("unrelated-runtime-task"), @("oct-src-reaper", "oct-src-reaper"), @("oct-dsp-src-0", "oct-src-reaper"), @("oct-dsp-src-2", "oct-src-reaper")) }
    )) {
    $fixture = @'
set -eu
__AUDIT_FUNCTION__
make_tasks() {
  root="$1"; shift
  pid=123
  index=1
  mkdir -p "$root/$pid/task"
  for comm in "$@"; do
    mkdir -p "$root/$pid/task/$index"
    printf '%s\n' "$comm" > "$root/$pid/task/$index/comm"
    index=$((index + 1))
  done
}
run_case() {
  expected="$1"; shift
  root="$(mktemp -d)"
  make_tasks "$root" "$@"
  if [ "$expected" = pass ]; then
    validate_benchmark_worker_threads 123 "$root"
  elif validate_benchmark_worker_threads 123 "$root"; then
    exit 1
  fi
  rm -rf -- "$root"
}
run_case pass __VALID_TASKS__
__INVALID_CASES__
rm -rf -- "$root"
'@
    $invalidCases = @()
    foreach ($invalid in $case.Invalid) {
      $invalidCases += "run_case fail $($invalid -join ' ')"
    }
    $fixture = $fixture.Replace("__AUDIT_FUNCTION__", $case.Function).Replace("__VALID_TASKS__", ($case.Valid -join " ")).Replace("__INVALID_CASES__", ($invalidCases -join "`n"))
    $fixture | & bash -s
    if ($LASTEXITCODE -ne 0) { throw "Generated $($case.Name) worker-task audit fixture failed." }
  }

  $inlineEvidence = Get-OrangeGeneratedLiveFunction $InlinePayload "validate_benchmark_worker_evidence" "validate_benchmark_worker_threads"
  $inlineReadiness = Get-OrangeGeneratedLiveFunction $InlinePayload "wait_for_benchmark_readiness" "find_dac_hw_params"
  $inlineReadiness = $inlineReadiness.Replace('validate_benchmark_worker_threads "$pid"', 'validate_benchmark_worker_threads "$pid" "$task_root"')
  $inlineTerminal = Get-OrangeGeneratedLiveFunction $InlinePayload "wait_for_benchmark_terminal" "capture_transient_evidence"
  if ([regex]::Matches($inlineTerminal, 'validate_benchmark_worker_threads "\$pid"').Count -ne 0) { throw "Generated terminal loop still performs a live worker-task audit." }
  $fixture = @'
set -eu
__INLINE_AUDIT__
__INLINE_EVIDENCE__
__INLINE_READINESS__
__INLINE_TERMINAL__
root="$(mktemp -d)"
task_root="$(mktemp -d)"
pid=123
invocation=fixture-invocation
benchmark_pid=
benchmark_invocation=
unit=fixture.service
readiness="$root/readiness.json"
result="$root/result.json"
sensor_abort="$root/sensor_abort"
release_marker="$root/release-marker"
study_status=0
study_class=infrastructure_failure
benchmark_active=true
write_readiness() { printf '{\n"executor_mode":"inline",\n"worker_health":"disabled",\n"worker_thread_name_0":"",\n"worker_thread_name_1":""\n}\n' > "$readiness"; }
write_tasks() {
  rm -rf -- "$task_root/$pid/task"
  case "$1" in
    valid)
      mkdir -p "$task_root/$pid/task/1" "$task_root/$pid/task/2"
      printf 'oct-src-reaper\n' > "$task_root/$pid/task/1/comm"
      printf 'unrelated-runtime-task\n' > "$task_root/$pid/task/2/comm"
      ;;
    missing)
      mkdir -p "$task_root/$pid/task/1"
      printf 'unrelated-runtime-task\n' > "$task_root/$pid/task/1/comm"
      ;;
    extra)
      mkdir -p "$task_root/$pid/task/1" "$task_root/$pid/task/2" "$task_root/$pid/task/3"
      printf 'oct-src-reaper\n' > "$task_root/$pid/task/1/comm"
      printf 'oct-src-reaper\n' > "$task_root/$pid/task/2/comm"
      printf 'unrelated-runtime-task\n' > "$task_root/$pid/task/3/comm"
      ;;
  esac
}
run_readiness_case() {
  expected="$1"
  write_tasks "$2"
  printf '0\n' > "$date_state"
  benchmark_pid=
  benchmark_invocation=
  if [ "$expected" = pass ]; then
    wait_for_benchmark_readiness
    [ "$benchmark_pid" = 123 ]
  elif wait_for_benchmark_readiness; then
    exit 1
  fi
}
unit_main_pid() { printf '123\n'; }
unit_invocation_id() { printf 'fixture-invocation\n'; }
positive_number() { case "$1" in ''|0|*[!0-9]*) return 1;; *) return 0;; esac; }
json_field() { sed -n "s/^[[:space:]]*\"$1\"[[:space:]]*:[[:space:]]*//p" "$2" | sed 's/[",]//g; s/^[[:space:]]*//; s/[[:space:]]*$//' | head -n 1; }
systemctl() { [ "$1" = is-active ] && [ "$benchmark_active" = true ]; }
sudo() { [ "$1" = -n ] && shift; "$@"; }
validate_benchmark_readiness() { validate_benchmark_worker_evidence "$1"; }
copy_evidence() { :; }
sleep() { :; }
write_readiness
mkdir -p "$task_root/$pid/task"
date_calls=0
date_state="$(mktemp)"
printf '0\n' > "$date_state"
cleanup_date_state() { rm -f -- "$date_state"; }
trap cleanup_date_state EXIT
date() {
  if [ "$1" = +%s ]; then
    date_calls="$(cat "$date_state")"
    date_calls=$((date_calls + 1))
    printf '%s\n' "$date_calls" > "$date_state"
    if [ "$date_calls" -le 2 ]; then printf '100\n'; else printf '999\n'; fi
  else
    command date "$@"
  fi
}
if wait_for_benchmark_readiness; then exit 1; fi
run_readiness_case fail missing
run_readiness_case fail extra
run_readiness_case pass valid
printf released > "$release_marker"
rm -rf -- "$task_root/$pid/task"
benchmark_active=false
printf '{\n"status":"pass",\n"executor_mode":"inline",\n"worker_health":"disabled",\n"worker_thread_name_0":"",\n"worker_thread_name_1":"",\n"joined_workers":0,\n"retirement_error":null\n}\n' > "$result"
wait_for_benchmark_terminal
[ "$study_status" = 0 ]
[ "$study_class" = pass ]
printf '{\n"status":"pass",\n"executor_mode":"inline",\n"worker_health":"disabled",\n"worker_thread_name_0":"",\n"worker_thread_name_1":"",\n"joined_workers":1,\n"retirement_error":"fixture-invalid"\n}\n' > "$result"
study_status=0
study_class=infrastructure_failure
if wait_for_benchmark_terminal; then exit 1; fi
[ "$study_status" = 66 ]
[ "$study_class" = infrastructure_failure ]
printf accepted >> "$release_marker"
grep -q released "$release_marker"
grep -q accepted "$release_marker"
rm -rf -- "$root" "$task_root"
'@
  $fixture = $fixture.Replace("__INLINE_AUDIT__", $inlineAudit).Replace("__INLINE_EVIDENCE__", $inlineEvidence).Replace("__INLINE_READINESS__", $inlineReadiness).Replace("__INLINE_TERMINAL__", $inlineTerminal)
  $fixture | & bash -s
  if ($LASTEXITCODE -ne 0) { throw "Inline readiness did not proceed to release in the generated-shell fixture." }
}

Export-ModuleMember -Function Assert-OrangeGeneratedLivePayloadSyntax, Assert-OrangeGeneratedWorkerTaskAudit
